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

/// On macOS the bundled yt-dlp is read-only signed code. Keep a managed copy
/// below the support root so `yt-dlp --update` never modifies the `.app`.
#[cfg(target_os = "macos")]
pub fn ensure_managed_ytdlp(paths: &crate::paths::AppPaths) -> Option<std::path::PathBuf> {
    use crate::paths::exe_name;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    let bundled = paths.bundled_ytdlp();
    let managed = paths.managed_bin().join(exe_name("yt-dlp"));

    if !bundled.is_file() {
        return managed.is_file().then_some(managed);
    }

    let marker = paths.managed_bin().join(".bundle-version");
    let current_version = env!("CARGO_PKG_VERSION");
    let copied_for = fs::read_to_string(&marker).ok();
    let same_app_version = copied_for.as_deref() == Some(current_version);
    let bundled_version = read_cached_version(paths.bin().join("yt-dlp.version"));
    let managed_version = read_cached_version(paths.managed_bin().join("yt-dlp.version"));
    let bundled_newer = match (&bundled_version, &managed_version) {
        (Some(bundled), Some(managed)) => version_newer(bundled, managed),
        _ => fs::metadata(&bundled)
            .and_then(|meta| meta.modified())
            .ok()
            .zip(fs::metadata(&managed).and_then(|meta| meta.modified()).ok())
            .is_some_and(|(bundled, managed)| bundled > managed),
    };

    let should_copy = !managed.is_file() || (!same_app_version && bundled_newer);
    if should_copy {
        fs::create_dir_all(paths.managed_bin()).ok()?;
        fs::copy(&bundled, &managed).ok()?;
        fs::set_permissions(&managed, fs::Permissions::from_mode(0o755)).ok()?;
        if sign_macos(&managed).is_err() {
            // An unsigned copy cannot run on Apple Silicon. Remove it and fall
            // back to the bundled binary.
            let _ = fs::remove_file(&managed);
            return None;
        }
    }
    let _ = fs::write(&marker, current_version);
    if (should_copy || !paths.managed_bin().join("yt-dlp.version").is_file())
        && let Some(version) = bundled_version
    {
        let _ = fs::write(paths.managed_bin().join("yt-dlp.version"), version);
    }

    managed.is_file().then_some(managed)
}

#[cfg(target_os = "macos")]
fn read_cached_version(path: std::path::PathBuf) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// Reads the yt-dlp version without launching the 36 MB PyInstaller bundle
/// when a cached version is available.
#[cfg(target_os = "macos")]
pub fn ytdlp_version(paths: &crate::paths::AppPaths) -> Option<String> {
    read_cached_version(paths.managed_bin().join("yt-dlp.version"))
        .or_else(|| read_cached_version(paths.bin().join("yt-dlp.version")))
        .or_else(|| {
            read_version(&paths.ytdlp(), "--version")
                .as_deref()
                .and_then(parse_ytdlp_version)
        })
}

#[cfg(target_os = "macos")]
fn version_newer(candidate: &str, current: &str) -> bool {
    fn key(value: &str) -> Vec<u32> {
        value
            .split(['.', '-'])
            .filter_map(|part| part.parse().ok())
            .collect()
    }
    key(candidate) > key(current)
}

#[cfg(target_os = "macos")]
pub fn sign_macos(path: &Path) -> std::io::Result<()> {
    let status = Command::new("codesign")
        .args(["--force", "--sign", "-", "--timestamp=none"])
        .arg(path)
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other(format!(
            "codesign failed for {}",
            path.display()
        )))
    }
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

    #[cfg(target_os = "macos")]
    #[test]
    fn manages_the_ytdlp_copy() {
        use crate::paths::AppPaths;

        let bundle_bin = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target/release/bundle/macos/Klipparkiv.app/Contents/Resources/bin");
        if !bundle_bin.join("yt-dlp").is_file() {
            return;
        }
        let temp = tempfile::tempdir().unwrap();
        let paths = AppPaths::installed(
            temp.path().join("root"),
            temp.path().join("support"),
            &bundle_bin,
        );
        let managed = ensure_managed_ytdlp(&paths).expect("managed yt-dlp");
        assert!(managed.is_file());
        assert!(managed.starts_with(temp.path()));
        assert!(paths.managed_bin().join(".bundle-version").is_file());
    }
}
