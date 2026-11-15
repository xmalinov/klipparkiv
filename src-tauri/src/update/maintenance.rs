use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tauri::AppHandle;

use crate::paths::AppPaths;
use crate::update::check::check;
/// Removes old partial downloads, extracted bundles and extra backups.
/// Called once at startup; failures are best-effort.
pub(crate) fn cleanup_stale_state(paths: &AppPaths) {
    fn older_than(path: &Path, max_age: Duration) -> bool {
        let Ok(modified) = std::fs::metadata(path).and_then(|meta| meta.modified()) else {
            return false;
        };
        SystemTime::now()
            .duration_since(modified)
            .map(|age| age > max_age)
            .unwrap_or(false)
    }

    let max_age = Duration::from_secs(7 * 24 * 60 * 60);
    let update = paths.update();
    if let Ok(entries) = std::fs::read_dir(&update) {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name();
            if name == std::ffi::OsStr::new("backup") {
                continue;
            }
            if !older_than(&path, max_age) {
                continue;
            }
            let result = if path.is_dir() {
                std::fs::remove_dir_all(&path)
            } else {
                std::fs::remove_file(&path)
            };
            let _ = result;
        }
    }

    // Keep the newest backup for rollback; remove older copies.
    let backups = paths.update_backups();
    if let Ok(entries) = std::fs::read_dir(&backups) {
        let mut items: Vec<_> = entries.flatten().map(|entry| entry.path()).collect();
        items.sort_by_key(|path| {
            std::fs::metadata(path)
                .and_then(|meta| meta.modified())
                .unwrap_or(UNIX_EPOCH)
        });
        for path in items.into_iter().rev().skip(1) {
            let _ = if path.is_dir() {
                std::fs::remove_dir_all(path)
            } else {
                std::fs::remove_file(path)
            };
        }
    }

    let log = paths.updater_log();
    if std::fs::metadata(&log)
        .map(|meta| meta.len() > 1024 * 1024)
        .unwrap_or(false)
    {
        let old = log.with_extension("log.old");
        let _ = std::fs::remove_file(&old);
        let _ = std::fs::rename(&log, old);
    }
}

pub(crate) fn schedule_startup_check(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(8)).await;
        let _ = check(&app, true).await;
    });
}
