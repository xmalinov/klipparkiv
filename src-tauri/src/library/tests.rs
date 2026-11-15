use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};

use rusqlite::{Connection, params};

use crate::library::LibraryPaths;
use crate::library::commands::{read_text_file, sweep_part_files};
use crate::library::copy::{
    CopyFailure, copy_with_progress, file_sha256, sanitize_file_name, unique_target,
};
use crate::library::db::{Library, query_page};
use crate::library::guard::resolve_media;
use crate::library::import::{ImportState, import_media};
use crate::library::model::{
    ImportReport, ImportSkipReason, LibraryQuery, MediaKind, MediaRow, ScanStats, SortKey,
};
use crate::library::scan::{probe_info, scan_dir};
use crate::library::thumbs::extract_thumb;
use crate::library::thumbs::resolve_thumb;
use crate::library::util::{display_title, mtime_secs, now_secs};
use std::io::Write;

struct Fixture {
    root: PathBuf,
    library: Library,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn fixture(name: &str) -> Fixture {
    let root = std::env::temp_dir().join(format!("klipparkiv-lib-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("audio")).unwrap();
    fs::create_dir_all(root.join("video")).unwrap();
    fs::create_dir_all(root.join("data").join("thumbs")).unwrap();
    fs::create_dir_all(root.join("bin")).unwrap();

    let library = Library::open(&root.join("data").join("library.db")).unwrap();
    Fixture { root, library }
}

fn write(path: &Path, bytes: &[u8]) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    let mut file = fs::File::create(path).unwrap();
    file.write_all(bytes).unwrap();
}

fn scan(fixture: &Fixture, full: bool) -> ScanStats {
    scan_dir(
        &fixture.library,
        &LibraryPaths::new(&fixture.root),
        Path::new("missing-ffprobe"),
        5,
        full,
        |_| {},
    )
    .unwrap()
}

#[test]
fn strips_youtube_id_prefix_from_titles() {
    assert_eq!(display_title("dQw4w9WgXcQ - Rick Astley"), "Rick Astley");
    assert_eq!(display_title("abc123 - Song"), "Song");
    assert_eq!(display_title("a - Alpha"), "a - Alpha");
    assert_eq!(display_title("plain title"), "plain title");
    assert_eq!(display_title("dQw4w9WgXcQ - "), "dQw4w9WgXcQ - ");
    assert_eq!(
        display_title("dQw4w9WgXcQ - Never Gonna Give You Up [00-01-30-00-05-00]"),
        "Never Gonna Give You Up · клип 00:01:30–00:05:00"
    );
    assert_eq!(
        display_title("dQw4w9WgXcQ - Song [00-01-30-end]"),
        "Song · клип 00:01:30–конец"
    );
    assert_eq!(display_title("jNQXAC9IVRw_0-5"), "Клип 0–5");
    assert_eq!(display_title("abc123_00:10-00:20"), "Клип 00:10–00:20");
    assert_eq!(display_title("abc123_00：10-00：20"), "Клип 00:10–00:20");
    assert_eq!(display_title("abc123_00-10-00-20"), "Клип 00:10–00:20");
}

#[test]
fn indexes_media_and_ignores_bin_and_data() {
    let fixture = fixture("index");
    write(&fixture.root.join("audio").join("a1 - Song.mp3"), b"audio");
    write(&fixture.root.join("video").join("v1.mp4"), b"video");
    write(&fixture.root.join("bin").join("tool.exe"), b"exe");
    write(&fixture.root.join("data").join("db.sqlite"), b"db");
    write(&fixture.root.join("readme.txt"), b"text");

    let stats = scan(&fixture, true);

    assert_eq!(stats.added, 2);
    let page = query_page(
        &fixture.library,
        &LibraryQuery {
            search: None,
            kind: None,
            sort: SortKey::DateDesc,
            limit: None,
            offset: None,
        },
    )
    .unwrap();
    assert_eq!(page.total, 2);
    assert!(
        page.items
            .iter()
            .all(|item| item.kind == MediaKind::Audio || item.kind == MediaKind::Video)
    );
}

#[test]
fn finds_cached_thumbnail_by_id() {
    let fixture = fixture("thumb");
    write(
        &fixture.root.join("audio").join("abc123 - Song.mp3"),
        b"audio",
    );
    write(
        &fixture.root.join("data").join("thumbs").join("abc123.jpg"),
        b"jpg",
    );

    scan(&fixture, true);

    let page = query_page(
        &fixture.library,
        &LibraryQuery {
            search: Some("Song".into()),
            kind: None,
            sort: SortKey::DateDesc,
            limit: None,
            offset: None,
        },
    )
    .unwrap();
    assert_eq!(page.items.len(), 1);
    assert!(
        page.items[0]
            .thumb_path
            .as_ref()
            .unwrap()
            .ends_with("abc123.jpg")
    );
}

#[test]
fn incremental_scan_skips_unchanged_files() {
    let fixture = fixture("incremental");
    write(&fixture.root.join("video").join("v1.mp4"), b"video");

    let first = scan(&fixture, false);
    let second = scan(&fixture, false);

    assert_eq!(first.added, 1);
    assert_eq!(second.added, 0);
    assert_eq!(second.updated, 0);
}

#[test]
fn full_scan_removes_missing_files() {
    let fixture = fixture("remove");
    write(&fixture.root.join("video").join("v1.mp4"), b"video");
    scan(&fixture, true);

    fs::remove_file(fixture.root.join("video").join("v1.mp4")).unwrap();
    let stats = scan(&fixture, true);

    assert_eq!(stats.removed, 1);
    let page = query_page(
        &fixture.library,
        &LibraryQuery {
            search: None,
            kind: None,
            sort: SortKey::DateDesc,
            limit: None,
            offset: None,
        },
    )
    .unwrap();
    assert_eq!(page.total, 0);
}

#[test]
fn media_kinds_round_trip_through_the_wire_and_the_database() {
    assert_eq!(
        serde_json::to_value(MediaKind::Audio).unwrap(),
        serde_json::json!("audio")
    );
    assert_eq!(
        serde_json::to_value(MediaKind::Video).unwrap(),
        serde_json::json!("video")
    );
    assert_eq!(MediaKind::parse("audio"), Some(MediaKind::Audio));
    assert_eq!(MediaKind::parse("video"), Some(MediaKind::Video));
    assert_eq!(MediaKind::parse("opus"), None);
    assert_eq!(MediaKind::Audio.as_str(), "audio");
}

#[test]
fn sort_keys_decode_from_the_wire() {
    assert_eq!(
        serde_json::from_str::<SortKey>("\"dateDesc\"").unwrap(),
        SortKey::DateDesc
    );
    assert_eq!(
        serde_json::from_str::<SortKey>("\"channel\"").unwrap(),
        SortKey::Channel
    );
    assert!(serde_json::from_str::<SortKey>("\"mtime\"").is_err());
}

#[test]
fn a_query_without_a_sort_uses_the_default() {
    let query: LibraryQuery = serde_json::from_str("{}").unwrap();
    assert_eq!(query.sort, SortKey::DateDesc);
}

#[test]
fn filters_and_sorts() {
    let fixture = fixture("query");
    write(&fixture.root.join("audio").join("a - Alpha.mp3"), b"12345");
    write(
        &fixture.root.join("video").join("b - Beta.mp4"),
        b"1234567890",
    );

    scan(&fixture, true);

    let audio = query_page(
        &fixture.library,
        &LibraryQuery {
            search: None,
            kind: Some(MediaKind::Audio),
            sort: SortKey::DateDesc,
            limit: None,
            offset: None,
        },
    )
    .unwrap();
    assert_eq!(audio.total, 1);
    assert_eq!(audio.items[0].title, "a - Alpha");

    let by_size = query_page(
        &fixture.library,
        &LibraryQuery {
            search: None,
            kind: None,
            sort: SortKey::Size,
            limit: None,
            offset: None,
        },
    )
    .unwrap();
    assert_eq!(by_size.items[0].title, "b - Beta");
}

#[test]
fn thumbnail_resolution_stays_inside_cache() {
    let fixture = fixture("thumbguard");
    let thumb = fixture.root.join("data").join("thumbs").join("x.jpg");
    write(&thumb, b"jpg");
    let outside = std::env::temp_dir().join("outside.jpg");
    write(&outside, b"jpg");

    assert!(resolve_thumb(&LibraryPaths::new(&fixture.root), &thumb).is_ok());
    assert!(resolve_thumb(&LibraryPaths::new(&fixture.root), &outside).is_err());
    assert!(
        resolve_thumb(
            &LibraryPaths::new(&fixture.root),
            &fixture.root.join("audio").join("x.jpg")
        )
        .is_err()
    );
}

#[test]
fn media_resolution_stays_inside_root() {
    let fixture = fixture("mediaguard");
    let media = fixture.root.join("video").join("clip.mp4");
    write(&media, b"mp4");
    let outside = std::env::temp_dir().join("klipparkiv-outside.mp4");
    write(&outside, b"mp4");
    let other = fixture.root.join("video").join("notes.txt");
    write(&other, b"txt");

    assert_eq!(
        resolve_media(&fixture.root, &media).unwrap(),
        fs::canonicalize(&media).unwrap()
    );
    assert!(resolve_media(&fixture.root, &outside).is_err());
    assert!(resolve_media(&fixture.root, &other).is_err());
    assert!(
        resolve_media(
            &fixture.root,
            &fixture.root.join("video").join("missing.mp4")
        )
        .is_err()
    );
}

#[test]
fn practice_state_is_invalidated_by_mtime() {
    let fixture = fixture("practice-mtime");
    let path = "C:\\root\\audio\\a.mp3";
    fixture
        .library
        .practice_save(path, 10, "{\"version\":1}", 100)
        .unwrap();
    assert_eq!(
        fixture.library.practice_get(path, 10).unwrap().as_deref(),
        Some("{\"version\":1}")
    );
    assert!(fixture.library.practice_get(path, 11).unwrap().is_none());
    assert!(fixture.library.practice_get(path, 10).unwrap().is_none());
}

#[test]
fn practice_rows_are_pruned_with_missing_media() {
    let fixture = fixture("practice-prune");
    fixture
        .library
        .practice_save("missing.mp3", 1, "{}", 1)
        .unwrap();
    fixture.library.practice_prune().unwrap();
    assert!(
        fixture
            .library
            .practice_get("missing.mp3", 1)
            .unwrap()
            .is_none()
    );
}

#[test]
fn hidden_practice_rows_disappear_from_recent() {
    let fixture = fixture("practice-hidden");
    {
        let conn = fixture.library.conn.lock().unwrap();
        conn.execute(
                "INSERT INTO media(path, kind, title, channel, duration_secs, width, height, size_bytes, mtime, thumb_path, indexed_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    "a.mp3",
                    "audio",
                    "A",
                    None::<String>,
                    None::<f64>,
                    None::<u32>,
                    None::<u32>,
                    1_i64,
                    1_i64,
                    None::<String>,
                    1_i64
                ],
            )
            .unwrap();
    }
    fixture.library.practice_save("a.mp3", 1, "{}", 1).unwrap();
    assert_eq!(fixture.library.practice_recent(10).unwrap().len(), 1);
    fixture.library.practice_hide("a.mp3").unwrap();
    assert_eq!(fixture.library.practice_recent(10).unwrap().len(), 0);
    fixture.library.practice_save("a.mp3", 1, "{}", 2).unwrap();
    assert_eq!(fixture.library.practice_recent(10).unwrap().len(), 0);
    fixture.library.practice_unhide("a.mp3").unwrap();
    assert_eq!(fixture.library.practice_recent(10).unwrap().len(), 1);
}

#[test]
fn query_page_pages_through_a_large_library() {
    let fixture = fixture("bulk-query");
    {
        let mut conn = fixture.library.conn.lock().unwrap();
        let transaction = conn.transaction().unwrap();
        for index in 0..1000 {
            transaction
                    .execute(
                        "INSERT INTO media(path, kind, title, channel, duration_secs, width, height, size_bytes, mtime, thumb_path, indexed_at)
                         VALUES (?1, 'audio', ?2, NULL, NULL, NULL, NULL, 1, 1, NULL, 1)",
                        params![format!("audio/{index}.mp3"), format!("Track {index}")],
                    )
                    .unwrap();
        }
        transaction.commit().unwrap();
    }

    let first = query_page(
        &fixture.library,
        &LibraryQuery {
            search: None,
            kind: None,
            sort: SortKey::Title,
            limit: Some(200),
            offset: Some(0),
        },
    )
    .unwrap();
    assert_eq!(first.items.len(), 200);
    assert_eq!(first.total, 1000);

    let last = query_page(
        &fixture.library,
        &LibraryQuery {
            search: None,
            kind: None,
            sort: SortKey::Title,
            limit: Some(200),
            offset: Some(800),
        },
    )
    .unwrap();
    assert_eq!(last.items.len(), 200);
    assert_eq!(last.total, 1000);

    let beyond = query_page(
        &fixture.library,
        &LibraryQuery {
            search: None,
            kind: None,
            sort: SortKey::Title,
            limit: Some(200),
            offset: Some(1000),
        },
    )
    .unwrap();
    assert!(beyond.items.is_empty());
}

fn import(fixture: &Fixture, paths: Vec<String>) -> ImportReport {
    let cancel = AtomicBool::new(false);
    import_media(
        &fixture.library,
        &LibraryPaths::new(&fixture.root),
        Path::new("missing-ffprobe"),
        Path::new("missing-ffmpeg"),
        paths,
        &cancel,
        7,
        |_| {},
    )
    .unwrap()
}

fn audio_files(fixture: &Fixture) -> Vec<String> {
    fs::read_dir(fixture.root.join("audio"))
        .unwrap()
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect()
}

#[test]
fn sanitizes_imported_file_names() {
    assert_eq!(sanitize_file_name(r"..\..\evil.mp3", "audio"), "evil.mp3");
    assert_eq!(sanitize_file_name("CON.mp3", "audio"), "_CON.mp3");
    assert_eq!(sanitize_file_name("a:b?.mp3", "audio"), "a_b_.mp3");
    assert_eq!(sanitize_file_name("", "audio"), "audio");
    assert_eq!(sanitize_file_name("..", "video"), "video");
    assert_eq!(sanitize_file_name("Урок 1.mp3", "audio"), "Урок 1.mp3");
    assert_eq!(sanitize_file_name("Трек 🎸.mp3", "audio"), "Трек 🎸.mp3");

    let long = format!("{}.mp3", "x".repeat(200));
    let sanitized = sanitize_file_name(&long, "audio");
    assert!(sanitized.ends_with(".mp3"));
    assert_eq!(sanitized.chars().count(), 124);
}

#[test]
fn unique_target_avoids_case_insensitive_conflicts() {
    let dir = tempfile::tempdir().unwrap();
    write(&dir.path().join("Song.mp3"), b"audio");
    let mut taken = HashSet::new();

    let first = unique_target(dir.path(), "song.mp3", &mut taken).unwrap();
    assert_eq!(first.file_name().unwrap(), "song (2).mp3");

    let second = unique_target(dir.path(), "song.mp3", &mut taken).unwrap();
    assert_eq!(second.file_name().unwrap(), "song (3).mp3");
}

#[test]
fn sha256_matches_only_identical_content() {
    let dir = tempfile::tempdir().unwrap();
    let first = dir.path().join("a.mp3");
    let second = dir.path().join("b.mp3");
    let other = dir.path().join("c.mp3");
    write(&first, b"same");
    write(&second, b"same");
    write(&other, b"different");

    let first_hash = file_sha256(&first).unwrap();
    assert_eq!(first_hash, file_sha256(&second).unwrap());
    assert_ne!(first_hash, file_sha256(&other).unwrap());
}

#[test]
fn imports_and_indexes_a_file_without_ffprobe() {
    let fixture = fixture("import-copy");
    let source = tempfile::tempdir().unwrap();
    let path = source.path().join("Artist - Song.mp3");
    write(&path, b"1234567890");

    let report = import(&fixture, vec![path.to_string_lossy().into_owned()]);

    assert!(report.imported.len() == 1);
    assert!(report.existing.is_empty());
    assert!(report.failed.is_empty());
    assert_eq!(audio_files(&fixture), vec!["Artist - Song.mp3"]);

    let item = &report.imported[0];
    assert_eq!(item.title, "Artist - Song");
    assert_eq!(item.channel, None);
    assert_eq!(item.size_bytes, 10);
    assert!((now_secs() - item.mtime).abs() <= 5);
    assert!(
        fixture
            .root
            .join("audio")
            .join("Artist - Song.mp3")
            .is_file()
    );
    // The source is copied, never moved or rewritten.
    assert_eq!(fs::read(&path).unwrap(), b"1234567890");
}

#[test]
fn video_import_survives_missing_ffmpeg_and_has_no_poster() {
    let fixture = fixture("import-video-no-ffmpeg");
    let source = tempfile::tempdir().unwrap();
    let path = source.path().join("clip.mp4");
    write(&path, b"not-really-a-video");

    let report = import(&fixture, vec![path.to_string_lossy().into_owned()]);

    assert_eq!(report.imported.len(), 1);
    assert_eq!(report.imported[0].kind, MediaKind::Video);
    assert_eq!(report.imported[0].thumb_path, None);
    assert!(fixture.root.join("video").join("clip.mp4").is_file());
}

#[test]
fn second_import_of_the_same_content_is_reported_as_existing() {
    let fixture = fixture("import-duplicate");
    let source = tempfile::tempdir().unwrap();
    let path = source.path().join("Song.mp3");
    write(&path, b"identical-bytes");
    let path_text = path.to_string_lossy().into_owned();

    assert_eq!(import(&fixture, vec![path_text.clone()]).imported.len(), 1);
    let second = import(&fixture, vec![path_text]);

    assert!(second.imported.is_empty());
    assert_eq!(second.existing.len(), 1);
    assert_eq!(audio_files(&fixture), vec!["Song.mp3"]);
}

#[test]
fn duplicate_inside_one_batch_reuses_the_first_import() {
    let fixture = fixture("import-batch-duplicate");
    let first_dir = tempfile::tempdir().unwrap();
    let second_dir = tempfile::tempdir().unwrap();
    write(&first_dir.path().join("first.mp3"), b"same-content");
    write(&second_dir.path().join("second.mp3"), b"same-content");

    let report = import(
        &fixture,
        vec![
            first_dir
                .path()
                .join("first.mp3")
                .to_string_lossy()
                .into_owned(),
            second_dir
                .path()
                .join("second.mp3")
                .to_string_lossy()
                .into_owned(),
        ],
    );

    assert_eq!(report.imported.len(), 1);
    assert_eq!(report.existing.len(), 1);
    assert!(report.skipped.is_empty());
    assert_eq!(audio_files(&fixture), vec!["first.mp3"]);
    assert_eq!(report.existing[0].path, report.imported[0].path);
}

#[test]
fn name_conflict_gets_a_suffix_and_never_overwrites() {
    let fixture = fixture("import-conflict");
    let first_dir = tempfile::tempdir().unwrap();
    let second_dir = tempfile::tempdir().unwrap();
    write(&first_dir.path().join("tone.mp3"), b"AAAA");
    write(&second_dir.path().join("tone.mp3"), b"BBBB");

    let report = import(
        &fixture,
        vec![
            first_dir
                .path()
                .join("tone.mp3")
                .to_string_lossy()
                .into_owned(),
            second_dir
                .path()
                .join("tone.mp3")
                .to_string_lossy()
                .into_owned(),
        ],
    );

    assert_eq!(report.imported.len(), 2);
    let mut files = audio_files(&fixture);
    files.sort();
    assert_eq!(files, vec!["tone (2).mp3", "tone.mp3"]);
    assert_eq!(
        fs::read(fixture.root.join("audio").join("tone.mp3")).unwrap(),
        b"AAAA"
    );
    assert_eq!(
        fs::read(fixture.root.join("audio").join("tone (2).mp3")).unwrap(),
        b"BBBB"
    );
}

#[test]
fn file_already_in_the_library_is_not_copied_again() {
    let fixture = fixture("import-inside");
    let path = fixture.root.join("audio").join("inside.mp3");
    write(&path, b"audio");
    scan(&fixture, true);

    let report = import(&fixture, vec![path.to_string_lossy().into_owned()]);

    assert!(report.imported.is_empty());
    assert_eq!(report.existing.len(), 1);
    assert_eq!(report.existing[0].path, path.to_string_lossy());
    assert_eq!(audio_files(&fixture), vec!["inside.mp3"]);
}

#[test]
fn folders_and_unsupported_files_are_skipped() {
    let fixture = fixture("import-skip");
    let source = tempfile::tempdir().unwrap();
    let folder = source.path().join("lesson");
    fs::create_dir_all(&folder).unwrap();
    let pdf = source.path().join("notes.pdf");
    write(&pdf, b"pdf");

    let report = import(
        &fixture,
        vec![
            folder.to_string_lossy().into_owned(),
            pdf.to_string_lossy().into_owned(),
        ],
    );

    assert!(report.imported.is_empty());
    assert_eq!(report.skipped.len(), 2);
    assert!(matches!(report.skipped[0].reason, ImportSkipReason::Folder));
    assert!(matches!(
        report.skipped[1].reason,
        ImportSkipReason::Unsupported
    ));
}

#[test]
fn copy_stops_when_cancelled_between_chunks() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("big.bin");
    let part = dir.path().join("big.bin.part");
    let bytes = vec![7u8; 3 * 1024 * 1024];
    write(&source, &bytes);
    let cancel = AtomicBool::new(false);

