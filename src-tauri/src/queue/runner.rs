use std::collections::VecDeque;
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use tauri::{AppHandle, Manager};
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::i18n;
use crate::process;
use crate::progress;
use crate::queue::events::{LogEvent, ProgressEvent, emit_log, emit_progress, emit_state};
use crate::queue::pump::task_finished;
use crate::queue::store::{TaskState, TaskStore};
use crate::queue::throttle::{PROGRESS_EMIT_INTERVAL, ProgressThrottle};

pub(crate) async fn run_task(app: AppHandle, id: String, exe: PathBuf, argv: Vec<OsString>) {
    let store = app.state::<Arc<TaskStore>>().inner().clone();
    let stderr_tail: Arc<Mutex<VecDeque<String>>> = Arc::new(Mutex::new(VecDeque::new()));

    let spawn_result = process::prepare(&exe, &argv).spawn();
    let mut child = match spawn_result {
        Ok(child) => child,
        Err(error) => {
            store.finish(
                &id,
                TaskState::Failed,
                Some(process::spawn_error("yt-dlp", &exe, &error)),
            );
            if let Some(snapshot) = store.snapshot(&id) {
                emit_state(&app, &snapshot);
            }
            task_finished(&app);
            return;
        }
    };

    if let Some(pid) = child.id() {
        {
            let mut inner = store.inner.lock().unwrap();
            if let Some(entry) = inner.tasks.get_mut(&id) {
                entry.pid = Some(pid);
            }
        }
        if store.is_canceled(&id) {
            process::kill_tree(pid).await;
        }
    }

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    let stdout_job = stdout.map(|stream| {
        let app = app.clone();
        let store = store.clone();
        let id = id.clone();
        tauri::async_runtime::spawn(async move {
            read_stdout(app, store, id, stream).await;
        })
    });

    let stderr_job = stderr.map(|stream| {
        let app = app.clone();
        let store = store.clone();
        let id = id.clone();
        let tail = stderr_tail.clone();
        tauri::async_runtime::spawn(async move {
            read_stderr(app, store, id, stream, tail).await;
        })
    });

    let status = child.wait().await;
    if let Some(job) = stdout_job {
        let _ = job.await;
    }
    if let Some(job) = stderr_job {
        let _ = job.await;
    }

    let canceled = store.is_canceled(&id);
    if canceled {
        store.finish(&id, TaskState::Canceled, None);
    } else if matches!(&status, Ok(s) if s.success()) {
        store.finish_completed(&id);
    } else {
        let reason = stderr_tail
            .lock()
            .unwrap()
            .iter()
            .rev()
            .find(|line| !line.trim().is_empty())
            .cloned()
            .or_else(|| status.err().map(|error| error.to_string()))
            .unwrap_or_else(|| i18n::t("yt-dlp завершился с ошибкой", "yt-dlp failed"));
        store.finish(&id, TaskState::Failed, Some(reason));
    }

    if let Some(snapshot) = store.snapshot(&id) {
        emit_state(&app, &snapshot);
    }
    task_finished(&app);
}

async fn read_stdout(
    app: AppHandle,
    store: Arc<TaskStore>,
    id: String,
    stream: tokio::process::ChildStdout,
) {
    let mut lines = BufReader::new(stream).lines();
    let mut throttle = ProgressThrottle::new(PROGRESS_EMIT_INTERVAL);
    while let Ok(Some(line)) = lines.next_line().await {
        if let Some(parsed) = progress::parse_line(&line) {
            // Always update the snapshot; only the event is rate limited.
            let snapshot = store.apply_progress(
                &id,
                parsed.percent,
                parsed.speed_bps,
                parsed.eta_secs,
                parsed.playlist_index,
                parsed.playlist_count,
            );
            if !throttle.allow(Instant::now()) {
                continue;
            }
            emit_progress(
                &app,
                &ProgressEvent {
                    id: id.clone(),
                    percent: snapshot.as_ref().map_or(parsed.percent, |s| s.percent),
                    speed_bps: snapshot.as_ref().and_then(|s| s.speed_bps),
                    eta_secs: snapshot.as_ref().and_then(|s| s.eta_secs),
                    playlist_index: parsed.playlist_index,
                    playlist_count: parsed.playlist_count,
                },
            );
        } else {
            if let Some(path) = downloaded_path(&line) {
                store.note_destination(&id, &path);
            }
            if is_skip_line(&line) {
                store.mark_skipped(&id);
            }
            emit_log(
                &app,
                &LogEvent {
                    id: id.clone(),
                    line,
                },
            );
        }
    }
}

pub(crate) fn downloaded_path(line: &str) -> Option<PathBuf> {
    let line = line.trim();
    let rest = line.strip_prefix("[download] Destination: ").or_else(|| {
        line.strip_prefix("[download] ")?
            .strip_suffix(" has already been downloaded")
    })?;
    if rest.is_empty() {
        None
    } else {
        Some(PathBuf::from(rest))
    }
}

pub(crate) fn is_skip_line(line: &str) -> bool {
    let line = line.trim();
    line.ends_with("has already been downloaded")
        || line.contains("has already been recorded in the archive")
}

async fn read_stderr(
    app: AppHandle,
    store: Arc<TaskStore>,
    id: String,
    stream: tokio::process::ChildStderr,
    tail: Arc<Mutex<VecDeque<String>>>,
) {
    let mut lines = BufReader::new(stream).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        {
            let mut buffer = tail.lock().unwrap();
            if buffer.len() == 200 {
                buffer.pop_front();
            }
            buffer.push_back(line.clone());
        }
        if line.contains("ERROR:") {
            store.note_error(&id);
        }
        emit_log(
            &app,
            &LogEvent {
                id: id.clone(),
                line,
            },
        );
    }
}
