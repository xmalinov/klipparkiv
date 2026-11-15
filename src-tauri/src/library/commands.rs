use std::fs;
use std::path::Path;

use base64::{Engine as _, engine::general_purpose::STANDARD};
use rusqlite::params;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::error::AppError;
use crate::i18n;
use crate::paths::AppPaths;
use crate::settings::SettingsStore;

use crate::library::LibraryPaths;
use crate::library::db::{Library, query_page};
use crate::library::guard::resolve_media;
use crate::library::import::{ImportState, import_media};
use crate::library::model::{ImportReport, LibraryPage, LibraryQuery, ScanStats};
use crate::library::scan::scan_dir;
use crate::library::thumbs::resolve_thumb;
/// Opens a downloaded file with the system default player.
///
/// This goes through Rust instead of the opener plugin's `openPath`, because
/// the plugin command is blocked by the ACL capability while a Rust command
/// can validate the path against the library root first.
#[tauri::command]
pub fn library_open(app: AppHandle, path: String) -> Result<(), AppError> {
    let root = app.state::<AppPaths>().root().to_path_buf();
    let resolved = resolve_media(&root, Path::new(&path)).map_err(AppError::access)?;
    tauri_plugin_opener::open_path(&resolved, None::<&str>).map_err(|error| {
        AppError::access(i18n::tf(
            || format!("не удалось открыть файл: {error}"),
            || format!("could not open the file: {error}"),
        ))
    })
}

#[tauri::command]
pub fn library_query(
    library: State<'_, Library>,
    query: LibraryQuery,
) -> Result<LibraryPage, AppError> {
    query_page(&library, &query).map_err(AppError::from)
}

#[tauri::command]
pub async fn library_scan(app: AppHandle, full: bool) -> Result<ScanStats, AppError> {
    let app_for_scan = app.clone();
    let stats = tauri::async_runtime::spawn_blocking(move || {
        let paths = app_for_scan.state::<AppPaths>();
        let library = app_for_scan.state::<Library>();
        let layout = LibraryPaths::from_app(&paths);
        let ffprobe = paths.ffprobe();
        let depth = app_for_scan.state::<SettingsStore>().get().scan_depth;
        let stats = scan_dir(&library, &layout, &ffprobe, depth, full, |scanned| {
            let _ = app_for_scan.emit("library://scan-progress", scanned);
        })?;
        library.practice_prune().ok();
        Ok::<ScanStats, String>(stats)
    })
    .await
    .map_err(|error| error.to_string())??;

    let _ = app.emit("library://updated", &stats);
    Ok(stats)
}

#[tauri::command]
pub fn library_remove(library: State<'_, Library>, id: String) -> Result<(), AppError> {
    let conn = library.conn.lock().map_err(|error| error.to_string())?;
    conn.execute("DELETE FROM media WHERE path = ?1", params![id])
        .map_err(|error| error.to_string())?;
    conn.execute("DELETE FROM practice WHERE path = ?1", params![id])
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn library_import(app: AppHandle, paths: Vec<String>) -> Result<ImportReport, AppError> {
    let (job_id, cancel) = app.state::<ImportState>().begin().map_err(AppError::busy)?;
    let app_for_import = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let paths_state = app_for_import.state::<AppPaths>();
        let library = app_for_import.state::<Library>();
        let layout = LibraryPaths::from_app(&paths_state);
        import_media(
            &library,
            &layout,
            &paths_state.ffprobe(),
            &paths_state.ffmpeg(),
            paths,
            &cancel,
            job_id,
            |progress| {
                let _ = app_for_import.emit("library://import-progress", progress);
            },
        )
    })
    .await
    .map_err(|error| error.to_string())?;

    app.state::<ImportState>().finish(job_id);
    let report = result?;
    let _ = app.emit("library://updated", &report);
    Ok(report)
}

#[tauri::command]
pub fn library_import_cancel(state: State<'_, ImportState>) -> bool {
    state.cancel()
}

/// Reads a dropped `.txt` list for the Download screen. Native drag and drop
/// exposes paths only, so the old `File.text()` path needs this small command.
#[tauri::command]
pub fn read_text_file(path: String) -> Result<String, AppError> {
    let candidate = Path::new(&path);
    let is_txt = candidate
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("txt"));
    if !is_txt {
        return Err(AppError::invalid_input(i18n::t(
            "поддерживаются только файлы .txt",
            "only .txt files are supported",
        )));
    }
    let meta = fs::metadata(candidate).map_err(|error| {
        AppError::access(i18n::tf(
            || format!("не удалось прочитать файл: {error}"),
            || format!("could not read the file: {error}"),
        ))
    })?;
    if meta.len() > 2 * 1024 * 1024 {
        return Err(AppError::invalid_input(i18n::t(
            "файл списка слишком большой",
            "the list file is too large",
        )));
    }
    fs::read_to_string(candidate).map_err(|error| {
        AppError::access(i18n::tf(
            || format!("не удалось прочитать файл: {error}"),
            || format!("could not read the file: {error}"),
        ))
    })
}

/// Removes leftovers from an interrupted copy. `.part` has no media extension,
/// so the scanner never indexes them.
pub fn sweep_part_files(paths: &AppPaths) {
    for dir in [paths.audio(), paths.video()] {
        let Ok(entries) = fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("part"))
            {
                let _ = fs::remove_file(path);
            }
        }
    }
}

#[tauri::command]
pub fn library_thumb(app: AppHandle, path: String) -> Result<String, AppError> {
    let paths = app.state::<AppPaths>();
    let layout = LibraryPaths::from_app(&paths);
    let resolved = resolve_thumb(&layout, Path::new(&path)).map_err(AppError::not_found)?;
    let bytes = fs::read(resolved).map_err(|error| {
        AppError::not_found(i18n::tf(
            || format!("не удалось прочитать обложку: {error}"),
            || format!("could not read the cover: {error}"),
        ))
    })?;
    Ok(format!("data:image/jpeg;base64,{}", STANDARD.encode(bytes)))
}
