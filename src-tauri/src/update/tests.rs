use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::{Duration, SystemTime};

use serde_json::json;
use sha2::{Digest, Sha256};
use tokio::io::AsyncWrite;
use url::Url;

use minisign_verify::{PublicKey, Signature};

use crate::paths::AppPaths;
use crate::update::download::{
    AssetError, ChunkSource, ReceiveError, fetch_to_part, receive_update,
};
use crate::update::install_macos::extract_app_tarball;
#[cfg(target_os = "macos")]
use crate::update::install_macos::verify_macos_bundle;
use crate::update::maintenance::cleanup_stale_state;
use crate::update::manifest::{
    MAX_UPDATE_BYTES, current_version, is_configured, parse_manifest, staged_file_name,
    update_kind, update_target, validate_update_url,
};
use crate::update::state::{STATE_IDLE, STATE_READY, StagedAsset, UpdateStatus, UpdateStore};
use crate::update::verify::{
    decode_signature, signed_version, staged_path, verify_file, verify_signature_only,
};

const MANIFEST: &str = r#"{
        "schema": 1,
        "version": "99.0.0",
        "minVersion": "1.0.0",
        "notes": "Test release",
        "platforms": {
            "windows-x86_64": {
                "kind": "exe",
                "url": "https://github.com/xmalinov/klipparkiv/releases/download/v99.0.0/klipparkiv-99.0.0-win-x64.exe",
                "size": 100,
                "sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "signature": "signature"
            },
            "darwin-x86_64": {
                "kind": "app-tar-gz",
                "url": "https://github.com/xmalinov/klipparkiv/releases/download/v99.0.0/klipparkiv-99.0.0-mac-x64.app.tar.gz",
                "size": 100,
                "sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "signature": "signature",
                "manual": {
                    "kind": "dmg",
                    "url": "https://github.com/xmalinov/klipparkiv/releases/download/v99.0.0/klipparkiv-99.0.0-mac-x64.dmg",
                    "size": 100,
                    "sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                    "signature": "signature"
                }
            },
            "darwin-aarch64": {
                "kind": "app-tar-gz",
                "url": "https://github.com/xmalinov/klipparkiv/releases/download/v99.0.0/klipparkiv-99.0.0-mac-arm64.app.tar.gz",
                "size": 100,
                "sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "signature": "signature",
                "manual": {
                    "kind": "dmg",
                    "url": "https://github.com/xmalinov/klipparkiv/releases/download/v99.0.0/klipparkiv-99.0.0-mac-arm64.dmg",
                    "size": 100,
                    "sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                    "signature": "signature"
                }
            }
        }
    }"#;

#[test]
fn parses_manifest() {
    let asset = parse_manifest(MANIFEST).unwrap().unwrap();
    assert_eq!(asset.version, "99.0.0");
    assert_eq!(asset.size, 100);
    assert_eq!(asset.notes.as_deref(), Some("Test release"));
}

#[test]
fn update_target_matches_the_host() {
    let expected = if cfg!(windows) {
        "windows-x86_64"
    } else if cfg!(target_arch = "aarch64") {
        "darwin-aarch64"
    } else {
        "darwin-x86_64"
    };
    assert_eq!(update_target(), expected);
}

#[test]
fn parses_the_platform_asset() {
    let asset = parse_manifest(MANIFEST).unwrap().unwrap();
    if cfg!(target_os = "macos") {
        assert!(asset.manual.is_some());
    }
}

#[test]
fn staged_filename_matches_the_platform() {
    if cfg!(windows) {
        assert_eq!(staged_file_name(), "klipparkiv.exe");
    } else {
        assert_eq!(staged_file_name(), "Klipparkiv.app.tar.gz");
    }
}

#[cfg(target_os = "macos")]
#[test]
fn parses_the_manual_dmg_asset() {
    let asset = parse_manifest(MANIFEST).unwrap().unwrap();
    let manual = asset.manual.expect("macOS manifest must have a DMG");
    assert_eq!(manual.size, 100);
    assert_eq!(manual.url.scheme(), "https");
}

