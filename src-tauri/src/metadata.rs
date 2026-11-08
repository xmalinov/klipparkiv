use std::ffi::OsString;
use std::path::Path;

use serde::Serialize;
use serde_json::Value;

use crate::i18n;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistEntry {
    pub id: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VideoInfo {
    /// `video` for a single item, `playlist` when the link refers to a list.
    pub kind: String,
    pub id: String,
    pub title: String,
    pub channel: Option<String>,
    pub duration_secs: Option<f64>,
    pub thumbnail_url: Option<String>,
    pub upload_date: Option<String>,
    pub view_count: Option<u64>,
    pub playlist_count: Option<u32>,
    /// First few entries, enough for the preview card.
    pub entries: Vec<PlaylistEntry>,
}

/// `--flat-playlist` keeps the probe cheap: yt-dlp lists playlist items without
/// extracting every video. Links that contain both a video and a list resolve
/// to the playlist, so the UI can ask what the user actually wants.
pub const PROBE_ARGS: &[&str] = &[
    "--ignore-config",
    "--no-warnings",
    "--flat-playlist",
    "--no-color",
    "--skip-download",
    "--dump-single-json",
];

const PREVIEW_ENTRIES: usize = 5;

pub fn probe_args(url: &str) -> Vec<OsString> {
    PROBE_ARGS
        .iter()
        .map(OsString::from)
        .chain(std::iter::once(OsString::from(url)))
        .collect()
}

pub async fn probe(exe: &Path, url: &str) -> Result<VideoInfo, String> {
    let output = crate::process::prepare(exe, &probe_args(url))
        .output()
        .await
        .map_err(|error| crate::process::spawn_error("yt-dlp", exe, &error))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let reason = stderr
            .lines()
            .rev()
            .find(|line| line.starts_with("ERROR"))
            .map(str::to_owned)
            .unwrap_or_else(|| {
                i18n::t(
                    "yt-dlp не смог прочитать метаданные",
                    "yt-dlp could not read the metadata",
                )
            });
        return Err(reason.trim().to_string());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_json(&stdout)
}

/// Parses the single JSON object printed by `--dump-single-json`.
pub fn parse_json(raw: &str) -> Result<VideoInfo, String> {
    let value: Value = serde_json::from_str(raw.trim()).map_err(|error| {
        i18n::tf(
            || format!("некорректный JSON от yt-dlp: {error}"),
            || format!("invalid JSON from yt-dlp: {error}"),
        )
    })?;

    let id = string_field(&value, "id")
        .ok_or_else(|| i18n::t("в ответе нет id видео", "the response has no video id"))?;
    let title = string_field(&value, "title").unwrap_or_else(|| id.clone());
    let channel = string_field(&value, "channel").or_else(|| string_field(&value, "uploader"));

    if is_playlist(&value) {
        let entries: Vec<PlaylistEntry> = value
            .get("entries")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| {
                        let id = string_field(item, "id")?;
                        let title = string_field(item, "title").unwrap_or_else(|| id.clone());
                        Some(PlaylistEntry { id, title })
                    })
                    .take(PREVIEW_ENTRIES)
                    .collect()
            })
            .unwrap_or_default();
        let count = value
            .get("playlist_count")
            .and_then(Value::as_u64)
            .or_else(|| value.get("n_entries").and_then(Value::as_u64))
            .or_else(|| {
                value
                    .get("entries")
                    .and_then(Value::as_array)
                    .map(|items| items.len() as u64)
            })
            .unwrap_or(entries.len() as u64) as u32;
        let thumbnail = string_field(&value, "thumbnail").or_else(|| {
            value
                .get("entries")
                .and_then(Value::as_array)
                .and_then(|items| items.first())
                .and_then(|item| string_field(item, "thumbnail"))
        });

        return Ok(VideoInfo {
            kind: "playlist".to_string(),
            id,
            title,
            channel,
            duration_secs: None,
            thumbnail_url: thumbnail,
            upload_date: None,
            view_count: None,
            playlist_count: Some(count),
            entries,
        });
    }

    Ok(VideoInfo {
        kind: "video".to_string(),
        id,
        title,
        channel,
        duration_secs: value.get("duration").and_then(Value::as_f64),
        thumbnail_url: string_field(&value, "thumbnail"),
        upload_date: string_field(&value, "upload_date"),
        view_count: value.get("view_count").and_then(Value::as_u64),
        playlist_count: None,
        entries: Vec::new(),
    })
}

