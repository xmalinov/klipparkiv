use std::collections::{HashMap, VecDeque};
use std::ffi::OsString;
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::Serialize;

use crate::i18n;
use crate::queue::validate::overall_percent;

/// What a task produces. `both` is two tasks, not a mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum TaskMode {
    Audio,
    Video,
}

/// Lifecycle of one download. The wire format stays the same strings the UI
/// already knows (`queued`, `running`, …), but the backend can no longer pass an
/// arbitrary string around.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum TaskState {
    Queued,
    Running,
    Done,
    Skipped,
    Partial,
    Failed,
    Canceled,
}

impl TaskState {
    /// The task will not change anymore: it can be removed and counted in the
    /// final summary.
    pub(crate) fn is_terminal(self) -> bool {
        matches!(
            self,
            TaskState::Done
                | TaskState::Skipped
                | TaskState::Partial
                | TaskState::Failed
                | TaskState::Canceled
        )
    }
}

/// Default parallel downloads until the settings are loaded.
pub(crate) const DEFAULT_CONCURRENCY: usize = 2;

/// Finished tasks stay in the history list, but not forever: a long session
/// must not grow the process memory without bound.
pub(crate) const MAX_FINISHED_TASKS: usize = 200;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskSnapshot {
    pub id: String,
    pub url: String,
    /// Human title once known: from the preview card or from yt-dlp output.
    pub title: Option<String>,
    pub mode: TaskMode,
    pub clip: bool,
    pub state: TaskState,
    pub percent: f32,
    pub speed_bps: Option<f64>,
    pub eta_secs: Option<u64>,
    pub error: Option<String>,
    pub playlist_index: Option<u32>,
    pub playlist_count: Option<u32>,
    pub output_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueState {
    pub paused: bool,
    pub running: usize,
    pub queued: usize,
}

pub(crate) struct Entry {
    pub(crate) snapshot: TaskSnapshot,
    pub(crate) pid: Option<u32>,
    pub(crate) canceled: bool,
    /// yt-dlp reported that the target file already exists (or is in the archive).
    pub(crate) skipped: bool,
    /// Number of `ERROR:` lines; with `--ignore-errors` the exit code stays 0.
    pub(crate) error_count: usize,
    pub(crate) argv: Vec<OsString>,
}

pub(crate) struct Inner {
    pub(crate) tasks: HashMap<String, Entry>,
    pub(crate) queue: VecDeque<String>,
    pub(crate) paused: bool,
    pub(crate) running: usize,
    pub(crate) max_concurrent: usize,
}

impl Default for Inner {
    fn default() -> Self {
        Self {
            tasks: HashMap::new(),
            queue: VecDeque::new(),
            paused: false,
            running: 0,
            max_concurrent: DEFAULT_CONCURRENCY,
        }
    }
}

#[derive(Default)]
pub(crate) struct TaskStore {
    pub(crate) inner: Mutex<Inner>,
    pub(crate) next_id: AtomicU64,
}

impl TaskStore {
    pub(crate) fn next_id(&self) -> String {
        format!("t{}", self.next_id.fetch_add(1, Ordering::Relaxed) + 1)
    }

    pub(crate) fn insert(&self, snapshot: TaskSnapshot, argv: Vec<OsString>) -> TaskSnapshot {
        let mut inner = self.inner.lock().unwrap();
        inner.queue.push_back(snapshot.id.clone());
        inner.tasks.insert(
            snapshot.id.clone(),
            Entry {
                snapshot: snapshot.clone(),
                pid: None,
                canceled: false,
                skipped: false,
                error_count: 0,
                argv,
            },
        );
        snapshot
    }

    pub(crate) fn snapshot(&self, id: &str) -> Option<TaskSnapshot> {
        self.inner
            .lock()
            .unwrap()
            .tasks
            .get(id)
            .map(|entry| entry.snapshot.clone())
    }

    pub(crate) fn all(&self) -> Vec<TaskSnapshot> {
        self.inner
            .lock()
            .unwrap()
            .tasks
            .values()
            .map(|entry| entry.snapshot.clone())
            .collect()
    }

    pub(crate) fn update<F>(&self, id: &str, change: F) -> Option<TaskSnapshot>
    where
        F: FnOnce(&mut TaskSnapshot),
    {
        let mut inner = self.inner.lock().unwrap();
        let entry = inner.tasks.get_mut(id)?;
        change(&mut entry.snapshot);
        Some(entry.snapshot.clone())
    }

    pub(crate) fn take_cancel(&self, id: &str) -> Option<u32> {
        let mut inner = self.inner.lock().unwrap();
        let entry = inner.tasks.get_mut(id)?;
        entry.canceled = true;
        entry.pid
    }

    pub(crate) fn is_canceled(&self, id: &str) -> bool {
        self.inner
            .lock()
            .unwrap()
            .tasks
            .get(id)
            .is_some_and(|entry| entry.canceled)
    }

    pub(crate) fn finish(&self, id: &str, state: TaskState, error: Option<String>) {
        let mut inner = self.inner.lock().unwrap();
        let Some(entry) = inner.tasks.get_mut(id) else {
            return;
        };
        entry.snapshot.state = state;
        entry.snapshot.error = error;
        // A finished task has no live process: keeping the pid made a later
        // "remove" try to kill a dead process and do nothing at all.
        entry.pid = None;
        if matches!(state, TaskState::Done | TaskState::Skipped) {
            entry.snapshot.percent = 100.0;
        }
        prune_finished(&mut inner);
    }

    /// Drops a finished task from the list. A queued or running task must be
    /// cancelled first, so the UI never hides a download that is still writing.
    pub(crate) fn remove(&self, id: &str) -> Result<(), String> {
        let mut inner = self.inner.lock().unwrap();
        let Some(entry) = inner.tasks.get(id) else {
            return Ok(());
        };
        if !entry.snapshot.state.is_terminal() {
            return Err(i18n::t(
                "незавершённую загрузку нельзя убрать — сначала отмените её",
                "a running download cannot be removed — cancel it first",
            ));
        }
        inner.tasks.remove(id);
        inner.queue.retain(|queued| queued != id);
        Ok(())
    }

    pub(crate) fn mark_skipped(&self, id: &str) {
        let mut inner = self.inner.lock().unwrap();
        if let Some(entry) = inner.tasks.get_mut(id) {
            entry.skipped = true;
        }
    }

    pub(crate) fn note_error(&self, id: &str) {
        let mut inner = self.inner.lock().unwrap();
        if let Some(entry) = inner.tasks.get_mut(id) {
            entry.error_count += 1;
        }
    }

    pub(crate) fn note_destination(&self, id: &str, path: &Path) {
        let path_text = path.to_string_lossy().into_owned();
        let title = crate::library::display_title(
            path.file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or(path_text.as_str()),
        );
        let _ = self.update(id, |snapshot| {
            snapshot.output_path = Some(path_text.clone());
            if snapshot.title.is_none() && !title.trim().is_empty() {
                snapshot.title = Some(title);
            }
        });
    }

    /// Maps the exit code and the collected flags to a user-visible state.
    pub(crate) fn finish_completed(&self, id: &str) {
        let (skipped, errors) = {
            let inner = self.inner.lock().unwrap();
            match inner.tasks.get(id) {
                Some(entry) => (entry.skipped, entry.error_count),
                None => (false, 0),
            }
        };
        if skipped {
            self.finish(id, TaskState::Skipped, None);
        } else if errors > 0 {
            self.finish(
                id,
                TaskState::Partial,
                Some(i18n::tf(
                    || format!("ошибок в плейлисте: {errors}"),
                    || format!("playlist errors: {errors}"),
                )),
            );
        } else {
            self.finish(id, TaskState::Done, None);
        }
    }

    pub(crate) fn apply_progress(
        &self,
        id: &str,
        percent: f32,
        speed_bps: Option<f64>,
        eta_secs: Option<u64>,
        playlist_index: Option<u32>,
        playlist_count: Option<u32>,
    ) -> Option<TaskSnapshot> {
        self.update(id, |snapshot| {
            snapshot.percent = overall_percent(percent, playlist_index, playlist_count);
            if let Some(speed) = speed_bps {
                snapshot.speed_bps = Some(speed);
            }
            if let Some(eta) = eta_secs {
                snapshot.eta_secs = Some(eta);
            }
            if playlist_index.is_some() {
                snapshot.playlist_index = playlist_index;
            }
            if playlist_count.is_some() {
                snapshot.playlist_count = playlist_count;
            }
        })
    }

    pub fn queue_state(&self) -> QueueState {
        let inner = self.inner.lock().unwrap();
        QueueState {
            paused: inner.paused,
            running: inner.running,
            queued: inner
                .queue
                .iter()
                .filter(|id| inner.tasks.get(*id).is_some_and(|e| !e.canceled))
                .count(),
        }
    }

    pub fn set_paused(&self, paused: bool) -> QueueState {
        {
            let mut inner = self.inner.lock().unwrap();
            inner.paused = paused;
        }
        self.queue_state()
    }

    pub fn set_concurrency(&self, concurrency: usize) {
        let mut inner = self.inner.lock().unwrap();
        inner.max_concurrent = concurrency.clamp(1, 4);
    }

    pub(crate) fn finish_running(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.running = inner.running.saturating_sub(1);
    }
}

/// Ids are `t1`, `t2`, …; anything else sorts last.
fn id_order(id: &str) -> u64 {
    id.strip_prefix('t')
        .and_then(|number| number.parse().ok())
        .unwrap_or(u64::MAX)
}

/// Drops the oldest finished tasks once the history limit is exceeded.
fn prune_finished(inner: &mut Inner) {
    let mut finished: Vec<(u64, String)> = inner
        .tasks
        .iter()
        .filter(|(_, entry)| entry.snapshot.state.is_terminal())
        .map(|(id, _)| (id_order(id), id.clone()))
        .collect();
    if finished.len() <= MAX_FINISHED_TASKS {
        return;
    }
    finished.sort_by_key(|(order, _)| *order);
    let excess = finished.len() - MAX_FINISHED_TASKS;
    for (_, id) in finished.into_iter().take(excess) {
        inner.tasks.remove(&id);
        inner.queue.retain(|queued| queued != &id);
    }
}
