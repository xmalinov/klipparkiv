mod decode;
pub mod engine;
mod ffi;
mod looping;
mod metronome;
mod output;
mod shared;
mod state;
mod stretch;
pub mod waveform;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::error::AppError;
use crate::library::{Library, LibraryItem, resolve_media};
use crate::media::probe_duration;
use crate::paths::AppPaths;
use crate::practice::metronome::{GridPayload, MetronomeGrid};
use crate::time_util::{mtime_secs_checked, now_secs};

use engine::{Player, Snapshot};
use state::PracticeState as PracticeData;

pub struct PracticeState {
    player: Mutex<Player>,
}

impl PracticeState {
    pub fn new(ffmpeg: PathBuf) -> Self {
        Self {
            player: Mutex::new(Player::new(ffmpeg)),
        }
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Player>, String> {
        self.player
            .lock()
            .map_err(|_| "practice player lock is poisoned".to_string())
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PracticeOpened {
    pub path: String,
    pub duration_secs: Option<f64>,
    pub sample_rate: Option<u32>,
    pub state: PracticeData,
}

#[derive(Debug, Clone, Serialize)]
struct ErrorPayload {
    message: String,
}

fn player_error(message: String) -> AppError {
    if message.contains("no track is open") {
        AppError::busy(message)
    } else {
        AppError::invalid_input(message)
    }
}

fn load_state(library: &Library, path: &str, mtime: i64) -> PracticeData {
    library
        .practice_get(path, mtime)
        .ok()
        .flatten()
        .and_then(|json| PracticeData::from_json(&json).ok())
        .unwrap_or_default()
}

#[tauri::command]
pub fn practice_open(
    app: AppHandle,
    library: State<'_, Library>,
    practice: State<'_, PracticeState>,
    path: String,
) -> Result<PracticeOpened, AppError> {
    let paths = app.state::<AppPaths>();
    let key = path.clone();
    let resolved = resolve_media(paths.root(), Path::new(&key)).map_err(AppError::access)?;
    let mtime = mtime_secs_checked(&resolved).map_err(AppError::access)?;
    let state = load_state(&library, &key, mtime);
    let duration_secs = probe_duration(&paths.ffprobe(), &resolved);
    library.practice_unhide(&key).ok();

    let mut player = practice.lock().map_err(AppError::internal)?;
    let opened = player
        .open(resolved.clone(), duration_secs)
        .map_err(AppError::internal)?;
    player.apply_state(&state);

    Ok(PracticeOpened {
        path: resolved.display().to_string(),
        duration_secs: opened.duration_secs,
        sample_rate: opened.sample_rate,
        state,
    })
}

#[tauri::command]
pub fn practice_state_get(
    app: AppHandle,
    library: State<'_, Library>,
    path: String,
) -> Result<PracticeData, AppError> {
    let paths = app.state::<AppPaths>();
    let resolved = resolve_media(paths.root(), Path::new(&path)).map_err(AppError::access)?;
    let mtime = mtime_secs_checked(&resolved).map_err(AppError::access)?;
    Ok(load_state(&library, &path, mtime))
}

#[tauri::command]
pub fn practice_state_save(
    app: AppHandle,
    library: State<'_, Library>,
    path: String,
    mut state: PracticeData,
) -> Result<(), AppError> {
    let paths = app.state::<AppPaths>();
    let resolved = resolve_media(paths.root(), Path::new(&path)).map_err(AppError::access)?;
    let mtime = mtime_secs_checked(&resolved).map_err(AppError::access)?;
    state.sanitize();
    let json = state.to_json().map_err(AppError::internal)?;
    library
        .practice_save(&path, mtime, &json, now_secs())
        .map_err(AppError::internal)
}

#[tauri::command]
pub fn practice_recent(
    library: State<'_, Library>,
    limit: u32,
) -> Result<Vec<LibraryItem>, AppError> {
    library
        .practice_recent(limit.clamp(1, 50))
        .map_err(AppError::internal)
}

#[tauri::command]
pub fn practice_recent_hide(library: State<'_, Library>, path: String) -> Result<(), AppError> {
    library.practice_hide(&path).map_err(AppError::internal)
}

#[tauri::command]
pub async fn practice_waveform(
    app: AppHandle,
    path: String,
) -> Result<waveform::Waveform, AppError> {
    let paths = app.state::<AppPaths>();
    let root = paths.root().to_path_buf();
    let ffmpeg = paths.ffmpeg();
    let cache_dir = paths.data().join("waveforms");

    tauri::async_runtime::spawn_blocking(move || {
        let resolved = resolve_media(&root, Path::new(&path))?;
        waveform::load_or_compute(&ffmpeg, &cache_dir, &resolved)
    })
    .await
    .map_err(|error| AppError::internal(error.to_string()))?
    .map_err(AppError::from)
}

#[tauri::command]
pub fn practice_play(state: State<'_, PracticeState>) -> Result<(), AppError> {
    state.lock()?.play().map_err(player_error)
}

#[tauri::command]
pub fn practice_pause(state: State<'_, PracticeState>) -> Result<(), AppError> {
    state.lock()?.pause().map_err(player_error)
}

#[tauri::command]
pub fn practice_seek(state: State<'_, PracticeState>, position_secs: f64) -> Result<(), AppError> {
    state.lock()?.seek(position_secs).map_err(player_error)
}

#[tauri::command]
pub fn practice_set_speed(state: State<'_, PracticeState>, speed: f32) -> Result<(), AppError> {
    state.lock()?.set_speed(speed).map_err(player_error)
}

#[tauri::command]
pub fn practice_set_pitch(
    state: State<'_, PracticeState>,
    semitones: f32,
    cents: f32,
) -> Result<(), AppError> {
    state
        .lock()?
        .set_pitch(semitones, cents)
        .map_err(player_error)
}

#[tauri::command]
pub fn practice_set_volume(state: State<'_, PracticeState>, volume: f32) -> Result<(), AppError> {
    state.lock()?.set_volume(volume).map_err(player_error)
}

#[tauri::command]
pub fn practice_set_loop(
    state: State<'_, PracticeState>,
    enabled: bool,
    start_secs: Option<f64>,
    end_secs: Option<f64>,
    pre_roll: bool,
    pre_roll_secs: Option<f64>,
) -> Result<(), AppError> {
    state
        .lock()?
        .set_loop(enabled, start_secs, end_secs, pre_roll, pre_roll_secs)
        .map_err(player_error)
}

#[tauri::command]
pub fn practice_set_grid(
    state: State<'_, PracticeState>,
    grid: Option<GridPayload>,
) -> Result<(), AppError> {
    let grid = grid
        .map(MetronomeGrid::try_from)
        .transpose()
        .map_err(AppError::invalid_input)?
        .map(Arc::new);
    state.lock().map_err(AppError::internal)?.set_grid(grid);
    Ok(())
}

#[tauri::command]
pub fn practice_set_metronome(
    state: State<'_, PracticeState>,
    enabled: bool,
    volume: f32,
) -> Result<(), AppError> {
    state.lock()?.set_metronome(enabled, volume);
    Ok(())
}

#[tauri::command]
pub fn practice_position(state: State<'_, PracticeState>) -> Result<Option<Snapshot>, AppError> {
    Ok(state.lock().map_err(AppError::internal)?.snapshot())
}

#[tauri::command]
pub fn practice_close(state: State<'_, PracticeState>) -> Result<(), AppError> {
    state.lock().map_err(AppError::internal)?.close();
    Ok(())
}

/// Emits playback position to the webview at ~30 Hz and forwards engine errors.
pub fn spawn_events(app: AppHandle) {
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(Duration::from_millis(33));

            let state = app.state::<PracticeState>();
            let (snapshot, error) = {
                let player = match state.lock() {
                    Ok(player) => player,
                    Err(_) => continue,
                };
                if !player.has_track() {
                    continue;
                }
                (player.snapshot(), player.take_error())
            };

            if let Some(snapshot) = snapshot {
                let _ = app.emit("practice://position", snapshot);
            }
            if let Some(message) = error {
                let _ = app.emit("practice://error", ErrorPayload { message });
            }
        }
    });
}