#[cfg(target_os = "macos")]
#[test]
fn rejects_the_wrong_update_kind() {
    let wrong = MANIFEST.replace("app-tar-gz", "exe");
    assert!(parse_manifest(&wrong).is_err());
}

#[test]
fn ignores_old_version() {
    let old = MANIFEST.replace("99.0.0", "0.0.1");
    assert!(parse_manifest(&old).unwrap().is_none());
}

#[test]
fn rejects_insecure_url() {
    let insecure = MANIFEST.replace("https://", "ftp://");
    assert!(parse_manifest(&insecure).is_err());
}

#[test]
fn extracts_signed_version() {
    assert_eq!(
        signed_version("timestamp:1\tfile:app.exe\tversion:1.2.3"),
        Some("1.2.3")
    );
    assert_eq!(signed_version("timestamp:1\tfile:app.exe"), None);
}

#[test]
fn verifies_minisign_signature() {
    // Test vector from the minisign-verify documentation.
    let public_key = "RWQf6LRCGA9i53mlYecO4IzT51TGPpvWucNSCh1CBM0QTaLn73Y7GFO3";
    let signature = "untrusted comment: signature from minisign secret key\nRUQf6LRCGA9i559r3g7V1qNyJDApGip8MfqcadIgT9CuhV3EMhHoN1mGTkUidF/z7SrlQgXdy8ofjb7bNJJylDOocrCo8KLzZwo=\ntrusted comment: timestamp:1633700835\tfile:test\tprehashed\nwLMDjy9FLAuxZ3q4NlEvkgtyhrr0gtTu6KC4KBJdITbbOeAi1zBIYo0v4iTgt8jJpIidRJnp94ABQkJAgAooBQ==";
    let key = PublicKey::from_base64(public_key).unwrap();
    let signature = Signature::decode(signature).unwrap();
    key.verify(b"test", &signature, false).unwrap();
}

#[test]
fn decodes_tauri_base64_signature() {
    use base64::Engine;

    let raw = "untrusted comment: signature from minisign secret key\nRUQf6LRCGA9i559r3g7V1qNyJDApGip8MfqcadIgT9CuhV3EMhHoN1mGTkUidF/z7SrlQgXdy8ofjb7bNJJylDOocrCo8KLzZwo=\ntrusted comment: timestamp:1633700835\tfile:test\tprehashed\nwLMDjy9FLAuxZ3q4NlEvkgtyhrr0gtTu6KC4KBJdITbbOeAi1zBIYo0v4iTgt8jJpIidRJnp94ABQkJAgAooBQ==";
    let encoded = base64::engine::general_purpose::STANDARD.encode(raw);
    let signature = decode_signature(&encoded).unwrap();
    assert!(signature.trusted_comment().contains("file:test"));
}

#[test]
fn cleans_stale_updates_and_keeps_the_newest_backup() {
    use std::time::SystemTime;

    fn set_age(path: &Path, age: Duration) {
        let file = std::fs::OpenOptions::new().write(true).open(path).unwrap();
        let times = std::fs::FileTimes::new().set_modified(SystemTime::now() - age);
        file.set_times(times).unwrap();
    }

    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::portable(temp.path());
    std::fs::create_dir_all(paths.update()).unwrap();
    std::fs::create_dir_all(paths.update_backups()).unwrap();

    let old_part = paths.update().join("old.part");
    let new_part = paths.update().join("new.part");
    std::fs::write(&old_part, b"old").unwrap();
    std::fs::write(&new_part, b"new").unwrap();
    set_age(&old_part, Duration::from_secs(10 * 24 * 60 * 60));
    set_age(&new_part, Duration::from_secs(60));

    let old_backup = paths.update_backups().join("old.exe");
    let new_backup = paths.update_backups().join("new.exe");
    std::fs::write(&old_backup, b"old").unwrap();
    std::fs::write(&new_backup, b"new").unwrap();
    set_age(&old_backup, Duration::from_secs(10 * 24 * 60 * 60));
    set_age(&new_backup, Duration::from_secs(60));

    let log = paths.updater_log();
    std::fs::write(&log, vec![0u8; 1024 * 1024 + 1]).unwrap();

    cleanup_stale_state(&paths);

    assert!(!old_part.exists());
    assert!(new_part.exists());
    assert!(!old_backup.exists());
    assert!(new_backup.exists());
    assert!(!log.exists());
    assert!(log.with_extension("log.old").is_file());
}

