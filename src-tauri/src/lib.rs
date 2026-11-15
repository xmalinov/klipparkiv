mod analysis;
mod args;
mod binaries;
mod error;
mod i18n;
mod ipc;
mod library;
mod media;
mod metadata;
mod paths;
pub mod practice;
mod process;
mod progress;
mod queue;
mod settings;
mod time_util;
mod update;

use paths::AppPaths;
use queue::TaskStore;
use serde::Serialize;
use settings::SettingsStore;
use std::sync::Arc;
use tauri::Manager;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use update::UpdateStore;

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
    support_root: String,
    install_kind: String,
    writable: bool,
    version: String,
    update_configured: bool,
    bin_dir: String,
    ytdlp: Option<String>,
    ffmpeg: Option<String>,
    ffprobe: Option<String>,
    binaries: Vec<binaries::BinaryStatus>,
}

fn app_info_for(paths: &AppPaths) -> AppInfo {
    let writable = paths.ensure_layout().is_ok();
    #[cfg(target_os = "macos")]
    let _ = binaries::ensure_managed_ytdlp(paths);

    let ytdlp = {
        #[cfg(target_os = "macos")]
        {
            match binaries::ytdlp_version(paths) {
                Ok(version) => binaries::status("yt-dlp", &paths.ytdlp(), Some(version), None),
                Err(error) => binaries::status("yt-dlp", &paths.ytdlp(), None, Some(error)),
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            binaries::inspect(
                "yt-dlp",
                &paths.ytdlp(),
                "--version",
                binaries::parse_ytdlp_version,
            )
        }
    };
    let ffmpeg = binaries::inspect(
        "ffmpeg",
        &paths.ffmpeg(),
        "-version",
        binaries::parse_ffmpeg_version,
    );
    let ffprobe = binaries::inspect(
        "ffprobe",
        &paths.ffprobe(),
        "-version",
        binaries::parse_ffmpeg_version,
    );

    AppInfo {
        root: paths.root().display().to_string(),
        support_root: paths.support().display().to_string(),
        install_kind: match paths.kind() {
            paths::InstallKind::Portable => "portable",
            paths::InstallKind::Installed => "installed",
        }
        .to_string(),
        writable,
        version: env!("CARGO_PKG_VERSION").to_string(),
        update_configured: update::is_configured(),
        bin_dir: paths.bin().display().to_string(),
        ytdlp: ytdlp.version.clone(),
        ffmpeg: ffmpeg.version.clone(),
        ffprobe: ffprobe.version.clone(),
        binaries: vec![ytdlp, ffmpeg, ffprobe],
    }
}

#[tauri::command]
fn app_info(paths: tauri::State<'_, AppPaths>) -> AppInfo {
    app_info_for(&paths)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let paths = AppPaths::current().expect("cannot resolve the app root");
    std::fs::create_dir_all(paths.data()).ok();
    std::fs::create_dir_all(paths.support()).ok();
    let library = library::Library::open(&paths.data().join("library.db"))
        .expect("cannot open library database");
    let settings_store = SettingsStore::load(paths.data().join("settings.json"));
    let tasks = Arc::new(TaskStore::default());
    tasks.set_concurrency(settings_store.get().concurrency);
    let practice = practice::PracticeState::new(paths.ffmpeg());

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
        .manage(library::ImportState::default())
        .manage(settings_store)
        .manage(tasks)
        .manage(practice)
        .manage(analysis::AnalysisState::default())
        .manage(UpdateStore::default())
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_menu(Menu::default(app.handle())?)?;

            let paths = app.state::<AppPaths>().inner().clone();
            let cleanup_paths = paths.clone();
            let sweep_paths = paths.clone();
            tauri::async_runtime::spawn_blocking(move || {
                update::cleanup_stale_state(&cleanup_paths);
                library::commands::sweep_part_files(&sweep_paths);
            });
            #[cfg(target_os = "macos")]
            tauri::async_runtime::spawn_blocking(move || {
                let _ = binaries::ensure_managed_ytdlp(&paths);
            });

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

            #[cfg(target_os = "macos")]
            let tray_icon =
                tauri::image::Image::from_bytes(include_bytes!("../icons/trayTemplate.png")).ok();
            #[cfg(not(target_os = "macos"))]
            let tray_icon = app.default_window_icon().cloned();

            if let Some(icon) = tray_icon {
                let builder = TrayIconBuilder::with_id("main")
                    .icon(icon)
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
                            ipc::tasks::toggle_pause(app);
                        }
                        "quit" => app.exit(0),
                        _ => {}
                    });
                #[cfg(target_os = "macos")]
                let builder = builder.icon_as_template(true);
                builder.build(app)?;
            }
            update::schedule_startup_check(app.handle().clone());
            practice::spawn_events(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_info,
            ipc::tasks::enqueue_task,
            ipc::tasks::enqueue_batch,
            ipc::tasks::enqueue_links,
            ipc::tasks::probe_metadata,
            ipc::tasks::cancel_task,
            ipc::tasks::remove_task,
            ipc::tasks::pause_queue,
            ipc::tasks::resume_queue,
            ipc::tasks::queue_state,
            ipc::tasks::list_tasks,
            ipc::settings::get_settings,
            ipc::settings::save_settings,
            ipc::settings::update_ytdlp,
            update::commands::get_update_status,
            update::commands::check_app_update,
            update::commands::download_app_update,
            update::commands::install_app_update,
            update::commands::open_manual_update,
            update::commands::cancel_pending_update,
            library::commands::library_query,
            library::commands::library_scan,
            library::commands::library_open,
            library::commands::library_remove,
            library::commands::library_import,
            library::commands::library_import_cancel,
            library::commands::read_text_file,
            library::commands::library_thumb,
            practice::practice_open,
            practice::practice_play,
            practice::practice_pause,
            practice::practice_seek,
            practice::practice_set_speed,
            practice::practice_set_pitch,
            practice::practice_set_volume,
            practice::practice_set_loop,
            practice::practice_set_grid,
            practice::practice_set_metronome,
            practice::practice_position,
            practice::practice_state_get,
            practice::practice_state_save,
            practice::practice_recent,
            practice::practice_recent_hide,
            practice::practice_waveform,
            practice::practice_close,
            analysis::practice_analysis_get,
            analysis::practice_analysis_start,
            analysis::practice_analysis_cancel
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "macos")]
    #[test]
    fn tray_template_is_a_valid_png() {
        tauri::image::Image::from_bytes(include_bytes!("../icons/trayTemplate.png")).unwrap();
        tauri::image::Image::from_bytes(include_bytes!("../icons/trayTemplate@2x.png")).unwrap();
    }

    #[test]
    fn app_info_reports_dev_root_and_bundled_binaries() {
        let paths = AppPaths::current().unwrap();
        let info = app_info_for(&paths);

        assert!(info.writable, "project root must be writable");
        assert!(
            std::path::Path::new(&info.root).join("src-tauri").is_dir(),
            "unexpected root {}",
            info.root
        );
        // CI does not ship the ~230 MB binaries; only check versions when they are present.
        assert_eq!(info.bin_dir, paths.bin().display().to_string());
        assert_eq!(info.binaries.len(), 3);
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
