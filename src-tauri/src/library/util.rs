use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::i18n;
pub(crate) use crate::time_util::{mtime_secs, now_secs};

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

/// Runs a child process with a hard timeout. Reader threads drain both pipes so
/// a verbose ffmpeg cannot fill the OS buffer and deadlock.
pub(crate) fn run_command(
    command: &mut Command,
    timeout: Duration,
) -> Option<std::process::Output> {
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
