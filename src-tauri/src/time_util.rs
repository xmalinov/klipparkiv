//! Clock and file-time helpers with one implementation.

use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// Unix timestamp in seconds; `0` when the clock is before the epoch.
pub(crate) fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

/// Modification time in seconds; `0` when the file cannot be read.
pub(crate) fn mtime_secs(path: &Path) -> i64 {
    fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .map(|modified| {
            modified
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_secs() as i64)
                .unwrap_or(0)
        })
        .unwrap_or(0)
}

/// Like [`mtime_secs`], but reports why the time is unavailable.
///
/// Practice treats a missing file as an error; the indexer treats it as "no
/// timestamp" and keeps the row. Both behaviours need the same clock source.
pub(crate) fn mtime_secs_checked(path: &Path) -> Result<i64, String> {
    let modified = fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .map_err(|error| format!("cannot read file time: {error}"))?;
    Ok(modified
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0))
}
