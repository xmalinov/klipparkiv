use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use rusqlite::params;
use walkdir::WalkDir;

use crate::i18n;
use crate::library::LibraryPaths;
use crate::library::db::Library;
use crate::library::guard::{excluded, kind_for};
use crate::library::model::{MediaKind, ScanStats};
use crate::library::thumbs::thumb_id;
use crate::library::util::{display_title, mtime_secs, now_secs, run_command};
#[derive(Debug, Default, Clone)]
pub(crate) struct ProbeInfo {
    pub(crate) duration: Option<f64>,
    pub(crate) width: Option<u32>,
    pub(crate) height: Option<u32>,
    pub(crate) title: Option<String>,
    pub(crate) artist: Option<String>,
    pub(crate) attached_pic: bool,
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

pub(crate) fn probe_info(ffprobe: &Path, path: &Path, want_video: bool) -> ProbeInfo {
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

pub(crate) fn scan_dir(
    library: &Library,
    layout: &LibraryPaths,
    ffprobe: &Path,
    max_depth: usize,
    full: bool,
    mut on_progress: impl FnMut(u32),
) -> Result<ScanStats, String> {
    let thumbs = layout.thumbs();
    let mut stats = ScanStats::default();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let conn = library.conn.lock().map_err(|error| error.to_string())?;

    let walker = WalkDir::new(layout.root())
        .max_depth(max_depth)
        .into_iter()
        .filter_entry(|entry| entry.path() == layout.root() || !excluded(entry.path()));

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
        let info = probe_info(ffprobe, path, kind == MediaKind::Video);

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
                    kind.as_str(),
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