    let result = copy_with_progress(&source, &part, bytes.len() as u64, &cancel, |_| {
        cancel.store(true, Ordering::SeqCst)
    });

    assert!(matches!(result, Err(CopyFailure::Cancelled)));
    assert!(part.exists(), "caller removes the partial file");
}

#[test]
fn copy_failure_is_reported_without_a_final_file() {
    let fixture = fixture("import-write-error");
    let source = tempfile::tempdir().unwrap();
    let path = source.path().join("song.mp3");
    write(&path, b"audio");
    // A directory with the staging name makes `File::create` fail.
    fs::create_dir_all(fixture.root.join("audio").join("song.mp3.part")).unwrap();

    let report = import(&fixture, vec![path.to_string_lossy().into_owned()]);

    assert!(report.imported.is_empty());
    assert_eq!(report.failed.len(), 1);
    assert!(!fixture.root.join("audio").join("song.mp3").exists());
}

#[test]
fn cancelled_import_copies_nothing() {
    let fixture = fixture("import-cancel");
    let source = tempfile::tempdir().unwrap();
    let path = source.path().join("Song.mp3");
    write(&path, b"audio");
    let cancel = AtomicBool::new(true);

    let report = import_media(
        &fixture.library,
        &LibraryPaths::new(&fixture.root),
        Path::new("missing-ffprobe"),
        Path::new("missing-ffmpeg"),
        vec![path.to_string_lossy().into_owned()],
        &cancel,
        1,
        |_| {},
    )
    .unwrap();

    assert!(report.cancelled);
    assert!(report.imported.is_empty());
    assert!(audio_files(&fixture).is_empty());
    assert!(matches!(
        report.skipped[0].reason,
        ImportSkipReason::Cancelled
    ));
}

