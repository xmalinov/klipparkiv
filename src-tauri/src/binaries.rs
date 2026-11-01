use std::path::Path;
use std::process::Command;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// `yt-dlp --version` prints exactly one line, e.g. `2026.08.19`.
pub fn parse_ytdlp_version(stdout: &str) -> Option<String> {
    let line = stdout.lines().next()?.trim();
    let looks_like_version = !line.is_empty()
        && line
            .chars()
            .all(|c| c.is_ascii_digit() || c == '.' || c == '-');
    looks_like_version.then(|| line.to_string())
}

/// First line of `ffmpeg -version` / `ffprobe -version`:
/// `ffmpeg version 2026-10-29-git-31ba826726-essentials_build-www.gyan.dev`
pub fn parse_ffmpeg_version(stdout: &str) -> Option<String> {
    let line = stdout.lines().next()?.trim();
    let rest = line
        .strip_prefix("ffmpeg version ")
        .or_else(|| line.strip_prefix("ffprobe version "))?;
    let token = rest.split_whitespace().next()?;
    let build = token.split("-git-").next().unwrap_or(token);
    (!build.is_empty()).then(|| build.to_string())
}

/// Runs a binary and returns its first output line, hidden from the console.
pub fn read_version(exe: &Path, arg: &str) -> Option<String> {
    let mut command = Command::new(exe);
    command.arg(arg);
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);
    let output = command.output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    Some(text.lines().next()?.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ytdlp_version() {
        assert_eq!(
            parse_ytdlp_version("2026.08.19\n").as_deref(),
            Some("2026.08.19")
        );
    }

    #[test]
    fn rejects_ytdlp_noise() {
        assert_eq!(parse_ytdlp_version(""), None);
        assert_eq!(parse_ytdlp_version("ERROR: something\n"), None);
    }

    #[test]
    fn parses_ffmpeg_git_build() {
        let line = "ffmpeg version 2026-10-29-git-31ba826726-essentials_build-www.gyan.dev Copyright (c) 2000-2026 the FFmpeg developers";
        assert_eq!(parse_ffmpeg_version(line).as_deref(), Some("2026-10-29"));
    }

    #[test]
    fn parses_ffmpeg_release_build() {
        assert_eq!(
            parse_ffmpeg_version("ffmpeg version 7.1.1 Copyright (c)").as_deref(),
            Some("7.1.1")
        );
    }

    #[test]
    fn parses_ffprobe_version() {
        assert_eq!(
            parse_ffmpeg_version("ffprobe version 2026-10-29-git-31ba826726-essentials_build-www.gyan.dev Copyright (c) 2007-2026 the FFmpeg developers")
                .as_deref(),
            Some("2026-10-29")
        );
    }

    #[test]
    fn ignores_unrelated_output() {
        assert_eq!(parse_ffmpeg_version("no ffmpeg here"), None);
        assert_eq!(parse_ffmpeg_version(""), None);
    }
}
