#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! Tiny helper that replaces `klipparkiv.exe` after the main process exits.
//!
//! Kept separate from the Tauri binary on purpose: it must not link WebView2
//! or depend on the frontend, and it stays running while the app is replaced.

use std::ffi::OsString;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[cfg(not(windows))]
fn main() {
    eprintln!("klipparkiv-updater is only supported on Windows");
    std::process::exit(1);
}

#[cfg(windows)]
fn main() {
    match parse_args(std::env::args_os().skip(1)) {
        Ok(args) => {
            if let Err(error) = run(args) {
                eprintln!("klipparkiv-updater: {error}");
                std::process::exit(1);
            }
        }
        Err(error) => {
            eprintln!("klipparkiv-updater: {error}");
            std::process::exit(2);
        }
    }
}

struct Args {
    wait_pid: u32,
    source: PathBuf,
    target: PathBuf,
    backup: PathBuf,
    log: PathBuf,
}

fn parse_args(args: impl Iterator<Item = OsString>) -> Result<Args, String> {
    let mut wait_pid = None;
    let mut source = None;
    let mut target = None;
    let mut backup = None;
    let mut log = None;
    let mut args = args;

    while let Some(flag) = args.next() {
        let value = args
            .next()
            .ok_or_else(|| format!("missing value for {}", flag.to_string_lossy()))?;
        match flag.to_string_lossy().as_ref() {
            "--wait-pid" => {
                wait_pid = Some(
                    value
                        .to_string_lossy()
                        .parse::<u32>()
                        .map_err(|_| "invalid --wait-pid".to_string())?,
                )
            }
            "--source" => source = Some(PathBuf::from(value)),
            "--target" => target = Some(PathBuf::from(value)),
            "--backup" => backup = Some(PathBuf::from(value)),
            "--log" => log = Some(PathBuf::from(value)),
            other => return Err(format!("unknown argument: {other}")),
        }
    }

    Ok(Args {
        wait_pid: wait_pid.ok_or_else(|| "missing --wait-pid".to_string())?,
        source: source.ok_or_else(|| "missing --source".to_string())?,
        target: target.ok_or_else(|| "missing --target".to_string())?,
        backup: backup.ok_or_else(|| "missing --backup".to_string())?,
        log: log.ok_or_else(|| "missing --log".to_string())?,
    })
}

#[cfg(windows)]
fn run(args: Args) -> Result<(), String> {
    let root = args
        .target
        .parent()
        .ok_or_else(|| "target has no parent folder".to_string())?
        .canonicalize()
        .map_err(|error| format!("cannot resolve the app folder: {error}"))?;
    if !is_inside(&root, &args.source) {
        return Err("source is outside the app folder".to_string());
    }
    if !is_inside(&root, &args.target) {
        return Err("target is outside the app folder".to_string());
    }
    if !args.source.is_file() {
        return Err(format!("source not found: {}", args.source.display()));
    }
    if !args.target.is_file() {
        return Err(format!("target not found: {}", args.target.display()));
    }
    if let Some(parent) = args.backup.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create the backup folder: {error}"))?;
    }
    if let Some(parent) = args.log.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create the log folder: {error}"))?;
    }
    if !is_inside(&root, &args.backup) {
        return Err("backup is outside the app folder".to_string());
    }
    if !is_inside(&root, &args.log) {
        return Err("log is outside the app folder".to_string());
    }

    let header = fs::read(&args.source)
        .map_err(|error| format!("cannot read the new executable: {error}"))?;
    if header.len() < 2 || !header.starts_with(b"MZ") {
        return Err("the new file is not a Windows executable".to_string());
    }

    log(&args.log, "waiting for the main process to exit");
    wait_for_pid(args.wait_pid, Duration::from_secs(60))?;
    log(&args.log, "main process exited; replacing the executable");

    if let Err(error) = replace_file(&args.source, &args.target, &args.backup) {
        log(&args.log, &format!("replace failed: {error}"));
        return Err(format!("cannot replace the executable: {error}"));
    }
    log(&args.log, "executable replaced; starting the new version");

    match spawn_app(&args.target, &root) {
        Ok(mut child) => {
            std::thread::sleep(Duration::from_secs(3));
            match child.try_wait() {
                Ok(Some(status)) if !status.success() => {
                    log(
                        &args.log,
                        &format!("new version exited with {status}; rolling back"),
                    );
                    restore(&args.target, &args.backup)?;
                    let _ = spawn_app(&args.target, &root);
                    Err("the new version failed to start; rolled back".to_string())
                }
                Ok(_) => {
                    log(&args.log, "new version is running");
                    Ok(())
                }
                Err(error) => {
                    log(&args.log, &format!("cannot check the new process: {error}"));
                    Ok(())
                }
            }
        }
        Err(error) => {
            log(
                &args.log,
                &format!("cannot start the new version: {error}; rolling back"),
            );
            restore(&args.target, &args.backup)?;
            let _ = spawn_app(&args.target, &root);
            Err(format!("cannot start the new version: {error}"))
        }
    }
}