#[test]
fn upgrades_a_170_database_with_the_origin_column() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("library.db");
    {
        let conn = Connection::open(&db).unwrap();
        conn.execute_batch(
            "CREATE TABLE media (
                   path TEXT PRIMARY KEY,
                   kind TEXT NOT NULL,
                   title TEXT NOT NULL,
                   channel TEXT,
                   duration_secs REAL,
                   width INTEGER,
                   height INTEGER,
                   size_bytes INTEGER NOT NULL,
                   mtime INTEGER NOT NULL,
                   thumb_path TEXT,
                   indexed_at INTEGER NOT NULL
                 );
                 INSERT INTO media
                   (path, kind, title, size_bytes, mtime, indexed_at)
                 VALUES ('old.mp3', 'audio', 'Old title', 10, 5, 5);",
        )
        .unwrap();
    }

    let library = Library::open(&db).unwrap();
    let origin: String = library
        .conn
        .lock()
        .unwrap()
        .query_row(
            "SELECT origin FROM media WHERE path = 'old.mp3'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(origin, "scan");
    assert_eq!(
        library.media_get("old.mp3").unwrap().unwrap().title,
        "Old title"
    );
}

#[test]
fn import_state_allows_one_job_at_a_time() {
    let state = ImportState::default();
    let (id, cancel) = state.begin().unwrap();
    assert!(!cancel.load(Ordering::SeqCst));
    assert!(state.begin().is_err());

    assert!(state.cancel());
    assert!(cancel.load(Ordering::SeqCst));
    state.finish(id);
    assert!(state.begin().is_ok());
}

