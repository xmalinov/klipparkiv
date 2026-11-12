use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
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
  indexed_at INTEGER NOT NULL,
  origin TEXT NOT NULL DEFAULT 'scan'
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

const AUDIO_EXT: &[&str] = &["mp3", "m4a", "ogg", "opus", "flac", "wav", "aac"];
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
        // 1.7.0 databases predate the import origin marker.
        let has_origin = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('media') WHERE name = 'origin'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap_or(0)
            > 0;
        if !has_origin {
            let _ = conn.execute(
                "ALTER TABLE media ADD COLUMN origin TEXT NOT NULL DEFAULT 'scan'",
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

    pub fn media_get(&self, path: &str) -> Result<Option<LibraryItem>, String> {
        let conn = self.conn.lock().map_err(|error| error.to_string())?;
        conn.query_row(
            "SELECT path, kind, title, channel, duration_secs, width, height,
                    size_bytes, mtime, thumb_path
             FROM media WHERE path = ?1",
            params![path],
            row_to_item,
        )
        .optional()
        .map_err(|error| error.to_string())
    }

    /// Paths of indexed files with exactly this size, used as duplicate
    /// candidates before hashing.
    pub fn media_by_size(&self, size: u64) -> Result<Vec<String>, String> {
        let conn = self.conn.lock().map_err(|error| error.to_string())?;
        let mut statement = conn
            .prepare("SELECT path FROM media WHERE size_bytes = ?1")
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map(params![size as i64], |row| row.get::<_, String>(0))
            .map_err(|error| error.to_string())?;
        Ok(rows.filter_map(Result::ok).collect())
    }

    pub fn media_upsert(&self, row: &MediaRow) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|error| error.to_string())?;
        conn.execute(
            "INSERT INTO media
               (path, kind, title, channel, duration_secs, width, height,
                size_bytes, mtime, thumb_path, indexed_at, origin)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
             ON CONFLICT(path) DO UPDATE SET
               kind = excluded.kind,
               title = excluded.title,
               channel = excluded.channel,
               duration_secs = excluded.duration_secs,
               width = excluded.width,
               height = excluded.height,
               size_bytes = excluded.size_bytes,
               mtime = excluded.mtime,
               thumb_path = excluded.thumb_path,
               indexed_at = excluded.indexed_at,
               origin = excluded.origin",
            params![
                row.path,
                row.kind,
                row.title,
                row.channel,
                row.duration_secs,
                row.width,
                row.height,
                row.size_bytes as i64,
                row.mtime,
                row.thumb_path,
                row.indexed_at,
                row.origin,
            ],
        )
        .map_err(|error| error.to_string())?;
        Ok(())
    }
}

fn row_to_item(row: &rusqlite::Row<'_>) -> rusqlite::Result<LibraryItem> {
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
}

/// One import job at a time. The cancellation flag is cooperative: the copy
/// loop checks it after every chunk, the pipeline checks it between files.
pub struct ImportState {
    job: Mutex<Option<ImportJob>>,
    next_id: AtomicU64,
}

struct ImportJob {
    id: u64,
    cancel: std::sync::Arc<AtomicBool>,
}

impl Default for ImportState {
    fn default() -> Self {
        Self {
            job: Mutex::new(None),
            next_id: AtomicU64::new(1),
        }
    }
}

impl ImportState {
    pub fn begin(&self) -> Result<(u64, std::sync::Arc<AtomicBool>), String> {
        let mut slot = self.job.lock().map_err(|error| error.to_string())?;
        if slot.is_some() {
            return Err(i18n::t(
                "импорт уже выполняется",
                "an import is already in progress",
            ));
        }
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let cancel = std::sync::Arc::new(AtomicBool::new(false));
        *slot = Some(ImportJob {
            id,
            cancel: cancel.clone(),
        });
        Ok((id, cancel))
    }

    pub fn cancel(&self) -> bool {
        let Ok(slot) = self.job.lock() else {
            return false;
        };
        match slot.as_ref() {
            Some(job) => {
                job.cancel.store(true, Ordering::SeqCst);
                true
            }
            None => false,
        }
    }