#[test]
fn verifies_a_tauri_signed_fixture() {
    let data = include_bytes!("../../tests/fixtures/update-1.2.3.txt");
    let signature = include_str!("../../tests/fixtures/update-1.2.3.txt.sig");
    let signature = verify_signature_only(data, signature).unwrap();
    assert_eq!(signed_version(signature.trusted_comment()), Some("1.2.3"));
}

#[test]
fn verifies_the_release_artifact_when_configured() {
    let Ok(file) = std::env::var("UPDATE_VERIFY_FILE") else {
        return;
    };
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("src-tauri must have a parent");
    let file = if Path::new(&file).is_absolute() {
        PathBuf::from(file)
    } else {
        repo_root.join(file)
    };
    let signature_path = std::env::var("UPDATE_VERIFY_SIG").expect("UPDATE_VERIFY_SIG is required");
    let signature_path = if Path::new(&signature_path).is_absolute() {
        PathBuf::from(signature_path)
    } else {
        repo_root.join(signature_path)
    };
    let version =
        std::env::var("UPDATE_VERIFY_VERSION").expect("UPDATE_VERIFY_VERSION is required");
    let signature = std::fs::read_to_string(signature_path).unwrap();
    verify_file(&file, &signature, &version).unwrap();
}

fn base_asset() -> serde_json::Value {
    json!({
        "kind": update_kind(),
        "url": "https://example.com/download/app",
        "size": 100,
        "sha256": "a".repeat(64),
        "signature": "signature"
    })
}

fn manifest_with(asset: serde_json::Value) -> serde_json::Value {
    let mut platforms = serde_json::Map::new();
    platforms.insert(update_target().to_string(), asset);
    json!({
        "schema": 1,
        "version": "99.0.0",
        "platforms": platforms
    })
}

fn parse_asset(asset: serde_json::Value) -> Result<Option<StagedAsset>, String> {
    parse_manifest(&manifest_with(asset).to_string())
}

#[test]
fn the_base_manifest_is_accepted() {
    let asset = parse_asset(base_asset()).unwrap().unwrap();
    assert_eq!(asset.version, "99.0.0");
    assert_eq!(asset.url.scheme(), "https");
}

#[test]
fn rejects_an_unsupported_manifest_schema() {
    let mut manifest = manifest_with(base_asset());
    manifest["schema"] = json!(2);
    assert!(parse_manifest(&manifest.to_string()).is_err());
}

#[test]
fn rejects_malformed_manifest_json() {
    assert!(parse_manifest("{").is_err());
    assert!(parse_manifest("null").is_err());
}

#[test]
fn rejects_an_invalid_remote_version() {
    let mut manifest = manifest_with(base_asset());
    manifest["version"] = json!("not-a-version");
    assert!(parse_manifest(&manifest.to_string()).is_err());
}

#[test]
fn ignores_the_current_version() {
    let mut manifest = manifest_with(base_asset());
    manifest["version"] = json!(current_version());
    assert!(parse_manifest(&manifest.to_string()).unwrap().is_none());
}

#[test]
fn requires_a_build_for_this_platform() {
    let mut manifest = manifest_with(base_asset());
    manifest["platforms"] = json!({ "linux-x86_64": base_asset() });
    let error = parse_manifest(&manifest.to_string()).unwrap_err();
    assert!(error.contains(update_target()));
}

#[test]
fn enforces_the_manifest_minimum_version() {
    let mut manifest = manifest_with(base_asset());
    manifest["minVersion"] = json!("99.0.0");
    let error = parse_manifest(&manifest.to_string()).unwrap_err();
    assert!(error.contains("99.0.0"));
}