#[test]
fn scan_preserves_imported_titles_artists_and_covers() {
    let fixture = fixture("import-preserve");
    let path = fixture.root.join("audio").join("track.mp3");
    write(&path, b"audio");
    let thumb = fixture
        .root
        .join("data")
        .join("thumbs")
        .join("import-abc.jpg");
    write(&thumb, b"jpg");

    fixture
        .library
        .media_upsert(&MediaRow {
            path: path.to_string_lossy().into_owned(),
            kind: MediaKind::Audio,
            title: "Tagged title".into(),
            channel: Some("Tagged artist".into()),
            duration_secs: Some(12.0),
            width: None,
            height: None,
            size_bytes: 5,
            mtime: mtime_secs(&path),
            thumb_path: Some(thumb.to_string_lossy().into_owned()),
            origin: "import".into(),
            indexed_at: now_secs(),
        })
        .unwrap();

    scan(&fixture, true);

    let item = fixture
        .library
        .media_get(&path.to_string_lossy())
        .unwrap()
        .expect("row must survive the scan");
    assert_eq!(item.title, "Tagged title");
    assert_eq!(item.channel.as_deref(), Some("Tagged artist"));
    assert_eq!(
        item.thumb_path.as_deref(),
        Some(thumb.to_string_lossy().as_ref())
    );
}

