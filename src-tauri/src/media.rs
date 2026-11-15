//! Media probes shared by the library, practice and analysis.

use std::path::Path;
use std::process::Command;

use crate::process;

/// Duration in seconds from `ffprobe`.
///
/// `None` when the tool is missing, the file is unreadable, or the output is
/// not a number. Callers treat the duration as optional metadata, never as a
/// reason to fail the whole operation.
pub(crate) fn probe_duration(ffprobe: &Path, path: &Path) -> Option<f64> {
    let mut command = Command::new(ffprobe);
    command
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
        ])
        .arg(path);
    process::hide_console_std(&mut command);
    let output = command.output().ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<f64>()
        .ok()
}