fn is_playlist(value: &Value) -> bool {
    value.get("_type").and_then(Value::as_str) == Some("playlist")
        || value
            .get("entries")
            .is_some_and(|entries| entries.is_array())
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(ToOwned::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
        "id": "dQw4w9WgXcQ",
        "title": "Rick Astley - Never Gonna Give You Up",
        "channel": "Rick Astley",
        "duration": 213.0,
        "thumbnail": "https://i.ytimg.com/vi/dQw4w9WgXcQ/hqdefault.jpg",
        "upload_date": "20091025",
        "view_count": 1200000000,
        "formats": [{"format_id": "251"}]
    }"#;

    const PLAYLIST: &str = r#"{
        "_type": "playlist",
        "id": "PL123",
        "title": "Best of Rick",
        "channel": "Rick Astley",
        "playlist_count": 42,
        "entries": [
            {"id": "a", "title": "One"},
            {"id": "b", "title": "Two"},
            {"id": "c", "title": "Three"}
        ]
    }"#;

    #[test]
    fn parses_full_metadata() {
        let info = parse_json(SAMPLE).unwrap();

        assert_eq!(info.kind, "video");
        assert_eq!(info.id, "dQw4w9WgXcQ");
        assert_eq!(info.title, "Rick Astley - Never Gonna Give You Up");
        assert_eq!(info.channel.as_deref(), Some("Rick Astley"));
        assert_eq!(info.duration_secs, Some(213.0));
        assert_eq!(info.view_count, Some(1_200_000_000));
        assert!(info.thumbnail_url.unwrap().starts_with("https://"));
        assert_eq!(info.playlist_count, None);
        assert!(info.entries.is_empty());
    }

    #[test]
    fn falls_back_to_uploader_and_id() {
        let info = parse_json(r#"{"id":"abc","uploader":"Some channel"}"#).unwrap();

        assert_eq!(info.title, "abc");
        assert_eq!(info.channel.as_deref(), Some("Some channel"));
        assert_eq!(info.duration_secs, None);
    }

    #[test]
    fn recognizes_playlists_and_keeps_a_preview() {
        let info = parse_json(PLAYLIST).unwrap();

        assert_eq!(info.kind, "playlist");
        assert_eq!(info.id, "PL123");
        assert_eq!(info.title, "Best of Rick");
        assert_eq!(info.playlist_count, Some(42));
        assert_eq!(info.entries.len(), 3);
        assert_eq!(info.entries[1].title, "Two");
        assert_eq!(info.duration_secs, None);
    }

    #[test]
    fn caps_the_playlist_preview() {
        let entries: Vec<String> = (0..20)
            .map(|index| format!(r#"{{"id":"v{index}","title":"Item {index}"}}"#))
            .collect();
        let raw = format!(
            r#"{{"_type":"playlist","id":"PL","title":"List","entries":[{}]}}"#,
            entries.join(",")
        );

        let info = parse_json(&raw).unwrap();
        assert_eq!(info.entries.len(), PREVIEW_ENTRIES);
        assert_eq!(info.playlist_count, Some(20));
    }

    #[test]
    fn rejects_missing_id_and_broken_json() {
        assert!(parse_json(r#"{"title":"no id"}"#).is_err());
        assert!(parse_json("not json").is_err());
        assert!(parse_json("").is_err());
    }

    #[test]
    fn probe_args_list_without_extracting_every_video() {
        let args: Vec<String> = super::probe_args("https://youtu.be/abc")
            .into_iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();

        assert!(args.contains(&"--dump-single-json".to_string()));
        assert!(args.contains(&"--skip-download".to_string()));
        assert!(args.contains(&"--flat-playlist".to_string()));
        assert!(!args.contains(&"--no-playlist".to_string()));
        assert_eq!(args.last().unwrap(), "https://youtu.be/abc");
    }
}