#[test]
fn reads_dropped_text_lists_and_rejects_other_files() {
    let dir = tempfile::tempdir().unwrap();
    let txt = dir.path().join("links.txt");
    write(&txt, b"https://youtu.be/a\nhttps://youtu.be/b");
    assert_eq!(
        read_text_file(txt.to_string_lossy().into_owned()).unwrap(),
        "https://youtu.be/a\nhttps://youtu.be/b"
    );

    let pdf = dir.path().join("notes.pdf");
    write(&pdf, b"pdf");
    assert!(read_text_file(pdf.to_string_lossy().into_owned()).is_err());
}

fn bundled_binary(name: &str) -> PathBuf {
    let file = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    };
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("bin")
        .join(file)
}

/// Builds a 1 s MP3 with `title`/`artist` tags and an embedded JPEG cover.
/// Returns `None` when the local ffmpeg lacks the MP3 encoder.
fn build_tagged_mp3(
    ffmpeg: &Path,
    dir: &Path,
    file: &str,
    title: &str,
    artist: &str,
    color: &str,
) -> Option<PathBuf> {
    let cover = dir.join(format!("{file}-cover.jpg"));
    let tagged = dir.join(file);
    let color = format!("color={color}:s=64x64");
    let title = format!("title={title}");
    let artist = format!("artist={artist}");

    let cover_ok = Command::new(ffmpeg)
        .args([
            "-y",
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            &color,
            "-frames:v",
            "1",
        ])
        .arg(&cover)
        .status()
        .is_ok_and(|status| status.success());
    if !cover_ok {
        return None;
    }
    let tag_ok = Command::new(ffmpeg)
        .args([
            "-y",
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:duration=1",
            "-i",
        ])
        .arg(&cover)
        .args([
            "-map",
            "0:a",
            "-map",
            "1:v",
            "-c:a",
            "libmp3lame",
            "-c:v",
            "mjpeg",
            "-id3v2_version",
            "3",
            "-metadata",
            &title,
            "-metadata",
            &artist,
            "-metadata:s:v",
            "title=Album cover",
            "-metadata:s:v",
            "comment=Cover (front)",
            "-disposition:v",
            "attached_pic",
        ])
        .arg(&tagged)
        .status()
        .is_ok_and(|status| status.success());
    if !tag_ok {
        return None;
    }
    Some(tagged)
}

