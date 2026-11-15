use std::path::Path;
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};
use tauri::{AppHandle, Manager};
use tokio::io::{AsyncWrite, AsyncWriteExt};
use url::Url;

use crate::i18n;
use crate::paths::AppPaths;
use crate::update::http;
use crate::update::manifest::{MAX_UPDATE_BYTES, staged_file_name};
use crate::update::state::{
    STATE_DEFERRED, STATE_DOWNLOADING, STATE_INSTALLING, STATE_READY, UpdateStatus, UpdateStore,
    error_status, set_status,
};
use crate::update::verify::{staged_path, verify_file};

/// How often a progress event is pushed to the UI while streaming.
const PROGRESS_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Debug)]
pub(crate) enum ReceiveError {
    Read(String),
    Write(String),
    TooLarge,
    SizeMismatch { received: u64, expected: u64 },
    ChecksumMismatch,
}

pub(crate) trait ChunkSource {
    async fn next_chunk(&mut self) -> Result<Option<Vec<u8>>, String>;
}

struct ResponseChunks<'a>(&'a mut reqwest::Response);

impl ChunkSource for ResponseChunks<'_> {
    async fn next_chunk(&mut self) -> Result<Option<Vec<u8>>, String> {
        match self.0.chunk().await {
            Ok(Some(chunk)) => Ok(Some(chunk.to_vec())),
            Ok(None) => Ok(None),
            Err(error) => Err(error.to_string()),
        }
    }
}

/// Streams the asset into `writer`, hashing and validating it on the fly.
///
/// Kept independent from HTTP and the UI so the chunking, size, checksum and
/// error paths can be tested without a server or a Tauri app handle.
pub(crate) async fn receive_update<S, W>(
    source: &mut S,
    writer: &mut W,
    expected_size: u64,
    expected_sha256: &str,
    max_size: u64,
    mut on_chunk: impl FnMut(u64, f32),
) -> Result<(), ReceiveError>
where
    S: ChunkSource,
    W: AsyncWrite + Unpin,
{
    let mut hasher = Sha256::new();
    let mut received = 0u64;

    loop {
        let chunk = match source.next_chunk().await {
            Ok(Some(chunk)) => chunk,
            Ok(None) => break,
            Err(message) => return Err(ReceiveError::Read(message)),
        };
        writer
            .write_all(&chunk)
            .await
            .map_err(|error| ReceiveError::Write(error.to_string()))?;
        hasher.update(&chunk);
        received += chunk.len() as u64;
        if received > max_size {
            return Err(ReceiveError::TooLarge);
        }
        let percent = if expected_size > 0 {
            (received as f32 / expected_size as f32 * 100.0).min(100.0)
        } else {
            100.0
        };
        on_chunk(received, percent);
    }

    if received != expected_size {
        return Err(ReceiveError::SizeMismatch {
            received,
            expected: expected_size,
        });
    }
    let digest = hex::encode(hasher.finalize());
    if !digest.eq_ignore_ascii_case(expected_sha256) {
        return Err(ReceiveError::ChecksumMismatch);
    }
    Ok(())
}

/// Everything that can go wrong between an HTTP request and a verified part
/// file. The callers turn this into a localized message, because "the update"
/// and "the DMG" read differently.
#[derive(Debug)]
pub(crate) enum AssetError {
    Http(reqwest::StatusCode),
    Request(String),
    File(String),
    Receive(ReceiveError),
}

/// Downloads `url` into `part_path` and validates size, checksum and the hard
/// cap while streaming.
///
/// Shared by the app update and the manual macOS DMG: one implementation, one
/// set of error paths, one place where a partial file is removed.
pub(crate) async fn fetch_to_part(
    client: &reqwest::Client,
    url: &Url,
    part_path: &Path,
    expected_size: u64,
    expected_sha256: &str,
    max_size: u64,
    mut on_progress: impl FnMut(u64, f32),
) -> Result<(), AssetError> {
    if let Some(parent) = part_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|error| AssetError::File(error.to_string()))?;
    }
    let _ = tokio::fs::remove_file(part_path).await;

    let response = client
        .get(url.clone())
        .send()
        .await
        .map_err(|error| AssetError::Request(error.to_string()))?;
    let status = response.status();
    if !status.is_success() {
        return Err(AssetError::Http(status));
    }
    let mut response = response;

    let mut file = tokio::fs::File::create(part_path)
        .await
        .map_err(|error| AssetError::File(error.to_string()))?;
    let mut last_emit = Instant::now();
    let mut source = ResponseChunks(&mut response);
    let streamed = receive_update(
        &mut source,
        &mut file,
        expected_size,
        expected_sha256,
        max_size,
        |downloaded, percent| {
            if last_emit.elapsed() >= PROGRESS_INTERVAL {
                last_emit = Instant::now();
                on_progress(downloaded, percent);
            }
        },
    )
    .await;

    let outcome = match streamed {
        Err(error) => Err(AssetError::Receive(error)),
        Ok(()) => match file.flush().await {
            Err(error) => Err(AssetError::File(error.to_string())),
            Ok(()) => {
                let _ = file.sync_all().await;
                Ok(())
            }
        },
    };
    drop(file);
    if outcome.is_err() {
        let _ = tokio::fs::remove_file(part_path).await;
    }
    outcome
}

