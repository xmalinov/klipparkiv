use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};
use walkdir::WalkDir;

use crate::i18n;
use crate::paths::AppPaths;
use crate::settings::SettingsStore;

const SCHEMA: &str = "\
CREATE TABLE IF NOT EXISTS media (
  path TEXT PRIMARY KEY,
  kind TEXT NOT NULL,
  title TEXT NOT NULL,
  channel TEXT,
  duration_secs REAL,
  width INTEGER,
  height INTEGER,
  size_bytes INTEGER NOT NULL,
  mtime INTEGER NOT NULL,
  thumb_path TEXT,
  indexed_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS media_mtime ON media(mtime);
CREATE INDEX IF NOT EXISTS media_kind ON media(kind);
CREATE TABLE IF NOT EXISTS practice (
  path TEXT PRIMARY KEY,
  mtime INTEGER NOT NULL,
  state_json TEXT NOT NULL,
  updated_at INTEGER NOT NULL,
  hidden INTEGER NOT NULL DEFAULT 0
);
";

const AUDIO_EXT: &[&str] = &["mp3", "m4a", "opus", "flac", "wav", "aac"];
const VIDEO_EXT: &[&str] = &["mp4", "mkv", "webm", "mov", "avi"];
const EXCLUDED_DIRS: &[&str] = &["bin", "data"];

pub struct Library {
    conn: Mutex<Connection>,
}

impl Library {
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let conn = Connection::open(path)?;
        conn.execute_batch(SCHEMA)?;
        // Existing 1.7.0 development databases may predate the hidden column.
        let has_hidden = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('practice') WHERE name = 'hidden'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap_or(0)
            > 0;
        if !has_hidden {
            let _ = conn.execute(
                "ALTER TABLE practice ADD COLUMN hidden INTEGER NOT NULL DEFAULT 0",
                [],
            );
        }
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Returns the saved practice state when the file has not changed since it
    /// was written. A changed mtime invalidates and removes the row.
    pub fn practice_get(&self, path: &str, mtime: i64) -> Result<Option<String>, String> {
        let conn = self.conn.lock().map_err(|error| error.to_string())?;
        let row: Option<(i64, String)> = conn
            .query_row(
                "SELECT mtime, state_json FROM practice WHERE path = ?1",
                params![path],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(|error| error.to_string())?;

        match row {
            Some((saved_mtime, state)) if saved_mtime == mtime => Ok(Some(state)),
            Some(_) => {
                conn.execute("DELETE FROM practice WHERE path = ?1", params![path])
                    .map_err(|error| error.to_string())?;
                Ok(None)
            }
            None => Ok(None),
        }
    }

    pub fn practice_save(
        &self,
        path: &str,
        mtime: i64,
        state_json: &str,
        updated_at: i64,
    ) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|error| error.to_string())?;
        conn.execute(
            "INSERT INTO practice(path, mtime, state_json, updated_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(path) DO UPDATE SET
               mtime = excluded.mtime,
               state_json = excluded.state_json,
               updated_at = excluded.updated_at",
            params![path, mtime, state_json, updated_at],
        )
        .map_err(|error| error.to_string())?;
        Ok(())
    }

    pub fn practice_delete(&self, path: &str) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|error| error.to_string())?;
        conn.execute("DELETE FROM practice WHERE path = ?1", params![path])
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    pub fn practice_recent(&self, limit: u32) -> Result<Vec<LibraryItem>, String> {
        let conn = self.conn.lock().map_err(|error| error.to_string())?;
        let mut statement = conn
            .prepare(
                "SELECT m.path, m.kind, m.title, m.channel, m.duration_secs,
                        m.width, m.height, m.size_bytes, m.mtime, m.thumb_path
                 FROM practice p
                 JOIN media m ON m.path = p.path
                 WHERE p.hidden = 0
                 ORDER BY p.updated_at DESC
                 LIMIT ?1",
            )
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map(params![limit], |row| {
                Ok(LibraryItem {
                    id: row.get::<_, String>(0)?,
                    path: row.get::<_, String>(0)?,
                    kind: row.get(1)?,
                    title: row.get(2)?,
                    channel: row.get(3)?,
                    duration_secs: row.get(4)?,
                    width: row.get(5)?,
                    height: row.get(6)?,
                    size_bytes: row.get::<_, i64>(7)? as u64,
                    mtime: row.get(8)?,
                    thumb_path: row.get(9)?,
                })
            })
            .map_err(|error| error.to_string())?;
        Ok(rows.filter_map(Result::ok).collect())
    }

    pub fn practice_prune(&self) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|error| error.to_string())?;
        conn.execute(
            "DELETE FROM practice WHERE path NOT IN (SELECT path FROM media)",
            [],
        )
        .map_err(|error| error.to_string())?;
        Ok(())
    }

    pub fn practice_hide(&self, path: &str) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|error| error.to_string())?;
        conn.execute(
            "UPDATE practice SET hidden = 1 WHERE path = ?1",
            params![path],
        )
        .map_err(|error| error.to_string())?;
        Ok(())
    }

    pub fn practice_unhide(&self, path: &str) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|error| error.to_string())?;
        conn.execute(
            "UPDATE practice SET hidden = 0 WHERE path = ?1",
            params![path],
        )
        .map_err(|error| error.to_string())?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryItem {
    pub id: String,
    pub path: String,
    pub kind: String,
    pub title: String,
    pub channel: Option<String>,
    pub duration_secs: Option<f64>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub size_bytes: u64,
    pub mtime: i64,
    pub thumb_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryPage {
    pub items: Vec<LibraryItem>,
    pub total: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryQuery {
    pub search: Option<String>,
    pub kind: Option<String>,
    pub sort: String,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanStats {
    pub scanned: u32,
    pub added: u32,
    pub updated: u32,
    pub removed: u32,
}

fn kind_for(path: &Path) -> Option<&'static str> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    if AUDIO_EXT.contains(&ext.as_str()) {
        Some("audio")
    } else if VIDEO_EXT.contains(&ext.as_str()) {
        Some("video")
    } else {
        None
    }
}

fn excluded(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| EXCLUDED_DIRS.contains(&name))
}

