use std::sync::Arc;

use serde::Deserialize;
use tauri::{AppHandle, Manager, State};

use crate::error::AppError;
use crate::i18n;
use crate::metadata;
use crate::paths::AppPaths;
use crate::process;
use crate::queue::events::{emit_queue, emit_state};
use crate::queue::pump::{Job, enqueue_job, pump};
use crate::queue::store::{QueueState, TaskSnapshot, TaskState, TaskStore};
use crate::queue::validate::{
    parse_audio_kbps, parse_clip, parse_list, parse_modes, parse_playlist, parse_quality,
    resolve_out_dir, validate_url,
};
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewTask {
    pub url: String,
    pub mode: String,
    pub quality: Option<String>,
    pub audio_kbps: Option<u32>,
    pub out_dir: Option<String>,
    pub clip_start: Option<String>,
    pub clip_end: Option<String>,
    pub title: Option<String>,
    pub playlist: Option<bool>,
    pub playlist_start: Option<u32>,
    pub playlist_end: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchRequest {
    pub path: String,
    pub mode: String,
    pub quality: Option<String>,
    pub audio_kbps: Option<u32>,
    pub out_dir: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinksRequest {
    pub text: String,
    pub mode: String,
    pub quality: Option<String>,
    pub audio_kbps: Option<u32>,
    pub out_dir: Option<String>,
}

#[tauri::command]
pub fn enqueue_task(app: AppHandle, task: NewTask) -> Result<Vec<TaskSnapshot>, AppError> {
    let modes = parse_modes(&task.mode).map_err(AppError::invalid_input)?;
    let quality = parse_quality(task.quality.as_deref()).map_err(AppError::invalid_input)?;
    let audio_kbps = parse_audio_kbps(task.audio_kbps).map_err(AppError::invalid_input)?;
    let url = validate_url(&task.url).map_err(AppError::invalid_input)?;
    let clip = parse_clip(task.clip_start.as_deref(), task.clip_end.as_deref())
        .map_err(AppError::invalid_input)?;
    let playlist = parse_playlist(task.playlist, task.playlist_start, task.playlist_end)
        .map_err(AppError::invalid_input)?;
    if playlist.is_some() && clip.is_some() {
        return Err(AppError::invalid_input(i18n::t(
            "Клип можно вырезать только из одного видео, не из плейлиста",
            "A clip can only be cut from a single video, not from a playlist",
        )));
    }
    let out_dir = resolve_out_dir(task.out_dir.as_deref()).map_err(AppError::invalid_input)?;

    let mut created = Vec::with_capacity(modes.len());
    for mode in modes {
        created.push(enqueue_job(
            &app,
            Job {
                url: &url,
                mode,
                quality,
                audio_kbps,
                clip: clip.as_ref(),
                playlist,
                out_dir: out_dir.as_deref(),
                title: task.title.as_deref(),
            },
        )?);
    }

    emit_queue(&app);
    pump(&app);
    Ok(created)
}

fn enqueue_text(
    app: &AppHandle,
    text: &str,
    mode: &str,
    quality: Option<&str>,
    audio_kbps: Option<u32>,
    out_dir: Option<&str>,
) -> Result<Vec<TaskSnapshot>, AppError> {
    let modes = parse_modes(mode).map_err(AppError::invalid_input)?;
    let quality = parse_quality(quality).map_err(AppError::invalid_input)?;
    let audio_kbps = parse_audio_kbps(audio_kbps).map_err(AppError::invalid_input)?;
    let out_dir = resolve_out_dir(out_dir).map_err(AppError::invalid_input)?;
    let entries = parse_list(text).map_err(AppError::invalid_input)?;

    let mut created = Vec::new();
    for entry in &entries {
        for &mode in &modes {
            created.push(enqueue_job(
                app,
                Job {
                    url: &entry.url,
                    mode,
                    quality,
                    audio_kbps,
                    clip: entry.clip.as_ref(),
                    playlist: None,
                    out_dir: out_dir.as_deref(),
                    title: None,
                },
            )?);
        }
    }

    emit_queue(app);
    pump(app);
    Ok(created)
}

#[tauri::command]
pub fn enqueue_batch(app: AppHandle, request: BatchRequest) -> Result<Vec<TaskSnapshot>, AppError> {
    let text = std::fs::read(&request.path).map_err(|error| {
        AppError::access(i18n::tf(
            || format!("не удалось прочитать файл: {error}"),
            || format!("could not read the file: {error}"),
        ))
    })?;
    let text = String::from_utf8_lossy(&text);
    enqueue_text(
        &app,
        &text,
        &request.mode,
        request.quality.as_deref(),
        request.audio_kbps,
        request.out_dir.as_deref(),
    )
}

/// Accepts a list pasted from the clipboard (one link per line, optional
/// `HH:MM:SS-HH:MM:SS` range) without requiring a `.txt` file.
#[tauri::command]
pub fn enqueue_links(app: AppHandle, request: LinksRequest) -> Result<Vec<TaskSnapshot>, AppError> {
    enqueue_text(
        &app,
        &request.text,
        &request.mode,
        request.quality.as_deref(),
        request.audio_kbps,
        request.out_dir.as_deref(),
    )
}

#[tauri::command]
pub fn pause_queue(app: AppHandle) -> QueueState {
    let state = app.state::<Arc<TaskStore>>().set_paused(true);
    emit_queue(&app);
    state
}

#[tauri::command]
pub fn resume_queue(app: AppHandle) -> QueueState {
    let state = app.state::<Arc<TaskStore>>().set_paused(false);
    emit_queue(&app);
    pump(&app);
    state
}

/// Tray menu action: flips the pause flag.
pub fn toggle_pause(app: &AppHandle) -> QueueState {
    let store = app.state::<Arc<TaskStore>>();
    let paused = !store.queue_state().paused;
    let state = store.set_paused(paused);
    emit_queue(app);
    if !paused {
        pump(app);
    }
    state
}

#[tauri::command]
pub fn queue_state(store: State<'_, Arc<TaskStore>>) -> QueueState {
    store.queue_state()
}

#[tauri::command]
pub async fn probe_metadata(app: AppHandle, url: String) -> Result<metadata::VideoInfo, AppError> {
    let url = validate_url(&url).map_err(AppError::invalid_input)?;
    let exe = {
        let paths = app.state::<AppPaths>();
        paths.ytdlp()
    };
    metadata::probe(&exe, &url)
        .await
        .map_err(AppError::internal)
}

#[tauri::command]
pub async fn cancel_task(app: AppHandle, id: String) -> Result<(), AppError> {
    let store = app.state::<Arc<TaskStore>>();
    let pid = store.take_cancel(&id);

    match pid {
        Some(pid) => {
            process::kill_tree(pid).await;
        }
        None => {
            store.finish(&id, TaskState::Canceled, None);
            if let Some(snapshot) = store.snapshot(&id) {
                emit_state(&app, &snapshot);
            }
            emit_queue(&app);
            pump(&app);
            crate::update::maybe_install_when_idle(&app);
        }
    }
    Ok(())
}

#[tauri::command]
pub fn list_tasks(store: State<'_, Arc<TaskStore>>) -> Vec<TaskSnapshot> {
    store.all()
}

/// Drops a finished task from the queue history.
#[tauri::command]
pub fn remove_task(app: AppHandle, id: String) -> Result<(), AppError> {
    app.state::<Arc<TaskStore>>()
        .remove(&id)
        .map_err(AppError::busy)?;
    emit_queue(&app);
    Ok(())
}
