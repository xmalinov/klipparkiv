use std::fs;
use std::path::{Path, PathBuf};

use crate::i18n;
use crate::library::model::MediaKind;

const AUDIO_EXT: &[&str] = &["mp3", "m4a", "ogg", "opus", "flac", "wav", "aac"];
const VIDEO_EXT: &[&str] = &["mp4", "mkv", "webm", "mov", "avi"];
const EXCLUDED_DIRS: &[&str] = &["bin", "data"];

pub(crate) fn kind_for(path: &Path) -> Option<MediaKind> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    if AUDIO_EXT.contains(&ext.as_str()) {
        Some(MediaKind::Audio)
    } else if VIDEO_EXT.contains(&ext.as_str()) {
        Some(MediaKind::Video)
    } else {
        None
    }
}

pub(crate) fn excluded(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| EXCLUDED_DIRS.contains(&name))
}

/// Resolves a downloaded file for the system player.
///
/// The path must point to an existing audio or video file inside the app root,
/// so a corrupted library row cannot open arbitrary files.
pub(crate) fn resolve_media(root: &Path, candidate: &Path) -> Result<PathBuf, String> {
    let canonical_root = fs::canonicalize(root).map_err(|error| error.to_string())?;
    let canonical = fs::canonicalize(candidate)
        .map_err(|_| i18n::t("файл не найден", "the file was not found"))?;
    if !canonical.starts_with(&canonical_root) {
        return Err(i18n::t(
            "файл вне папки приложения",
            "the file is outside the app folder",
        ));
    }
    if kind_for(&canonical).is_none() {
        return Err(i18n::t(
            "это не аудио и не видео",
            "this is not an audio or video file",
        ));
    }
    if !canonical.is_file() {
        return Err(i18n::t("файл не найден", "the file was not found"));
    }
    Ok(canonical)
}
