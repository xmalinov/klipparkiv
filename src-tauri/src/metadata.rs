use std::ffi::OsString;
use std::path::Path;

use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VideoInfo {
    pub id: String,
    pub title: String,
    pub channel: Option<String>,
    pub duration_secs: Option<f64>,
    pub thumbnail_url: Option<String>,
    pub upload_date: Option<String>,
    pub view_count: Option<u64>,
}

pub const PROBE_ARGS: &[&str] = &[
    "--ignore-config",
    "--no-warnings",
    "--no-playlist",
    "--no-color",
    "--skip-download",
    "--dump-single-json",
];

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
        .map_err(|error| format!("не удалось запустить yt-dlp: {error}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let reason = stderr
            .lines()
            .rev()
            .find(|line| line.starts_with("ERROR"))
            .unwrap_or("yt-dlp не смог прочитать метаданные");
        return Err(reason.trim().to_string());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_json(&stdout)
}

/// Parses the single JSON object printed by `--dump-single-json`.
pub fn parse_json(raw: &str) -> Result<VideoInfo, String> {
    let value: Value = serde_json::from_str(raw.trim())
        .map_err(|error| format!("некорректный JSON от yt-dlp: {error}"))?;

    let id = string_field(&value, "id").ok_or_else(|| "в ответе нет id видео".to_string())?;
    let title = string_field(&value, "title").unwrap_or_else(|| id.clone());

    Ok(VideoInfo {
        id,
        title,
        channel: string_field(&value, "channel").or_else(|| string_field(&value, "uploader")),
        duration_secs: value.get("duration").and_then(Value::as_f64),
        thumbnail_url: string_field(&value, "thumbnail"),
        upload_date: string_field(&value, "upload_date"),
        view_count: value.get("view_count").and_then(Value::as_u64),
    })
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

    #[test]
    fn parses_full_metadata() {
        let info = parse_json(SAMPLE).unwrap();

        assert_eq!(info.id, "dQw4w9WgXcQ");
        assert_eq!(info.title, "Rick Astley - Never Gonna Give You Up");
        assert_eq!(info.channel.as_deref(), Some("Rick Astley"));
        assert_eq!(info.duration_secs, Some(213.0));
        assert_eq!(info.view_count, Some(1_200_000_000));
        assert!(info.thumbnail_url.unwrap().starts_with("https://"));
    }

    #[test]
    fn falls_back_to_uploader_and_id() {
        let info = parse_json(r#"{"id":"abc","uploader":"Some channel"}"#).unwrap();

        assert_eq!(info.title, "abc");
        assert_eq!(info.channel.as_deref(), Some("Some channel"));
        assert_eq!(info.duration_secs, None);
    }

    #[test]
    fn rejects_missing_id_and_broken_json() {
        assert!(parse_json(r#"{"title":"no id"}"#).is_err());
        assert!(parse_json("not json").is_err());
        assert!(parse_json("").is_err());
    }

    #[test]
    fn probe_args_dump_json_without_download() {
        let args: Vec<String> = super::probe_args("https://youtu.be/abc")
            .into_iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();

        assert!(args.contains(&"--dump-single-json".to_string()));
        assert!(args.contains(&"--skip-download".to_string()));
        assert!(args.contains(&"--no-playlist".to_string()));
        assert_eq!(args.last().unwrap(), "https://youtu.be/abc");
    }
}