fn mtime_secs(path: &Path) -> i64 {
    fs::metadata(path)
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

/// `dQw4w9WgXcQ - Title.mp3` → `Title`.
/// `dQw4w9WgXcQ - Title [00-01-30-00-05-00].mp4` → `Title · клип 00:01:30–00:05:00`.
/// `id_0-5.mp4` (legacy) → `Клип 0-5`.
pub(crate) fn display_title(stem: &str) -> String {
    // Older yt-dlp builds could turn `:` into a full-width colon on Windows.
    let stem = stem.replace('：', ":");
    if let Some((prefix, rest)) = stem.split_once(" - ") {
        let looks_like_id = (5..=20).contains(&prefix.len())
            && prefix
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
        if looks_like_id && !rest.trim().is_empty() {
            let rest = rest.trim();
            if let Some((title, range)) = split_clip_suffix(rest) {
                return format!("{title} · {} {range}", i18n::t("клип", "clip"));
            }
            return rest.to_string();
        }
    }
    if let Some((prefix, range)) = stem.split_once('_') {
        let looks_like_id = (5..=20).contains(&prefix.len())
            && prefix
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
        if looks_like_id && let Some(range) = humanize_legacy_range(range) {
            return format!("{} {range}", i18n::t("Клип", "Clip"));
        }
    }
    stem.to_string()
}

/// Splits `Title [00-01-30-00-05-00]` into the title and a readable range.
fn split_clip_suffix(rest: &str) -> Option<(String, String)> {
    let inner = rest.strip_suffix(']')?;
    let open = inner.rfind('[')?;
    let title = inner[..open].trim();
    if title.is_empty() {
        return None;
    }
    let range = humanize_compact_range(&inner[open + 1..])?;
    Some((title.to_string(), range))
}

/// `00-01-30-00-05-00` → `00:01:30–00:05:00`, `00-01-30-end` → `00:01:30–конец`.
fn humanize_compact_range(raw: &str) -> Option<String> {
    let parts: Vec<&str> = raw.split('-').collect();
    match parts.as_slice() {
        [h1, m1, s1, h2, m2, s2] => {
            if parts
                .iter()
                .all(|part| part.chars().all(|c| c.is_ascii_digit()))
            {
                Some(format!("{h1}:{m1}:{s1}–{h2}:{m2}:{s2}"))
            } else {
                None
            }
        }
        [h1, m1, s1, "end"] => Some(format!("{h1}:{m1}:{s1}–{}", i18n::t("конец", "end"))),
        // A legacy `MM:SS-MM:SS` range after yt-dlp replaced `:` with `-`.
        [m1, s1, m2, s2]
            if parts
                .iter()
                .all(|part| part.chars().all(|c| c.is_ascii_digit())) =>
        {
            Some(format!("{m1}:{s1}–{m2}:{s2}"))
        }
        _ => None,
    }
}

/// Legacy `id_00:10-00:20` ranges, including full-width colons and `-` instead
/// of `:` from an older sanitizer.
fn humanize_legacy_range(range: &str) -> Option<String> {
    let range = range.replace('：', ":");
    if let Some(compact) = humanize_compact_range(&range) {
        return Some(compact);
    }
    let normalized: String = range
        .chars()
        .map(|c| {
            if c.is_ascii_digit() || c == ':' || c == '-' {
                c
            } else {
                ' '
            }
        })
        .collect();
    let range = normalized.trim();
    let (start, end) = range.split_once('-')?;
    if start.is_empty() || end.is_empty() {
        return None;
    }
    if !start.chars().all(|c| c.is_ascii_digit() || c == ':')
        || !end.chars().all(|c| c.is_ascii_digit() || c == ':')
    {
        return None;
    }
    Some(format!("{start}–{end}"))
}

fn thumb_id(stem: &str) -> &str {
    if let Some((id, _)) = stem.split_once(" - ") {
        return id;
    }
    stem.split_once('_').map_or(stem, |(id, _)| id)
}

fn probe_media(
    ffprobe: &Path,
    path: &Path,
    want_video: bool,
) -> (Option<f64>, Option<u32>, Option<u32>) {
    let mut command = std::process::Command::new(ffprobe);
    command.args([
        "-v",
        "quiet",
        "-print_format",
        "json",
        "-show_format",
        "-show_streams",
    ]);
    command.arg(path);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }

    let Ok(output) = command.output() else {
        return (None, None, None);
    };
    if !output.status.success() {
        return (None, None, None);
    }
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(&output.stdout) else {
        return (None, None, None);
    };

    let duration = value
        .get("format")
        .and_then(|format| format.get("duration"))
        .and_then(|duration| duration.as_str())
        .and_then(|duration| duration.parse::<f64>().ok())
        .or_else(|| {
            value
                .get("format")
                .and_then(|f| f.get("duration"))
                .and_then(|d| d.as_f64())
        });

    let video_stream = if want_video {
        value
            .get("streams")
            .and_then(|streams| streams.as_array())
            .and_then(|streams| {
                streams.iter().find(|stream| {
                    stream.get("codec_type").and_then(|t| t.as_str()) == Some("video")
                })
            })
    } else {
        None
    };

    let width = video_stream
        .and_then(|stream| stream.get("width"))
        .and_then(|width| width.as_u64())
        .map(|width| width as u32);
    let height = video_stream
        .and_then(|stream| stream.get("height"))
        .and_then(|height| height.as_u64())
        .map(|height| height as u32);

    (duration, width, height)
}

