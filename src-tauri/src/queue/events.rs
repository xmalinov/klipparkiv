use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::queue::store::{TaskSnapshot, TaskStore};
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProgressEvent {
    pub(crate) id: String,
    pub(crate) percent: f32,
    pub(crate) speed_bps: Option<f64>,
    pub(crate) eta_secs: Option<u64>,
    pub(crate) playlist_index: Option<u32>,
    pub(crate) playlist_count: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LogEvent {
    pub(crate) id: String,
    pub(crate) line: String,
}

pub(crate) fn emit_state(app: &AppHandle, snapshot: &TaskSnapshot) {
    let _ = app.emit("task://state", snapshot);
}

pub(crate) fn emit_queue(app: &AppHandle) {
    let state = app.state::<Arc<TaskStore>>().queue_state();
    let _ = app.emit("queue://changed", state);
}
pub(crate) fn emit_progress(app: &AppHandle, event: &ProgressEvent) {
    let _ = app.emit("task://progress", event);
}

pub(crate) fn emit_log(app: &AppHandle, event: &LogEvent) {
    let _ = app.emit("task://log", event);
}
