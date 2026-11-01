#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! Tiny helper that replaces the application after the main process exits.
//!
//! On Windows it replaces `klipparkiv.exe`. On macOS it replaces the whole
//! `Klipparkiv.app` bundle, because the signed bundle cannot be patched in
//! place. The helper is copied outside the bundle before it runs.

use std::ffi::OsString;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[cfg(not(any(windows, target_os = "macos")))]
fn main() {
    eprintln!("klipparkiv-updater is only supported on Windows and macOS");
    std::process::exit(1);
}

#[cfg(any(windows, target_os = "macos"))]
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
    support: Option<PathBuf>,
    relaunch: bool,
}

fn parse_args(args: impl Iterator<Item = OsString>) -> Result<Args, String> {
    let mut wait_pid = None;
    let mut source = None;
    let mut target = None;
    let mut backup = None;
    let mut log = None;
    let mut support = None;
    let mut relaunch = false;
    let mut args = args;

    while let Some(flag) = args.next() {
        if flag == "--relaunch" {
            relaunch = true;
            continue;
        }
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
            "--support" => support = Some(PathBuf::from(value)),
            other => return Err(format!("unknown argument: {other}")),
        }
    }

    Ok(Args {
        wait_pid: wait_pid.ok_or_else(|| "missing --wait-pid".to_string())?,
        source: source.ok_or_else(|| "missing --source".to_string())?,
        target: target.ok_or_else(|| "missing --target".to_string())?,
        backup: backup.ok_or_else(|| "missing --backup".to_string())?,
        log: log.ok_or_else(|| "missing --log".to_string())?,
        support,
        relaunch,
    })
}

#[cfg(windows)]
fn run(args: Args) -> Result<(), String> {
    let _ = (&args.support, args.relaunch);
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

#[cfg(target_os = "macos")]
fn run(args: Args) -> Result<(), String> {
    let support = args
        .support
        .as_ref()
        .ok_or_else(|| "missing --support".to_string())?
        .canonicalize()
        .map_err(|error| format!("cannot resolve the support folder: {error}"))?;
    let target = args
        .target
        .canonicalize()
        .map_err(|error| format!("cannot resolve the app bundle: {error}"))?;
    let source = args
        .source
        .canonicalize()
        .map_err(|error| format!("cannot resolve the new app bundle: {error}"))?;

    if !is_app_bundle(&target) {
        return Err(format!("target is not an app bundle: {}", target.display()));
    }
    if !is_app_bundle(&source) {
        return Err(format!("source is not an app bundle: {}", source.display()));
    }
    if source == target {
        return Err("source and target are the same bundle".to_string());
    }
    if !is_inside(&support, &source) {
        return Err("source is outside the support folder".to_string());
    }
    if let Some(parent) = args.backup.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create the backup folder: {error}"))?;
    }
    if let Some(parent) = args.log.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create the log folder: {error}"))?;
    }
    if !is_inside(&support, &args.backup) {
        return Err("backup is outside the support folder".to_string());
    }
    if !is_inside(&support, &args.log) {
        return Err("log is outside the support folder".to_string());
    }

    log(&args.log, "waiting for the main process to exit");
    wait_for_pid(args.wait_pid, Duration::from_secs(60))?;
    log(&args.log, "main process exited; replacing the app bundle");

    let backup = match replace_bundle(&source, &target, &args.backup) {
        Ok(backup) => backup,
        Err(error) => {
            log(&args.log, &format!("replace failed: {error}"));
            return Err(format!("cannot replace the app bundle: {error}"));
        }
    };
    // The archive was verified with minisign before extraction. Drop any
    // quarantine xattr that could have been inherited during extraction.
    let _ = std::process::Command::new("xattr")
        .args(["-dr", "com.apple.quarantine"])
        .arg(&target)
        .status();
    log(&args.log, "app bundle replaced");

    if args.relaunch && !relaunch(&target) {
        log(
            &args.log,
            "new version did not start; restoring the previous bundle",
        );
        restore_bundle(&target, &backup)?;
        let _ = relaunch(&target);
        return Err("the new version failed to start; rolled back".to_string());
    }

    if let Some(extract_dir) = source.parent() {
        let _ = fs::remove_dir_all(extract_dir);
    }
    log(&args.log, "new version is running");
    Ok(())
}

