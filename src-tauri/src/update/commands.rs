use tauri::{AppHandle, Manager, State};

use crate::i18n;
use crate::paths::AppPaths;
use crate::queue::TaskStore;
use crate::update::check::check;
use crate::update::download::download;
#[cfg(target_os = "macos")]
use crate::update::download::download_asset_file;
use crate::update::install::start_install;
#[cfg(target_os = "macos")]
use crate::update::state::manual_status;
use crate::update::state::{
    STATE_DEFERRED, STATE_INSTALLING, STATE_READY, UpdateStatus, UpdateStore, error_status,
    set_status,
};
use crate::update::verify::{staged_path, verify_file};

#[tauri::command]
pub async fn open_manual_update(app: AppHandle) -> UpdateStatus {
    #[cfg(not(target_os = "macos"))]
    {
        error_status(
            &app,
            i18n::t(
                "ручная установка доступна только на macOS",
                "manual installation is only available on macOS",
            ),
        )
    }

    #[cfg(target_os = "macos")]
    {
        let store = app.state::<UpdateStore>();
        let status = store.status();
        if status.state == STATE_INSTALLING {
            return status;
        }
        let Some(asset) = store.asset() else {
            return error_status(
                &app,
                i18n::t(
                    "сначала проверьте наличие обновления",
                    "check for an update first",
                ),
            );
        };
        let Some(manual) = asset.manual.clone() else {
            return error_status(
                &app,
                i18n::t(
                    "в манифесте нет DMG для ручной установки",
                    "the manifest has no DMG for manual installation",
                ),
            );
        };
        let paths = app.state::<AppPaths>().inner().clone();
        let dmg = paths.update().join(&asset.version).join("manual.dmg");
        if dmg.is_file() {
            let verify_dmg = dmg.clone();
            let verify_signature = manual.signature.clone();
            let verify_version = asset.version.clone();
            let valid = tokio::task::spawn_blocking(move || {
                verify_file(&verify_dmg, &verify_signature, &verify_version)
            })
            .await
            .ok()
            .and_then(Result::ok)
            .is_some();
            if !valid {
                let _ = tokio::fs::remove_file(&dmg).await;
            }
        }
        if !dmg.is_file()
            && let Err(error) = download_asset_file(
                &app,
                &manual.url,
                manual.size,
                &manual.sha256,
                &manual.signature,
                &asset.version,
                &dmg,
            )
            .await
        {
            return error_status(&app, error);
        }

        match std::process::Command::new("open").arg(&dmg).spawn() {
            Ok(_) => manual_status(&app, &asset),
            Err(error) => error_status(
                &app,
                i18n::tf(
                    || format!("не удалось открыть DMG: {error}"),
                    || format!("could not open the DMG: {error}"),
                ),
            ),
        }
    }
}

pub fn install(app: &AppHandle) -> UpdateStatus {
    let store = app.state::<UpdateStore>();
    let status = store.status();
    if matches!(status.state.as_str(), STATE_INSTALLING | STATE_DEFERRED) {
        return status;
    }
    let Some(asset) = store.asset() else {
        return error_status(
            app,
            i18n::t("сначала скачайте обновление", "download the update first"),
        );
    };
    let staged = {
        let paths = app.state::<AppPaths>();
        staged_path(&paths, &asset.version)
    };
    let verify_signature = asset.signature.clone();
    let verify_version = asset.version.clone();
    if let Err(error) = verify_file(&staged, &verify_signature, &verify_version) {
        return error_status(app, error);
    }

    let queue = app.state::<std::sync::Arc<TaskStore>>().queue_state();
    if queue.running + queue.queued > 0 {
        return set_status(
            app,
            store.update_status(|status| {
                status.state = STATE_DEFERRED.to_string();
                status.version = Some(asset.version.clone());
                status.percent = 100.0;
                status.error = None;
            }),
        );
    }

    start_install(app, &asset)
}

pub fn maybe_install_when_idle(app: &AppHandle) -> bool {
    let Some(store) = app.try_state::<UpdateStore>() else {
        return false;
    };
    if store.status().state != STATE_DEFERRED {
        return false;
    }
    let queue = app.state::<std::sync::Arc<TaskStore>>().queue_state();
    if queue.running + queue.queued > 0 {
        return false;
    }
    let Some(asset) = store.asset() else {
        return false;
    };
    let status = start_install(app, &asset);
    status.state == STATE_INSTALLING
}

#[tauri::command]
pub fn get_update_status(store: State<'_, UpdateStore>) -> UpdateStatus {
    store.status()
}

#[tauri::command]
pub async fn check_app_update(app: AppHandle, silent: Option<bool>) -> UpdateStatus {
    check(&app, silent.unwrap_or(false)).await
}

#[tauri::command]
pub async fn download_app_update(app: AppHandle) -> UpdateStatus {
    download(&app).await
}

#[tauri::command]
pub fn install_app_update(app: AppHandle) -> UpdateStatus {
    install(&app)
}

#[tauri::command]
pub fn cancel_pending_update(app: AppHandle) -> UpdateStatus {
    let store = app.state::<UpdateStore>();
    let status = store.status();
    if status.state != STATE_DEFERRED {
        return status;
    }
    set_status(
        &app,
        store.update_status(|status| {
            status.state = STATE_READY.to_string();
        }),
    )
}
