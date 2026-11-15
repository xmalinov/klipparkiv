use std::path::{Path, PathBuf};
use std::sync::Arc;

use tauri::{AppHandle, Manager};
use tauri_plugin_notification::NotificationExt;

use crate::args::{self, ArgsRequest, ClipSpec, Mode, PlaylistSpec, Quality};
use crate::i18n;
use crate::paths::AppPaths;
use crate::queue::events::{emit_queue, emit_state};
use crate::queue::runner::run_task;
use crate::queue::store::{TaskMode, TaskSnapshot, TaskState, TaskStore};
use crate::settings::SettingsStore;
fn folder_error(error: std::io::Error) -> String {
    i18n::tf(
        || format!("нет доступа к папке: {error}"),
        || format!("cannot access the folder: {error}"),
    )
}

pub(crate) struct Job<'a> {
    pub(crate) url: &'a str,
    pub(crate) mode: Mode,
    pub(crate) quality: Quality,
    pub(crate) audio_kbps: u32,
    pub(crate) clip: Option<&'a ClipSpec>,
    pub(crate) playlist: Option<PlaylistSpec>,
    pub(crate) out_dir: Option<&'a Path>,
    pub(crate) title: Option<&'a str>,
}

pub(crate) fn enqueue_job(app: &AppHandle, job: Job<'_>) -> Result<TaskSnapshot, String> {
    let paths = app.state::<AppPaths>();
    let store = app.state::<Arc<TaskStore>>();
    let settings = app.state::<SettingsStore>().get();

    let out_dir = match job.out_dir {
        Some(path) => path.to_path_buf(),
        None => {
            let subdir = match job.mode {
                Mode::Audio => &settings.audio_subdir,
                Mode::Video => &settings.video_subdir,
            };
            paths.root().join(subdir)
        }
    };
    std::fs::create_dir_all(&out_dir).map_err(folder_error)?;

    let archive = paths
        .data()
        .join(args::archive_file(job.mode, job.clip.is_some()));
    std::fs::create_dir_all(paths.data()).map_err(folder_error)?;

    let thumbs = paths.thumbs();
    std::fs::create_dir_all(&thumbs).map_err(folder_error)?;

    let argv = args::build(&ArgsRequest {
        url: job.url,
        mode: job.mode,
        quality: job.quality,
        audio_kbps: job.audio_kbps,
        clip: job.clip,
        playlist: job.playlist,
        out_dir: &out_dir,
        archive: &archive,
        ffmpeg_dir: &paths.bin(),
        thumbs_dir: Some(&thumbs),
        embed_thumbnail: settings.embed_thumbnail,
    })
    .map_err(|error| error.to_string())?;

    let id = store.next_id();
    let title = job
        .title
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let snapshot = store.insert(
        TaskSnapshot {
            id,
            url: job.url.to_string(),
            title,
            mode: match job.mode {
                Mode::Audio => TaskMode::Audio,
                Mode::Video => TaskMode::Video,
            },
            clip: job.clip.is_some(),
            state: TaskState::Queued,
            percent: 0.0,
            speed_bps: None,
            eta_secs: None,
            error: None,
            playlist_index: None,
            playlist_count: job.playlist.map(|_| 0),
            output_path: None,
        },
        argv,
    );
    emit_state(app, &snapshot);
    Ok(snapshot)
}

/// Starts queued tasks while concurrency allows and the queue is not paused.
pub(crate) fn pump(app: &AppHandle) {
    let store = app.state::<Arc<TaskStore>>().inner().clone();
    let exe: PathBuf = app.state::<AppPaths>().ytdlp();
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
                entry.snapshot.state = TaskState::Running;
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

pub(crate) fn task_finished(app: &AppHandle) {
    app.state::<Arc<TaskStore>>().finish_running();
    emit_queue(app);
    pump(app);

    if crate::update::maybe_install_when_idle(app) {
        return;
    }

    let queue = app.state::<Arc<TaskStore>>().queue_state();
    if queue.running == 0 && queue.queued == 0 {
        let tasks = app.state::<Arc<TaskStore>>().all();
        let done = tasks
            .iter()
            .filter(|task| task.state == TaskState::Done)
            .count();
        let skipped = tasks
            .iter()
            .filter(|task| task.state == TaskState::Skipped)
            .count();
        let failed = tasks
            .iter()
            .filter(|task| task.state == TaskState::Failed || task.state == TaskState::Partial)
            .count();
        if done + skipped + failed > 0 {
            let body = if failed == 0 && skipped == 0 {
                i18n::tf(|| format!("Готово: {done}"), || format!("Done: {done}"))
            } else if failed == 0 {
                i18n::tf(
                    || format!("Готово: {done}, уже было: {skipped}"),
                    || format!("Done: {done}, already present: {skipped}"),
                )
            } else {
                i18n::tf(
                    || format!("Готово: {done}, уже было: {skipped}, ошибок: {failed}"),
                    || format!("Done: {done}, already present: {skipped}, errors: {failed}"),
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
