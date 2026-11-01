mod args;
mod binaries;
mod commands;
mod i18n;
mod library;
mod metadata;
mod paths;
mod process;
mod progress;
mod settings;
mod updater;

use commands::TaskStore;
use paths::PortablePaths;
use serde::Serialize;
use settings::SettingsStore;
use std::sync::Arc;
use tauri::Manager;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use updater::UpdateStore;

/// Handles to the tray menu items so they can follow the UI language.
pub struct TrayMenuItems {
    pub open: MenuItem<tauri::Wry>,
    pub pause: MenuItem<tauri::Wry>,
    pub quit: MenuItem<tauri::Wry>,
}

pub fn refresh_tray_language(app: &tauri::AppHandle) {
    if let Some(items) = app.try_state::<TrayMenuItems>() {
        let _ = items.open.set_text(i18n::t("Открыть", "Open"));
        let _ = items
            .pause
            .set_text(i18n::t("Пауза очереди", "Pause queue"));
        let _ = items.quit.set_text(i18n::t("Выход", "Quit"));
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AppInfo {
    root: String,
    writable: bool,
    version: String,
    update_configured: bool,
    ytdlp: Option<String>,
    ffmpeg: Option<String>,
    ffprobe: Option<String>,
}

#[tauri::command]
fn app_info(paths: tauri::State<'_, PortablePaths>) -> AppInfo {
    let writable = paths.ensure_layout().is_ok();
    let ffmpeg = binaries::read_version(&paths.ffmpeg(), "-version")
        .as_deref()
        .and_then(binaries::parse_ffmpeg_version);
    let ffprobe = binaries::read_version(&paths.ffprobe(), "-version")
        .as_deref()
        .and_then(binaries::parse_ffmpeg_version);

    AppInfo {
        root: paths.root().display().to_string(),
        writable,
        version: env!("CARGO_PKG_VERSION").to_string(),
        update_configured: updater::is_configured(),
        ytdlp: binaries::read_version(&paths.ytdlp(), "--version")
            .as_deref()
            .and_then(binaries::parse_ytdlp_version),
        ffmpeg,
        ffprobe,
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let paths = PortablePaths::current().expect("cannot resolve portable root");
    std::fs::create_dir_all(paths.data()).ok();
    let library = library::Library::open(&paths.data().join("library.db"))
        .expect("cannot open library database");
    let settings_store = SettingsStore::load(paths.data().join("settings.json"));
    let tasks = Arc::new(TaskStore::default());
    tasks.set_concurrency(settings_store.get().concurrency);

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .manage(paths)
        .manage(library)
        .manage(settings_store)
        .manage(tasks)
        .manage(UpdateStore::default())
        .setup(|app| {
            let open =
                MenuItem::with_id(app, "open", i18n::t("Открыть", "Open"), true, None::<&str>)?;
            let pause = MenuItem::with_id(
                app,
                "pause",
                i18n::t("Пауза очереди", "Pause queue"),
                true,
                None::<&str>,
            )?;
            let quit =
                MenuItem::with_id(app, "quit", i18n::t("Выход", "Quit"), true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &pause, &quit])?;
            let _ = app.manage(TrayMenuItems {
                open: open.clone(),
                pause: pause.clone(),
                quit: quit.clone(),
            });

            if let Some(icon) = app.default_window_icon() {
                TrayIconBuilder::with_id("main")
                    .icon(icon.clone())
                    .tooltip("Klipparkiv")
                    .menu(&menu)
                    .show_menu_on_left_click(true)
                    .on_menu_event(|app, event| match event.id.as_ref() {
                        "open" => {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                        "pause" => {
                            commands::toggle_pause(app);
                        }
                        "quit" => app.exit(0),
                        _ => {}
                    })
                    .build(app)?;
            }
            updater::schedule_startup_check(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_info,
            commands::enqueue_task,
            commands::enqueue_batch,
            commands::probe_metadata,
            commands::cancel_task,
            commands::pause_queue,
            commands::resume_queue,
            commands::queue_state,
            commands::list_tasks,
            commands::get_settings,
            commands::save_settings,
            commands::update_ytdlp,
            updater::get_update_status,
            updater::check_app_update,
            updater::download_app_update,
            updater::install_app_update,
            updater::cancel_pending_update,
            library::library_query,
            library::library_scan,
            library::library_remove,
            library::library_thumb
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_info_reports_dev_root_and_bundled_binaries() {
        let paths = PortablePaths::current().unwrap();
        let info = AppInfo {
            root: paths.root().display().to_string(),
            writable: paths.ensure_layout().is_ok(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            update_configured: updater::is_configured(),
            ytdlp: binaries::read_version(&paths.ytdlp(), "--version")
                .as_deref()
                .and_then(binaries::parse_ytdlp_version),
            ffmpeg: binaries::read_version(&paths.ffmpeg(), "-version")
                .as_deref()
                .and_then(binaries::parse_ffmpeg_version),
            ffprobe: binaries::read_version(&paths.ffprobe(), "-version")
                .as_deref()
                .and_then(binaries::parse_ffmpeg_version),
        };

        assert!(info.writable, "project root must be writable");
        assert!(
            std::path::Path::new(&info.root).join("src-tauri").is_dir(),
            "unexpected root {}",
            info.root
        );
        // CI does not ship the ~230 MB binaries; only check versions when they are present.
        if paths.ytdlp().is_file() {
            let ytdlp = info.ytdlp.expect("yt-dlp version not detected");
            assert!(
                ytdlp.chars().all(|c| c.is_ascii_digit() || c == '.'),
                "unexpected yt-dlp version {ytdlp}"
            );
        }
        if paths.ffmpeg().is_file() {
            assert!(info.ffmpeg.is_some(), "ffmpeg version not detected");
        }
        if paths.ffprobe().is_file() {
            assert!(info.ffprobe.is_some(), "ffprobe version not detected");
        }
    }
}
