use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};

use crate::i18n;
use crate::library::LibraryPaths;
use crate::library::copy::{
    CopyFailure, copy_with_progress, hash_cached, sanitize_file_name, set_modified_now,
    unique_target,
};
use crate::library::db::Library;
use crate::library::guard::kind_for;
use crate::library::model::{
    ImportFailure, ImportProgress, ImportReport, ImportSkip, ImportSkipReason, MediaKind, MediaRow,
};
use crate::library::scan::{ProbeInfo, probe_info};
use crate::library::thumbs::extract_thumb;
use crate::library::util::{mtime_secs, now_secs};

/// One import job at a time. The cancellation flag is cooperative: the copy
/// loop checks it after every chunk, the pipeline checks it between files.
pub struct ImportState {
    job: Mutex<Option<ImportJob>>,
    next_id: AtomicU64,
}

struct ImportJob {
    id: u64,
    cancel: std::sync::Arc<AtomicBool>,
}

impl Default for ImportState {
    fn default() -> Self {
        Self {
            job: Mutex::new(None),
            next_id: AtomicU64::new(1),
        }
    }
}

impl ImportState {
    pub fn begin(&self) -> Result<(u64, std::sync::Arc<AtomicBool>), String> {
        let mut slot = self.job.lock().map_err(|error| error.to_string())?;
        if slot.is_some() {
            return Err(i18n::t(
                "импорт уже выполняется",
                "an import is already in progress",
            ));
        }
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let cancel = std::sync::Arc::new(AtomicBool::new(false));
        *slot = Some(ImportJob {
            id,
            cancel: cancel.clone(),
        });
        Ok((id, cancel))
    }

    pub fn cancel(&self) -> bool {
        let Ok(slot) = self.job.lock() else {
            return false;
        };
        match slot.as_ref() {
            Some(job) => {
                job.cancel.store(true, Ordering::SeqCst);
                true
            }
            None => false,
        }
    }

    pub fn finish(&self, id: u64) {
        if let Ok(mut slot) = self.job.lock()
            && slot.as_ref().is_some_and(|job| job.id == id)
        {
            *slot = None;
        }
    }
}

fn media_row_for(
    path: &Path,
    kind: MediaKind,
    info: ProbeInfo,
    thumb_path: Option<String>,
    origin: &str,
) -> Result<MediaRow, String> {
    let meta = fs::metadata(path).map_err(|error| error.to_string())?;
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(str::to_owned)
        .unwrap_or_else(|| i18n::t("Без названия", "Untitled"));
    let title = info
        .title
        .as_deref()
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .map(str::to_owned)
        .unwrap_or(stem);
    Ok(MediaRow {
        path: path.to_string_lossy().into_owned(),
        kind,
        title,
        channel: info.artist,
        duration_secs: info.duration,
        width: info.width,
        height: info.height,
        size_bytes: meta.len(),
        mtime: mtime_secs(path),
        thumb_path,
        origin: origin.to_string(),
        indexed_at: now_secs(),
    })
}