#[cfg(windows)]
fn is_inside(root: &Path, path: &Path) -> bool {
    let resolved = if path.exists() {
        path.canonicalize().ok()
    } else {
        path.parent()
            .and_then(|parent| parent.canonicalize().ok())
            .map(|parent| parent.join(path.file_name().unwrap_or_default()))
    };
    resolved.is_some_and(|path| path.starts_with(root))
}

#[cfg(windows)]
fn log(path: &Path, message: &str) {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();
    if let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "[{timestamp}] {message}");
    }
}

#[cfg(windows)]
fn restore(target: &Path, backup: &Path) -> Result<(), String> {
    let _ = fs::remove_file(target);
    fs::rename(backup, target).map_err(|error| {
        format!(
            "cannot restore {} from {}: {error}",
            target.display(),
            backup.display()
        )
    })
}

#[cfg(windows)]
fn spawn_app(path: &Path, root: &Path) -> std::io::Result<std::process::Child> {
    use std::os::windows::process::CommandExt;
    let mut command = std::process::Command::new(path);
    command
        .current_dir(root)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .creation_flags(0x0800_0000);
    command.spawn()
}

#[cfg(windows)]
fn wait_for_pid(pid: u32, timeout: Duration) -> Result<(), String> {
    use windows_sys::Win32::Foundation::{CloseHandle, WAIT_TIMEOUT};
    use windows_sys::Win32::System::Threading::{
        OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject,
    };

    let handle = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, pid) };
    if handle.is_null() {
        // The process may have exited between spawn and this call, or the
        // handle may be unavailable. Give the app a moment to shut down.
        std::thread::sleep(Duration::from_secs(2));
        return Ok(());
    }
    let result =
        unsafe { WaitForSingleObject(handle, timeout.as_millis().min(u32::MAX as u128) as u32) };
    unsafe {
        CloseHandle(handle);
    }
    if result == WAIT_TIMEOUT {
        Err("timed out waiting for the app to exit".to_string())
    } else {
        Ok(())
    }
}

#[cfg(windows)]
fn replace_file(source: &Path, target: &Path, backup: &Path) -> std::io::Result<()> {
    let _ = fs::remove_file(backup);
    let mut last_error = None;
    for _ in 0..40 {
        match replace_file_once(source, target, backup) {
            Ok(()) => return Ok(()),
            Err(error) => {
                last_error = Some(error);
                std::thread::sleep(Duration::from_millis(250));
            }
        }
    }

    // FAT/exFAT and some antivirus setups reject ReplaceFileW. Keep a
    // best-effort rename fallback with the same rollback invariant.
    let _ = fs::remove_file(backup);
    fs::rename(target, backup)?;
    match fs::rename(source, target) {
        Ok(()) => Ok(()),
        Err(error) => {
            let _ = fs::rename(backup, target);
            Err(error)
        }
    }
    .map_err(|error| {
        last_error.unwrap_or_else(|| {
            std::io::Error::other(format!(
                "ReplaceFile failed and rename fallback failed: {error}"
            ))
        })
    })
}

#[cfg(windows)]
fn replace_file_once(source: &Path, target: &Path, backup: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::ReplaceFileW;

    fn wide(path: &Path) -> Vec<u16> {
        path.as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }

    let target = wide(target);
    let source = wide(source);
    let backup = wide(backup);
    let result = unsafe {
        ReplaceFileW(
            target.as_ptr(),
            source.as_ptr(),
            backup.as_ptr(),
            0,
            std::ptr::null(),
            std::ptr::null(),
        )
    };
    if result == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> impl Iterator<Item = OsString> {
        values
            .iter()
            .map(OsString::from)
            .collect::<Vec<_>>()
            .into_iter()
    }

    #[test]
    fn parses_required_arguments() {
        let parsed = parse_args(args(&[
            "--wait-pid",
            "42",
            "--source",
            "C:\\app\\data\\update\\next.exe",
            "--target",
            "C:\\app\\klipparkiv.exe",
            "--backup",
            "C:\\app\\data\\update\\backup\\old.exe",
            "--log",
            "C:\\app\\data\\update\\updater.log",
        ]))
        .unwrap();
        assert_eq!(parsed.wait_pid, 42);
        assert_eq!(parsed.source, PathBuf::from(r"C:\app\data\update\next.exe"));
    }

    #[test]
    fn rejects_missing_arguments() {
        assert!(parse_args(args(&["--wait-pid", "42"])).is_err());
    }
}