pub fn scan_dir(
    library: &Library,
    root: &Path,
    ffprobe: &Path,
    max_depth: usize,
    full: bool,
    mut on_progress: impl FnMut(u32),
) -> Result<ScanStats, String> {
    let thumbs = root.join("data").join("thumbs");
    let mut stats = ScanStats::default();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let conn = library.conn.lock().map_err(|error| error.to_string())?;

    let walker = WalkDir::new(root)
        .max_depth(max_depth)
        .into_iter()
        .filter_entry(|entry| entry.path() == root || !excluded(entry.path()));

    for entry in walker.filter_map(Result::ok) {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        let Some(kind) = kind_for(path) else {
            continue;
        };
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        let size = meta.len();
        let mtime = mtime_secs(path);
        let path_text = path.to_string_lossy().into_owned();
        seen.insert(path_text.clone());

        let existing: Option<(i64, i64)> = conn
            .query_row(
                "SELECT mtime, size_bytes FROM media WHERE path = ?1",
                params![path_text],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .ok();

        if !full
            && let Some((old_mtime, old_size)) = existing
            && old_mtime == mtime
            && old_size == size as i64
        {
            stats.scanned += 1;
            continue;
        }

        let stem = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .map(str::to_owned)
            .unwrap_or_else(|| i18n::t("Без названия", "Untitled"));
        let title = display_title(&stem);
        let candidate = thumbs.join(format!("{}.jpg", thumb_id(&stem)));
        let thumb_path = candidate
            .is_file()
            .then(|| candidate.to_string_lossy().into_owned());

        let (duration, width, height) = probe_media(ffprobe, path, kind == "video");

        let changed = conn
            .execute(
                "INSERT INTO media
                   (path, kind, title, channel, duration_secs, width, height, size_bytes, mtime, thumb_path, indexed_at)
                 VALUES (?1, ?2, ?3, NULL, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                 ON CONFLICT(path) DO UPDATE SET
                   kind = excluded.kind,
                   title = excluded.title,
                   duration_secs = excluded.duration_secs,
                   width = excluded.width,
                   height = excluded.height,
                   size_bytes = excluded.size_bytes,
                   mtime = excluded.mtime,
                   thumb_path = excluded.thumb_path,
                   indexed_at = excluded.indexed_at",
                params![
                    path_text,
                    kind,
                    title,
                    duration,
                    width,
                    height,
                    size as i64,
                    mtime,
                    thumb_path,
                    now_secs(),
                ],
            )
            .map_err(|error| error.to_string())?;

        if existing.is_some() {
            stats.updated += if changed > 0 { 1 } else { 0 };
        } else {
            stats.added += 1;
        }
        stats.scanned += 1;

        if stats.scanned % 20 == 0 {
            on_progress(stats.scanned);
        }
    }

    if full {
        let existing_paths: Vec<String> = {
            let mut statement = conn
                .prepare("SELECT path FROM media")
                .map_err(|error| error.to_string())?;
            let rows = statement
                .query_map([], |row| row.get::<_, String>(0))
                .map_err(|error| error.to_string())?;
            rows.filter_map(Result::ok).collect()
        };
        for path in existing_paths {
            if !seen.contains(&path) {
                conn.execute("DELETE FROM media WHERE path = ?1", params![path])
                    .map_err(|error| error.to_string())?;
                stats.removed += 1;
            }
        }
    }

    on_progress(stats.scanned);
    Ok(stats)
}

pub fn query_page(library: &Library, query: &LibraryQuery) -> Result<LibraryPage, String> {
    let conn = library.conn.lock().map_err(|error| error.to_string())?;
    let limit = query.limit.unwrap_or(200).clamp(1, 500) as i64;
    let offset = query.offset.unwrap_or(0) as i64;

    let search = query
        .search
        .as_deref()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(|text| format!("%{text}%"));
    let kind = query
        .kind
        .as_deref()
        .filter(|kind| *kind == "audio" || *kind == "video");

    let order = match query.sort.as_str() {
        "dateAsc" => "mtime ASC",
        "title" => "title COLLATE NOCASE ASC",
        "duration" => "duration_secs DESC NULLS LAST",
        "size" => "size_bytes DESC",
        "channel" => "channel COLLATE NOCASE ASC NULLS LAST",
        _ => "mtime DESC",
    };
    let sql = format!(
        "SELECT path, kind, title, channel, duration_secs, width, height, size_bytes, mtime, thumb_path
         FROM media
         WHERE (?1 IS NULL OR title LIKE ?1 OR channel LIKE ?1)
           AND (?2 IS NULL OR kind = ?2)
         ORDER BY {order}
         LIMIT ?3 OFFSET ?4"
    );

    let mut statement = conn.prepare(&sql).map_err(|error| error.to_string())?;
    let rows = statement
        .query_map(params![search, kind, limit, offset], |row| {
            Ok(LibraryItem {
                id: row.get::<_, String>(0)?,
                path: row.get::<_, String>(0)?,
                kind: row.get(1)?,
                title: row.get(2)?,
                channel: row.get(3)?,
                duration_secs: row.get(4)?,
                width: row.get(5)?,
                height: row.get(6)?,
                size_bytes: row.get::<_, i64>(7)? as u64,
                mtime: row.get(8)?,
                thumb_path: row.get(9)?,
            })
        })
        .map_err(|error| error.to_string())?;

    let items: Vec<LibraryItem> = rows.filter_map(Result::ok).collect();
    let total: u32 = conn
        .query_row(
            "SELECT COUNT(*) FROM media
             WHERE (?1 IS NULL OR title LIKE ?1 OR channel LIKE ?1)
               AND (?2 IS NULL OR kind = ?2)",
            params![search, kind],
            |row| row.get(0),
        )
        .unwrap_or(items.len() as u32);

    Ok(LibraryPage { items, total })
}

/// Reads a cover only from the library cache inside the portable root.
pub fn resolve_thumb(root: &Path, candidate: &Path) -> Result<PathBuf, String> {
    let normalized = candidate.strip_prefix(root).map_err(|_| {
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

/// Resolves a downloaded file for the system player.
///
/// The path must point to an existing audio or video file inside the app root,
/// so a corrupted library row cannot open arbitrary files.
pub fn resolve_media(root: &Path, candidate: &Path) -> Result<PathBuf, String> {
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

/// Opens a downloaded file with the system default player.
///
/// This goes through Rust instead of the opener plugin's `openPath`, because
/// the plugin command is blocked by the ACL capability while a Rust command
/// can validate the path against the library root first.
#[tauri::command]
pub fn library_open(app: AppHandle, path: String) -> Result<(), String> {
    let root = app.state::<AppPaths>().root().to_path_buf();
    let resolved = resolve_media(&root, Path::new(&path))?;
    tauri_plugin_opener::open_path(&resolved, None::<&str>).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn library_query(
    library: State<'_, Library>,
    query: LibraryQuery,
) -> Result<LibraryPage, String> {
    query_page(&library, &query)
}

#[tauri::command]
pub async fn library_scan(app: AppHandle, full: bool) -> Result<ScanStats, String> {
    let app_for_scan = app.clone();
    let stats = tauri::async_runtime::spawn_blocking(move || {
        let paths = app_for_scan.state::<AppPaths>();
        let library = app_for_scan.state::<Library>();
        let root = paths.root().to_path_buf();
        let ffprobe = paths.ffprobe();
        let depth = app_for_scan.state::<SettingsStore>().get().scan_depth;
        let stats = scan_dir(&library, &root, &ffprobe, depth, full, |scanned| {
            let _ = app_for_scan.emit("library://scan-progress", scanned);
        })?;
        library.practice_prune().ok();
        Ok::<ScanStats, String>(stats)
    })
    .await
    .map_err(|error| error.to_string())??;

    let _ = app.emit("library://updated", &stats);
    Ok(stats)
}

#[tauri::command]
pub fn library_remove(library: State<'_, Library>, id: String) -> Result<(), String> {
    let conn = library.conn.lock().map_err(|error| error.to_string())?;
    conn.execute("DELETE FROM media WHERE path = ?1", params![id])
        .map_err(|error| error.to_string())?;
    conn.execute("DELETE FROM practice WHERE path = ?1", params![id])
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn library_thumb(app: AppHandle, path: String) -> Result<String, String> {
    let root = app.state::<AppPaths>().root().to_path_buf();
    let resolved = resolve_thumb(&root, Path::new(&path))?;
    let bytes = fs::read(resolved).map_err(|error| error.to_string())?;
    Ok(format!("data:image/jpeg;base64,{}", STANDARD.encode(bytes)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    struct Fixture {
        root: PathBuf,
        library: Library,
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn fixture(name: &str) -> Fixture {
        let root =
            std::env::temp_dir().join(format!("klipparkiv-lib-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("audio")).unwrap();
        fs::create_dir_all(root.join("video")).unwrap();
        fs::create_dir_all(root.join("data").join("thumbs")).unwrap();
        fs::create_dir_all(root.join("bin")).unwrap();

        let library = Library::open(&root.join("data").join("library.db")).unwrap();
        Fixture { root, library }
    }

    fn write(path: &Path, bytes: &[u8]) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let mut file = fs::File::create(path).unwrap();
        file.write_all(bytes).unwrap();
    }

    fn scan(fixture: &Fixture, full: bool) -> ScanStats {
        scan_dir(
            &fixture.library,
            &fixture.root,
            Path::new("missing-ffprobe"),
            5,
            full,
            |_| {},
        )
        .unwrap()
    }

    #[test]
    fn strips_youtube_id_prefix_from_titles() {
        assert_eq!(display_title("dQw4w9WgXcQ - Rick Astley"), "Rick Astley");
        assert_eq!(display_title("abc123 - Song"), "Song");
        assert_eq!(display_title("a - Alpha"), "a - Alpha");
        assert_eq!(display_title("plain title"), "plain title");
        assert_eq!(display_title("dQw4w9WgXcQ - "), "dQw4w9WgXcQ - ");
        assert_eq!(
            display_title("dQw4w9WgXcQ - Never Gonna Give You Up [00-01-30-00-05-00]"),
            "Never Gonna Give You Up · клип 00:01:30–00:05:00"
        );
        assert_eq!(
            display_title("dQw4w9WgXcQ - Song [00-01-30-end]"),
            "Song · клип 00:01:30–конец"
        );
        assert_eq!(display_title("jNQXAC9IVRw_0-5"), "Клип 0–5");
        assert_eq!(display_title("abc123_00:10-00:20"), "Клип 00:10–00:20");
        assert_eq!(display_title("abc123_00：10-00：20"), "Клип 00:10–00:20");
        assert_eq!(display_title("abc123_00-10-00-20"), "Клип 00:10–00:20");
    }

    #[test]
    fn indexes_media_and_ignores_bin_and_data() {
        let fixture = fixture("index");
        write(&fixture.root.join("audio").join("a1 - Song.mp3"), b"audio");
        write(&fixture.root.join("video").join("v1.mp4"), b"video");
        write(&fixture.root.join("bin").join("tool.exe"), b"exe");
        write(&fixture.root.join("data").join("db.sqlite"), b"db");
        write(&fixture.root.join("readme.txt"), b"text");

        let stats = scan(&fixture, true);

        assert_eq!(stats.added, 2);
        let page = query_page(
            &fixture.library,
            &LibraryQuery {
                search: None,
                kind: None,
                sort: "dateDesc".into(),
                limit: None,
                offset: None,
            },
        )
        .unwrap();
        assert_eq!(page.total, 2);
        assert!(
            page.items
                .iter()
                .all(|item| item.kind == "audio" || item.kind == "video")
        );
    }

    #[test]
    fn finds_cached_thumbnail_by_id() {
        let fixture = fixture("thumb");
        write(
            &fixture.root.join("audio").join("abc123 - Song.mp3"),
            b"audio",
        );
        write(
            &fixture.root.join("data").join("thumbs").join("abc123.jpg"),
            b"jpg",
        );

        scan(&fixture, true);

        let page = query_page(
            &fixture.library,
            &LibraryQuery {
                search: Some("Song".into()),
                kind: None,
                sort: "dateDesc".into(),
                limit: None,
                offset: None,
            },
        )
        .unwrap();
        assert_eq!(page.items.len(), 1);
        assert!(
            page.items[0]
                .thumb_path
                .as_ref()
                .unwrap()
                .ends_with("abc123.jpg")
        );
    }

    #[test]
    fn incremental_scan_skips_unchanged_files() {
        let fixture = fixture("incremental");
        write(&fixture.root.join("video").join("v1.mp4"), b"video");

        let first = scan(&fixture, false);
        let second = scan(&fixture, false);

        assert_eq!(first.added, 1);
        assert_eq!(second.added, 0);
        assert_eq!(second.updated, 0);
    }

    #[test]
    fn full_scan_removes_missing_files() {
        let fixture = fixture("remove");
        write(&fixture.root.join("video").join("v1.mp4"), b"video");
        scan(&fixture, true);

        fs::remove_file(fixture.root.join("video").join("v1.mp4")).unwrap();
        let stats = scan(&fixture, true);

        assert_eq!(stats.removed, 1);
        let page = query_page(
            &fixture.library,
            &LibraryQuery {
                search: None,
                kind: None,
                sort: "dateDesc".into(),
                limit: None,
                offset: None,
            },
        )
        .unwrap();
        assert_eq!(page.total, 0);
    }

    #[test]
    fn filters_and_sorts() {
        let fixture = fixture("query");
        write(&fixture.root.join("audio").join("a - Alpha.mp3"), b"12345");
        write(
            &fixture.root.join("video").join("b - Beta.mp4"),
            b"1234567890",
        );

        scan(&fixture, true);

        let audio = query_page(
            &fixture.library,
            &LibraryQuery {
                search: None,
                kind: Some("audio".into()),
                sort: "dateDesc".into(),
                limit: None,
                offset: None,
            },
        )
        .unwrap();
        assert_eq!(audio.total, 1);
        assert_eq!(audio.items[0].title, "a - Alpha");

        let by_size = query_page(
            &fixture.library,
            &LibraryQuery {
                search: None,
                kind: None,
                sort: "size".into(),
                limit: None,
                offset: None,
            },
        )
        .unwrap();
        assert_eq!(by_size.items[0].title, "b - Beta");
    }

    #[test]
    fn thumbnail_resolution_stays_inside_cache() {
        let fixture = fixture("thumbguard");
        let thumb = fixture.root.join("data").join("thumbs").join("x.jpg");
        write(&thumb, b"jpg");
        let outside = std::env::temp_dir().join("outside.jpg");
        write(&outside, b"jpg");

        assert!(resolve_thumb(&fixture.root, &thumb).is_ok());
        assert!(resolve_thumb(&fixture.root, &outside).is_err());
        assert!(resolve_thumb(&fixture.root, &fixture.root.join("audio").join("x.jpg")).is_err());
    }

    #[test]
    fn media_resolution_stays_inside_root() {
        let fixture = fixture("mediaguard");
        let media = fixture.root.join("video").join("clip.mp4");
        write(&media, b"mp4");
        let outside = std::env::temp_dir().join("klipparkiv-outside.mp4");
        write(&outside, b"mp4");
        let other = fixture.root.join("video").join("notes.txt");
        write(&other, b"txt");

        assert_eq!(
            resolve_media(&fixture.root, &media).unwrap(),
            fs::canonicalize(&media).unwrap()
        );
        assert!(resolve_media(&fixture.root, &outside).is_err());
        assert!(resolve_media(&fixture.root, &other).is_err());
        assert!(
            resolve_media(
                &fixture.root,
                &fixture.root.join("video").join("missing.mp4")
            )
            .is_err()
        );
    }

    #[test]
    fn practice_state_is_invalidated_by_mtime() {
        let fixture = fixture("practice-mtime");
        let path = "C:\\root\\audio\\a.mp3";
        fixture
            .library
            .practice_save(path, 10, "{\"version\":1}", 100)
            .unwrap();
        assert_eq!(
            fixture.library.practice_get(path, 10).unwrap().as_deref(),
            Some("{\"version\":1}")
        );
        assert!(fixture.library.practice_get(path, 11).unwrap().is_none());
        assert!(fixture.library.practice_get(path, 10).unwrap().is_none());
    }

    #[test]
    fn practice_rows_are_pruned_with_missing_media() {
        let fixture = fixture("practice-prune");
        fixture
            .library
            .practice_save("missing.mp3", 1, "{}", 1)
            .unwrap();
        fixture.library.practice_prune().unwrap();
        assert!(
            fixture
                .library
                .practice_get("missing.mp3", 1)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn hidden_practice_rows_disappear_from_recent() {
        let fixture = fixture("practice-hidden");
        {
            let conn = fixture.library.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO media(path, kind, title, channel, duration_secs, width, height, size_bytes, mtime, thumb_path, indexed_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    "a.mp3",
                    "audio",
                    "A",
                    None::<String>,
                    None::<f64>,
                    None::<u32>,
                    None::<u32>,
                    1_i64,
                    1_i64,
                    None::<String>,
                    1_i64
                ],
            )
            .unwrap();
        }
        fixture.library.practice_save("a.mp3", 1, "{}", 1).unwrap();
        assert_eq!(fixture.library.practice_recent(10).unwrap().len(), 1);
        fixture.library.practice_hide("a.mp3").unwrap();
        assert_eq!(fixture.library.practice_recent(10).unwrap().len(), 0);
        fixture.library.practice_save("a.mp3", 1, "{}", 2).unwrap();
        assert_eq!(fixture.library.practice_recent(10).unwrap().len(), 0);
        fixture.library.practice_unhide("a.mp3").unwrap();
        assert_eq!(fixture.library.practice_recent(10).unwrap().len(), 1);
    }

    #[test]
    fn query_page_pages_through_a_large_library() {
        let fixture = fixture("bulk-query");
        {
            let mut conn = fixture.library.conn.lock().unwrap();
            let transaction = conn.transaction().unwrap();
            for index in 0..1000 {
                transaction
                    .execute(
                        "INSERT INTO media(path, kind, title, channel, duration_secs, width, height, size_bytes, mtime, thumb_path, indexed_at)
                         VALUES (?1, 'audio', ?2, NULL, NULL, NULL, NULL, 1, 1, NULL, 1)",
                        params![format!("audio/{index}.mp3"), format!("Track {index}")],
                    )
                    .unwrap();
            }
            transaction.commit().unwrap();
        }

        let first = query_page(
            &fixture.library,
            &LibraryQuery {
                search: None,
                kind: None,
                sort: "title".into(),
                limit: Some(200),
                offset: Some(0),
            },
        )
        .unwrap();
        assert_eq!(first.items.len(), 200);
        assert_eq!(first.total, 1000);

        let last = query_page(
            &fixture.library,
            &LibraryQuery {
                search: None,
                kind: None,
                sort: "title".into(),
                limit: Some(200),
                offset: Some(800),
            },
        )
        .unwrap();
        assert_eq!(last.items.len(), 200);
        assert_eq!(last.total, 1000);

        let beyond = query_page(
            &fixture.library,
            &LibraryQuery {
                search: None,
                kind: None,
                sort: "title".into(),
                limit: Some(200),
                offset: Some(1000),
            },
        )
        .unwrap();
        assert!(beyond.items.is_empty());
    }
}