#[test]
fn the_asset_minimum_version_overrides_the_manifest_one() {
    let mut manifest = manifest_with(base_asset());
    manifest["minVersion"] = json!("0.0.1");
    manifest["platforms"][update_target()]["minVersion"] = json!("99.0.0");
    let error = parse_manifest(&manifest.to_string()).unwrap_err();
    assert!(error.contains("99.0.0"));
}

#[test]
fn rejects_an_invalid_minimum_version() {
    let mut manifest = manifest_with(base_asset());
    manifest["minVersion"] = json!("soon");
    assert!(parse_manifest(&manifest.to_string()).is_err());
}

#[test]
fn rejects_a_foreign_update_kind() {
    let mut asset = base_asset();
    asset["kind"] = json!("zip");
    assert!(parse_asset(asset).is_err());
}

#[test]
fn rejects_impossible_asset_sizes() {
    let mut empty = base_asset();
    empty["size"] = json!(0);
    assert!(parse_asset(empty).is_err());

    let mut huge = base_asset();
    huge["size"] = json!(MAX_UPDATE_BYTES + 1);
    assert!(parse_asset(huge).is_err());
}

#[test]
fn rejects_invalid_checksums() {
    let mut short = base_asset();
    short["sha256"] = json!("abc");
    assert!(parse_asset(short).is_err());

    let mut non_hex = base_asset();
    non_hex["sha256"] = json!("z".repeat(64));
    assert!(parse_asset(non_hex).is_err());
}

#[test]
fn rejects_an_unsigned_update() {
    let mut asset = base_asset();
    asset["signature"] = json!("   ");
    assert!(parse_asset(asset).is_err());
}

#[test]
fn normalises_the_checksum_and_truncates_notes() {
    let mut asset = base_asset();
    asset["sha256"] = json!("A".repeat(64));
    asset["signature"] = json!("  signature  ");

    let mut manifest = manifest_with(asset);
    manifest["notes"] = json!("x".repeat(9_000));

    let parsed = parse_manifest(&manifest.to_string()).unwrap().unwrap();
    assert_eq!(parsed.sha256, "a".repeat(64));
    assert_eq!(parsed.notes.as_deref().map(str::len), Some(8_000));
    assert_eq!(parsed.signature, "  signature  ");
}

#[test]
fn parses_a_manual_dmg_asset_on_any_platform() {
    let mut asset = base_asset();
    asset["manual"] = json!({
        "kind": "dmg",
        "url": "https://example.com/download/app.dmg",
        "size": 100,
        "sha256": "B".repeat(64),
        "signature": "dmg-signature"
    });

    let parsed = parse_asset(asset).unwrap().unwrap();
    let manual = parsed.manual.expect("manual asset");
    assert_eq!(manual.sha256, "b".repeat(64));
    assert_eq!(manual.url.scheme(), "https");
}

#[test]
fn validates_the_manual_dmg_asset() {
    let manual = |kind: &str, size: u64, sha: String, signature: &str| {
        json!({
            "kind": kind,
            "url": "https://example.com/download/app.dmg",
            "size": size,
            "sha256": sha,
            "signature": signature
        })
    };

    let mut wrong_kind = base_asset();
    wrong_kind["manual"] = manual("zip", 100, "b".repeat(64), "sig");
    assert!(parse_asset(wrong_kind).is_err());

    let mut empty = base_asset();
    empty["manual"] = manual("dmg", 0, "b".repeat(64), "sig");
    assert!(parse_asset(empty).is_err());

    let mut bad_sha = base_asset();
    bad_sha["manual"] = manual("dmg", 100, "abc".into(), "sig");
    assert!(parse_asset(bad_sha).is_err());

    let mut unsigned = base_asset();
    unsigned["manual"] = manual("dmg", 100, "b".repeat(64), "  ");
    assert!(parse_asset(unsigned).is_err());
}

