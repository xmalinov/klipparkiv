use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::i18n;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Audio,
    Video,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quality {
    P2160,
    P1440,
    P1080,
    P720,
    P480,
    Best,
}

impl Quality {
    fn height(self) -> Option<u32> {
        match self {
            Quality::P2160 => Some(2160),
            Quality::P1440 => Some(1440),
            Quality::P1080 => Some(1080),
            Quality::P720 => Some(720),
            Quality::P480 => Some(480),
            Quality::Best => None,
        }
    }

    fn format_selector(self) -> String {
        match self.height() {
            Some(height) => format!(
                "bestvideo[height<={height}][ext=mp4]+bestaudio[ext=m4a]/best[height<={height}][ext=mp4]"
            ),
            None => "bestvideo+bestaudio/best".to_string(),
        }
    }

    /// Clips keep the historical, more permissive selector from the CLI notes.
    fn clip_selector(self) -> String {
        match self.height() {
            Some(height) => format!("bestvideo[height<={height}]+bestaudio/best[height<={height}]"),
            None => "bestvideo+bestaudio/best".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipSpec {
    pub start: String,
    pub end: String,
}

impl ClipSpec {
    pub fn new(start: impl Into<String>, end: impl Into<String>) -> Self {
        Self {
            start: start.into(),
            end: end.into(),
        }
    }

    /// Accepts `MM:SS` or `HH:MM:SS`, normalizes to `HH:MM:SS` and requires start < end.
    pub fn parse(start: &str, end: &str) -> Result<Self, ArgsError> {
        let start_secs = parse_timecode(start).ok_or(ArgsError::InvalidClipTime)?;
        let end_secs = parse_timecode(end).ok_or(ArgsError::InvalidClipTime)?;
        if start_secs >= end_secs {
            return Err(ArgsError::InvalidClipTime);
        }
        Ok(Self::new(
            format_timecode(start_secs),
            format_timecode(end_secs),
        ))
    }

    fn section(&self) -> String {
        format!("*{}-{}", self.start, self.end)
    }
}

/// Parses `90`, `1:30` or `00:01:30` into seconds.
pub fn parse_timecode(raw: &str) -> Option<u32> {
    let text = raw.trim();
    if text.is_empty() {
        return None;
    }
    let parts: Vec<&str> = text.split(':').collect();
    if parts.len() > 3 || parts.iter().any(|part| part.is_empty()) {
        return None;
    }
    let mut seconds: u32 = 0;
    for (index, part) in parts.iter().enumerate() {
        if !part.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        let value: u32 = part.parse().ok()?;
        let is_hours = parts.len() == 3 && index == 0;
        if parts.len() > 1 && !is_hours && value >= 60 {
            return None;
        }
        seconds = seconds.checked_mul(60)?.checked_add(value)?;
    }
    Some(seconds)
}

pub fn format_timecode(seconds: u32) -> String {
    format!(
        "{:02}:{:02}:{:02}",
        seconds / 3600,
        (seconds % 3600) / 60,
        seconds % 60
    )
}

/// Each mode keeps its own archive: a shared file would mark the same video as
/// downloaded for both audio and video.
pub fn archive_file(mode: Mode, clip: bool) -> &'static str {
    match (mode, clip) {
        (Mode::Audio, false) => "archive_audio.txt",
        (Mode::Video, false) => "archive_video.txt",
        (Mode::Audio, true) => "archive_audio_clips.txt",
        (Mode::Video, true) => "archive_video_clips.txt",
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArgsError {
    EmptyUrl,
    EmptyClipTime,
    InvalidClipTime,
}

impl std::fmt::Display for ArgsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let english = i18n::is_english();
        match self {
            ArgsError::EmptyUrl => write!(
                f,
                "{}",
                if english {
                    "url is empty"
                } else {
                    "ссылка не указана"
                }
            ),
            ArgsError::EmptyClipTime => write!(
                f,
                "{}",
                if english {
                    "clip start and end are required"
                } else {
                    "нужны начало и конец клипа"
                }
            ),
            ArgsError::InvalidClipTime => write!(
                f,
                "{}",
                if english {
                    "clip time must be MM:SS or HH:MM:SS and start < end"
                } else {
                    "время клипа должно быть MM:SS или HH:MM:SS, начало раньше конца"
                }
            ),
        }
    }
}

impl std::error::Error for ArgsError {}

pub struct ArgsRequest<'a> {
    pub url: &'a str,
    pub mode: Mode,
    pub quality: Quality,
    pub audio_kbps: u32,
    pub clip: Option<&'a ClipSpec>,
    pub out_dir: &'a Path,
    pub archive: &'a Path,
    pub ffmpeg_dir: &'a Path,
    pub thumbs_dir: Option<&'a Path>,
}

const AUDIO_NAME: &str = "%(id)s - %(title)s.%(ext)s";
const CLIP_NAME: &str = "%(id)s_%(section_start)s-%(section_end)s.%(ext)s";

/// Builds the full yt-dlp argv (without the exe itself).
/// Golden tests compare it with the commands in `..\README.md`.
pub fn build(request: &ArgsRequest<'_>) -> Result<Vec<OsString>, ArgsError> {
    let url = request.url.trim();
    if url.is_empty() {
        return Err(ArgsError::EmptyUrl);
    }
    if let Some(clip) = request.clip
        && (clip.start.trim().is_empty() || clip.end.trim().is_empty())
    {
        return Err(ArgsError::EmptyClipTime);
    }

    let mut args: Vec<OsString> = Vec::new();

    push(&mut args, "--ignore-config");
    push(&mut args, "--newline");
    push(&mut args, "--progress-template");
    push(&mut args, crate::progress::PROGRESS_TEMPLATE);
    push(&mut args, "--no-color");
    push(&mut args, "--no-playlist");
    push(&mut args, "--ffmpeg-location");
    args.push(request.ffmpeg_dir.as_os_str().to_owned());

    if let Some(thumbs) = request.thumbs_dir {
        push(&mut args, "--write-thumbnail");
        push(&mut args, "--convert-thumbnails");
        push(&mut args, "jpg");
        // `-P thumbnail:` is ignored when the media `-o` is absolute, so the
        // thumbnail location is a separate typed output template.
        push(&mut args, "-o");
        let mut template = OsString::from("thumbnail:");
        template.push(thumbs.join("%(id)s.%(ext)s"));
        args.push(template);
    }

    match request.mode {
        Mode::Audio => {
            push(&mut args, "-x");
            push(&mut args, "--audio-format");
            push(&mut args, "mp3");
            push(&mut args, "--audio-quality");
            let quality = format!("{}K", request.audio_kbps);
            push(&mut args, &quality);
            push(&mut args, "--embed-thumbnail");
            push(&mut args, "--add-metadata");
        }
        Mode::Video => {
            let selector = match request.clip {
                Some(_) => request.quality.clip_selector(),
                None => request.quality.format_selector(),
            };
            push(&mut args, "-f");
            push(&mut args, &selector);
            push(&mut args, "--merge-output-format");
            push(&mut args, "mp4");
            push(&mut args, "-N");
            push(&mut args, "8");
        }
    }

    if let Some(clip) = request.clip {
        push(&mut args, "--download-sections");
        args.push(OsString::from(clip.section()));
    }

    push(&mut args, "--download-archive");
    args.push(request.archive.as_os_str().to_owned());

    push(&mut args, "-o");
    let template = match request.clip {
        Some(_) => CLIP_NAME,
        None => AUDIO_NAME,
    };
    let mut output = PathBuf::from(request.out_dir);
    output.push(template);
    args.push(output.into_os_string());

    args.push(OsString::from(url));
    Ok(args)
}

fn push(args: &mut Vec<OsString>, value: &str) {
    args.push(OsString::from(value));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    const ROOT: &str = r"C:\apps\klipparkiv";
    #[cfg(not(windows))]
    const ROOT: &str = "/apps/klipparkiv";
    const URL: &str = "https://www.youtube.com/watch?v=dQw4w9WgXcQ";

    fn path(parts: &[&str]) -> String {
        let mut path = PathBuf::from(ROOT);
        for part in parts {
            path.push(part);
        }
        path.to_string_lossy().into_owned()
    }

    fn paths() -> (PathBuf, PathBuf, PathBuf) {
        let root = PathBuf::from(ROOT);
        (
            root.join("audio"),
            root.join("data").join("archive_audio.txt"),
            root.join("bin"),
        )
    }

    fn strings(args: Vec<OsString>) -> Vec<String> {
        args.into_iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect()
    }

    fn request<'a>(
        mode: Mode,
        quality: Quality,
        clip: Option<&'a ClipSpec>,
        out: &'a Path,
        archive: &'a Path,
        bin: &'a Path,
    ) -> ArgsRequest<'a> {
        ArgsRequest {
            url: URL,
            mode,
            quality,
            audio_kbps: 320,
            clip,
            out_dir: out,
            archive,
            ffmpeg_dir: bin,
            thumbs_dir: None,
        }
    }

    #[test]
    fn audio_matches_cli_command() {
        let (out, archive, bin) = paths();
        let args = build(&request(
            Mode::Audio,
            Quality::P1080,
            None,
            &out,
            &archive,
            &bin,
        ))
        .unwrap();

        let expected: Vec<String> = vec![
            "--ignore-config".to_string(),
            "--newline".to_string(),
            "--progress-template".to_string(),
            crate::progress::PROGRESS_TEMPLATE.to_string(),
            "--no-color".to_string(),
            "--no-playlist".to_string(),
            "--ffmpeg-location".to_string(),
            path(&["bin"]),
            "-x".to_string(),
            "--audio-format".to_string(),
            "mp3".to_string(),
            "--audio-quality".to_string(),
            "320K".to_string(),
            "--embed-thumbnail".to_string(),
            "--add-metadata".to_string(),
            "--download-archive".to_string(),
            path(&["data", "archive_audio.txt"]),
            "-o".to_string(),
            path(&["audio", AUDIO_NAME]),
            URL.to_string(),
        ];
        assert_eq!(strings(args), expected);
    }

    #[test]
    fn video_1080_matches_cli_command() {
        let (_, archive, bin) = paths();
        let out = PathBuf::from(ROOT).join("video");
        let args = build(&request(
            Mode::Video,
            Quality::P1080,
            None,
            &out,
            &archive,
            &bin,
        ))
        .unwrap();
        let args = strings(args);

        assert!(args.contains(&"-f".to_string()));
        assert!(
            args.contains(
                &"bestvideo[height<=1080][ext=mp4]+bestaudio[ext=m4a]/best[height<=1080][ext=mp4]"
                    .to_string()
            )
        );
        assert_eq!(
            args[args.len() - 3..].to_vec(),
            vec![
                "-o".to_string(),
                path(&["video", AUDIO_NAME]),
                URL.to_string(),
            ]
        );
        assert_eq!(args.last().unwrap(), URL);
        assert!(args.windows(2).any(|w| w == ["-N", "8"]));
        assert!(
            args.windows(2)
                .any(|w| w == ["--merge-output-format", "mp4"])
        );
    }

    #[test]
    fn clip_uses_section_template_and_range() {
        let (out, archive, bin) = paths();
        let clip = ClipSpec::new("00:01:30", "00:05:00");
        let args = build(&request(
            Mode::Audio,
            Quality::P1080,
            Some(&clip),
            &out,
            &archive,
            &bin,
        ))
        .unwrap();
        let args = strings(args);

        assert!(
            args.windows(2)
                .any(|w| w == ["--download-sections", "*00:01:30-00:05:00"])
        );
        assert!(args.contains(&path(&["audio", CLIP_NAME]).to_string()));
    }

    #[test]
    fn clip_quality_uses_permissive_selector() {
        let (out, archive, bin) = paths();
        let clip = ClipSpec::new("00:00:00", "00:00:20");
        let args = build(&request(
            Mode::Video,
            Quality::P1080,
            Some(&clip),
            &out,
            &archive,
            &bin,
        ))
        .unwrap();
        let args = strings(args);

        assert!(args.contains(&"bestvideo[height<=1080]+bestaudio/best[height<=1080]".to_string()));
    }

    #[test]
    fn archive_and_output_are_absolute_app_paths() {
        let (out, archive, bin) = paths();
        let args = build(&request(
            Mode::Audio,
            Quality::P1080,
            None,
            &out,
            &archive,
            &bin,
        ))
        .unwrap();
        let args = strings(args);

        assert!(args.contains(&path(&["data", "archive_audio.txt"])));
        assert!(args.iter().all(|a| !a.contains("..")));
    }

    #[test]
    fn rejects_empty_url() {
        let (out, archive, bin) = paths();
        let mut req = request(Mode::Audio, Quality::P1080, None, &out, &archive, &bin);
        req.url = "   ";
        assert_eq!(build(&req).unwrap_err(), ArgsError::EmptyUrl);
    }

    #[test]
    fn rejects_empty_clip_time() {
        let (out, archive, bin) = paths();
        let clip = ClipSpec::new("", "00:05:00");
        assert_eq!(
            build(&request(
                Mode::Video,
                Quality::P1080,
                Some(&clip),
                &out,
                &archive,
                &bin
            ))
            .unwrap_err(),
            ArgsError::EmptyClipTime
        );
    }

    #[test]
    fn writes_thumbnails_into_cache_when_enabled() {
        let (out, archive, bin) = paths();
        let thumbs = PathBuf::from(ROOT).join("data").join("thumbs");
        let mut req = request(Mode::Audio, Quality::P1080, None, &out, &archive, &bin);
        req.thumbs_dir = Some(&thumbs);

        let args = strings(build(&req).unwrap());

        assert!(
            args.windows(2)
                .any(|w| w == ["--convert-thumbnails", "jpg"])
        );
        assert!(args.contains(&format!(
            "thumbnail:{}",
            path(&["data", "thumbs", "%(id)s.%(ext)s"])
        )));
        assert!(!args.contains(&"-P".to_string()));
    }

    #[test]
    fn audio_bitrate_is_configurable() {
        let (out, archive, bin) = paths();
        let mut req = request(Mode::Audio, Quality::P1080, None, &out, &archive, &bin);
        req.audio_kbps = 128;

        let args = strings(build(&req).unwrap());
        assert!(args.contains(&"128K".to_string()));
        assert!(!args.contains(&"--postprocessor-args".to_string()));
    }

    #[test]
    fn video_2160_uses_height_filter() {
        let (out, archive, bin) = paths();
        let args = strings(
            build(&request(
                Mode::Video,
                Quality::P2160,
                None,
                &out,
                &archive,
                &bin,
            ))
            .unwrap(),
        );

        assert!(
            args.contains(
                &"bestvideo[height<=2160][ext=mp4]+bestaudio[ext=m4a]/best[height<=2160][ext=mp4]"
                    .to_string()
            )
        );
    }

    #[test]
    fn parses_timecodes() {
        assert_eq!(parse_timecode("90"), Some(90));
        assert_eq!(parse_timecode("1:30"), Some(90));
        assert_eq!(parse_timecode("00:01:30"), Some(90));
        assert_eq!(parse_timecode("1:02:03"), Some(3723));
        assert_eq!(parse_timecode("01:60"), None);
        assert_eq!(parse_timecode("aa:bb"), None);
        assert_eq!(parse_timecode(""), None);
    }

    #[test]
    fn formats_timecodes_with_leading_zeros() {
        assert_eq!(format_timecode(90), "00:01:30");
        assert_eq!(format_timecode(3723), "01:02:03");
        assert_eq!(format_timecode(0), "00:00:00");
    }

    #[test]
    fn clip_spec_normalizes_and_validates_order() {
        let clip = ClipSpec::parse("1:30", "00:05:00").unwrap();
        assert_eq!(clip.start, "00:01:30");
        assert_eq!(clip.end, "00:05:00");

        assert_eq!(
            ClipSpec::parse("00:05:00", "00:01:30").unwrap_err(),
            ArgsError::InvalidClipTime
        );
        assert_eq!(
            ClipSpec::parse("bad", "00:01:30").unwrap_err(),
            ArgsError::InvalidClipTime
        );
    }

    #[test]
    fn archives_are_separate_per_mode_and_clip() {
        assert_eq!(archive_file(Mode::Audio, false), "archive_audio.txt");
        assert_eq!(archive_file(Mode::Video, false), "archive_video.txt");
        assert_eq!(archive_file(Mode::Audio, true), "archive_audio_clips.txt");
        assert_eq!(archive_file(Mode::Video, true), "archive_video_clips.txt");
    }
}
