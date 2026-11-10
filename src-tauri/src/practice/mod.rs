mod decode;
pub mod engine;
mod ffi;
mod looping;
mod output;
mod shared;
mod state;
mod stretch;
pub mod waveform;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::library::{Library, LibraryItem, resolve_media};
use crate::paths::AppPaths;

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

fn probe_duration(ffprobe: &Path, path: &Path) -> Option<f64> {
    let output = Command::new(ffprobe)
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
        ])
        .arg(path)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<f64>()
        .ok()
}

fn file_mtime(path: &Path) -> Result<i64, String> {
    let modified = fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .map_err(|error| format!("cannot read file time: {error}"))?;
    Ok(modified
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0))
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
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
) -> Result<PracticeOpened, String> {
    let paths = app.state::<AppPaths>();
    let key = path.clone();
    let resolved = resolve_media(paths.root(), Path::new(&key))?;
    let mtime = file_mtime(&resolved)?;
    let state = load_state(&library, &key, mtime);
    let duration_secs = probe_duration(&paths.ffprobe(), &resolved);
    library.practice_unhide(&key).ok();

    let mut player = practice.lock()?;
    let opened = player.open(resolved.clone(), duration_secs)?;
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
) -> Result<PracticeData, String> {
    let paths = app.state::<AppPaths>();
    let resolved = resolve_media(paths.root(), Path::new(&path))?;
    let mtime = file_mtime(&resolved)?;
    Ok(load_state(&library, &path, mtime))
}

#[tauri::command]
pub fn practice_state_save(
    app: AppHandle,
    library: State<'_, Library>,
    path: String,
    mut state: PracticeData,
) -> Result<(), String> {
    let paths = app.state::<AppPaths>();
    let resolved = resolve_media(paths.root(), Path::new(&path))?;
    let mtime = file_mtime(&resolved)?;
    state.sanitize();
    let json = state.to_json()?;
    library.practice_save(&path, mtime, &json, now_secs())
}

#[tauri::command]
pub fn practice_recent(
    library: State<'_, Library>,
    limit: u32,
) -> Result<Vec<LibraryItem>, String> {
    library.practice_recent(limit.clamp(1, 50))
}

#[tauri::command]
pub fn practice_recent_hide(library: State<'_, Library>, path: String) -> Result<(), String> {
    library.practice_hide(&path)
}

#[tauri::command]
pub async fn practice_waveform(app: AppHandle, path: String) -> Result<waveform::Waveform, String> {
    let paths = app.state::<AppPaths>();
    let root = paths.root().to_path_buf();
    let ffmpeg = paths.ffmpeg();
    let cache_dir = paths.data().join("waveforms");

    tauri::async_runtime::spawn_blocking(move || {
        let resolved = resolve_media(&root, Path::new(&path))?;
        waveform::load_or_compute(&ffmpeg, &cache_dir, &resolved)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub fn practice_play(state: State<'_, PracticeState>) -> Result<(), String> {
    state.lock()?.play()
}

#[tauri::command]
pub fn practice_pause(state: State<'_, PracticeState>) -> Result<(), String> {
    state.lock()?.pause()
}

#[tauri::command]
pub fn practice_seek(state: State<'_, PracticeState>, position_secs: f64) -> Result<(), String> {
    state.lock()?.seek(position_secs)
}

#[tauri::command]
pub fn practice_set_speed(state: State<'_, PracticeState>, speed: f32) -> Result<(), String> {
    state.lock()?.set_speed(speed)
}

#[tauri::command]
pub fn practice_set_pitch(
    state: State<'_, PracticeState>,
    semitones: f32,
    cents: f32,
) -> Result<(), String> {
    state.lock()?.set_pitch(semitones, cents)
}

#[tauri::command]
pub fn practice_set_volume(state: State<'_, PracticeState>, volume: f32) -> Result<(), String> {
    state.lock()?.set_volume(volume)
}

#[tauri::command]
pub fn practice_set_loop(
    state: State<'_, PracticeState>,
    enabled: bool,
    start_secs: Option<f64>,
    end_secs: Option<f64>,
    pre_roll: bool,
) -> Result<(), String> {
    state
        .lock()?
        .set_loop(enabled, start_secs, end_secs, pre_roll)
}

#[tauri::command]
pub fn practice_position(state: State<'_, PracticeState>) -> Result<Option<Snapshot>, String> {
    Ok(state.lock()?.snapshot())
}

#[tauri::command]
pub fn practice_close(state: State<'_, PracticeState>) -> Result<(), String> {
    state.lock()?.close();
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
