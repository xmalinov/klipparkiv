use std::ffi::OsString;
use std::path::Path;
use tokio::process::Command;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Builds a hidden, piped command. Used for yt-dlp and ffprobe.
pub fn prepare(exe: &Path, args: &[OsString]) -> Command {
    let mut command = Command::new(exe);
    command
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(false);
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);
    command
}

/// Kills the whole process tree. yt-dlp spawns ffmpeg, so killing the parent
/// alone leaves orphans.
pub async fn kill_tree(pid: u32) -> bool {
    let status = Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .await;
    matches!(status, Ok(s) if s.success())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[tokio::test]
    async fn prepares_piped_command_without_window() {
        let command = prepare(
            Path::new(r"C:\bin\yt-dlp.exe"),
            &[OsString::from("--version")],
        );
        let debug = format!("{command:?}");

        assert!(debug.contains("yt-dlp.exe"));
        assert!(debug.contains("--version"));
    }

    #[tokio::test]
    async fn kill_tree_reports_missing_process() {
        assert!(!kill_tree(999_999).await);
    }

    #[test]
    fn args_are_passed_verbatim_without_shell_split() {
        let exe = PathBuf::from(r"C:\Program Files\yt-dlp.exe");
        let out = PathBuf::from(r"C:\a b\out\%(id)s.mp3");
        let command = prepare(&exe, &[out.clone().into_os_string()]);
        let std_command = command.as_std();

        assert_eq!(std_command.get_program(), exe.as_os_str());
        let args: Vec<_> = std_command.get_args().collect();
        assert_eq!(args, vec![out.as_os_str()]);
    }
}
