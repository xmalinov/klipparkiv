use std::path::PathBuf;

use crate::args::{ClipSpec, Mode, PlaylistSpec, Quality};
use crate::i18n;
pub(crate) const MAX_BATCH_LINES: usize = 500;
pub(crate) const MAX_PLAYLIST_ITEMS: u32 = 500;

pub(crate) struct BatchEntry {
    pub url: String,
    pub clip: Option<ClipSpec>,
}

pub fn parse_modes(raw: &str) -> Result<Vec<Mode>, String> {
    match raw {
        "audio" => Ok(vec![Mode::Audio]),
        "video" => Ok(vec![Mode::Video]),
        "both" => Ok(vec![Mode::Audio, Mode::Video]),
        other => Err(i18n::tf(
            || format!("неизвестный режим: {other}"),
            || format!("unknown mode: {other}"),
        )),
    }
}

pub fn parse_quality(raw: Option<&str>) -> Result<Quality, String> {
    match raw.unwrap_or("1080") {
        "2160" => Ok(Quality::P2160),
        "1440" => Ok(Quality::P1440),
        "1080" => Ok(Quality::P1080),
        "720" => Ok(Quality::P720),
        "480" => Ok(Quality::P480),
        "best" => Ok(Quality::Best),
        other => Err(i18n::tf(
            || format!("неизвестное качество: {other}"),
            || format!("unknown quality: {other}"),
        )),
    }
}

pub fn parse_audio_kbps(raw: Option<u32>) -> Result<u32, String> {
    let kbps = raw.unwrap_or(320);
    if [128, 192, 256, 320].contains(&kbps) {
        Ok(kbps)
    } else {
        Err(i18n::tf(
            || format!("неизвестный битрейт аудио: {kbps}"),
            || format!("unknown audio bitrate: {kbps}"),
        ))
    }
}

pub fn parse_clip(start: Option<&str>, end: Option<&str>) -> Result<Option<ClipSpec>, String> {
    match (start.map(str::trim), end.map(str::trim)) {
        (None, None) => Ok(None),
        (Some(""), Some("")) => Ok(None),
        (Some(start), Some(end)) => ClipSpec::parse(start, end)
            .map(Some)
            .map_err(|error| error.to_string()),
        (Some(start), None) => ClipSpec::parse(start, "")
            .map(Some)
            .map_err(|error| error.to_string()),
        (None, Some(end)) => ClipSpec::parse("", end)
            .map(Some)
            .map_err(|error| error.to_string()),
    }
}

pub fn parse_playlist(
    enabled: Option<bool>,
    start: Option<u32>,
    end: Option<u32>,
) -> Result<Option<PlaylistSpec>, String> {
    if !enabled.unwrap_or(false) {
        if start.is_some() || end.is_some() {
            return Err(i18n::t(
                "диапазон плейлиста указан без самого плейлиста",
                "a playlist range was given without a playlist",
            ));
        }
        return Ok(None);
    }
    if start == Some(0) || end == Some(0) {
        return Err(i18n::t(
            "номер видео в плейлисте начинается с 1",
            "playlist item numbers start at 1",
        ));
    }
    if let (Some(first), Some(last)) = (start, end)
        && first > last
    {
        return Err(i18n::t(
            "начало диапазона больше конца",
            "the range start is greater than its end",
        ));
    }
    if start.is_some_and(|value| value > MAX_PLAYLIST_ITEMS)
        || end.is_some_and(|value| value > MAX_PLAYLIST_ITEMS)
    {
        return Err(i18n::tf(
            || format!("за раз можно скачать не больше {MAX_PLAYLIST_ITEMS} видео"),
            || format!("at most {MAX_PLAYLIST_ITEMS} videos can be downloaded at once"),
        ));
    }
    Ok(Some(PlaylistSpec {
        start,
        // A hard cap keeps a 5000-item playlist from running for hours unnoticed.
        end: Some(end.unwrap_or(MAX_PLAYLIST_ITEMS)),
    }))
}

pub(crate) fn overall_percent(percent: f32, index: Option<u32>, count: Option<u32>) -> f32 {
    match (index, count) {
        (Some(index), Some(count)) if count > 1 && index >= 1 => {
            let item = (percent / 100.0).clamp(0.0, 1.0);
            (((index - 1) as f32 + item) / count as f32 * 100.0).clamp(0.0, 100.0)
        }
        _ => percent.clamp(0.0, 100.0),
    }
}

pub(crate) fn resolve_out_dir(raw: Option<&str>) -> Result<Option<PathBuf>, String> {
    match raw.map(str::trim).filter(|dir| !dir.is_empty()) {
        Some(dir) => {
            let path = PathBuf::from(dir);
            if !path.is_absolute() {
                return Err(i18n::t(
                    "Папка назначения должна быть абсолютным путём",
                    "The destination folder must be an absolute path",
                ));
            }
            if !path.is_dir() {
                return Err(i18n::t(
                    "Папка назначения не найдена",
                    "The destination folder was not found",
                ));
            }
            Ok(Some(path))
        }
        None => Ok(None),
    }
}

pub fn validate_url(raw: &str) -> Result<String, String> {
    let url = raw.trim();
    if url.is_empty() {
        return Err(i18n::t("Вставьте ссылку", "Paste a link"));
    }
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err(i18n::t(
            "Ссылка должна начинаться с http:// или https://",
            "The link must start with http:// or https://",
        ));
    }
    Ok(url.to_string())
}

/// Parses a `.txt` list: one URL per line, optionally followed by
/// `HH:MM:SS-HH:MM:SS` (the format used by `urls-clips.txt`).
pub fn parse_list(text: &str) -> Result<Vec<BatchEntry>, String> {
    // Notepad writes UTF-8 with BOM; U+FEFF is not whitespace, so it must be
    // stripped before the first line is parsed.
    let text = text.trim_start_matches('\u{feff}');
    let mut entries = Vec::new();

    for (index, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split_whitespace();
        let url = parts.next().ok_or_else(|| {
            i18n::tf(
                || format!("строка {}: пустая ссылка", index + 1),
                || format!("line {}: empty link", index + 1),
            )
        })?;
        let url = validate_url(url).map_err(|error| {
            i18n::tf(
                || format!("строка {}: {error}", index + 1),
                || format!("line {}: {error}", index + 1),
            )
        })?;

        let clip = match parts.next() {
            None => None,
            Some(range) => {
                let (start, end) = range.split_once('-').ok_or_else(|| {
                    i18n::tf(
                        || {
                            format!(
                                "строка {}: таймкод должен быть вида 00:01:30-00:05:00",
                                index + 1
                            )
                        },
                        || {
                            format!(
                                "line {}: timecode must look like 00:01:30-00:05:00",
                                index + 1
                            )
                        },
                    )
                })?;
                let clip = ClipSpec::parse(start, end).map_err(|error| {
                    i18n::tf(
                        || format!("строка {}: {error}", index + 1),
                        || format!("line {}: {error}", index + 1),
                    )
                })?;
                Some(clip)
            }
        };

        entries.push(BatchEntry { url, clip });

        if entries.len() > MAX_BATCH_LINES {
            return Err(i18n::tf(
                || format!("в файле больше {MAX_BATCH_LINES} ссылок"),
                || format!("the file contains more than {MAX_BATCH_LINES} links"),
            ));
        }
    }

    if entries.is_empty() {
        return Err(i18n::t(
            "в файле не найдено ссылок",
            "no links found in the file",
        ));
    }
    Ok(entries)
}