fn update_error(error: AssetError) -> String {
    match error {
        AssetError::Http(status) => i18n::tf(
            || format!("сервер обновлений ответил {status}"),
            || format!("the update server returned {status}"),
        ),
        AssetError::Request(message) => i18n::tf(
            || format!("не удалось скачать обновление: {message}"),
            || format!("could not download the update: {message}"),
        ),
        AssetError::File(message) => i18n::tf(
            || format!("не удалось записать обновление: {message}"),
            || format!("could not write the update: {message}"),
        ),
        AssetError::Receive(error) => match error {
            ReceiveError::Read(message) => i18n::tf(
                || format!("не удалось скачать обновление: {message}"),
                || format!("could not download the update: {message}"),
            ),
            ReceiveError::Write(message) => i18n::tf(
                || format!("не удалось записать обновление: {message}"),
                || format!("could not write the update: {message}"),
            ),
            ReceiveError::TooLarge => i18n::t(
                "обновление больше допустимого размера",
                "the update is larger than allowed",
            ),
            ReceiveError::SizeMismatch { received, expected } => i18n::tf(
                || format!("скачано {received} байт вместо {expected}"),
                || format!("downloaded {received} bytes instead of {expected}"),
            ),
            ReceiveError::ChecksumMismatch => i18n::t(
                "контрольная сумма обновления не совпала",
                "the update checksum does not match",
            ),
        },
    }
}

pub(crate) async fn download(app: &AppHandle) -> UpdateStatus {
    let status = app.state::<UpdateStore>().status();
    if matches!(
        status.state.as_str(),
        STATE_DOWNLOADING | STATE_DEFERRED | STATE_INSTALLING
    ) {
        return status;
    }
    let Some(asset) = app.state::<UpdateStore>().asset() else {
        return error_status(
            app,
            i18n::t(
                "сначала проверьте наличие обновления",
                "check for an update first",
            ),
        );
    };

    let paths = app.state::<AppPaths>().inner().clone();
    let final_path = staged_path(&paths, &asset.version);
    if final_path.is_file() {
        let verify_path = final_path.clone();
        let verify_signature = asset.signature.clone();
        let verify_version = asset.version.clone();
        if let Ok(Ok(())) = tokio::task::spawn_blocking(move || {
            verify_file(&verify_path, &verify_signature, &verify_version)
        })
        .await
        {
            return set_status(
                app,
                app.state::<UpdateStore>().update_status(|status| {
                    status.state = STATE_READY.to_string();
                    status.version = Some(asset.version.clone());
                    status.downloaded = asset.size;
                    status.percent = 100.0;
                    status.error = None;
                }),
            );
        }
    }

    let dir = paths.update().join(&asset.version);
    let part_path = dir.join(format!("{}.part", staged_file_name()));

    set_status(
        app,
        app.state::<UpdateStore>().update_status(|status| {
            status.state = STATE_DOWNLOADING.to_string();
            status.version = Some(asset.version.clone());
            status.notes = asset.notes.clone();
            status.size = Some(asset.size);
            status.downloaded = 0;
            status.percent = 0.0;
            status.error = None;
        }),
    );

    let client = match http::client(Duration::from_secs(1800)) {
        Ok(client) => client,
        Err(error) => {
            return error_status(
                app,
                i18n::tf(
                    || format!("не удалось создать HTTP-клиент: {error}"),
                    || format!("could not create the HTTP client: {error}"),
                ),
            );
        }
    };

    let received = fetch_to_part(
        &client,
        &asset.url,
        &part_path,
        asset.size,
        &asset.sha256,
        MAX_UPDATE_BYTES,
        |downloaded, percent| {
            set_status(
                app,
                app.state::<UpdateStore>().update_status(|status| {
                    status.downloaded = downloaded;
                    status.percent = percent;
                }),
            );
        },
    )
    .await;
    if let Err(error) = received {
        return error_status(app, update_error(error));
    }

    let verify_path = part_path.clone();
    let verify_signature = asset.signature.clone();
    let verify_version = asset.version.clone();
    match tokio::task::spawn_blocking(move || {
        verify_file(&verify_path, &verify_signature, &verify_version)
    })
    .await
    {
        Ok(Ok(())) => {}
        Ok(Err(error)) => {
            let _ = tokio::fs::remove_file(&part_path).await;
            return error_status(app, error);
        }
        Err(error) => {
            let _ = tokio::fs::remove_file(&part_path).await;
            return error_status(
                app,
                i18n::tf(
                    || format!("не удалось проверить обновление: {error}"),
                    || format!("could not verify the update: {error}"),
                ),
            );
        }
    }

    let _ = tokio::fs::remove_file(&final_path).await;
    if let Err(error) = tokio::fs::rename(&part_path, &final_path).await {
        let _ = tokio::fs::remove_file(&part_path).await;
        return error_status(
            app,
            i18n::tf(
                || format!("не удалось сохранить обновление: {error}"),
                || format!("could not save the update: {error}"),
            ),
        );
    }

    set_status(
        app,
        app.state::<UpdateStore>().update_status(|status| {
            status.state = STATE_READY.to_string();
            status.version = Some(asset.version.clone());
            status.downloaded = asset.size;
            status.percent = 100.0;
            status.error = None;
        }),
    )
}