/// Copies every dropped file into `audio/` or `video/`, deduplicates by
/// content, extracts tags/covers/posters and writes the row immediately so the
/// UI does not need a rescan. Errors are per file; only an unusable root aborts.
#[allow(clippy::too_many_arguments)]
pub(crate) fn import_media(
    library: &Library,
    layout: &LibraryPaths,
    ffprobe: &Path,
    ffmpeg: &Path,
    paths: Vec<String>,
    cancel: &AtomicBool,
    job_id: u64,
    mut on_progress: impl FnMut(ImportProgress),
) -> Result<ImportReport, String> {
    let canonical_root = fs::canonicalize(layout.root()).map_err(|error| error.to_string())?;
    let audio_dir = layout.audio();
    let video_dir = layout.video();
    let thumbs = layout.thumbs();
    fs::create_dir_all(&audio_dir).map_err(|error| error.to_string())?;
    fs::create_dir_all(&video_dir).map_err(|error| error.to_string())?;
    let _ = fs::create_dir_all(&thumbs);

    let mut report = ImportReport::default();
    let total = paths.len() as u32;
    let mut taken_audio: HashSet<String> = HashSet::new();
    let mut taken_video: HashSet<String> = HashSet::new();
    let mut batch_seen: HashSet<String> = HashSet::new();
    let mut batch: Vec<(String, u64, Option<String>)> = Vec::new();
    let mut hash_cache: HashMap<String, String> = HashMap::new();

    'files: for (position, raw) in paths.iter().enumerate() {
        if cancel.load(Ordering::SeqCst) {
            report.cancelled = true;
            for path in &paths[position..] {
                report.skipped.push(ImportSkip {
                    path: path.clone(),
                    reason: ImportSkipReason::Cancelled,
                });
            }
            break 'files;
        }

        let source = Path::new(raw);
        let canonical = match fs::canonicalize(source) {
            Ok(path) => path,
            Err(error) => {
                report.failed.push(ImportFailure {
                    path: raw.clone(),
                    error: error.to_string(),
                });
                continue;
            }
        };
        let Ok(meta) = fs::metadata(&canonical) else {
            report.failed.push(ImportFailure {
                path: raw.clone(),
                error: i18n::t("файл не найден", "the file was not found").to_string(),
            });
            continue;
        };
        if meta.is_dir() {
            report.skipped.push(ImportSkip {
                path: raw.clone(),
                reason: ImportSkipReason::Folder,
            });
            continue;
        }
        let Some(kind) = kind_for(&canonical) else {
            report.skipped.push(ImportSkip {
                path: raw.clone(),
                reason: ImportSkipReason::Unsupported,
            });
            continue;
        };
        let size = meta.len();
        let canonical_text = canonical.to_string_lossy().into_owned();
        if !batch_seen.insert(canonical_text.clone()) {
            report.skipped.push(ImportSkip {
                path: raw.clone(),
                reason: ImportSkipReason::DuplicateInBatch,
            });
            continue;
        }

        // A file that already lives inside the app folder is not copied.
        // Keep the OS-supplied path for the DB key: on Windows `canonicalize`
        // adds the `\\?\` prefix, which would not match rows written by the
        // scanner.
        if canonical.starts_with(&canonical_root) {
            let inside = if source.is_absolute() {
                source.to_path_buf()
            } else {
                canonical.clone()
            };
            let inside_text = inside.to_string_lossy().into_owned();
            match library.media_get(&inside_text) {
                Ok(Some(item)) => {
                    report.existing.push(item);
                }
                Ok(None) => {
                    let info = probe_info(ffprobe, &inside, kind == MediaKind::Video);
                    match media_row_for(&inside, kind, info, None, "import").and_then(|row| {
                        library.media_upsert(&row)?;
                        Ok(row)
                    }) {
                        Ok(row) => report.imported.push(row.into_item()),
                        Err(error) => report.failed.push(ImportFailure {
                            path: raw.clone(),
                            error,
                        }),
                    }
                }
                Err(error) => report.failed.push(ImportFailure {
                    path: raw.clone(),
                    error,
                }),
            }
            continue;
        }

        let display_name = source
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_owned)
            .unwrap_or_else(|| raw.clone());
        let name = sanitize_file_name(&display_name, kind.as_str());

        // Duplicate detection: hash only when some indexed/batched file has the
        // same size. Unique files never pay for a full read.
        let db_candidates = library.media_by_size(size).unwrap_or_default();
        let batch_has_same_size = batch.iter().any(|(_, batch_size, _)| *batch_size == size);
        let mut source_hash: Option<String> = None;
        let mut duplicate: Option<String> = None;
        if !db_candidates.is_empty() || batch_has_same_size {
            source_hash = hash_cached(&mut hash_cache, &canonical);
            if let Some(hash) = source_hash.as_deref() {
                for candidate in &db_candidates {
                    if candidate == &canonical_text {
                        continue;
                    }
                    if hash_cached(&mut hash_cache, Path::new(candidate)).as_deref() == Some(hash) {
                        duplicate = Some(candidate.clone());
                        break;
                    }
                }
                if duplicate.is_none() {
                    for (candidate, batch_size, _) in &batch {
                        if *batch_size == size
                            && candidate != &canonical_text
                            && hash_cached(&mut hash_cache, Path::new(candidate)).as_deref()
                                == Some(hash)
                        {
                            duplicate = Some(candidate.clone());
                            break;
                        }
                    }
                }
            }
        }
        if let Some(candidate) = duplicate {
            if let Ok(Some(item)) = library.media_get(&candidate) {
                report.existing.push(item);
            } else if let Some(item) = report
                .imported
                .iter()
                .find(|item| item.path == candidate)
                .cloned()
            {
                report.existing.push(item);
            } else {
                report.skipped.push(ImportSkip {
                    path: raw.clone(),
                    reason: ImportSkipReason::AlreadyInLibrary,
                });
            }
            continue;
        }

        let target = {
            let (dir, taken) = if kind == MediaKind::Audio {
                (&audio_dir, &mut taken_audio)
            } else {
                (&video_dir, &mut taken_video)
            };
            match unique_target(dir, &name, taken) {
                Ok(target) => target,
                Err(error) => {
                    report.failed.push(ImportFailure {
                        path: raw.clone(),
                        error,
                    });
                    continue;
                }
            }
        };
        let mut part = target.clone().into_os_string();
        part.push(".part");
        let part = PathBuf::from(part);
        let _ = fs::remove_file(&part);

        on_progress(ImportProgress {
            job_id,
            index: position as u32 + 1,
            total,
            name: display_name.clone(),
            copied_bytes: 0,
            total_bytes: size,
            phase: "copying".into(),
        });
        let mut last_emit = Instant::now();
        let copy_result = copy_with_progress(&canonical, &part, size, cancel, |copied| {
            if copied < size && last_emit.elapsed() < Duration::from_millis(250) {
                return;
            }
            last_emit = Instant::now();
            on_progress(ImportProgress {
                job_id,
                index: position as u32 + 1,
                total,
                name: display_name.clone(),
                copied_bytes: copied,
                total_bytes: size,
                phase: "copying".into(),
            });
        });
        match copy_result {
            Ok(_) => {}
            Err(CopyFailure::Cancelled) => {
                let _ = fs::remove_file(&part);
                report.cancelled = true;
                for path in &paths[position..] {
                    report.skipped.push(ImportSkip {
                        path: path.clone(),
                        reason: ImportSkipReason::Cancelled,
                    });
                }
                break 'files;
            }
            Err(CopyFailure::Failed(error)) => {
                let _ = fs::remove_file(&part);
                report.failed.push(ImportFailure {
                    path: raw.clone(),
                    error,
                });
                continue;
            }
        }

        if let Err(error) = set_modified_now(&part) {
            let _ = fs::remove_file(&part);
            report.failed.push(ImportFailure {
                path: raw.clone(),
                error: error.to_string(),
            });
            continue;
        }
        if let Err(error) = fs::rename(&part, &target) {
            let _ = fs::remove_file(&part);
            report.failed.push(ImportFailure {
                path: raw.clone(),
                error: error.to_string(),
            });
            continue;
        }

        on_progress(ImportProgress {
            job_id,
            index: position as u32 + 1,
            total,
            name: display_name.clone(),
            copied_bytes: size,
            total_bytes: size,
            phase: "probing".into(),
        });
        let info = probe_info(ffprobe, &target, kind == MediaKind::Video);

        let wants_thumb =
            (kind == MediaKind::Audio && info.attached_pic) || kind == MediaKind::Video;
        let thumb_path = if wants_thumb {
            let hash = hex::encode(Sha256::digest(target.to_string_lossy().as_bytes()));
            let out = thumbs.join(format!("import-{}.jpg", &hash[..8]));
            let timestamp = if kind == MediaKind::Video {
                info.duration
                    .map(|duration| (duration * 0.1).min(3.0))
                    .or(Some(0.5))
            } else {
                None
            };
            let _ = on_progress_phase(
                &mut on_progress,
                job_id,
                position as u32 + 1,
                total,
                &display_name,
                size,
                "thumbnailing",
            );
            if extract_thumb(ffmpeg, &target, &out, timestamp) {
                Some(out.to_string_lossy().into_owned())
            } else {
                let _ = fs::remove_file(&out);
                None
            }
        } else {
            None
        };

        let row = match media_row_for(&target, kind, info, thumb_path, "import") {
            Ok(row) => row,
            Err(error) => {
                report.failed.push(ImportFailure {
                    path: raw.clone(),
                    error,
                });
                continue;
            }
        };
        if let Err(error) = library.media_upsert(&row) {
            report.failed.push(ImportFailure {
                path: raw.clone(),
                error,
            });
            continue;
        }
        // Store the destination path: later duplicates in the same batch must
        // resolve to the indexed row, not to the external source.
        batch.push((target.to_string_lossy().into_owned(), size, source_hash));
        report.imported.push(row.into_item());
        let _ = on_progress_phase(
            &mut on_progress,
            job_id,
            position as u32 + 1,
            total,
            &display_name,
            size,
            "done",
        );
    }

    Ok(report)
}

fn on_progress_phase(
    on_progress: &mut impl FnMut(ImportProgress),
    job_id: u64,
    index: u32,
    total: u32,
    name: &str,
    size: u64,
    phase: &str,
) -> Option<()> {
    on_progress(ImportProgress {
        job_id,
        index,
        total,
        name: name.to_string(),
        copied_bytes: size,
        total_bytes: size,
        phase: phase.to_string(),
    });
    Some(())
}