#[cfg(any(windows, target_os = "macos"))]
fn log(path: &Path, message: &str) {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();
    if let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "[{timestamp}] {message}");
    }
}

#[cfg(any(windows, target_os = "macos"))]
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

#[cfg(target_os = "macos")]
fn is_app_bundle(path: &Path) -> bool {
    path.is_dir()
        && path.extension().and_then(|ext| ext.to_str()) == Some("app")
        && path.join("Contents").join("Info.plist").is_file()
}

#[cfg(target_os = "macos")]
fn wait_for_pid(pid: u32, timeout: Duration) -> Result<(), String> {
    use std::time::Instant;

    let start = Instant::now();
    loop {
        let result = unsafe { libc::kill(pid as i32, 0) };
        if result != 0 {
            let error = std::io::Error::last_os_error();
            match error.raw_os_error() {
                Some(libc::EPERM) => {}
                _ => return Ok(()),
            }
        }
        if start.elapsed() >= timeout {
            return Err("timed out waiting for the app to exit".to_string());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Replaces the installed bundle with `source` and returns the path of the
/// previous bundle kept for rollback.
///
/// APFS can exchange the two directories in one step (`RENAME_SWAP`), so the
/// install path never disappears and a crash cannot leave it missing. Where the
/// file system does not support the swap (HFS+, some network volumes), it falls
/// back to two renames, restoring the original if the second one fails.
#[cfg(target_os = "macos")]
fn replace_bundle(source: &Path, target: &Path, backup: &Path) -> Result<PathBuf, String> {
    let _ = fs::remove_dir_all(backup);

    if swap_paths(source, target).is_ok() {
        // `source` now holds the previous bundle. Keep it as the rollback copy.
        // The update itself has succeeded, so failing to move the backup must
        // not abort it; `source` stays a valid rollback path until the extract
        // folder is removed after a successful relaunch.
        if let Some(parent) = backup.parent() {
            let _ = fs::create_dir_all(parent);
        }
        return match move_dir(source, backup) {
            Ok(()) => Ok(backup.to_path_buf()),
            Err(_) => Ok(source.to_path_buf()),
        };
    }

    // Two renames with a restore, for file systems without RENAME_SWAP.
    if let Err(error) = move_dir(target, backup) {
        return Err(format!("cannot back up the app bundle: {error}"));
    }
    if let Err(error) = move_dir(source, target) {
        let _ = move_dir(backup, target);
        return Err(format!("cannot move the new app bundle: {error}"));
    }
    Ok(backup.to_path_buf())
}

#[cfg(target_os = "macos")]
fn restore_bundle(target: &Path, backup: &Path) -> Result<(), String> {
    // Swap back atomically where possible; `backup` then holds the failed new
    // bundle and is removed.
    if swap_paths(backup, target).is_ok() {
        let _ = fs::remove_dir_all(backup);
        return Ok(());
    }
    let _ = fs::remove_dir_all(target);
    move_dir(backup, target).map_err(|error| format!("cannot restore the app bundle: {error}"))
}

/// Exchanges two paths in a single step using `renamex_np(RENAME_SWAP)`.
///
/// Available on APFS; other file systems return an error and callers fall back
/// to two renames.
#[cfg(target_os = "macos")]
fn swap_paths(first: &Path, second: &Path) -> std::io::Result<()> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    fn c_path(path: &Path) -> std::io::Result<CString> {
        CString::new(path.as_os_str().as_bytes())
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "path contains NUL"))
    }

    let first = c_path(first)?;
    let second = c_path(second)?;
    // SAFETY: both pointers are valid NUL-terminated strings that outlive the
    // call, and renamex_np only reads them.
    let result = unsafe { libc::renamex_np(first.as_ptr(), second.as_ptr(), libc::RENAME_SWAP) };
    if result == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(target_os = "macos")]
