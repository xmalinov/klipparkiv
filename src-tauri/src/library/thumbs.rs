use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use crate::i18n;
use crate::library::LibraryPaths;
use crate::library::util::run_command;

pub(crate) fn thumb_id(stem: &str) -> &str {
    if let Some((id, _)) = stem.split_once(" - ") {
        return id;
    }
    stem.split_once('_').map_or(stem, |(id, _)| id)
}

pub(crate) fn extract_thumb(
    ffmpeg: &Path,
    source: &Path,
    out: &Path,
    timestamp: Option<f64>,
) -> bool {
    let mut command = Command::new(ffmpeg);
    command.args(["-y", "-v", "error", "-nostdin"]);
    if let Some(seconds) = timestamp {
        command.args(["-ss", &format!("{seconds:.3}")]);
    }
    command.arg("-i").arg(source);
    command.args([
        "-map",
        "0:v:0",
        "-frames:v",
        "1",
        "-an",
        "-vf",
        "scale=640:-2:force_original_aspect_ratio=decrease",
        "-c:v",
        "mjpeg",
        "-q:v",
        "3",
    ]);
    command.arg(out);
    run_command(&mut command, Duration::from_secs(5)).is_some_and(|output| output.status.success())
        && out.is_file()
}

/// Reads a cover only from the library cache inside the portable root.
pub(crate) fn resolve_thumb(paths: &LibraryPaths, candidate: &Path) -> Result<PathBuf, String> {
    let normalized = candidate.strip_prefix(paths.root()).map_err(|_| {
        i18n::t(
            "обложка вне папки приложения",
            "the cover is outside the app folder",
        )
    })?;
    if !normalized.starts_with("data") || !normalized.to_string_lossy().contains("thumbs") {
        return Err(i18n::t(
            "обложка вне кэша",
            "the cover is outside the cache",
        ));
    }
    if !candidate.is_file() {
        return Err(i18n::t("обложка не найдена", "the cover was not found"));
    }
    Ok(candidate.to_path_buf())
}