fn build_video(ffmpeg: &Path, dir: &Path) -> Option<PathBuf> {
    let video = dir.join("clip.mp4");
    let created = Command::new(ffmpeg)
        .args([
            "-y",
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc=size=320x240:rate=10",
            "-t",
            "1",
            "-pix_fmt",
            "yuv420p",
        ])
        .arg(&video)
        .status()
        .is_ok_and(|status| status.success());
    created.then_some(video)
}

#[test]
fn probe_reads_id3_tags_and_cover_when_ffmpeg_is_available() {
    let ffmpeg = bundled_binary("ffmpeg");
    let ffprobe = bundled_binary("ffprobe");
    if !ffmpeg.is_file() || !ffprobe.is_file() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let Some(tagged) = build_tagged_mp3(
        &ffmpeg,
        dir.path(),
        "tagged.mp3",
        "Tagged title",
        "Tagged artist",
        "red",
    ) else {
        eprintln!("skipping tag probe test: local ffmpeg cannot build the fixture");
        return;
    };

    let info = probe_info(&ffprobe, &tagged, false);
    assert_eq!(info.title.as_deref(), Some("Tagged title"));
    assert_eq!(info.artist.as_deref(), Some("Tagged artist"));
    assert!(info.attached_pic, "cover art must be detected");
    assert!(info.duration.is_some());
}