#[cfg(target_os = "macos")]
fn dmg_error(error: AssetError) -> String {
    match error {
        AssetError::Http(status) => i18n::tf(
            || format!("сервер обновлений ответил {status}"),
            || format!("the update server returned {status}"),
        ),
        AssetError::Request(message) | AssetError::Receive(ReceiveError::Read(message)) => {
            i18n::tf(
                || format!("не удалось скачать DMG: {message}"),
                || format!("could not download the DMG: {message}"),
            )
        }
        AssetError::File(message) | AssetError::Receive(ReceiveError::Write(message)) => i18n::tf(
            || format!("не удалось записать DMG: {message}"),
            || format!("could not write the DMG: {message}"),
        ),
        AssetError::Receive(ReceiveError::TooLarge) => i18n::t(
            "DMG больше допустимого размера",
            "the DMG is larger than allowed",
        ),
        AssetError::Receive(ReceiveError::SizeMismatch { received, expected }) => i18n::tf(
            || format!("скачано {received} байт вместо {expected}"),
            || format!("downloaded {received} bytes instead of {expected}"),
        ),
        AssetError::Receive(ReceiveError::ChecksumMismatch) => i18n::t(
            "контрольная сумма DMG не совпала",
            "the DMG checksum does not match",
        ),
    }
}

/// Downloads the manual DMG for macOS, validates it and leaves it next to the
/// update folder for the user to mount.
#[cfg(target_os = "macos")]
pub(crate) async fn download_asset_file(
    app: &AppHandle,
    url: &Url,
    size: u64,
    sha256: &str,
    signature: &str,
    version: &str,
    final_path: &Path,
) -> Result<(), String> {
    let part_path = final_path.with_extension("dmg.part");

    set_status(
        app,
        app.state::<UpdateStore>().update_status(|status| {
            status.state = STATE_DOWNLOADING.to_string();
            status.size = Some(size);
            status.downloaded = 0;
            status.percent = 0.0;
            status.error = None;
        }),
    );

    let client = http::client(Duration::from_secs(1800)).map_err(|error| {
        i18n::tf(
            || format!("не удалось создать HTTP-клиент: {error}"),
            || format!("could not create the HTTP client: {error}"),
        )
    })?;

    fetch_to_part(
        &client,
        url,
        &part_path,
        size,
        sha256,
        MAX_UPDATE_BYTES,
        |downloaded, percent| {
            set_status(
                app,
                app.state::<UpdateStore>().update_status(|status| {
                    status.downloaded = downloaded;
                    status.percent = percent;
                }),
            );
        },
    )
    .await
    .map_err(dmg_error)?;

    let verify_path = part_path.clone();
    let verify_signature = signature.to_string();
    let verify_version = version.to_string();
    let verified = tokio::task::spawn_blocking(move || {
        verify_file(&verify_path, &verify_signature, &verify_version)
    })
    .await
    .map_err(|error| error.to_string())?;
    if let Err(error) = verified {
        let _ = tokio::fs::remove_file(&part_path).await;
        return Err(error);
    }

    let _ = tokio::fs::remove_file(final_path).await;
    tokio::fs::rename(&part_path, final_path)
        .await
        .map_err(|error| {
            i18n::tf(
                || format!("не удалось сохранить DMG: {error}"),
                || format!("could not save the DMG: {error}"),
            )
        })
}
