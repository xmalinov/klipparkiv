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

    /// Prefers H.264/AAC inside MP4. yt-dlp would otherwise pick AV1 or VP9,
    /// which QuickTime on macOS cannot decode even when muxed into MP4, so the
    /// player shows a black frame with audio only.
    fn format_selector(self) -> String {
        match self.height() {
            Some(height) => format!(
                "bestvideo[height<={height}][vcodec^=avc1]+bestaudio[acodec^=mp4a]/bestvideo[height<={height}][ext=mp4]+bestaudio[ext=m4a]/best[height<={height}][ext=mp4]"
            ),
            None => "bestvideo[vcodec^=avc1]+bestaudio[acodec^=mp4a]/bestvideo[ext=mp4]+bestaudio[ext=m4a]/best[ext=mp4]/best".to_string(),
        }
    }

    /// Clips keep the historical, more permissive fallbacks from the CLI notes.
    fn clip_selector(self) -> String {
        match self.height() {
            Some(height) => format!(
                "bestvideo[height<={height}][vcodec^=avc1]+bestaudio[acodec^=mp4a]/best[height<={height}][vcodec^=avc1]/bestvideo[height<={height}]+bestaudio/best[height<={height}]"
            ),
            None => "bestvideo[vcodec^=avc1]+bestaudio[acodec^=mp4a]/best[vcodec^=avc1]/bestvideo+bestaudio/best".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipSpec {
    pub start: String,
    /// Normalized timecode or `inf` when the clip runs to the end of the video.
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
    /// An empty start means `00:00:00`, an empty end means `inf` (to the end).
    pub fn parse(start: &str, end: &str) -> Result<Self, ArgsError> {
        let start = start.trim();
        let end = end.trim();
        if start.is_empty() && end.is_empty() {
            return Err(ArgsError::EmptyClipTime);
        }

        let start_secs = if start.is_empty() {
            0
        } else {
            parse_timecode(start).ok_or(ArgsError::InvalidClipTime)?
        };

        let end = if end.is_empty() {
            "inf".to_string()
        } else {
            let end_secs = parse_timecode(end).ok_or(ArgsError::InvalidClipTime)?;
            if start_secs >= end_secs {
                return Err(ArgsError::InvalidClipTime);
            }
            format_timecode(end_secs)
        };

        Ok(Self::new(format_timecode(start_secs), end))
    }

    fn section(&self) -> String {
        format!("*{}-{}", self.start, self.end)
    }

    /// Deterministic suffix for the output template, e.g. `[00-01-30-00-05-00]`.
    /// Colons are avoided so the name is valid on Windows without relying on
    /// yt-dlp sanitizing `%(section_start)s`.
    fn filename_suffix(&self) -> String {
        let start = self.start.replace(':', "-");
        let end = if self.end == "inf" {
            "end".to_string()
        } else {
            self.end.replace(':', "-")
        };
        format!("[{start}-{end}]")
    }
}

/// Playlist options. `None` in `ArgsRequest` means "single video only".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlaylistSpec {
    pub start: Option<u32>,
    pub end: Option<u32>,
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
    PlaylistClip,
    InvalidPlaylistRange,
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
                    "clip start and end are empty"
                } else {
                    "начало и конец клипа не указаны"
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
            ArgsError::PlaylistClip => write!(
                f,
                "{}",
                if english {
                    "clips are not supported for playlists"
                } else {
                    "клипы не поддерживаются для плейлистов"
                }
            ),
            ArgsError::InvalidPlaylistRange => write!(
                f,
                "{}",
                if english {
                    "playlist range must be positive and start <= end"
                } else {
                    "диапазон плейлиста должен быть положительным, начало не больше конца"
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
    pub playlist: Option<PlaylistSpec>,
    pub out_dir: &'a Path,
    pub archive: &'a Path,
    pub ffmpeg_dir: &'a Path,
    pub thumbs_dir: Option<&'a Path>,
}

const MEDIA_NAME: &str = "%(id)s - %(title)s.%(ext)s";
/// Keep room for the directory and the ` - title [range]` suffix below the
/// 260-character Windows limit.
const TRIM_FILENAMES: &str = "150";

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
    if request.playlist.is_some() && request.clip.is_some() {
        return Err(ArgsError::PlaylistClip);
    }
    if let Some(playlist) = request.playlist
        && let (Some(start), Some(end)) = (playlist.start, playlist.end)
        && (start == 0 || start > end)
    {
        return Err(ArgsError::InvalidPlaylistRange);
    }

    let mut args: Vec<OsString> = Vec::new();

    push(&mut args, "--ignore-config");
    push(&mut args, "--newline");
    push(&mut args, "--progress-template");
    push(&mut args, crate::progress::PROGRESS_TEMPLATE);
    push(&mut args, "--no-color");
    match request.playlist {
        Some(_) => {
            push(&mut args, "--yes-playlist");
            // A playlist item may fail without aborting the whole run; the app
            // counts `ERROR:` lines and reports a partial result.
            push(&mut args, "--ignore-errors");
            if let Some(range) = playlist_range(request.playlist) {
                push(&mut args, "-I");
                push(&mut args, &range);
            }
        }
        None => push(&mut args, "--no-playlist"),
    }
    push(&mut args, "--ffmpeg-location");
    args.push(request.ffmpeg_dir.as_os_str().to_owned());
    push(&mut args, "--trim-filenames");
    push(&mut args, TRIM_FILENAMES);

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

    // `--download-archive` stores only the video id, so a second clip from the
    // same video would be skipped. Clips rely on the deterministic filename and
    // yt-dlp's default "do not overwrite" instead.
    if request.clip.is_none() {
        push(&mut args, "--download-archive");
        args.push(request.archive.as_os_str().to_owned());
    }

    push(&mut args, "-o");
    let template = match request.clip {
        Some(clip) => format!("%(id)s - %(title)s {}.%(ext)s", clip.filename_suffix()),
        None => MEDIA_NAME.to_string(),
    };
    let mut output = PathBuf::from(request.out_dir);
    if request.playlist.is_some() {
        output.push("%(playlist_title)s");
    }
    output.push(template);
    args.push(output.into_os_string());

    args.push(OsString::from(url));
    Ok(args)
}

fn playlist_range(playlist: Option<PlaylistSpec>) -> Option<String> {
    let playlist = playlist?;
    match (playlist.start, playlist.end) {
        (None, None) => None,
        (Some(start), None) => Some(format!("{start}:")),
        (None, Some(end)) => Some(format!("1:{end}")),
        (Some(start), Some(end)) => Some(format!("{start}:{end}")),
    }
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
            playlist: None,
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
            "--trim-filenames".to_string(),
            TRIM_FILENAMES.to_string(),
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
            path(&["audio", MEDIA_NAME]),
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
        assert!(args.contains(
            &"bestvideo[height<=1080][vcodec^=avc1]+bestaudio[acodec^=mp4a]/bestvideo[height<=1080][ext=mp4]+bestaudio[ext=m4a]/best[height<=1080][ext=mp4]"
                .to_string()
        ));
        assert_eq!(
            args[args.len() - 3..].to_vec(),
            vec![
                "-o".to_string(),
                path(&["video", MEDIA_NAME]),
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
        assert!(args.contains(&path(&[
            "audio",
            "%(id)s - %(title)s [00-01-30-00-05-00].%(ext)s"
        ])));
        assert!(!args.contains(&"--download-archive".to_string()));
        assert!(!args.iter().any(|arg| arg.contains("%(section_start)s")));
    }

    #[test]
    fn clip_open_end_uses_inf_and_readable_suffix() {
        let (out, archive, bin) = paths();
        let clip = ClipSpec::new("00:01:30", "inf");
        let args = strings(
            build(&request(
                Mode::Video,
                Quality::P1080,
                Some(&clip),
                &out,
                &archive,
                &bin,
            ))
            .unwrap(),
        );

        assert!(
            args.windows(2)
                .any(|w| w == ["--download-sections", "*00:01:30-inf"])
        );
        assert!(args.contains(&path(&[
            "audio",
            "%(id)s - %(title)s [00-01-30-end].%(ext)s"
        ])));
    }

    #[test]
    fn playlist_downloads_use_range_and_subfolder() {
        let (out, archive, bin) = paths();
        let playlist = PlaylistSpec {
            start: Some(2),
            end: Some(10),
        };
        let mut req = request(Mode::Audio, Quality::P1080, None, &out, &archive, &bin);
        req.playlist = Some(playlist);

        let args = strings(build(&req).unwrap());

        assert!(args.contains(&"--yes-playlist".to_string()));
        assert!(args.contains(&"--ignore-errors".to_string()));
        assert!(args.windows(2).any(|w| w == ["-I", "2:10"]));
        assert!(!args.contains(&"--no-playlist".to_string()));
        assert!(args.contains(&path(&["audio", "%(playlist_title)s", MEDIA_NAME])));
    }

    #[test]
    fn single_video_keeps_no_playlist() {
        let (out, archive, bin) = paths();
        let args = strings(
            build(&request(
                Mode::Audio,
                Quality::P1080,
                None,
                &out,
                &archive,
                &bin,
            ))
            .unwrap(),
        );
        assert!(args.contains(&"--no-playlist".to_string()));
        assert!(!args.contains(&"--yes-playlist".to_string()));
    }

    #[test]
    fn rejects_playlist_clips_and_bad_ranges() {
        let (out, archive, bin) = paths();
        let clip = ClipSpec::new("00:01:30", "00:05:00");
        let mut req = request(
            Mode::Video,
            Quality::P1080,
            Some(&clip),
            &out,
            &archive,
            &bin,
        );
        req.playlist = Some(PlaylistSpec {
            start: None,
            end: None,
        });
        assert_eq!(build(&req).unwrap_err(), ArgsError::PlaylistClip);

        let mut req = request(Mode::Audio, Quality::P1080, None, &out, &archive, &bin);
        req.playlist = Some(PlaylistSpec {
            start: Some(0),
            end: Some(3),
        });
        assert_eq!(build(&req).unwrap_err(), ArgsError::InvalidPlaylistRange);

        req.playlist = Some(PlaylistSpec {
            start: Some(5),
            end: Some(3),
        });
        assert_eq!(build(&req).unwrap_err(), ArgsError::InvalidPlaylistRange);
    }

    #[test]
    fn clip_quality_prefers_h264_and_keeps_fallbacks() {
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

        assert!(args.contains(
            &"bestvideo[height<=1080][vcodec^=avc1]+bestaudio[acodec^=mp4a]/best[height<=1080][vcodec^=avc1]/bestvideo[height<=1080]+bestaudio/best[height<=1080]"
                .to_string()
        ));
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
        let clip = ClipSpec::new("", "");
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

        assert!(args.contains(
            &"bestvideo[height<=2160][vcodec^=avc1]+bestaudio[acodec^=mp4a]/bestvideo[height<=2160][ext=mp4]+bestaudio[ext=m4a]/best[height<=2160][ext=mp4]"
                .to_string()
        ));
    }

    #[test]
    fn best_quality_prefers_h264_for_quicktime() {
        let (out, archive, bin) = paths();
        let args = strings(
            build(&request(
                Mode::Video,
                Quality::Best,
                None,
                &out,
                &archive,
                &bin,
            ))
            .unwrap(),
        );

        assert!(args.contains(
            &"bestvideo[vcodec^=avc1]+bestaudio[acodec^=mp4a]/bestvideo[ext=mp4]+bestaudio[ext=m4a]/best[ext=mp4]/best"
                .to_string()
        ));
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

        let from_start = ClipSpec::parse("", "2:00").unwrap();
        assert_eq!(from_start.start, "00:00:00");
        assert_eq!(from_start.end, "00:02:00");

        let to_end = ClipSpec::parse("1:30", "").unwrap();
        assert_eq!(to_end.start, "00:01:30");
        assert_eq!(to_end.end, "inf");

        assert_eq!(
            ClipSpec::parse("", "").unwrap_err(),
            ArgsError::EmptyClipTime
        );
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