fn move_dir(source: &Path, target: &Path) -> std::io::Result<()> {
    match fs::rename(source, target) {
        Ok(()) => Ok(()),
        Err(_) => {
            // Different volumes: ditto preserves metadata, symlinks and
            // permissions, unlike a plain recursive copy.
            let status = std::process::Command::new("ditto")
                .arg(source)
                .arg(target)
                .status()?;
            if !status.success() {
                return Err(std::io::Error::other(format!("ditto failed with {status}")));
            }
            fs::remove_dir_all(source)
        }
    }
}

#[cfg(target_os = "macos")]
fn relaunch(target: &Path) -> bool {
    let opened = std::process::Command::new("open")
        .arg("-n")
        .arg(target)
        .status();
    if !opened.is_ok_and(|status| status.success()) {
        return false;
    }
    std::thread::sleep(Duration::from_secs(3));
    let marker = format!("{}/Contents/MacOS/klipparkiv", target.display());
    std::process::Command::new("pgrep")
        .args(["-f", &marker])
        .status()
        .is_ok_and(|status| status.success())
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

#[cfg(all(test, any(windows, target_os = "macos")))]
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
            "/tmp/app/next",
            "--target",
            "/tmp/app/current",
            "--backup",
            "/tmp/app/backup",
            "--log",
            "/tmp/app/updater.log",
        ]))
        .unwrap();
        assert_eq!(parsed.wait_pid, 42);
        assert_eq!(parsed.source, PathBuf::from("/tmp/app/next"));
        assert!(!parsed.relaunch);
    }

    #[test]
    fn parses_support_and_relaunch() {
        let parsed = parse_args(args(&[
            "--wait-pid",
            "42",
            "--source",
            "/tmp/app/next",
            "--target",
            "/tmp/app/current",
            "--backup",
            "/tmp/app/backup",
            "--log",
            "/tmp/app/updater.log",
            "--support",
            "/tmp/support",
            "--relaunch",
        ]))
        .unwrap();
        assert_eq!(parsed.support, Some(PathBuf::from("/tmp/support")));
        assert!(parsed.relaunch);
    }

    #[test]
    fn rejects_missing_arguments() {
        assert!(parse_args(args(&["--wait-pid", "42"])).is_err());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn replaces_and_restores_an_app_bundle() {
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("Klipparkiv.app");
        let source = temp.path().join("support/Klipparkiv.app");
        let backup = temp.path().join("support/backup/Klipparkiv-1.app");
        fs::create_dir_all(&target).unwrap();
        fs::create_dir_all(&source).unwrap();
        fs::write(target.join("version"), b"old").unwrap();
        fs::write(source.join("version"), b"new").unwrap();

        let rollback = replace_bundle(&source, &target, &backup).unwrap();
        assert_eq!(rollback, backup);
        assert_eq!(fs::read_to_string(target.join("version")).unwrap(), "new");
        assert_eq!(fs::read_to_string(backup.join("version")).unwrap(), "old");
        assert!(!source.exists());

        restore_bundle(&target, &backup).unwrap();
        assert_eq!(fs::read_to_string(target.join("version")).unwrap(), "old");
        assert!(!backup.exists());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn swap_exchanges_two_bundles_atomically() {
        let temp = tempfile::tempdir().unwrap();
        let first = temp.path().join("first.app");
        let second = temp.path().join("second.app");
        fs::create_dir_all(&first).unwrap();
        fs::create_dir_all(&second).unwrap();
        fs::write(first.join("version"), b"first").unwrap();
        fs::write(second.join("version"), b"second").unwrap();

        swap_paths(&first, &second).unwrap();

        assert_eq!(fs::read_to_string(first.join("version")).unwrap(), "second");
        assert_eq!(fs::read_to_string(second.join("version")).unwrap(), "first");
    }
}