    pub fn finish(&self, id: u64) {
        if let Ok(mut slot) = self.job.lock()
            && slot.as_ref().is_some_and(|job| job.id == id)
        {
            *slot = None;
        }
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

pub struct MediaRow {
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
    pub origin: String,
    pub indexed_at: i64,
}

impl MediaRow {
    fn into_item(self) -> LibraryItem {
        LibraryItem {
            id: self.path.clone(),
            path: self.path,
            kind: self.kind,
            title: self.title,
            channel: self.channel,
            duration_secs: self.duration_secs,
            width: self.width,
            height: self.height,
            size_bytes: self.size_bytes,
            mtime: self.mtime,
            thumb_path: self.thumb_path,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportProgress {
    pub job_id: u64,
    pub index: u32,
    pub total: u32,
    pub name: String,
    pub copied_bytes: u64,
    pub total_bytes: u64,
    pub phase: String,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    pub imported: Vec<LibraryItem>,
    pub existing: Vec<LibraryItem>,
    pub skipped: Vec<ImportSkip>,
    pub failed: Vec<ImportFailure>,
    pub cancelled: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSkip {
    pub path: String,
    pub reason: ImportSkipReason,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ImportSkipReason {
    Unsupported,
    Folder,
    DuplicateInBatch,
    AlreadyInLibrary,
    Cancelled,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportFailure {
    pub path: String,
    pub error: String,
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

#[derive(Debug, Default, Clone)]
struct ProbeInfo {
    duration: Option<f64>,
    width: Option<u32>,
    height: Option<u32>,
    title: Option<String>,
    artist: Option<String>,
    attached_pic: bool,
}

/// Runs a child process with a hard timeout. Reader threads drain both pipes so
/// a verbose ffmpeg cannot fill the OS buffer and deadlock.
fn run_command(command: &mut Command, timeout: Duration) -> Option<std::process::Output> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .ok()?;
    let mut stdout = child.stdout.take()?;
    let mut stderr = child.stderr.take()?;
    let stdout_reader = std::thread::spawn(move || {
        let mut buffer = Vec::new();
        let _ = stdout.read_to_end(&mut buffer);
        buffer
    });
    let stderr_reader = std::thread::spawn(move || {
        let mut buffer = Vec::new();
        let _ = stderr.read_to_end(&mut buffer);
        buffer
    });

    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                if started.elapsed() >= timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                std::thread::sleep(Duration::from_millis(25));
            }
            Err(_) => {
                let _ = child.kill();
                return None;
            }
        }
    };

    Some(std::process::Output {
        status,
        stdout: stdout_reader.join().unwrap_or_default(),
        stderr: stderr_reader.join().unwrap_or_default(),
    })
}

fn tag_value(tags: Option<&serde_json::Value>, names: &[&str]) -> Option<String> {
    let object = tags?.as_object()?;
    for name in names {
        for (key, value) in object {
            if key.eq_ignore_ascii_case(name)
                && let Some(text) = value.as_str()
            {
                let text = text.trim();
                if !text.is_empty() {
                    return Some(text.to_string());
                }
            }
        }
    }
    None
}

fn probe_info(ffprobe: &Path, path: &Path, want_video: bool) -> ProbeInfo {
    let mut command = Command::new(ffprobe);
    command.args([
        "-v",
        "quiet",
        "-print_format",
        "json",
        "-show_format",
        "-show_streams",
    ]);
    command.arg(path);

    let Some(output) = run_command(&mut command, Duration::from_secs(5)) else {
        return ProbeInfo::default();
    };
    if !output.status.success() {
        return ProbeInfo::default();
    }
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(&output.stdout) else {
        return ProbeInfo::default();
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

    let streams = value.get("streams").and_then(|streams| streams.as_array());
    let video_stream = streams.and_then(|streams| {
        streams.iter().find(|stream| {
            stream.get("codec_type").and_then(|t| t.as_str()) == Some("video")
                && stream
                    .get("disposition")
                    .and_then(|disposition| disposition.get("attached_pic"))
                    .and_then(|flag| flag.as_u64())
                    .unwrap_or(0)
                    == 0
        })
    });
    let attached_pic = streams.is_some_and(|streams| {
        streams.iter().any(|stream| {
            stream.get("codec_type").and_then(|t| t.as_str()) == Some("video")
                && stream
                    .get("disposition")
                    .and_then(|disposition| disposition.get("attached_pic"))
                    .and_then(|flag| flag.as_u64())
                    .unwrap_or(0)
                    == 1
        })
    });

    let width = if want_video {
        video_stream
            .and_then(|stream| stream.get("width"))
            .and_then(|width| width.as_u64())
            .map(|width| width as u32)
    } else {
        None
    };
    let height = if want_video {
        video_stream
            .and_then(|stream| stream.get("height"))
            .and_then(|height| height.as_u64())
            .map(|height| height as u32)
    } else {
        None
    };

    let format_tags = value.get("format").and_then(|format| format.get("tags"));
    let stream_tags = streams.and_then(|streams| {
        streams
            .iter()
            .find_map(|stream| stream.get("tags").filter(|tags| tags.is_object()))
    });
    let title = tag_value(format_tags, &["title"]).or_else(|| tag_value(stream_tags, &["title"]));
    let artist = tag_value(format_tags, &["artist", "album_artist", "albumartist"])
        .or_else(|| tag_value(stream_tags, &["artist", "album_artist", "albumartist"]));

    ProbeInfo {
        duration,
        width,
        height,
        title,
        artist,
        attached_pic,
    }
}

/// The subset of a `media` row the scanner needs to decide what to preserve.
struct ExistingRow {
    mtime: i64,
    size_bytes: i64,
    title: String,
    channel: Option<String>,
    thumb_path: Option<String>,
    origin: String,
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

        let existing: Option<ExistingRow> = conn
            .query_row(
                "SELECT mtime, size_bytes, title, channel, thumb_path, origin
                 FROM media WHERE path = ?1",
                params![path_text],
                |row| {
                    Ok(ExistingRow {
                        mtime: row.get(0)?,
                        size_bytes: row.get(1)?,
                        title: row.get(2)?,
                        channel: row.get(3)?,
                        thumb_path: row.get(4)?,
                        origin: row.get(5)?,
                    })
                },
            )
            .ok();

        if !full
            && let Some(row) = &existing
            && row.mtime == mtime
            && row.size_bytes == size as i64
        {
            stats.scanned += 1;
            continue;
        }

        let stem = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .map(str::to_owned)
            .unwrap_or_else(|| i18n::t("Без названия", "Untitled"));
        let info = probe_info(ffprobe, path, kind == "video");

        // Imported files carry their own title/artist (tags or the full file
        // name) and a hashed cover. A later scan must not rewrite them with the
        // yt-dlp filename heuristic.
        let (title, channel, thumb, origin) = match existing.as_ref() {
            Some(row) if row.origin == "import" => {
                let title = info
                    .title
                    .clone()
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or_else(|| row.title.clone());
                let channel = info.artist.clone().or_else(|| row.channel.clone());
                let kept = row
                    .thumb_path
                    .as_deref()
                    .map(PathBuf::from)
                    .filter(|candidate| candidate.is_file());
                let thumb = kept.or_else(|| {
                    let candidate = thumbs.join(format!("{}.jpg", thumb_id(&stem)));
                    candidate.is_file().then_some(candidate)
                });
                (title, channel, thumb, "import".to_string())
            }
            _ => {
                let title = display_title(&stem);
                let candidate = thumbs.join(format!("{}.jpg", thumb_id(&stem)));
                let thumb = candidate.is_file().then_some(candidate);
                (title, None, thumb, "scan".to_string())
            }
        };
        let thumb_path = thumb.map(|path| path.to_string_lossy().into_owned());

        let changed = conn
            .execute(
                "INSERT INTO media
                   (path, kind, title, channel, duration_secs, width, height, size_bytes, mtime, thumb_path, indexed_at, origin)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
                 ON CONFLICT(path) DO UPDATE SET
                   kind = excluded.kind,
                   title = excluded.title,
                   channel = excluded.channel,
                   duration_secs = excluded.duration_secs,
                   width = excluded.width,
                   height = excluded.height,
                   size_bytes = excluded.size_bytes,
                   mtime = excluded.mtime,
                   thumb_path = excluded.thumb_path,
                   indexed_at = excluded.indexed_at,
                   origin = excluded.origin",
                params![
                    path_text,
                    kind,
                    title,
                    channel,
                    info.duration,
                    info.width,
                    info.height,
                    size as i64,
                    mtime,
                    thumb_path,
                    now_secs(),
                    origin,
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

const STEM_LIMIT: usize = 120;
const WINDOWS_RESERVED: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

fn split_name(name: &str) -> (String, String) {
    match name.rfind('.') {
        Some(index) if index > 0 && index + 1 < name.len() => {
            (name[..index].to_string(), name[index..].to_string())
        }
        _ => (name.to_string(), String::new()),
    }
}

/// Keeps only the base name and makes it safe for the destination folder.
fn sanitize_file_name(raw: &str, fallback: &str) -> String {
    let base = raw.rsplit(['/', '\\']).next().unwrap_or(raw);
    let cleaned: String = base
        .chars()
        .filter(|character| !character.is_control())
        .map(|character| match character {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            _ => character,
        })
        .collect();
    let mut name = cleaned.trim_end_matches([' ', '.']).to_string();
    if name.is_empty() || name == "." || name == ".." {
        name = fallback.to_string();
    }
    let (mut stem, extension) = split_name(&name);
    if stem.chars().count() > STEM_LIMIT {
        stem = stem.chars().take(STEM_LIMIT).collect();
    }
    if stem.is_empty() {
        stem = fallback.to_string();
    }
    if WINDOWS_RESERVED
        .iter()
        .any(|reserved| stem.eq_ignore_ascii_case(reserved))
    {
        stem.insert(0, '_');
    }
    format!("{stem}{extension}")
}

fn unique_target(dir: &Path, name: &str, taken: &mut HashSet<String>) -> Result<PathBuf, String> {
    let (stem, extension) = split_name(name);
    let mut counter = 1u32;
    loop {
        let candidate = if counter == 1 {
            format!("{stem}{extension}")
        } else {
            format!("{stem} ({counter}){extension}")
        };
        let key = candidate.to_lowercase();
        if !taken.contains(&key) && !dir.join(&candidate).exists() {
            taken.insert(key);
            return Ok(dir.join(candidate));
        }
        counter += 1;
        if counter > 999 {
            return Err(i18n::t(
                "слишком много файлов с таким именем",
                "too many files with this name",
            ));
        }
    }
}

fn file_sha256(path: &Path) -> std::io::Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn hash_cached(cache: &mut HashMap<String, String>, path: &Path) -> Option<String> {
    let key = path.to_string_lossy().into_owned();
    if let Some(hash) = cache.get(&key) {
        return Some(hash.clone());
    }
    let hash = file_sha256(path).ok()?;
    cache.insert(key, hash.clone());
    Some(hash)
}

fn set_modified_now(path: &Path) -> std::io::Result<()> {
    fs::OpenOptions::new()
        .write(true)
        .open(path)?
        .set_modified(SystemTime::now())
}

enum CopyFailure {
    Cancelled,
    Failed(String),
}

fn copy_with_progress(
    source: &Path,
    part: &Path,
    total: u64,
    cancel: &AtomicBool,
    mut on_bytes: impl FnMut(u64),
) -> Result<u64, CopyFailure> {
    if cancel.load(Ordering::SeqCst) {
        return Err(CopyFailure::Cancelled);
    }
    let mut input =
        fs::File::open(source).map_err(|error| CopyFailure::Failed(error.to_string()))?;
    let mut output =
        fs::File::create(part).map_err(|error| CopyFailure::Failed(error.to_string()))?;
    let mut buffer = vec![0u8; 1024 * 1024];
    let mut copied = 0u64;
    loop {
        if cancel.load(Ordering::SeqCst) {
            return Err(CopyFailure::Cancelled);
        }
        let read = input
            .read(&mut buffer)
            .map_err(|error| CopyFailure::Failed(error.to_string()))?;
        if read == 0 {
            break;
        }
        output
            .write_all(&buffer[..read])
            .map_err(|error| CopyFailure::Failed(error.to_string()))?;
        copied += read as u64;
        on_bytes(copied);
    }
    output
        .flush()
        .map_err(|error| CopyFailure::Failed(error.to_string()))?;
    let _ = total;
    Ok(copied)
}

fn extract_thumb(ffmpeg: &Path, source: &Path, out: &Path, timestamp: Option<f64>) -> bool {
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

fn media_row_for(
    path: &Path,
    kind: &str,
    info: ProbeInfo,
    thumb_path: Option<String>,
    origin: &str,
) -> Result<MediaRow, String> {
    let meta = fs::metadata(path).map_err(|error| error.to_string())?;
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(str::to_owned)
        .unwrap_or_else(|| i18n::t("Без названия", "Untitled"));
    let title = info
        .title
        .as_deref()
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .map(str::to_owned)
        .unwrap_or(stem);
    Ok(MediaRow {
        path: path.to_string_lossy().into_owned(),
        kind: kind.to_string(),
        title,
        channel: info.artist,
        duration_secs: info.duration,
        width: info.width,
        height: info.height,
        size_bytes: meta.len(),
        mtime: mtime_secs(path),
        thumb_path,
        origin: origin.to_string(),
        indexed_at: now_secs(),
    })
}

/// Copies every dropped file into `audio/` or `video/`, deduplicates by
/// content, extracts tags/covers/posters and writes the row immediately so the
/// UI does not need a rescan. Errors are per file; only an unusable root aborts.
#[allow(clippy::too_many_arguments)]
pub fn import_media(
    library: &Library,
    root: &Path,
    ffprobe: &Path,
    ffmpeg: &Path,
    paths: Vec<String>,
    cancel: &AtomicBool,
    job_id: u64,
    mut on_progress: impl FnMut(ImportProgress),
) -> Result<ImportReport, String> {
    let canonical_root = fs::canonicalize(root).map_err(|error| error.to_string())?;
    let audio_dir = root.join(crate::paths::AUDIO_DIR);
    let video_dir = root.join(crate::paths::VIDEO_DIR);
    let thumbs = root.join("data").join("thumbs");
    fs::create_dir_all(&audio_dir).map_err(|error| error.to_string())?;
    fs::create_dir_all(&video_dir).map_err(|error| error.to_string())?;
    let _ = fs::create_dir_all(&thumbs);

    let mut report = ImportReport::default();
    let total = paths.len() as u32;
    let mut taken_audio: HashSet<String> = HashSet::new();
    let mut taken_video: HashSet<String> = HashSet::new();
    let mut batch_seen: HashSet<String> = HashSet::new();
    let mut batch: Vec<(String, u64, Option<String>)> = Vec::new();
    let mut hash_cache: HashMap<String, String> = HashMap::new();

    'files: for (position, raw) in paths.iter().enumerate() {
        if cancel.load(Ordering::SeqCst) {
            report.cancelled = true;
            for path in &paths[position..] {
                report.skipped.push(ImportSkip {
                    path: path.clone(),
                    reason: ImportSkipReason::Cancelled,
                });
            }
            break 'files;
        }

        let source = Path::new(raw);
        let canonical = match fs::canonicalize(source) {
            Ok(path) => path,
            Err(error) => {
                report.failed.push(ImportFailure {
                    path: raw.clone(),
                    error: error.to_string(),
                });
                continue;
            }
        };
        let Ok(meta) = fs::metadata(&canonical) else {
            report.failed.push(ImportFailure {
                path: raw.clone(),
                error: i18n::t("файл не найден", "the file was not found").to_string(),
            });
            continue;
        };
        if meta.is_dir() {
            report.skipped.push(ImportSkip {
                path: raw.clone(),
                reason: ImportSkipReason::Folder,
            });
            continue;
        }
        let Some(kind) = kind_for(&canonical) else {
            report.skipped.push(ImportSkip {
                path: raw.clone(),
                reason: ImportSkipReason::Unsupported,
            });
            continue;
        };
        let size = meta.len();
        let canonical_text = canonical.to_string_lossy().into_owned();
        if !batch_seen.insert(canonical_text.clone()) {
            report.skipped.push(ImportSkip {
                path: raw.clone(),
                reason: ImportSkipReason::DuplicateInBatch,
            });
            continue;
        }

        // A file that already lives inside the app folder is not copied.
        // Keep the OS-supplied path for the DB key: on Windows `canonicalize`
        // adds the `\\?\` prefix, which would not match rows written by the
        // scanner.
        if canonical.starts_with(&canonical_root) {
            let inside = if source.is_absolute() {
                source.to_path_buf()
            } else {
                canonical.clone()
            };
            let inside_text = inside.to_string_lossy().into_owned();
            match library.media_get(&inside_text) {
                Ok(Some(item)) => {
                    report.existing.push(item);
                }
                Ok(None) => {
                    let info = probe_info(ffprobe, &inside, kind == "video");
                    match media_row_for(&inside, kind, info, None, "import").and_then(|row| {
                        library.media_upsert(&row)?;
                        Ok(row)
                    }) {
                        Ok(row) => report.imported.push(row.into_item()),
                        Err(error) => report.failed.push(ImportFailure {
                            path: raw.clone(),
                            error,
                        }),
                    }
                }
                Err(error) => report.failed.push(ImportFailure {
                    path: raw.clone(),
                    error,
                }),
            }
            continue;
        }

        let display_name = source
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_owned)
            .unwrap_or_else(|| raw.clone());
        let name = sanitize_file_name(&display_name, kind);

        // Duplicate detection: hash only when some indexed/batched file has the
        // same size. Unique files never pay for a full read.
        let db_candidates = library.media_by_size(size).unwrap_or_default();
        let batch_has_same_size = batch.iter().any(|(_, batch_size, _)| *batch_size == size);
        let mut source_hash: Option<String> = None;
        let mut duplicate: Option<String> = None;
        if !db_candidates.is_empty() || batch_has_same_size {
            source_hash = hash_cached(&mut hash_cache, &canonical);
            if let Some(hash) = source_hash.as_deref() {
                for candidate in &db_candidates {
                    if candidate == &canonical_text {
                        continue;
                    }
                    if hash_cached(&mut hash_cache, Path::new(candidate)).as_deref() == Some(hash) {
                        duplicate = Some(candidate.clone());
                        break;
                    }
                }
                if duplicate.is_none() {
                    for (candidate, batch_size, _) in &batch {
                        if *batch_size == size
                            && candidate != &canonical_text
                            && hash_cached(&mut hash_cache, Path::new(candidate)).as_deref()
                                == Some(hash)
                        {
                            duplicate = Some(candidate.clone());
                            break;
                        }
                    }
                }
            }
        }
        if let Some(candidate) = duplicate {
            if let Ok(Some(item)) = library.media_get(&candidate) {
                report.existing.push(item);
            } else if let Some(item) = report
                .imported
                .iter()
                .find(|item| item.path == candidate)
                .cloned()
            {
                report.existing.push(item);
            } else {
                report.skipped.push(ImportSkip {
                    path: raw.clone(),
                    reason: ImportSkipReason::AlreadyInLibrary,
                });
            }
            continue;
        }

        let target = {
            let (dir, taken) = if kind == "audio" {
                (&audio_dir, &mut taken_audio)
            } else {
                (&video_dir, &mut taken_video)
            };
            match unique_target(dir, &name, taken) {
                Ok(target) => target,
                Err(error) => {
                    report.failed.push(ImportFailure {
                        path: raw.clone(),
                        error,
                    });
                    continue;
                }
            }
        };
        let mut part = target.clone().into_os_string();
        part.push(".part");
        let part = PathBuf::from(part);
        let _ = fs::remove_file(&part);

        on_progress(ImportProgress {
            job_id,
            index: position as u32 + 1,
            total,
            name: display_name.clone(),
            copied_bytes: 0,
            total_bytes: size,
            phase: "copying".into(),
        });
        let mut last_emit = Instant::now();
        let copy_result = copy_with_progress(&canonical, &part, size, cancel, |copied| {
            if copied < size && last_emit.elapsed() < Duration::from_millis(250) {
                return;
            }
            last_emit = Instant::now();
            on_progress(ImportProgress {
                job_id,
                index: position as u32 + 1,
                total,
                name: display_name.clone(),
                copied_bytes: copied,
                total_bytes: size,
                phase: "copying".into(),
            });
        });
        match copy_result {
            Ok(_) => {}
            Err(CopyFailure::Cancelled) => {
                let _ = fs::remove_file(&part);
                report.cancelled = true;
                for path in &paths[position..] {
                    report.skipped.push(ImportSkip {
                        path: path.clone(),
                        reason: ImportSkipReason::Cancelled,
                    });
                }
                break 'files;
            }
            Err(CopyFailure::Failed(error)) => {
                let _ = fs::remove_file(&part);
                report.failed.push(ImportFailure {
                    path: raw.clone(),
                    error,
                });
                continue;
            }
        }

        if let Err(error) = set_modified_now(&part) {
            let _ = fs::remove_file(&part);
            report.failed.push(ImportFailure {
                path: raw.clone(),
                error: error.to_string(),
            });
            continue;
        }
        if let Err(error) = fs::rename(&part, &target) {
            let _ = fs::remove_file(&part);
            report.failed.push(ImportFailure {
                path: raw.clone(),
                error: error.to_string(),
            });
            continue;
        }

        on_progress(ImportProgress {
            job_id,
            index: position as u32 + 1,
            total,
            name: display_name.clone(),
            copied_bytes: size,
            total_bytes: size,
            phase: "probing".into(),
        });
        let info = probe_info(ffprobe, &target, kind == "video");

        let wants_thumb = (kind == "audio" && info.attached_pic) || kind == "video";
        let thumb_path = if wants_thumb {
            let hash = hex::encode(Sha256::digest(target.to_string_lossy().as_bytes()));
            let out = thumbs.join(format!("import-{}.jpg", &hash[..8]));
            let timestamp = if kind == "video" {
                info.duration
                    .map(|duration| (duration * 0.1).min(3.0))
                    .or(Some(0.5))
            } else {
                None
            };
            let _ = on_progress_phase(
                &mut on_progress,
                job_id,
                position as u32 + 1,
                total,
                &display_name,
                size,
                "thumbnailing",
            );
            if extract_thumb(ffmpeg, &target, &out, timestamp) {
                Some(out.to_string_lossy().into_owned())
            } else {
                let _ = fs::remove_file(&out);
                None
            }
        } else {
            None
        };

        let row = match media_row_for(&target, kind, info, thumb_path, "import") {
            Ok(row) => row,
            Err(error) => {
                report.failed.push(ImportFailure {
                    path: raw.clone(),
                    error,
                });
                continue;
            }
        };
        if let Err(error) = library.media_upsert(&row) {
            report.failed.push(ImportFailure {
                path: raw.clone(),
                error,
            });
            continue;
        }
        // Store the destination path: later duplicates in the same batch must
        // resolve to the indexed row, not to the external source.
        batch.push((target.to_string_lossy().into_owned(), size, source_hash));
        report.imported.push(row.into_item());
        let _ = on_progress_phase(
            &mut on_progress,
            job_id,
            position as u32 + 1,
            total,
            &display_name,
            size,
            "done",
        );
    }

    Ok(report)
}

fn on_progress_phase(
    on_progress: &mut impl FnMut(ImportProgress),
    job_id: u64,
    index: u32,
    total: u32,
    name: &str,
    size: u64,
    phase: &str,
) -> Option<()> {
    on_progress(ImportProgress {
        job_id,
        index,
        total,
        name: name.to_string(),
        copied_bytes: size,
        total_bytes: size,
        phase: phase.to_string(),
    });
    Some(())
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
pub async fn library_import(app: AppHandle, paths: Vec<String>) -> Result<ImportReport, String> {
    let (job_id, cancel) = app.state::<ImportState>().begin()?;
    let app_for_import = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let paths_state = app_for_import.state::<AppPaths>();
        let library = app_for_import.state::<Library>();
        import_media(
            &library,
            paths_state.root(),
            &paths_state.ffprobe(),
            &paths_state.ffmpeg(),
            paths,
            &cancel,
            job_id,
            |progress| {
                let _ = app_for_import.emit("library://import-progress", progress);
            },
        )
    })
    .await
    .map_err(|error| error.to_string())?;

    app.state::<ImportState>().finish(job_id);
    let report = result?;
    let _ = app.emit("library://updated", &report);
    Ok(report)
}

#[tauri::command]
pub fn library_import_cancel(state: State<'_, ImportState>) -> bool {
    state.cancel()
}

/// Reads a dropped `.txt` list for the Download screen. Native drag and drop
/// exposes paths only, so the old `File.text()` path needs this small command.
#[tauri::command]
pub fn read_text_file(path: String) -> Result<String, String> {
    let candidate = Path::new(&path);
    let is_txt = candidate
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("txt"));
    if !is_txt {
        return Err(i18n::t(
            "поддерживаются только файлы .txt",
            "only .txt files are supported",
        )
        .to_string());
    }
    let meta = fs::metadata(candidate).map_err(|error| error.to_string())?;
    if meta.len() > 2 * 1024 * 1024 {
        return Err(
            i18n::t("файл списка слишком большой", "the list file is too large").to_string(),
        );
    }
    fs::read_to_string(candidate).map_err(|error| error.to_string())
}

/// Removes leftovers from an interrupted copy. `.part` has no media extension,
/// so the scanner never indexes them.
pub fn sweep_part_files(paths: &AppPaths) {
    for dir in [paths.audio(), paths.video()] {
        let Ok(entries) = fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("part"))
            {
                let _ = fs::remove_file(path);
            }
        }
    }
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

    fn import(fixture: &Fixture, paths: Vec<String>) -> ImportReport {
        let cancel = AtomicBool::new(false);
        import_media(
            &fixture.library,
            &fixture.root,
            Path::new("missing-ffprobe"),
            Path::new("missing-ffmpeg"),
            paths,
            &cancel,
            7,
            |_| {},
        )
        .unwrap()
    }

    fn audio_files(fixture: &Fixture) -> Vec<String> {
        fs::read_dir(fixture.root.join("audio"))
            .unwrap()
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn sanitizes_imported_file_names() {
        assert_eq!(sanitize_file_name(r"..\..\evil.mp3", "audio"), "evil.mp3");
        assert_eq!(sanitize_file_name("CON.mp3", "audio"), "_CON.mp3");
        assert_eq!(sanitize_file_name("a:b?.mp3", "audio"), "a_b_.mp3");
        assert_eq!(sanitize_file_name("", "audio"), "audio");
        assert_eq!(sanitize_file_name("..", "video"), "video");
        assert_eq!(sanitize_file_name("Урок 1.mp3", "audio"), "Урок 1.mp3");
        assert_eq!(sanitize_file_name("Трек 🎸.mp3", "audio"), "Трек 🎸.mp3");

        let long = format!("{}.mp3", "x".repeat(200));
        let sanitized = sanitize_file_name(&long, "audio");
        assert!(sanitized.ends_with(".mp3"));
        assert_eq!(sanitized.chars().count(), 124);
    }

    #[test]
    fn unique_target_avoids_case_insensitive_conflicts() {
        let dir = tempfile::tempdir().unwrap();
        write(&dir.path().join("Song.mp3"), b"audio");
        let mut taken = HashSet::new();

        let first = unique_target(dir.path(), "song.mp3", &mut taken).unwrap();
        assert_eq!(first.file_name().unwrap(), "song (2).mp3");

        let second = unique_target(dir.path(), "song.mp3", &mut taken).unwrap();
        assert_eq!(second.file_name().unwrap(), "song (3).mp3");
    }

    #[test]
    fn sha256_matches_only_identical_content() {
        let dir = tempfile::tempdir().unwrap();
        let first = dir.path().join("a.mp3");
        let second = dir.path().join("b.mp3");
        let other = dir.path().join("c.mp3");
        write(&first, b"same");
        write(&second, b"same");
        write(&other, b"different");

        let first_hash = file_sha256(&first).unwrap();
        assert_eq!(first_hash, file_sha256(&second).unwrap());
        assert_ne!(first_hash, file_sha256(&other).unwrap());
    }

    #[test]
    fn imports_and_indexes_a_file_without_ffprobe() {
        let fixture = fixture("import-copy");
        let source = tempfile::tempdir().unwrap();
        let path = source.path().join("Artist - Song.mp3");
        write(&path, b"1234567890");

        let report = import(&fixture, vec![path.to_string_lossy().into_owned()]);

        assert!(report.imported.len() == 1);
        assert!(report.existing.is_empty());
        assert!(report.failed.is_empty());
        assert_eq!(audio_files(&fixture), vec!["Artist - Song.mp3"]);

        let item = &report.imported[0];
        assert_eq!(item.title, "Artist - Song");
        assert_eq!(item.channel, None);
        assert_eq!(item.size_bytes, 10);
        assert!((now_secs() - item.mtime).abs() <= 5);
        assert!(
            fixture
                .root
                .join("audio")
                .join("Artist - Song.mp3")
                .is_file()
        );
        // The source is copied, never moved or rewritten.
        assert_eq!(fs::read(&path).unwrap(), b"1234567890");
    }

    #[test]
    fn video_import_survives_missing_ffmpeg_and_has_no_poster() {
        let fixture = fixture("import-video-no-ffmpeg");
        let source = tempfile::tempdir().unwrap();
        let path = source.path().join("clip.mp4");
        write(&path, b"not-really-a-video");

        let report = import(&fixture, vec![path.to_string_lossy().into_owned()]);

        assert_eq!(report.imported.len(), 1);
        assert_eq!(report.imported[0].kind, "video");
        assert_eq!(report.imported[0].thumb_path, None);
        assert!(fixture.root.join("video").join("clip.mp4").is_file());
    }

    #[test]
    fn second_import_of_the_same_content_is_reported_as_existing() {
        let fixture = fixture("import-duplicate");
        let source = tempfile::tempdir().unwrap();
        let path = source.path().join("Song.mp3");
        write(&path, b"identical-bytes");
        let path_text = path.to_string_lossy().into_owned();

        assert_eq!(import(&fixture, vec![path_text.clone()]).imported.len(), 1);
        let second = import(&fixture, vec![path_text]);

        assert!(second.imported.is_empty());
        assert_eq!(second.existing.len(), 1);
        assert_eq!(audio_files(&fixture), vec!["Song.mp3"]);
    }

    #[test]
    fn duplicate_inside_one_batch_reuses_the_first_import() {
        let fixture = fixture("import-batch-duplicate");
        let first_dir = tempfile::tempdir().unwrap();
        let second_dir = tempfile::tempdir().unwrap();
        write(&first_dir.path().join("first.mp3"), b"same-content");
        write(&second_dir.path().join("second.mp3"), b"same-content");

        let report = import(
            &fixture,
            vec![
                first_dir
                    .path()
                    .join("first.mp3")
                    .to_string_lossy()
                    .into_owned(),
                second_dir
                    .path()
                    .join("second.mp3")
                    .to_string_lossy()
                    .into_owned(),
            ],
        );

        assert_eq!(report.imported.len(), 1);
        assert_eq!(report.existing.len(), 1);
        assert!(report.skipped.is_empty());
        assert_eq!(audio_files(&fixture), vec!["first.mp3"]);
        assert_eq!(report.existing[0].path, report.imported[0].path);
    }

    #[test]
    fn name_conflict_gets_a_suffix_and_never_overwrites() {
        let fixture = fixture("import-conflict");
        let first_dir = tempfile::tempdir().unwrap();
        let second_dir = tempfile::tempdir().unwrap();
        write(&first_dir.path().join("tone.mp3"), b"AAAA");
        write(&second_dir.path().join("tone.mp3"), b"BBBB");

        let report = import(
            &fixture,
            vec![
                first_dir
                    .path()
                    .join("tone.mp3")
                    .to_string_lossy()
                    .into_owned(),
                second_dir
                    .path()
                    .join("tone.mp3")
                    .to_string_lossy()
                    .into_owned(),
            ],
        );

        assert_eq!(report.imported.len(), 2);
        let mut files = audio_files(&fixture);
        files.sort();
        assert_eq!(files, vec!["tone (2).mp3", "tone.mp3"]);
        assert_eq!(
            fs::read(fixture.root.join("audio").join("tone.mp3")).unwrap(),
            b"AAAA"
        );
        assert_eq!(
            fs::read(fixture.root.join("audio").join("tone (2).mp3")).unwrap(),
            b"BBBB"
        );
    }

    #[test]
    fn file_already_in_the_library_is_not_copied_again() {
        let fixture = fixture("import-inside");
        let path = fixture.root.join("audio").join("inside.mp3");
        write(&path, b"audio");
        scan(&fixture, true);

        let report = import(&fixture, vec![path.to_string_lossy().into_owned()]);

        assert!(report.imported.is_empty());
        assert_eq!(report.existing.len(), 1);
        assert_eq!(report.existing[0].path, path.to_string_lossy());
        assert_eq!(audio_files(&fixture), vec!["inside.mp3"]);
    }

    #[test]
    fn folders_and_unsupported_files_are_skipped() {
        let fixture = fixture("import-skip");
        let source = tempfile::tempdir().unwrap();
        let folder = source.path().join("lesson");
        fs::create_dir_all(&folder).unwrap();
        let pdf = source.path().join("notes.pdf");
        write(&pdf, b"pdf");

        let report = import(
            &fixture,
            vec![
                folder.to_string_lossy().into_owned(),
                pdf.to_string_lossy().into_owned(),
            ],
        );

        assert!(report.imported.is_empty());
        assert_eq!(report.skipped.len(), 2);
        assert!(matches!(report.skipped[0].reason, ImportSkipReason::Folder));
        assert!(matches!(
            report.skipped[1].reason,
            ImportSkipReason::Unsupported
        ));
    }

    #[test]
    fn copy_stops_when_cancelled_between_chunks() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("big.bin");
        let part = dir.path().join("big.bin.part");
        let bytes = vec![7u8; 3 * 1024 * 1024];
        write(&source, &bytes);
        let cancel = AtomicBool::new(false);

        let result = copy_with_progress(&source, &part, bytes.len() as u64, &cancel, |_| {
            cancel.store(true, Ordering::SeqCst)
        });

        assert!(matches!(result, Err(CopyFailure::Cancelled)));
        assert!(part.exists(), "caller removes the partial file");
    }

    #[test]
    fn copy_failure_is_reported_without_a_final_file() {
        let fixture = fixture("import-write-error");
        let source = tempfile::tempdir().unwrap();
        let path = source.path().join("song.mp3");
        write(&path, b"audio");
        // A directory with the staging name makes `File::create` fail.
        fs::create_dir_all(fixture.root.join("audio").join("song.mp3.part")).unwrap();

        let report = import(&fixture, vec![path.to_string_lossy().into_owned()]);

        assert!(report.imported.is_empty());
        assert_eq!(report.failed.len(), 1);
        assert!(!fixture.root.join("audio").join("song.mp3").exists());
    }

    #[test]
    fn cancelled_import_copies_nothing() {
        let fixture = fixture("import-cancel");
        let source = tempfile::tempdir().unwrap();
        let path = source.path().join("Song.mp3");
        write(&path, b"audio");
        let cancel = AtomicBool::new(true);

        let report = import_media(
            &fixture.library,
            &fixture.root,
            Path::new("missing-ffprobe"),
            Path::new("missing-ffmpeg"),
            vec![path.to_string_lossy().into_owned()],
            &cancel,
            1,
            |_| {},
        )
        .unwrap();

        assert!(report.cancelled);
        assert!(report.imported.is_empty());
        assert!(audio_files(&fixture).is_empty());
        assert!(matches!(
            report.skipped[0].reason,
            ImportSkipReason::Cancelled
        ));
    }

    #[test]
    fn upgrades_a_170_database_with_the_origin_column() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("library.db");
        {
            let conn = Connection::open(&db).unwrap();
            conn.execute_batch(
                "CREATE TABLE media (
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
                 INSERT INTO media
                   (path, kind, title, size_bytes, mtime, indexed_at)
                 VALUES ('old.mp3', 'audio', 'Old title', 10, 5, 5);",
            )
            .unwrap();
        }

        let library = Library::open(&db).unwrap();
        let origin: String = library
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT origin FROM media WHERE path = 'old.mp3'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(origin, "scan");
        assert_eq!(
            library.media_get("old.mp3").unwrap().unwrap().title,
            "Old title"
        );
    }

    #[test]
    fn import_state_allows_one_job_at_a_time() {
        let state = ImportState::default();
        let (id, cancel) = state.begin().unwrap();
        assert!(!cancel.load(Ordering::SeqCst));
        assert!(state.begin().is_err());

        assert!(state.cancel());
        assert!(cancel.load(Ordering::SeqCst));
        state.finish(id);
        assert!(state.begin().is_ok());
    }

    #[test]
    fn scan_preserves_imported_titles_artists_and_covers() {
        let fixture = fixture("import-preserve");
        let path = fixture.root.join("audio").join("track.mp3");
        write(&path, b"audio");
        let thumb = fixture
            .root
            .join("data")
            .join("thumbs")
            .join("import-abc.jpg");
        write(&thumb, b"jpg");

        fixture
            .library
            .media_upsert(&MediaRow {
                path: path.to_string_lossy().into_owned(),
                kind: "audio".into(),
                title: "Tagged title".into(),
                channel: Some("Tagged artist".into()),
                duration_secs: Some(12.0),
                width: None,
                height: None,
                size_bytes: 5,
                mtime: mtime_secs(&path),
                thumb_path: Some(thumb.to_string_lossy().into_owned()),
                origin: "import".into(),
                indexed_at: now_secs(),
            })
            .unwrap();

        scan(&fixture, true);

        let item = fixture
            .library
            .media_get(&path.to_string_lossy())
            .unwrap()
            .expect("row must survive the scan");
        assert_eq!(item.title, "Tagged title");
        assert_eq!(item.channel.as_deref(), Some("Tagged artist"));
        assert_eq!(
            item.thumb_path.as_deref(),
            Some(thumb.to_string_lossy().as_ref())
        );
    }

    #[test]
    fn reads_dropped_text_lists_and_rejects_other_files() {
        let dir = tempfile::tempdir().unwrap();
        let txt = dir.path().join("links.txt");
        write(&txt, b"https://youtu.be/a\nhttps://youtu.be/b");
        assert_eq!(
            read_text_file(txt.to_string_lossy().into_owned()).unwrap(),
            "https://youtu.be/a\nhttps://youtu.be/b"
        );

        let pdf = dir.path().join("notes.pdf");
        write(&pdf, b"pdf");
        assert!(read_text_file(pdf.to_string_lossy().into_owned()).is_err());
    }

    fn bundled_binary(name: &str) -> PathBuf {
        let file = if cfg!(windows) {
            format!("{name}.exe")
        } else {
            name.to_string()
        };
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("bin")
            .join(file)
    }

    /// Builds a 1 s MP3 with `title`/`artist` tags and an embedded JPEG cover.
    /// Returns `None` when the local ffmpeg lacks the MP3 encoder.
    fn build_tagged_mp3(
        ffmpeg: &Path,
        dir: &Path,
        file: &str,
        title: &str,
        artist: &str,
        color: &str,
    ) -> Option<PathBuf> {
        let cover = dir.join(format!("{file}-cover.jpg"));
        let tagged = dir.join(file);
        let color = format!("color={color}:s=64x64");
        let title = format!("title={title}");
        let artist = format!("artist={artist}");

        let cover_ok = Command::new(ffmpeg)
            .args([
                "-y",
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                &color,
                "-frames:v",
                "1",
            ])
            .arg(&cover)
            .status()
            .is_ok_and(|status| status.success());
        if !cover_ok {
            return None;
        }
        let tag_ok = Command::new(ffmpeg)
            .args([
                "-y",
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:duration=1",
                "-i",
            ])
            .arg(&cover)
            .args([
                "-map",
                "0:a",
                "-map",
                "1:v",
                "-c:a",
                "libmp3lame",
                "-c:v",
                "mjpeg",
                "-id3v2_version",
                "3",
                "-metadata",
                &title,
                "-metadata",
                &artist,
                "-metadata:s:v",
                "title=Album cover",
                "-metadata:s:v",
                "comment=Cover (front)",
                "-disposition:v",
                "attached_pic",
            ])
            .arg(&tagged)
            .status()
            .is_ok_and(|status| status.success());
        if !tag_ok {
            return None;
        }
        Some(tagged)
    }

    fn build_video(ffmpeg: &Path, dir: &Path) -> Option<PathBuf> {
        let video = dir.join("clip.mp4");
        let created = Command::new(ffmpeg)
            .args([
                "-y",
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "testsrc=size=320x240:rate=10",
                "-t",
                "1",
                "-pix_fmt",
                "yuv420p",
            ])
            .arg(&video)
            .status()
            .is_ok_and(|status| status.success());
        created.then_some(video)
    }

    #[test]
    fn probe_reads_id3_tags_and_cover_when_ffmpeg_is_available() {
        let ffmpeg = bundled_binary("ffmpeg");
        let ffprobe = bundled_binary("ffprobe");
        if !ffmpeg.is_file() || !ffprobe.is_file() {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let Some(tagged) = build_tagged_mp3(
            &ffmpeg,
            dir.path(),
            "tagged.mp3",
            "Tagged title",
            "Tagged artist",
            "red",
        ) else {
            eprintln!("skipping tag probe test: local ffmpeg cannot build the fixture");
            return;
        };

        let info = probe_info(&ffprobe, &tagged, false);
        assert_eq!(info.title.as_deref(), Some("Tagged title"));
        assert_eq!(info.artist.as_deref(), Some("Tagged artist"));
        assert!(info.attached_pic, "cover art must be detected");
        assert!(info.duration.is_some());
    }

    #[test]
    fn extracts_a_video_poster_when_ffmpeg_is_available() {
        let ffmpeg = bundled_binary("ffmpeg");
        if !ffmpeg.is_file() {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let Some(video) = build_video(&ffmpeg, dir.path()) else {
            eprintln!("skipping poster test: local ffmpeg cannot build the fixture");
            return;
        };
        let poster = dir.path().join("poster.jpg");

        assert!(extract_thumb(&ffmpeg, &video, &poster, Some(0.5)));
        assert!(poster.is_file());
        assert!(fs::metadata(&poster).unwrap().len() > 0);
    }

    #[test]
    fn imports_tags_cover_and_keeps_them_after_a_rescan() {
        let ffmpeg = bundled_binary("ffmpeg");
        let ffprobe = bundled_binary("ffprobe");
        if !ffmpeg.is_file() || !ffprobe.is_file() {
            return;
        }
        let fixture = fixture("import-tags");
        let source = tempfile::tempdir().unwrap();
        let Some(mp3) = build_tagged_mp3(
            &ffmpeg,
            source.path(),
            "tagged.mp3",
            "Tagged title",
            "Tagged artist",
            "red",
        ) else {
            return;
        };
        let cancel = AtomicBool::new(false);

        let report = import_media(
            &fixture.library,
            &fixture.root,
            &ffprobe,
            &ffmpeg,
            vec![mp3.to_string_lossy().into_owned()],
            &cancel,
            1,
            |_| {},
        )
        .unwrap();

        assert_eq!(report.imported.len(), 1);
        let item = &report.imported[0];
        assert_eq!(item.title, "Tagged title");
        assert_eq!(item.channel.as_deref(), Some("Tagged artist"));
        assert!(item.duration_secs.is_some());
        let thumb = item.thumb_path.as_deref().expect("cover must be extracted");
        assert!(Path::new(thumb).is_file());

        scan(&fixture, true);
        let after = fixture.library.media_get(&item.path).unwrap().unwrap();
        assert_eq!(after.title, "Tagged title");
        assert_eq!(after.channel.as_deref(), Some("Tagged artist"));
        assert_eq!(after.thumb_path.as_deref(), Some(thumb));
    }

    #[test]
    fn two_files_from_the_same_artist_get_distinct_covers() {
        let ffmpeg = bundled_binary("ffmpeg");
        let ffprobe = bundled_binary("ffprobe");
        if !ffmpeg.is_file() || !ffprobe.is_file() {
            return;
        }
        let fixture = fixture("import-cover-collision");
        let source = tempfile::tempdir().unwrap();
        let Some(first) = build_tagged_mp3(
            &ffmpeg,
            source.path(),
            "one.mp3",
            "One",
            "Same artist",
            "red",
        ) else {
            return;
        };
        let Some(second) = build_tagged_mp3(
            &ffmpeg,
            source.path(),
            "two.mp3",
            "Two",
            "Same artist",
            "blue",
        ) else {
            return;
        };
        let cancel = AtomicBool::new(false);

        let report = import_media(
            &fixture.library,
            &fixture.root,
            &ffprobe,
            &ffmpeg,
            vec![
                first.to_string_lossy().into_owned(),
                second.to_string_lossy().into_owned(),
            ],
            &cancel,
            1,
            |_| {},
        )
        .unwrap();

        assert_eq!(report.imported.len(), 2);
        let first_thumb = report.imported[0].thumb_path.clone().unwrap();
        let second_thumb = report.imported[1].thumb_path.clone().unwrap();
        assert_ne!(first_thumb, second_thumb);
        assert!(Path::new(&first_thumb).is_file());
        assert!(Path::new(&second_thumb).is_file());
        assert!(
            report
                .imported
                .iter()
                .all(|item| item.channel.as_deref() == Some("Same artist"))
        );
    }

    #[test]
    fn imports_a_video_with_a_poster() {
        let ffmpeg = bundled_binary("ffmpeg");
        let ffprobe = bundled_binary("ffprobe");
        if !ffmpeg.is_file() || !ffprobe.is_file() {
            return;
        }
        let fixture = fixture("import-video");
        let source = tempfile::tempdir().unwrap();
        let Some(video) = build_video(&ffmpeg, source.path()) else {
            return;
        };
        let cancel = AtomicBool::new(false);

        let report = import_media(
            &fixture.library,
            &fixture.root,
            &ffprobe,
            &ffmpeg,
            vec![video.to_string_lossy().into_owned()],
            &cancel,
            1,
            |_| {},
        )
        .unwrap();

        assert_eq!(report.imported.len(), 1);
        let item = &report.imported[0];
        assert_eq!(item.kind, "video");
        assert!(item.width.is_some_and(|width| width > 0));
        assert!(fixture.root.join("video").join("clip.mp4").is_file());
        assert!(audio_files(&fixture).is_empty());
        let thumb = item
            .thumb_path
            .as_deref()
            .expect("poster must be extracted");
        assert!(Path::new(thumb).is_file());
    }

    #[test]
    fn startup_sweep_removes_partial_downloads() {
        let fixture = fixture("import-sweep");
        let audio_part = fixture.root.join("audio").join("song.mp3.part");
        let video_part = fixture.root.join("video").join("clip.mp4.part");
        let keep = fixture.root.join("audio").join("song.mp3");
        write(&audio_part, b"partial");
        write(&video_part, b"partial");
        write(&keep, b"audio");

        let paths = crate::paths::AppPaths::portable(fixture.root.clone());
        sweep_part_files(&paths);

        assert!(!audio_part.exists());
        assert!(!video_part.exists());
        assert!(keep.exists());
    }
}
