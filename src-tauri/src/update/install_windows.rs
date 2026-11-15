use std::path::Path;

use tauri::{AppHandle, Manager};

use crate::i18n;
use crate::paths::AppPaths;
use crate::queue::TaskStore;
use crate::update::manifest::current_version;
use crate::update::state::{UpdateStatus, error_status, installing_status};
#[cfg(windows)]
pub(crate) fn start_install_windows(
    app: &AppHandle,
    paths: &AppPaths,
    staged: &Path,
) -> UpdateStatus {
    let target = match std::env::current_exe() {
        Ok(target) => target,
        Err(error) => {
            return error_status(
                app,
                i18n::tf(
                    || format!("не удалось определить путь к приложению: {error}"),
                    || format!("could not determine the app path: {error}"),
                ),
            );
        }
    };
    let root = paths
        .root()
        .canonicalize()
        .unwrap_or_else(|_| paths.root().to_path_buf());
    let target_canonical = target.canonicalize().unwrap_or_else(|_| target.clone());
    let target_parent_matches = target_canonical.parent().is_some_and(|parent| {
        parent
            .to_string_lossy()
            .eq_ignore_ascii_case(&root.to_string_lossy())
    });
    if !target_parent_matches {
        return error_status(
            app,
            i18n::t(
                "обновление доступно только в портативной сборке",
                "updates are only available in the portable build",
            ),
        );
    }

    let helper = paths
        .root()
        .join(crate::paths::exe_name("klipparkiv-updater"));
    if !helper.is_file() {
        return error_status(
            app,
            i18n::t(
                "не найден klipparkiv-updater.exe рядом с приложением",
                "klipparkiv-updater.exe was not found next to the app",
            ),
        );
    }

    let backup_dir = paths.update_backups();
    if let Err(error) = std::fs::create_dir_all(&backup_dir) {
        return error_status(
            app,
            i18n::tf(
                || format!("не удалось создать папку резервной копии: {error}"),
                || format!("could not create the backup folder: {error}"),
            ),
        );
    }
    let backup = backup_dir.join(format!("klipparkiv-{}.exe", current_version()));
    let log = paths.updater_log();

    let status = installing_status(app);
    app.state::<std::sync::Arc<TaskStore>>().set_paused(true);

    let mut command = std::process::Command::new(&helper);
    command
        .arg("--wait-pid")
        .arg(std::process::id().to_string())
        .arg("--source")
        .arg(staged)
        .arg("--target")
        .arg(&target)
        .arg("--backup")
        .arg(&backup)
        .arg("--log")
        .arg(&log)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }

    match command.spawn() {
        Ok(_) => {
            app.exit(0);
            status
        }
        Err(error) => {
            app.state::<std::sync::Arc<TaskStore>>().set_paused(false);
            error_status(
                app,
                i18n::tf(
                    || format!("не удалось запустить установщик обновления: {error}"),
                    || format!("could not start the update installer: {error}"),
                ),
            )
        }
    }
}