#[test]
fn validates_update_urls() {
    assert!(validate_update_url("https://example.com/update.json").is_ok());
    assert!(validate_update_url("not a url").is_err());
    assert!(validate_update_url("ftp://example.com/update.json").is_err());
    assert!(validate_update_url("file:///etc/passwd").is_err());
    if cfg!(debug_assertions) {
        assert!(validate_update_url("http://example.com/update.json").is_ok());
    } else {
        assert!(validate_update_url("http://example.com/update.json").is_err());
    }
}

#[test]
fn default_status_reports_the_running_version() {
    let status = UpdateStatus::default();
    assert_eq!(status.state, STATE_IDLE);
    assert_eq!(status.current_version, current_version());
    assert_eq!(status.downloaded, 0);
    assert_eq!(status.percent, 0.0);
    assert!(status.version.is_none());
}

#[test]
fn the_store_keeps_status_and_the_staged_asset() {
    let store = UpdateStore::default();
    assert!(store.asset().is_none());

    let updated = store.update_status(|status| {
        status.state = STATE_READY.into();
        status.percent = 42.0;
    });
    assert_eq!(updated.state, STATE_READY);
    assert_eq!(store.status().percent, 42.0);

    let asset = parse_asset(base_asset()).unwrap().unwrap();
    store.set_asset(Some(asset));
    assert_eq!(store.asset().unwrap().version, "99.0.0");

    store.set_asset(None);
    assert!(store.asset().is_none());
}

#[test]
fn the_staged_path_is_scoped_to_the_version() {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::portable(temp.path());
    let path = staged_path(&paths, "1.2.3");
    assert!(path.ends_with(Path::new("1.2.3").join(staged_file_name())));
}

#[test]
fn the_embedded_public_key_is_valid() {
    assert!(is_configured());
}

#[test]
fn a_bad_signature_encoding_is_reported() {
    assert!(decode_signature("not a minisign signature").is_err());

    use base64::Engine;
    let invalid_utf8 = base64::engine::general_purpose::STANDARD.encode([0xff, 0xfe]);
    let error = match decode_signature(&invalid_utf8) {
        Ok(_) => panic!("invalid UTF-8 must be rejected"),
        Err(error) => error,
    };
    assert!(!error.is_empty());
}

#[test]
fn verify_file_accepts_the_fixture_and_rejects_mismatches() {
    let data = include_bytes!("../../tests/fixtures/update-1.2.3.txt");
    let signature = include_str!("../../tests/fixtures/update-1.2.3.txt.sig");
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("update.bin");
    std::fs::write(&file, data).unwrap();

    verify_file(&file, signature, "1.2.3").unwrap();
    assert!(verify_file(&file, signature, "2.0.0").is_err());

    std::fs::write(&file, b"tampered").unwrap();
    assert!(verify_file(&file, signature, "1.2.3").is_err());

    assert!(verify_file(Path::new("missing-update.bin"), signature, "1.2.3").is_err());
}

