use std::collections::{HashMap, VecDeque};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_notification::NotificationExt;
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::args::{self, ArgsRequest, ClipSpec, Mode, Quality};
use crate::i18n;
use crate::metadata;
use crate::paths::PortablePaths;
use crate::process;
use crate::progress;
use crate::settings::{Settings, SettingsStore};

const STATE_QUEUED: &str = "queued";
const STATE_RUNNING: &str = "running";
const STATE_DONE: &str = "done";
const STATE_FAILED: &str = "failed";
const STATE_CANCELED: &str = "canceled";

/// Default parallel downloads. M5 makes it configurable from settings.
pub const DEFAULT_CONCURRENCY: usize = 2;
const MAX_BATCH_LINES: usize = 500;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskSnapshot {
    pub id: String,
    pub url: String,
    pub mode: String,
    pub clip: bool,
    pub state: String,
    pub percent: f32,
    pub speed_bps: Option<f64>,
    pub eta_secs: Option<u64>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueState {
    pub paused: bool,
    pub running: usize,
    pub queued: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProgressEvent {
    id: String,
    percent: f32,
    speed_bps: Option<f64>,
    eta_secs: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct LogEvent {
    id: String,
    line: String,
}

struct Entry {
    snapshot: TaskSnapshot,
    pid: Option<u32>,
    canceled: bool,
    argv: Vec<OsString>,
}

struct Inner {
    tasks: HashMap<String, Entry>,
    queue: VecDeque<String>,
    paused: bool,
    running: usize,
    max_concurrent: usize,
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
pub struct TaskStore {
    inner: Mutex<Inner>,
    next_id: AtomicU64,
}

impl TaskStore {
    fn next_id(&self) -> String {
        format!("t{}", self.next_id.fetch_add(1, Ordering::Relaxed) + 1)
    }

    fn insert(&self, snapshot: TaskSnapshot, argv: Vec<OsString>) -> TaskSnapshot {
        let mut inner = self.inner.lock().unwrap();
        inner.queue.push_back(snapshot.id.clone());
        inner.tasks.insert(
            snapshot.id.clone(),
            Entry {
                snapshot: snapshot.clone(),
                pid: None,
                canceled: false,
                argv,
            },
        );
        snapshot
    }

    fn snapshot(&self, id: &str) -> Option<TaskSnapshot> {
        self.inner
            .lock()
            .unwrap()
            .tasks
            .get(id)
            .map(|entry| entry.snapshot.clone())
    }

    fn all(&self) -> Vec<TaskSnapshot> {
        self.inner
            .lock()
            .unwrap()
            .tasks
            .values()
            .map(|entry| entry.snapshot.clone())
            .collect()
    }

    fn update<F>(&self, id: &str, change: F) -> Option<TaskSnapshot>
    where
        F: FnOnce(&mut TaskSnapshot),
    {
        let mut inner = self.inner.lock().unwrap();
        let entry = inner.tasks.get_mut(id)?;
        change(&mut entry.snapshot);
        Some(entry.snapshot.clone())
    }

    fn take_cancel(&self, id: &str) -> Option<u32> {
        let mut inner = self.inner.lock().unwrap();
        let entry = inner.tasks.get_mut(id)?;
        entry.canceled = true;
        entry.pid
    }

    fn is_canceled(&self, id: &str) -> bool {
        self.inner
            .lock()
            .unwrap()
            .tasks
            .get(id)
            .is_some_and(|entry| entry.canceled)
    }

    fn finish(&self, id: &str, state: &str, error: Option<String>) {
        let _ = self.update(id, |snapshot| {
            snapshot.state = state.to_string();
            snapshot.error = error;
            if state == STATE_DONE {
                snapshot.percent = 100.0;
            }
        });
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

    fn finish_running(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.running = inner.running.saturating_sub(1);
    }
}

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
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchRequest {
    pub path: String,
    pub mode: String,
    pub quality: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchEntry {
    pub url: String,
    pub clip: Option<ClipSpec>,
}

pub fn parse_modes(raw: &str) -> Result<Vec<Mode>, String> {
    match raw {
        "audio" => Ok(vec![Mode::Audio]),
        "video" => Ok(vec![Mode::Video]),
        "both" => Ok(vec![Mode::Audio, Mode::Video]),
        other => Err(i18n::tf(
            || format!("неизвестный режим: {other}"),
            || format!("unknown mode: {other}"),
        )),
    }
}

pub fn parse_quality(raw: Option<&str>) -> Result<Quality, String> {
    match raw.unwrap_or("1080") {
        "2160" => Ok(Quality::P2160),
        "1440" => Ok(Quality::P1440),
        "1080" => Ok(Quality::P1080),
        "720" => Ok(Quality::P720),
        "480" => Ok(Quality::P480),
        "best" => Ok(Quality::Best),
        other => Err(i18n::tf(
            || format!("неизвестное качество: {other}"),
            || format!("unknown quality: {other}"),
        )),
    }
}

pub fn parse_audio_kbps(raw: Option<u32>) -> Result<u32, String> {
    let kbps = raw.unwrap_or(320);
    if [128, 192, 256, 320].contains(&kbps) {
        Ok(kbps)
    } else {
        Err(i18n::tf(
            || format!("неизвестный битрейт аудио: {kbps}"),
            || format!("unknown audio bitrate: {kbps}"),
        ))
    }
}

pub fn parse_clip(start: Option<&str>, end: Option<&str>) -> Result<Option<ClipSpec>, String> {
    match (start.map(str::trim), end.map(str::trim)) {
        (None, None) | (Some(""), Some("")) => Ok(None),
        (Some(start), Some(end)) if !start.is_empty() && !end.is_empty() => {
            ClipSpec::parse(start, end)
                .map(Some)
                .map_err(|error| error.to_string())
        }
        _ => Err(i18n::t(
            "Укажите начало и конец клипа",
            "Specify both clip start and end",
        )),
    }
}

pub fn validate_url(raw: &str) -> Result<String, String> {
    let url = raw.trim();
    if url.is_empty() {
        return Err(i18n::t("Вставьте ссылку", "Paste a link"));
    }
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err(i18n::t(
            "Ссылка должна начинаться с http:// или https://",
            "The link must start with http:// or https://",
        ));
    }
    Ok(url.to_string())
}

/// Parses a `.txt` list: one URL per line, optionally followed by
/// `HH:MM:SS-HH:MM:SS` (the format used by `urls-clips.txt`).
pub fn parse_list(text: &str) -> Result<Vec<BatchEntry>, String> {
    // Notepad writes UTF-8 with BOM; U+FEFF is not whitespace, so it must be
    // stripped before the first line is parsed.
    let text = text.trim_start_matches('\u{feff}');
    let mut entries = Vec::new();

    for (index, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split_whitespace();
        let url = parts.next().ok_or_else(|| {
            i18n::tf(
                || format!("строка {}: пустая ссылка", index + 1),
                || format!("line {}: empty link", index + 1),
            )
        })?;
        let url = validate_url(url).map_err(|error| {
            i18n::tf(
                || format!("строка {}: {error}", index + 1),
                || format!("line {}: {error}", index + 1),
            )
        })?;

        let clip = match parts.next() {
            None => None,
            Some(range) => {
                let (start, end) = range.split_once('-').ok_or_else(|| {
                    i18n::tf(
                        || {
                            format!(
                                "строка {}: таймкод должен быть вида 00:01:30-00:05:00",
                                index + 1
                            )
                        },
                        || {
                            format!(
                                "line {}: timecode must look like 00:01:30-00:05:00",
                                index + 1
                            )
                        },
                    )
                })?;
                let clip = ClipSpec::parse(start, end).map_err(|error| {
                    i18n::tf(
                        || format!("строка {}: {error}", index + 1),
                        || format!("line {}: {error}", index + 1),
                    )
                })?;
                Some(clip)
            }
        };

        entries.push(BatchEntry { url, clip });

        if entries.len() > MAX_BATCH_LINES {
            return Err(i18n::tf(
                || format!("в файле больше {MAX_BATCH_LINES} ссылок"),
                || format!("the file contains more than {MAX_BATCH_LINES} links"),
            ));
        }
    }

    if entries.is_empty() {
        return Err(i18n::t(
            "в файле не найдено ссылок",
            "no links found in the file",
        ));
    }
    Ok(entries)
}

fn emit_state(app: &AppHandle, snapshot: &TaskSnapshot) {
    let _ = app.emit("task://state", snapshot);
}

fn emit_queue(app: &AppHandle) {
    let state = app.state::<Arc<TaskStore>>().queue_state();
    let _ = app.emit("queue://changed", state);
}

fn folder_error(error: std::io::Error) -> String {
    i18n::tf(
        || format!("нет доступа к папке: {error}"),
        || format!("cannot access the folder: {error}"),
    )
}

fn enqueue_for_mode(
    app: &AppHandle,
    url: &str,
    mode: Mode,
    quality: Quality,
    audio_kbps: u32,
    clip: Option<&ClipSpec>,
    out_dir_override: Option<&Path>,
) -> Result<TaskSnapshot, String> {
    let paths = app.state::<PortablePaths>();
    let store = app.state::<Arc<TaskStore>>();
    let settings = app.state::<SettingsStore>().get();

    let out_dir = match out_dir_override {
        Some(path) => path.to_path_buf(),
        None => {
            let subdir = match mode {
                Mode::Audio => &settings.audio_subdir,
                Mode::Video => &settings.video_subdir,
            };
            paths.root().join(subdir)
        }
    };
    std::fs::create_dir_all(&out_dir).map_err(folder_error)?;

    let archive = paths.data().join(args::archive_file(mode, clip.is_some()));
    std::fs::create_dir_all(paths.data()).map_err(folder_error)?;

    let thumbs = paths.thumbs();
    std::fs::create_dir_all(&thumbs).map_err(folder_error)?;

    let argv = args::build(&ArgsRequest {
        url,
        mode,
        quality,
        audio_kbps,
        clip,
        out_dir: &out_dir,
        archive: &archive,
        ffmpeg_dir: &paths.bin(),
        thumbs_dir: Some(&thumbs),
    })
    .map_err(|error| error.to_string())?;

    let id = store.next_id();
    let snapshot = store.insert(
        TaskSnapshot {
            id,
            url: url.to_string(),
            mode: match mode {
                Mode::Audio => "audio".to_string(),
                Mode::Video => "video".to_string(),
            },
            clip: clip.is_some(),
            state: STATE_QUEUED.to_string(),
            percent: 0.0,
            speed_bps: None,
            eta_secs: None,
            error: None,
        },
        argv,
    );
    emit_state(app, &snapshot);
    Ok(snapshot)
}

/// Starts queued tasks while concurrency allows and the queue is not paused.
fn pump(app: &AppHandle) {
    let store = app.state::<Arc<TaskStore>>().inner().clone();
    let exe: PathBuf = app.state::<PortablePaths>().ytdlp();
    let mut started_any = false;

    loop {
        let job = {
            let mut inner = store.inner.lock().unwrap();
            if inner.paused || inner.running >= inner.max_concurrent {
                break;
            }
            let Some(id) = inner.queue.pop_front() else {
                break;
            };
            let usable = inner.tasks.get(&id).is_some_and(|entry| !entry.canceled);
            if !usable {
                continue;
            }
            let (argv, snapshot) = {
                let entry = inner.tasks.get_mut(&id).unwrap();
                entry.snapshot.state = STATE_RUNNING.to_string();
                (entry.argv.clone(), entry.snapshot.clone())
            };
            inner.running += 1;
            (id, argv, snapshot)
        };

        let (id, argv, snapshot) = job;
        started_any = true;
        emit_state(app, &snapshot);
        let app_for_task = app.clone();
        let exe = exe.clone();
        tauri::async_runtime::spawn(async move {
            run_task(app_for_task, id, exe, argv).await;
        });
    }

    if started_any {
        emit_queue(app);
    }
}

fn task_finished(app: &AppHandle) {
    app.state::<Arc<TaskStore>>().finish_running();
    emit_queue(app);
    pump(app);

    if crate::updater::maybe_install_when_idle(app) {
        return;
    }

    let queue = app.state::<Arc<TaskStore>>().queue_state();
    if queue.running == 0 && queue.queued == 0 {
        let tasks = app.state::<Arc<TaskStore>>().all();
        let done = tasks.iter().filter(|task| task.state == STATE_DONE).count();
        let failed = tasks
            .iter()
            .filter(|task| task.state == STATE_FAILED)
            .count();
        if done + failed > 0 {
            let body = if failed == 0 {
                i18n::tf(|| format!("Готово: {done}"), || format!("Done: {done}"))
            } else {
                i18n::tf(
                    || format!("Готово: {done}, ошибок: {failed}"),
                    || format!("Done: {done}, errors: {failed}"),
                )
            };
            let _ = app
                .notification()
                .builder()
                .title("Klipparkiv")
                .body(body)
                .show();
        }
    }
}

#[tauri::command]
pub fn enqueue_task(app: AppHandle, task: NewTask) -> Result<Vec<TaskSnapshot>, String> {
    let modes = parse_modes(&task.mode)?;
    let quality = parse_quality(task.quality.as_deref())?;
    let audio_kbps = parse_audio_kbps(task.audio_kbps)?;
    let url = validate_url(&task.url)?;
    let clip = parse_clip(task.clip_start.as_deref(), task.clip_end.as_deref())?;

    let out_dir = match task
        .out_dir
        .as_deref()
        .map(str::trim)
        .filter(|dir| !dir.is_empty())
    {
        Some(dir) => {
            let path = PathBuf::from(dir);
            if !path.is_absolute() {
                return Err(i18n::t(
                    "Папка назначения должна быть абсолютным путём",
                    "The destination folder must be an absolute path",
                ));
            }
            if !path.is_dir() {
                return Err(i18n::t(
                    "Папка назначения не найдена",
                    "The destination folder was not found",
                ));
            }
            Some(path)
        }
        None => None,
    };

    let mut created = Vec::with_capacity(modes.len());
    for mode in modes {
        created.push(enqueue_for_mode(
            &app,
            &url,
            mode,
            quality,
            audio_kbps,
            clip.as_ref(),
            out_dir.as_deref(),
        )?);
    }

    emit_queue(&app);
    pump(&app);
    Ok(created)
}

#[tauri::command]
pub fn enqueue_batch(app: AppHandle, request: BatchRequest) -> Result<Vec<TaskSnapshot>, String> {
    let modes = parse_modes(&request.mode)?;
    let quality = parse_quality(request.quality.as_deref())?;

    let text = std::fs::read(&request.path).map_err(|error| {
        i18n::tf(
            || format!("не удалось прочитать файл: {error}"),
            || format!("could not read the file: {error}"),
        )
    })?;
    let entries = parse_list(&String::from_utf8_lossy(&text))?;

    let mut created = Vec::new();
    for entry in &entries {
        for &mode in &modes {
            created.push(enqueue_for_mode(
                &app,
                &entry.url,
                mode,
                quality,
                320,
                entry.clip.as_ref(),
                None,
            )?);
        }
    }

    emit_queue(&app);
    pump(&app);
    Ok(created)
}

#[tauri::command]
pub fn pause_queue(app: AppHandle) -> QueueState {
    let state = app.state::<Arc<TaskStore>>().set_paused(true);
    let _ = app.emit("queue://changed", &state);
    state
}

#[tauri::command]
pub fn resume_queue(app: AppHandle) -> QueueState {
    let state = app.state::<Arc<TaskStore>>().set_paused(false);
    let _ = app.emit("queue://changed", &state);
    pump(&app);
    state
}

/// Tray menu action: flips the pause flag.
pub fn toggle_pause(app: &AppHandle) -> QueueState {
    let store = app.state::<Arc<TaskStore>>();
    let paused = !store.queue_state().paused;
    let state = store.set_paused(paused);
    let _ = app.emit("queue://changed", &state);
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
pub async fn probe_metadata(app: AppHandle, url: String) -> Result<metadata::VideoInfo, String> {
    let url = validate_url(&url)?;
    let exe = {
        let paths = app.state::<PortablePaths>();
        paths.ytdlp()
    };
    metadata::probe(&exe, &url).await
}

async fn run_task(app: AppHandle, id: String, exe: PathBuf, argv: Vec<OsString>) {
    let store = app.state::<Arc<TaskStore>>().inner().clone();
    let stderr_tail: Arc<Mutex<VecDeque<String>>> = Arc::new(Mutex::new(VecDeque::new()));

    let spawn_result = process::prepare(&exe, &argv).spawn();
    let mut child = match spawn_result {
        Ok(child) => child,
        Err(error) => {
            store.finish(
                &id,
                STATE_FAILED,
                Some(i18n::tf(
                    || format!("не удалось запустить yt-dlp: {error}"),
                    || format!("could not start yt-dlp: {error}"),
                )),
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
        store.finish(&id, STATE_CANCELED, None);
    } else if matches!(&status, Ok(s) if s.success()) {
        store.finish(&id, STATE_DONE, None);
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
        store.finish(&id, STATE_FAILED, Some(reason));
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
    while let Ok(Some(line)) = lines.next_line().await {
        if let Some(parsed) = progress::parse_line(&line) {
            let _ = store.update(&id, |snapshot| {
                snapshot.percent = parsed.percent;
                snapshot.speed_bps = parsed.speed_bps;
                snapshot.eta_secs = parsed.eta_secs;
            });
            let _ = app.emit(
                "task://progress",
                &ProgressEvent {
                    id: id.clone(),
                    percent: parsed.percent,
                    speed_bps: parsed.speed_bps,
                    eta_secs: parsed.eta_secs,
                },
            );
        } else {
            let _ = app.emit(
                "task://log",
                &LogEvent {
                    id: id.clone(),
                    line,
                },
            );
        }
    }
}

async fn read_stderr(
    app: AppHandle,
    _store: Arc<TaskStore>,
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
        let _ = app.emit(
            "task://log",
            &LogEvent {
                id: id.clone(),
                line,
            },
        );
    }
}

#[tauri::command]
pub async fn cancel_task(app: AppHandle, id: String) -> Result<(), String> {
    let store = app.state::<Arc<TaskStore>>();
    let pid = store.take_cancel(&id);

    match pid {
        Some(pid) => {
            process::kill_tree(pid).await;
        }
        None => {
            store.finish(&id, STATE_CANCELED, None);
            if let Some(snapshot) = store.snapshot(&id) {
                emit_state(&app, &snapshot);
            }
            emit_queue(&app);
            pump(&app);
            crate::updater::maybe_install_when_idle(&app);
        }
    }
    Ok(())
}

#[tauri::command]
pub fn list_tasks(store: State<'_, Arc<TaskStore>>) -> Vec<TaskSnapshot> {
    store.all()
}

#[tauri::command]
pub fn get_settings(store: State<'_, SettingsStore>) -> Settings {
    store.get()
}

#[tauri::command]
pub fn save_settings(app: AppHandle, settings: Settings) -> Result<Settings, String> {
    let saved = app.state::<SettingsStore>().save(settings)?;
    app.state::<Arc<TaskStore>>()
        .set_concurrency(saved.concurrency);
    crate::refresh_tray_language(&app);
    Ok(saved)
}

/// Self-update of the bundled yt-dlp (`yt-dlp --update`).
#[tauri::command]
pub async fn update_ytdlp(app: AppHandle) -> Result<String, String> {
    let exe = {
        let paths = app.state::<PortablePaths>();
        paths.ytdlp()
    };
    let output = process::prepare(&exe, &[OsString::from("--update")])
        .output()
        .await
        .map_err(|error| {
            i18n::tf(
                || format!("не удалось запустить обновление: {error}"),
                || format!("could not start the update: {error}"),
            )
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let reason = stderr
            .lines()
            .rev()
            .find(|line| !line.trim().is_empty())
            .map(str::to_owned)
            .unwrap_or_else(|| i18n::t("yt-dlp не обновился", "yt-dlp was not updated"));
        return Err(reason.trim().to_string());
    }

    crate::binaries::read_version(&exe, "--version")
        .as_deref()
        .and_then(crate::binaries::parse_ytdlp_version)
        .ok_or_else(|| {
            i18n::t(
                "версия после обновления не определена",
                "could not detect the version after the update",
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_http_urls() {
        assert_eq!(
            validate_url("  https://youtu.be/abc  ").unwrap(),
            "https://youtu.be/abc"
        );
        assert!(validate_url("").is_err());
        assert!(validate_url("youtu.be/abc").is_err());
        assert!(validate_url("file:///etc/passwd").is_err());
    }

    #[test]
    fn parses_modes() {
        assert_eq!(parse_modes("audio").unwrap(), vec![Mode::Audio]);
        assert_eq!(parse_modes("video").unwrap(), vec![Mode::Video]);
        assert_eq!(parse_modes("both").unwrap(), vec![Mode::Audio, Mode::Video]);
        assert!(parse_modes("playlist").is_err());
    }

    #[test]
    fn parses_quality_and_audio_bitrate() {
        assert_eq!(parse_quality(None).unwrap(), Quality::P1080);
        assert_eq!(parse_quality(Some("2160")).unwrap(), Quality::P2160);
        assert_eq!(parse_quality(Some("1440")).unwrap(), Quality::P1440);
        assert_eq!(parse_quality(Some("480")).unwrap(), Quality::P480);
        assert_eq!(parse_quality(Some("best")).unwrap(), Quality::Best);
        assert!(parse_quality(Some("4k")).is_err());

        assert_eq!(parse_audio_kbps(None).unwrap(), 320);
        assert_eq!(parse_audio_kbps(Some(128)).unwrap(), 128);
        assert!(parse_audio_kbps(Some(64)).is_err());
    }

    #[test]
    fn parses_clip_range() {
        let clip = parse_clip(Some("1:30"), Some("2:00")).unwrap().unwrap();
        assert_eq!(clip.start, "00:01:30");
        assert_eq!(clip.end, "00:02:00");

        assert!(parse_clip(None, None).unwrap().is_none());
        assert!(parse_clip(Some(""), Some("")).unwrap().is_none());
        assert!(parse_clip(Some("1:30"), None).is_err());
        assert!(parse_clip(Some("bad"), Some("2:00")).is_err());
    }

    #[test]
    fn parses_batch_list_with_comments_and_timecodes() {
        let text = "\
# мои ссылки
https://youtu.be/a
https://youtu.be/b 00:01:30-00:05:00

  https://youtu.be/c  1:00-2:00
";
        let entries = parse_list(text).unwrap();

        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].clip, None);
        assert_eq!(entries[1].clip.as_ref().unwrap().start, "00:01:30");
        assert_eq!(entries[2].url, "https://youtu.be/c");
        assert_eq!(entries[2].clip.as_ref().unwrap().end, "00:02:00");
    }

    #[test]
    fn parses_batch_list_with_bom() {
        let text = "\u{feff}# список\nhttps://youtu.be/a\n";
        let entries = parse_list(text).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].url, "https://youtu.be/a");
    }

    #[test]
    fn rejects_broken_batch_lines() {
        assert!(parse_list("not-a-link").is_err());
        assert!(parse_list("https://youtu.be/a broken-range").is_err());
        assert!(parse_list("# only comments").is_err());
        assert!(parse_list("").is_err());
    }

    #[test]
    fn store_assigns_unique_ids_and_updates_state() {
        let store = TaskStore::default();
        let first = store.insert(
            TaskSnapshot {
                id: store.next_id(),
                url: "https://example.com".into(),
                mode: "audio".into(),
                clip: false,
                state: STATE_QUEUED.into(),
                percent: 0.0,
                speed_bps: None,
                eta_secs: None,
                error: None,
            },
            Vec::new(),
        );
        let second_id = store.next_id();
        assert_ne!(first.id, second_id);

        let _ = store.update(&first.id, |snapshot| snapshot.percent = 42.0);
        store.finish(&first.id, STATE_DONE, None);

        let snapshot = store.snapshot(&first.id).unwrap();
        assert_eq!(snapshot.percent, 100.0);
        assert_eq!(snapshot.state, STATE_DONE);
        assert!(!store.is_canceled(&first.id));
    }

    #[test]
    fn queue_state_tracks_pause_and_pending() {
        let store = TaskStore::default();
        let first = store.insert(
            TaskSnapshot {
                id: store.next_id(),
                url: "https://example.com/1".into(),
                mode: "audio".into(),
                clip: false,
                state: STATE_QUEUED.into(),
                percent: 0.0,
                speed_bps: None,
                eta_secs: None,
                error: None,
            },
            Vec::new(),
        );

        let state = store.queue_state();
        assert!(!state.paused);
        assert_eq!(state.queued, 1);
        assert_eq!(state.running, 0);

        store.set_paused(true);
        assert!(store.queue_state().paused);

        store.take_cancel(&first.id);
        assert_eq!(store.queue_state().queued, 0);
    }
}
