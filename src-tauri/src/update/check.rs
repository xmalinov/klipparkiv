use std::time::Duration;

use tauri::{AppHandle, Manager};

use crate::i18n;
use crate::update::manifest::{MAX_MANIFEST_BYTES, is_configured, parse_manifest, update_url};
use crate::update::state::{
    STATE_AVAILABLE, STATE_CHECKING, STATE_CURRENT, STATE_DEFERRED, STATE_DOWNLOADING, STATE_IDLE,
    STATE_INSTALLING, STATE_READY, UpdateStatus, UpdateStore, error_status, set_status,
};
pub(crate) async fn fetch_manifest() -> Result<String, String> {
    let client = crate::update::http::client(Duration::from_secs(30)).map_err(|error| {
        i18n::tf(
            || format!("не удалось создать HTTP-клиент: {error}"),
            || format!("could not create the HTTP client: {error}"),
        )
    })?;

    let response = client
        .get(update_url())
        .header(reqwest::header::ACCEPT, "application/json")
        .send()
        .await
        .map_err(|error| {
            i18n::tf(
                || format!("не удалось проверить обновления: {error}"),
                || format!("could not check for updates: {error}"),
            )
        })?;

    if !response.status().is_success() {
        return Err(i18n::tf(
            || format!("сервер обновлений ответил {}", response.status()),
            || format!("the update server returned {}", response.status()),
        ));
    }

    let bytes = response.bytes().await.map_err(|error| {
        i18n::tf(
            || format!("не удалось прочитать манифест: {error}"),
            || format!("could not read the manifest: {error}"),
        )
    })?;
    if bytes.len() > MAX_MANIFEST_BYTES {
        return Err(i18n::t(
            "манифест обновления слишком большой",
            "the update manifest is too large",
        ));
    }
    String::from_utf8(bytes.to_vec()).map_err(|error| {
        i18n::tf(
            || format!("манифест обновления не в UTF-8: {error}"),
            || format!("the update manifest is not UTF-8: {error}"),
        )
    })
}

pub(crate) async fn check(app: &AppHandle, silent: bool) -> UpdateStatus {
    let current = app.state::<UpdateStore>().status();
    if matches!(
        current.state.as_str(),
        STATE_DOWNLOADING | STATE_READY | STATE_DEFERRED | STATE_INSTALLING
    ) {
        return current;
    }

    if !is_configured() {
        if silent {
            return current;
        }
        return error_status(
            app,
            i18n::t(
                "обновления не настроены: отсутствует публичный ключ",
                "updates are not configured: the public key is missing",
            ),
        );
    }

    set_status(
        app,
        app.state::<UpdateStore>().update_status(|status| {
            status.state = STATE_CHECKING.to_string();
            status.error = None;
        }),
    );

    match fetch_manifest().await.and_then(|raw| parse_manifest(&raw)) {
        Ok(Some(asset)) => {
            let version = asset.version.clone();
            let notes = asset.notes.clone();
            let size = asset.size;
            app.state::<UpdateStore>().set_asset(Some(asset));
            set_status(
                app,
                app.state::<UpdateStore>().update_status(|status| {
                    status.state = STATE_AVAILABLE.to_string();
                    status.version = Some(version);
                    status.notes = notes;
                    status.size = Some(size);
                    status.downloaded = 0;
                    status.percent = 0.0;
                    status.error = None;
                }),
            )
        }
        Ok(None) => set_status(
            app,
            app.state::<UpdateStore>().update_status(|status| {
                status.state = STATE_CURRENT.to_string();
                status.version = None;
                status.notes = None;
                status.size = None;
                status.error = None;
            }),
        ),
        Err(message) => {
            if silent {
                set_status(
                    app,
                    app.state::<UpdateStore>().update_status(|status| {
                        status.state = STATE_IDLE.to_string();
                        status.error = None;
                    }),
                )
            } else {
                error_status(app, message)
            }
        }
    }
}