fn write_tarball(path: &Path, entries: &[(&str, &[u8])]) {
    let file = std::fs::File::create(path).expect("tarball");
    let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
    let mut builder = tar::Builder::new(encoder);
    for (name, data) in entries {
        let mut header = tar::Header::new_gnu();
        header.set_size(data.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder
            .append_data(&mut header, name, *data)
            .expect("tar entry");
    }
    builder.finish().expect("tar finish");
}

#[test]
fn extracts_a_valid_app_tarball() {
    let temp = tempfile::tempdir().unwrap();
    let archive = temp.path().join("update.tar.gz");
    let dest = temp.path().join("extracted");
    write_tarball(
        &archive,
        &[
            ("Klipparkiv.app/Contents/Info.plist", b"plist"),
            ("Klipparkiv.app/Contents/MacOS/klipparkiv", b"binary"),
        ],
    );

    let bundle = extract_app_tarball(&archive, &dest).unwrap();

    assert_eq!(bundle, dest.join("Klipparkiv.app"));
    assert!(bundle.is_dir());
    assert_eq!(
        std::fs::read_to_string(bundle.join("Contents").join("Info.plist")).unwrap(),
        "plist"
    );
}

#[test]
fn extraction_clears_the_previous_folder() {
    let temp = tempfile::tempdir().unwrap();
    let archive = temp.path().join("update.tar.gz");
    let dest = temp.path().join("extracted");
    std::fs::create_dir_all(&dest).unwrap();
    std::fs::write(dest.join("stale.txt"), b"stale").unwrap();
    write_tarball(&archive, &[("Klipparkiv.app/Contents/Info.plist", b"new")]);

    extract_app_tarball(&archive, &dest).unwrap();

    assert!(!dest.join("stale.txt").exists());
}

#[test]
fn rejects_a_tarball_with_another_app() {
    let temp = tempfile::tempdir().unwrap();
    let archive = temp.path().join("update.tar.gz");
    let dest = temp.path().join("extracted");
    write_tarball(&archive, &[("Other.app/Contents/Info.plist", b"plist")]);

    assert!(extract_app_tarball(&archive, &dest).is_err());
}

#[test]
fn rejects_a_tarball_with_multiple_apps() {
    let temp = tempfile::tempdir().unwrap();
    let archive = temp.path().join("update.tar.gz");
    let dest = temp.path().join("extracted");
    write_tarball(
        &archive,
        &[
            ("Klipparkiv.app/Contents/Info.plist", b"plist"),
            ("Other.app/Contents/Info.plist", b"plist"),
        ],
    );

    assert!(extract_app_tarball(&archive, &dest).is_err());
}

#[test]
fn rejects_an_empty_or_corrupt_tarball() {
    let temp = tempfile::tempdir().unwrap();
    let dest = temp.path().join("extracted");

    let empty = temp.path().join("empty.tar.gz");
    write_tarball(&empty, &[]);
    assert!(extract_app_tarball(&empty, &dest).is_err());

    let corrupt = temp.path().join("corrupt.tar.gz");
    std::fs::write(&corrupt, b"definitely not gzip").unwrap();
    assert!(extract_app_tarball(&corrupt, &dest).is_err());

    assert!(extract_app_tarball(&temp.path().join("missing.tar.gz"), &dest).is_err());
}

#[test]
fn rejects_a_bundle_entry_that_is_not_a_folder() {
    let temp = tempfile::tempdir().unwrap();
    let archive = temp.path().join("update.tar.gz");
    let dest = temp.path().join("extracted");
    write_tarball(&archive, &[("Klipparkiv.app", b"not a folder")]);

    assert!(extract_app_tarball(&archive, &dest).is_err());
}

struct ScriptedChunks(VecDeque<Result<Option<Vec<u8>>, String>>);

impl ChunkSource for ScriptedChunks {
    async fn next_chunk(&mut self) -> Result<Option<Vec<u8>>, String> {
        self.0.pop_front().unwrap_or(Ok(None))
    }
}

struct FailingWriter;

impl AsyncWrite for FailingWriter {
    fn poll_write(
        self: Pin<&mut Self>,
        _context: &mut Context<'_>,
        _buffer: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Poll::Ready(Err(std::io::Error::other("disk full")))
    }

    fn poll_flush(self: Pin<&mut Self>, _context: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(
        self: Pin<&mut Self>,
        _context: &mut Context<'_>,
    ) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

fn sha256(data: &[u8]) -> String {
    hex::encode(Sha256::digest(data))
}

async fn stream(
    chunks: Vec<Result<Option<Vec<u8>>, String>>,
    size: u64,
    sha: &str,
    max: u64,
) -> (Result<(), ReceiveError>, Vec<(u64, f32)>) {
    let mut source = ScriptedChunks(chunks.into());
    let mut writer = tokio::io::sink();
    let mut progress = Vec::new();
    let result = receive_update(
        &mut source,
        &mut writer,
        size,
        sha,
        max,
        |received, percent| progress.push((received, percent)),
    )
    .await;
    (result, progress)
}

#[tokio::test]
async fn receives_chunks_in_order_and_reports_progress() {
    let data = b"abcdef";
    let chunks = vec![Ok(Some(b"abc".to_vec())), Ok(Some(b"def".to_vec()))];

    let (result, progress) = stream(chunks, data.len() as u64, &sha256(data), 1_000).await;

    assert!(result.is_ok());
    assert_eq!(progress, vec![(3, 50.0), (6, 100.0)]);
}

#[tokio::test]
async fn rejects_a_short_download() {
    let chunks = vec![Ok(Some(b"abc".to_vec()))];

    let (result, _) = stream(chunks, 10, &sha256(b"abc"), 1_000).await;

    assert!(matches!(
        result,
        Err(ReceiveError::SizeMismatch {
            received: 3,
            expected: 10
        })
    ));
}

#[tokio::test]
async fn rejects_a_checksum_mismatch() {
    let chunks = vec![Ok(Some(b"abc".to_vec()))];

    let (result, _) = stream(chunks, 3, &"0".repeat(64), 1_000).await;

    assert!(matches!(result, Err(ReceiveError::ChecksumMismatch)));
}

#[tokio::test]
async fn rejects_an_oversized_download() {
    let chunks = vec![Ok(Some(b"abcd".to_vec()))];

    let (result, _) = stream(chunks, 4, &sha256(b"abcd"), 2).await;

    assert!(matches!(result, Err(ReceiveError::TooLarge)));
}

#[tokio::test]
async fn reports_read_errors() {
    let chunks = vec![Err("connection reset".to_string())];

    let (result, _) = stream(chunks, 3, &sha256(b"abc"), 1_000).await;

    assert!(matches!(
        result,
        Err(ReceiveError::Read(message)) if message == "connection reset"
    ));
}

#[tokio::test]
async fn reports_write_errors() {
    let mut source = ScriptedChunks(VecDeque::from(vec![Ok(Some(b"abc".to_vec()))]));
    let mut writer = FailingWriter;

    let result = receive_update(
        &mut source,
        &mut writer,
        3,
        &sha256(b"abc"),
        1_000,
        |_, _| {},
    )
    .await;

    assert!(matches!(result, Err(ReceiveError::Write(_))));
}

#[tokio::test]
async fn caps_progress_at_one_hundred_percent() {
    let chunks = vec![Ok(Some(b"abcd".to_vec()))];

    let (result, progress) = stream(chunks, 2, &sha256(b"abcd"), 1_000).await;

    assert_eq!(progress, vec![(4, 100.0)]);
    assert!(matches!(result, Err(ReceiveError::SizeMismatch { .. })));
}

#[tokio::test]
async fn accepts_an_empty_payload() {
    let (result, progress) = stream(Vec::new(), 0, &sha256(b""), 1_000).await;

    assert!(result.is_ok());
    assert!(progress.is_empty());
}

#[test]
fn cleanup_keeps_only_the_newest_backup() {
    fn set_age(path: &Path, age: Duration) {
        let file = std::fs::OpenOptions::new().write(true).open(path).unwrap();
        let times = std::fs::FileTimes::new().set_modified(SystemTime::now() - age);
        file.set_times(times).unwrap();
    }

    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::portable(temp.path());
    std::fs::create_dir_all(paths.update_backups()).unwrap();

    let oldest = paths.update_backups().join("oldest.exe");
    let middle = paths.update_backups().join("middle.exe");
    let newest = paths.update_backups().join("newest.exe");
    for path in [&oldest, &middle, &newest] {
        std::fs::write(path, b"backup").unwrap();
    }
    set_age(&oldest, Duration::from_secs(30 * 24 * 60 * 60));
    set_age(&middle, Duration::from_secs(20 * 24 * 60 * 60));
    set_age(&newest, Duration::from_secs(60));

    cleanup_stale_state(&paths);

    assert!(!oldest.exists());
    assert!(!middle.exists());
    assert!(newest.exists());
}

#[test]
fn cleanup_keeps_a_small_updater_log() {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::portable(temp.path());
    std::fs::create_dir_all(paths.update()).unwrap();
    let log = paths.updater_log();
    std::fs::write(&log, b"small log").unwrap();

    cleanup_stale_state(&paths);

    assert!(log.exists());
    assert!(!log.with_extension("log.old").exists());
}

#[test]
fn cleanup_is_safe_without_update_directories() {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::portable(temp.path());

    cleanup_stale_state(&paths);
    cleanup_stale_state(&paths);
}

#[cfg(all(test, target_os = "macos"))]
mod macos_tests {
    use super::*;

    #[test]
    fn extracts_and_verifies_the_built_app_archive() {
        let version = env!("CARGO_PKG_VERSION");
        let tarball = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../release/klipparkiv-{version}-mac-x64.app.tar.gz"
        ));
        if !tarball.is_file() {
            return;
        }
        let temp = tempfile::tempdir().unwrap();
        let bundle = extract_app_tarball(&tarball, &temp.path().join("extract")).unwrap();
        verify_macos_bundle(&bundle, version).unwrap();
    }
}

fn sha_hex(data: &[u8]) -> String {
    hex::encode(Sha256::digest(data))
}

async fn fetch_over_http(
    status: &'static str,
    body: Vec<u8>,
    expected_size: u64,
    expected_sha: &str,
    max_size: u64,
) -> (Result<(), AssetError>, tempfile::TempDir, PathBuf) {
    let url = Url::parse(&crate::update::tests_support::serve_once(status, body)).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let part = dir.path().join("asset.part");
    let client = crate::update::http::client(Duration::from_secs(5)).unwrap();
    let result = fetch_to_part(
        &client,
        &url,
        &part,
        expected_size,
        expected_sha,
        max_size,
        |_, _| {},
    )
    .await;
    (result, dir, part)
}

#[tokio::test]
async fn downloads_an_asset_into_the_part_file() {
    let body = b"abcdef".to_vec();

    let (result, dir, part) =
        fetch_over_http("200 OK", body.clone(), 6, &sha_hex(&body), 1024).await;

    assert!(result.is_ok());
    assert_eq!(std::fs::read(&part).unwrap(), body);
    drop(dir);
}

#[tokio::test]
async fn maps_an_http_error_status_and_leaves_no_part_file() {
    let body = b"missing".to_vec();

    let (result, _dir, part) =
        fetch_over_http("404 Not Found", body.clone(), 7, &sha_hex(&body), 1024).await;

    assert!(
        matches!(result, Err(AssetError::Http(status)) if status == reqwest::StatusCode::NOT_FOUND),
        "unexpected result: {result:?}"
    );
    assert!(!part.exists());
}

#[tokio::test]
async fn a_checksum_mismatch_removes_the_part_file() {
    let (result, _dir, part) =
        fetch_over_http("200 OK", b"abcdef".to_vec(), 6, &"0".repeat(64), 1024).await;

    assert!(matches!(
        result,
        Err(AssetError::Receive(ReceiveError::ChecksumMismatch))
    ));
    assert!(!part.exists());
}

#[tokio::test]
async fn a_body_over_the_cap_is_rejected() {
    let (result, _dir, part) = fetch_over_http(
        "200 OK",
        b"0123456789".to_vec(),
        10,
        &sha_hex(b"0123456789"),
        4,
    )
    .await;

    assert!(matches!(
        result,
        Err(AssetError::Receive(ReceiveError::TooLarge))
    ));
    assert!(!part.exists());
}

#[tokio::test]
async fn a_truncated_body_is_rejected_with_the_byte_counts() {
    let (result, _dir, part) =
        fetch_over_http("200 OK", b"abc".to_vec(), 10, &sha_hex(b"abc"), 1024).await;

    assert!(
        matches!(
            result,
            Err(AssetError::Receive(ReceiveError::SizeMismatch {
                received: 3,
                expected: 10
            }))
        ),
        "unexpected result: {result:?}"
    );
    assert!(!part.exists());
}

#[tokio::test]
async fn accepts_an_empty_http_body() {
    let (result, _dir, part) = fetch_over_http("200 OK", Vec::new(), 0, &sha_hex(b""), 16).await;

    assert!(result.is_ok());
    assert_eq!(std::fs::read(&part).unwrap(), Vec::<u8>::new());
}
