use std::ffi::OsString;
use std::sync::Arc;

use tauri::{AppHandle, Manager, State};

use crate::error::AppError;
use crate::i18n;
use crate::paths::AppPaths;
use crate::process;
use crate::queue::TaskStore;
use crate::settings::{Settings, SettingsStore};
#[tauri::command]
pub fn get_settings(store: State<'_, SettingsStore>) -> Settings {
    store.get()
}

#[tauri::command]
pub fn save_settings(app: AppHandle, settings: Settings) -> Result<Settings, AppError> {
    settings.validate().map_err(AppError::invalid_input)?;
    let saved = app
        .state::<SettingsStore>()
        .save(settings)
        .map_err(AppError::access)?;
    app.state::<Arc<TaskStore>>()
        .set_concurrency(saved.concurrency);
    crate::refresh_tray_language(&app);
    Ok(saved)
}

/// Self-update of the bundled yt-dlp (`yt-dlp --update`).
#[tauri::command]
pub async fn update_ytdlp(app: AppHandle) -> Result<String, AppError> {
    let exe = {
        let paths = app.state::<AppPaths>();
        #[cfg(target_os = "macos")]
        {
            crate::binaries::ensure_managed_ytdlp(&paths).ok_or_else(|| {
                AppError::access(i18n::t(
                    "не удалось подготовить обновляемую копию yt-dlp",
                    "could not prepare the managed yt-dlp copy",
                ))
            })?
        }
        #[cfg(not(target_os = "macos"))]
        {
            paths.ytdlp()
        }
    };
    let output = process::prepare(&exe, &[OsString::from("--update")])
        .output()
        .await
        .map_err(|error| AppError::access(process::spawn_error("yt-dlp", &exe, &error)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let reason = stderr
            .lines()
            .rev()
            .find(|line| !line.trim().is_empty())
            .map(str::to_owned)
            .unwrap_or_else(|| i18n::t("yt-dlp не обновился", "yt-dlp was not updated"));
        return Err(AppError::internal(reason.trim().to_string()));
    }

    #[cfg(target_os = "macos")]
    crate::binaries::sign_macos(&exe).map_err(|error| {
        AppError::access(i18n::tf(
            || format!("не удалось подписать обновлённый yt-dlp: {error}"),
            || format!("could not sign the updated yt-dlp: {error}"),
        ))
    })?;

    let line = crate::binaries::read_version(&exe, "--version").map_err(AppError::internal)?;
    let version = crate::binaries::parse_ytdlp_version(&line).ok_or_else(|| {
        AppError::internal(i18n::tf(
            || format!("не удалось разобрать вывод: {line}"),
            || format!("could not parse the output: {line}"),
        ))
    })?;
    #[cfg(target_os = "macos")]
    {
        let paths = app.state::<AppPaths>();
        let _ = std::fs::write(paths.managed_bin().join("yt-dlp.version"), &version);
    }
    Ok(version)
}
