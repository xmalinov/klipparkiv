mod beat;
mod bpm;
mod cache;
mod models;
pub mod types;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use tauri::{AppHandle, Emitter, Manager, State};

use crate::analysis::types::{ANALYSIS_VERSION, AnalysisError, AnalysisProgress, PracticeAnalysis};
use crate::library::resolve_media;
use crate::paths::AppPaths;

struct Job {
    path: String,
    cancel: Arc<AtomicBool>,
    progress: Arc<AtomicU32>,
}

#[derive(Default)]
pub struct AnalysisState {
    job: Mutex<Option<Job>>,
}

fn cancel_job(slot: &mut Option<Job>) {
    if let Some(job) = slot.take() {
        job.cancel.store(true, Ordering::Relaxed);
    }
}

fn file_identity(path: &Path) -> Result<(i64, u64), String> {
    let metadata = fs::metadata(path).map_err(|error| format!("cannot read file: {error}"))?;
    let mtime = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0);
    Ok((mtime, metadata.len()))
}

fn probe_duration(ffprobe: &Path, path: &Path) -> Option<f64> {
    let mut command = Command::new(ffprobe);
    command
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
        ])
        .arg(path);
    crate::process::hide_console_std(&mut command);
    let output = command.output().ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<f64>()
        .ok()
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

fn emit_progress(app: &AppHandle, path: &str, stage: &'static str, progress: f32) {
    let _ = app.emit(
        "analysis://progress",
        AnalysisProgress {
            path: path.to_string(),
            stage,
            progress: progress.clamp(0.0, 1.0),
        },
    );
}

fn emit_error(app: &AppHandle, path: &str, message: String) {
    let _ = app.emit(
        "analysis://error",
        AnalysisError {
            path: path.to_string(),
            message,
        },
    );
}

/// Return a cached analysis without starting a new job.
#[tauri::command]
pub fn practice_analysis_get(
    app: AppHandle,
    path: String,
) -> Result<Option<PracticeAnalysis>, String> {
    let paths = app.state::<AppPaths>();
    let resolved = resolve_media(paths.root(), Path::new(&path))?;
    let (mtime, size) = file_identity(&resolved)?;
    let key = cache::cache_key(&resolved, mtime, size);
    Ok(cache::load(&paths.analysis_cache(), &key))
}

/// Start a background beat-grid analysis. Returns immediately; progress and the
/// final result are delivered through `analysis://` events.
#[tauri::command]
pub fn practice_analysis_start(
    app: AppHandle,
    state: State<'_, AnalysisState>,
    path: String,
) -> Result<(), String> {
    let paths = app.state::<AppPaths>();
    let resolved = resolve_media(paths.root(), Path::new(&path))?;
    let (mtime, size) = file_identity(&resolved)?;
    let cache_dir = paths.analysis_cache();
    let key = cache::cache_key(&resolved, mtime, size);

    if let Some(analysis) = cache::load(&cache_dir, &key) {
        emit_progress(&app, &path, "cached", 1.0);
        let _ = app.emit("analysis://done", analysis);
        return Ok(());
    }

    let existing_progress = {
        let mut slot = state
            .job
            .lock()
            .map_err(|_| "analysis state lock is poisoned".to_string())?;
        if let Some(job) = slot.as_ref().filter(|job| job.path == path) {
            Some(job.progress.clone())
        } else {
            cancel_job(&mut slot);
            *slot = Some(Job {
                path: path.clone(),
                cancel: Arc::new(AtomicBool::new(false)),
                progress: Arc::new(AtomicU32::new(0.0f32.to_bits())),
            });
            None
        }
    };

    if let Some(progress) = existing_progress {
        let value = f32::from_bits(progress.load(Ordering::Relaxed));
        let stage = if value > 0.02 { "beat" } else { "model" };
        emit_progress(&app, &path, stage, value);
        return Ok(());
    }

    let (cancel, progress) = {
        let slot = state
            .job
            .lock()
            .map_err(|_| "analysis state lock is poisoned".to_string())?;
        let job = slot
            .as_ref()
            .ok_or_else(|| "analysis job was not created".to_string())?;
        (job.cancel.clone(), job.progress.clone())
    };

    let worker_app = app.clone();
    let worker_path = path.clone();
    let worker_resolved: PathBuf = resolved;
    tauri::async_runtime::spawn_blocking(move || {
        run_job(
            worker_app,
            worker_path,
            worker_resolved,
            cache_dir,
            key,
            cancel,
            progress,
        );
    });

    Ok(())
}

#[tauri::command]
pub fn practice_analysis_cancel(state: State<'_, AnalysisState>) -> Result<(), String> {
    let mut slot = state
        .job
        .lock()
        .map_err(|_| "analysis state lock is poisoned".to_string())?;
    cancel_job(&mut slot);
    Ok(())
}

fn run_job(
    app: AppHandle,
    original_path: String,
    resolved: PathBuf,
    cache_dir: PathBuf,
    cache_key: String,
    cancel: Arc<AtomicBool>,
    progress: Arc<AtomicU32>,
) {
    let result = run_job_inner(
        &app,
        &original_path,
        &resolved,
        &cache_dir,
        &cache_key,
        &cancel,
        &progress,
    );

    if let Some(state) = app.try_state::<AnalysisState>()
        && let Ok(mut slot) = state.job.lock()
        && slot.as_ref().is_some_and(|job| job.path == original_path)
    {
        *slot = None;
    }

    match result {
        Ok(analysis) => {
            progress.store(1.0f32.to_bits(), Ordering::Relaxed);
            emit_progress(&app, &original_path, "done", 1.0);
            let _ = app.emit("analysis://done", analysis);
        }
        Err(message) => {
            if !cancel.load(Ordering::Relaxed) {
                emit_error(&app, &original_path, message);
            }
        }
    }
}

fn run_job_inner(
    app: &AppHandle,
    original_path: &str,
    resolved: &Path,
    cache_dir: &Path,
    cache_key: &str,
    cancel: &AtomicBool,
    progress: &AtomicU32,
) -> Result<PracticeAnalysis, String> {
    let paths = app.state::<AppPaths>();
    let model_paths = models::resolve(&paths.models())?;
    let duration = probe_duration(&paths.ffprobe(), resolved).unwrap_or(0.0);

    progress.store(0.02f32.to_bits(), Ordering::Relaxed);
    emit_progress(app, original_path, "model", 0.02);
    let run = beat::analyze(
        &paths.ffmpeg(),
        &model_paths,
        resolved,
        (duration > 0.0).then_some(duration),
        cancel,
        |value| {
            progress.store(value.to_bits(), Ordering::Relaxed);
            emit_progress(app, original_path, "beat", value);
        },
    )?;

    let summary = bpm::summarize(&run.beats, &run.downbeats);
    let analysis = PracticeAnalysis {
        version: ANALYSIS_VERSION,
        path: original_path.to_string(),
        duration_secs: duration,
        bpm_raw: summary.raw,
        bpm: summary.suggested,
        octave: summary.octave,
        confidence: summary.confidence,
        beats: run.beats.into_iter().map(f64::from).collect(),
        downbeats: run.downbeats.into_iter().map(f64::from).collect(),
        beats_per_bar: summary.beats_per_bar,
        bar_offset: summary.bar_offset,
        model: cache::MODEL_ID.to_string(),
        analyzed_at: now_secs(),
    };
    cache::save(cache_dir, cache_key, &analysis)?;
    Ok(analysis)
}
