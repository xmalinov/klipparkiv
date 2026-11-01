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
    #[cfg(unix)]
    // yt-dlp spawns ffmpeg. Put both into their own process group so a cancel
    // kills the whole tree instead of only the parent.
    command.process_group(0);
    command
}

/// Kills the whole process tree. yt-dlp spawns ffmpeg, so killing the parent
/// alone leaves orphans.
pub async fn kill_tree(pid: u32) -> bool {
    #[cfg(windows)]
    {
        let status = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .await;
        matches!(status, Ok(s) if s.success())
    }

    #[cfg(unix)]
    {
        // The child was spawned with process_group(0), so its pgid equals its
        // pid. A negative pid signals the whole group.
        let group = unsafe { libc::kill(-(pid as i32), libc::SIGKILL) };
        if group == 0 {
            return true;
        }
        let single = unsafe { libc::kill(pid as i32, libc::SIGKILL) };
        single == 0
    }

    #[cfg(not(any(windows, unix)))]
    {
        let _ = pid;
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[tokio::test]
    async fn prepares_piped_command_without_window() {
        let command = prepare(
            Path::new(if cfg!(windows) {
                r"C:\bin\yt-dlp.exe"
            } else {
                "/tmp/yt-dlp"
            }),
            &[OsString::from("--version")],
        );
        let debug = format!("{command:?}");

        assert!(debug.contains("yt-dlp"));
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

    #[cfg(unix)]
    #[tokio::test]
    async fn kills_a_process_group() {
        use std::time::{Duration, Instant};

        let mut command = prepare(
            Path::new("/bin/sh"),
            &[OsString::from("-c"), OsString::from("sleep 30 & wait")],
        );
        let mut child = command.spawn().unwrap();
        let pid = child.id().unwrap();

        tokio::time::sleep(Duration::from_millis(150)).await;
        assert!(kill_tree(pid).await);
        // Reap the shell; otherwise it stays a zombie and kill(pid, 0) keeps
        // reporting it as alive.
        let _ = tokio::time::timeout(Duration::from_secs(3), child.wait()).await;

        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let alive = unsafe { libc::kill(pid as i32, 0) } == 0;
            if !alive {
                break;
            }
            assert!(Instant::now() < deadline, "process group survived SIGKILL");
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }
}