#[test]
fn extracts_a_video_poster_when_ffmpeg_is_available() {
    let ffmpeg = bundled_binary("ffmpeg");
    if !ffmpeg.is_file() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let Some(video) = build_video(&ffmpeg, dir.path()) else {
        eprintln!("skipping poster test: local ffmpeg cannot build the fixture");
        return;
    };
    let poster = dir.path().join("poster.jpg");

    assert!(extract_thumb(&ffmpeg, &video, &poster, Some(0.5)));
    assert!(poster.is_file());
    assert!(fs::metadata(&poster).unwrap().len() > 0);
}

#[test]
fn imports_tags_cover_and_keeps_them_after_a_rescan() {
    let ffmpeg = bundled_binary("ffmpeg");
    let ffprobe = bundled_binary("ffprobe");
    if !ffmpeg.is_file() || !ffprobe.is_file() {
        return;
    }
    let fixture = fixture("import-tags");
    let source = tempfile::tempdir().unwrap();
    let Some(mp3) = build_tagged_mp3(
        &ffmpeg,
        source.path(),
        "tagged.mp3",
        "Tagged title",
        "Tagged artist",
        "red",
    ) else {
        return;
    };
    let cancel = AtomicBool::new(false);

    let report = import_media(
        &fixture.library,
        &LibraryPaths::new(&fixture.root),
        &ffprobe,
        &ffmpeg,
        vec![mp3.to_string_lossy().into_owned()],
        &cancel,
        1,
        |_| {},
    )
    .unwrap();

    assert_eq!(report.imported.len(), 1);
    let item = &report.imported[0];
    assert_eq!(item.title, "Tagged title");
    assert_eq!(item.channel.as_deref(), Some("Tagged artist"));
    assert!(item.duration_secs.is_some());
    let thumb = item.thumb_path.as_deref().expect("cover must be extracted");
    assert!(Path::new(thumb).is_file());

    scan(&fixture, true);
    let after = fixture.library.media_get(&item.path).unwrap().unwrap();
    assert_eq!(after.title, "Tagged title");
    assert_eq!(after.channel.as_deref(), Some("Tagged artist"));
    assert_eq!(after.thumb_path.as_deref(), Some(thumb));
}

#[test]
fn two_files_from_the_same_artist_get_distinct_covers() {
    let ffmpeg = bundled_binary("ffmpeg");
    let ffprobe = bundled_binary("ffprobe");
    if !ffmpeg.is_file() || !ffprobe.is_file() {
        return;
    }
    let fixture = fixture("import-cover-collision");
    let source = tempfile::tempdir().unwrap();
    let Some(first) = build_tagged_mp3(
        &ffmpeg,
        source.path(),
        "one.mp3",
        "One",
        "Same artist",
        "red",
    ) else {
        return;
    };
    let Some(second) = build_tagged_mp3(
        &ffmpeg,
        source.path(),
        "two.mp3",
        "Two",
        "Same artist",
        "blue",
    ) else {
        return;
    };
    let cancel = AtomicBool::new(false);

    let report = import_media(
        &fixture.library,
        &LibraryPaths::new(&fixture.root),
        &ffprobe,
        &ffmpeg,
        vec![
            first.to_string_lossy().into_owned(),
            second.to_string_lossy().into_owned(),
        ],
        &cancel,
        1,
        |_| {},
    )
    .unwrap();

    assert_eq!(report.imported.len(), 2);
    let first_thumb = report.imported[0].thumb_path.clone().unwrap();
    let second_thumb = report.imported[1].thumb_path.clone().unwrap();
    assert_ne!(first_thumb, second_thumb);
    assert!(Path::new(&first_thumb).is_file());
    assert!(Path::new(&second_thumb).is_file());
    assert!(
        report
            .imported
            .iter()
            .all(|item| item.channel.as_deref() == Some("Same artist"))
    );
}

#[test]
fn imports_a_video_with_a_poster() {
    let ffmpeg = bundled_binary("ffmpeg");
    let ffprobe = bundled_binary("ffprobe");
    if !ffmpeg.is_file() || !ffprobe.is_file() {
        return;
    }
    let fixture = fixture("import-video");
    let source = tempfile::tempdir().unwrap();
    let Some(video) = build_video(&ffmpeg, source.path()) else {
        return;
    };
    let cancel = AtomicBool::new(false);

    let report = import_media(
        &fixture.library,
        &LibraryPaths::new(&fixture.root),
        &ffprobe,
        &ffmpeg,
        vec![video.to_string_lossy().into_owned()],
        &cancel,
        1,
        |_| {},
    )
    .unwrap();

    assert_eq!(report.imported.len(), 1);
    let item = &report.imported[0];
    assert_eq!(item.kind, MediaKind::Video);
    assert!(item.width.is_some_and(|width| width > 0));
    assert!(fixture.root.join("video").join("clip.mp4").is_file());
    assert!(audio_files(&fixture).is_empty());
    let thumb = item
        .thumb_path
        .as_deref()
        .expect("poster must be extracted");
    assert!(Path::new(thumb).is_file());
}

#[test]
fn startup_sweep_removes_partial_downloads() {
    let fixture = fixture("import-sweep");
    let audio_part = fixture.root.join("audio").join("song.mp3.part");
    let video_part = fixture.root.join("video").join("clip.mp4.part");
    let keep = fixture.root.join("audio").join("song.mp3");
    write(&audio_part, b"partial");
    write(&video_part, b"partial");
    write(&keep, b"audio");

    let paths = crate::paths::AppPaths::portable(fixture.root.clone());
    sweep_part_files(&paths);

    assert!(!audio_part.exists());
    assert!(!video_part.exists());
    assert!(keep.exists());
}
