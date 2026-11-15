//! Download queue: the task store, request validation, process execution and
//! the events the UI listens to.

pub(crate) mod events;
pub(crate) mod pump;
pub(crate) mod runner;
pub(crate) mod store;
pub(crate) mod throttle;
pub(crate) mod validate;

pub(crate) use store::TaskStore;

#[cfg(test)]
pub(crate) use runner::{downloaded_path, is_skip_line};
#[cfg(test)]
pub(crate) use store::{DEFAULT_CONCURRENCY, MAX_FINISHED_TASKS, TaskMode, TaskState};
#[cfg(test)]
pub(crate) use throttle::{PROGRESS_EMIT_INTERVAL, ProgressThrottle};
#[cfg(test)]
pub(crate) use validate::{
    MAX_PLAYLIST_ITEMS, overall_percent, parse_audio_kbps, parse_clip, parse_list, parse_modes,
    parse_playlist, parse_quality, validate_url,
};
#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::time::{Duration, Instant};

    use crate::args::{Mode, Quality};
    use crate::queue::store::TaskSnapshot;

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

        let to_end = parse_clip(Some("1:30"), None).unwrap().unwrap();
        assert_eq!(to_end.end, "inf");

        let from_start = parse_clip(None, Some("2:00")).unwrap().unwrap();
        assert_eq!(from_start.start, "00:00:00");

        assert!(parse_clip(Some("bad"), Some("2:00")).is_err());
    }

    #[test]
    fn parses_playlist_switch_and_range() {
        assert!(parse_playlist(None, None, None).unwrap().is_none());
        assert!(parse_playlist(Some(false), None, None).unwrap().is_none());
        assert!(parse_playlist(Some(false), Some(1), None).is_err());

        let all = parse_playlist(Some(true), None, None).unwrap().unwrap();
        assert_eq!(all.start, None);
        assert_eq!(all.end, Some(MAX_PLAYLIST_ITEMS));

        let range = parse_playlist(Some(true), Some(2), Some(10))
            .unwrap()
            .unwrap();
        assert_eq!(range.start, Some(2));
        assert_eq!(range.end, Some(10));

        assert!(parse_playlist(Some(true), Some(0), None).is_err());
        assert!(parse_playlist(Some(true), Some(5), Some(3)).is_err());
        assert!(parse_playlist(Some(true), Some(1), Some(MAX_PLAYLIST_ITEMS + 1)).is_err());
    }

    #[test]
    fn computes_overall_playlist_progress() {
        assert_eq!(overall_percent(45.0, None, None), 45.0);
        assert_eq!(overall_percent(50.0, Some(1), Some(4)), 12.5);
        assert_eq!(overall_percent(50.0, Some(2), Some(4)), 37.5);
        assert_eq!(overall_percent(100.0, Some(4), Some(4)), 100.0);
        assert_eq!(overall_percent(120.0, Some(1), Some(1)), 100.0);
    }

    #[test]
    fn recognizes_download_output_lines() {
        assert_eq!(
            downloaded_path("[download] Destination: C:\\media\\a.mp3"),
            Some(PathBuf::from(r"C:\media\a.mp3"))
        );
        assert_eq!(
            downloaded_path("[download] C:\\media\\a.mp3 has already been downloaded"),
            Some(PathBuf::from(r"C:\media\a.mp3"))
        );
        assert_eq!(downloaded_path("[download] 45.2% of 4MiB"), None);
        assert!(is_skip_line(
            "[download] C:\\media\\a.mp3 has already been downloaded"
        ));
        assert!(is_skip_line(
            "[youtube] abc: has already been recorded in the archive"
        ));
        assert!(!is_skip_line("[download] Destination: C:\\media\\a.mp3"));
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
                title: None,
                mode: TaskMode::Audio,
                clip: false,
                state: TaskState::Queued,
                percent: 0.0,
                speed_bps: None,
                eta_secs: None,
                error: None,
                playlist_index: None,
                playlist_count: None,
                output_path: None,
            },
            Vec::new(),
        );
        let second_id = store.next_id();
        assert_ne!(first.id, second_id);

        let _ = store.update(&first.id, |snapshot| snapshot.percent = 42.0);
        store.finish(&first.id, TaskState::Done, None);

        let snapshot = store.snapshot(&first.id).unwrap();
        assert_eq!(snapshot.percent, 100.0);
        assert_eq!(snapshot.state, TaskState::Done);
        assert!(!store.is_canceled(&first.id));
    }

    #[test]
    fn queue_state_tracks_pause_and_pending() {
        let store = TaskStore::default();
        let first = store.insert(
            TaskSnapshot {
                id: store.next_id(),
                url: "https://example.com/1".into(),
                title: None,
                mode: TaskMode::Audio,
                clip: false,
                state: TaskState::Queued,
                percent: 0.0,
                speed_bps: None,
                eta_secs: None,
                error: None,
                playlist_index: None,
                playlist_count: None,
                output_path: None,
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

    /// Characterization tests for the task store.
    ///
    /// `commands.rs` is scheduled to be split into `queue/` + `ipc/`, so the
    /// observable behaviour of the store (state transitions, playlist progress
    /// math, cancel semantics) is pinned here before the code moves. These
    /// tests must stay green across the split without changing a single
    /// assertion — that is what makes the move safe.
    mod queue_lifecycle {
        use super::*;

        fn snapshot(id: &str) -> TaskSnapshot {
            TaskSnapshot {
                id: id.into(),
                url: format!("https://example.com/{id}"),
                title: None,
                mode: TaskMode::Audio,
                clip: false,
                state: TaskState::Queued,
                percent: 0.0,
                speed_bps: None,
                eta_secs: None,
                error: None,
                playlist_index: None,
                playlist_count: None,
                output_path: None,
            }
        }

        fn enqueue(store: &TaskStore) -> TaskSnapshot {
            store.insert(snapshot(&store.next_id()), Vec::new())
        }

        fn max_concurrent(store: &TaskStore) -> usize {
            store.inner.lock().unwrap().max_concurrent
        }

        #[test]
        fn ids_start_at_one_and_grow() {
            let store = TaskStore::default();
            assert_eq!(store.next_id(), "t1");
            assert_eq!(store.next_id(), "t2");
        }

        #[test]
        fn insert_puts_the_task_into_the_queue() {
            let store = TaskStore::default();
            let task = enqueue(&store);

            assert_eq!(task.state, TaskState::Queued);
            assert_eq!(store.queue_state().queued, 1);
            assert_eq!(store.all().len(), 1);
            assert_eq!(store.snapshot(&task.id).unwrap().id, task.id);
        }

        #[test]
        fn concurrency_is_clamped_between_one_and_four() {
            let store = TaskStore::default();
            assert_eq!(max_concurrent(&store), DEFAULT_CONCURRENCY);

            store.set_concurrency(0);
            assert_eq!(max_concurrent(&store), 1);

            store.set_concurrency(99);
            assert_eq!(max_concurrent(&store), 4);
        }

        #[test]
        fn a_failed_task_keeps_its_progress_and_the_reason() {
            let store = TaskStore::default();
            let task = enqueue(&store);
            store.apply_progress(&task.id, 40.0, None, None, None, None);

            store.finish(&task.id, TaskState::Failed, Some("boom".into()));

            let snapshot = store.snapshot(&task.id).unwrap();
            assert_eq!(snapshot.state, TaskState::Failed);
            assert_eq!(snapshot.percent, 40.0);
            assert_eq!(snapshot.error.as_deref(), Some("boom"));
        }

        #[test]
        fn a_skipped_task_is_not_reported_as_done() {
            let store = TaskStore::default();
            let task = enqueue(&store);

            store.mark_skipped(&task.id);
            store.finish_completed(&task.id);

            let snapshot = store.snapshot(&task.id).unwrap();
            assert_eq!(snapshot.state, TaskState::Skipped);
            assert_eq!(snapshot.percent, 100.0);
            assert_eq!(snapshot.error, None);
        }

        #[test]
        fn playlist_errors_finish_as_partial_with_the_error_count() {
            let store = TaskStore::default();
            let task = enqueue(&store);
            store.note_error(&task.id);
            store.note_error(&task.id);

            store.finish_completed(&task.id);

            let snapshot = store.snapshot(&task.id).unwrap();
            assert_eq!(snapshot.state, TaskState::Partial);
            assert!(
                snapshot
                    .error
                    .as_deref()
                    .is_some_and(|text| text.contains('2')),
                "the message must name the number of errors: {:?}",
                snapshot.error
            );
            assert_eq!(snapshot.percent, 0.0);
        }

        #[test]
        fn a_clean_run_finishes_as_done_at_one_hundred_percent() {
            let store = TaskStore::default();
            let task = enqueue(&store);
            store.apply_progress(&task.id, 55.0, None, None, None, None);

            store.finish_completed(&task.id);

            let snapshot = store.snapshot(&task.id).unwrap();
            assert_eq!(snapshot.state, TaskState::Done);
            assert_eq!(snapshot.percent, 100.0);
        }

        #[test]
        fn playlist_item_progress_becomes_the_overall_percentage() {
            let store = TaskStore::default();
            let task = enqueue(&store);

            let snapshot = store
                .apply_progress(
                    &task.id,
                    50.0,
                    Some(1_000_000.0),
                    Some(30),
                    Some(2),
                    Some(4),
                )
                .unwrap();

            assert_eq!(snapshot.percent, 37.5);
            assert_eq!(snapshot.playlist_index, Some(2));
            assert_eq!(snapshot.playlist_count, Some(4));
            assert_eq!(snapshot.speed_bps, Some(1_000_000.0));
            assert_eq!(snapshot.eta_secs, Some(30));
        }

        #[test]
        fn progress_without_speed_keeps_the_last_known_value() {
            let store = TaskStore::default();
            let task = enqueue(&store);
            store.apply_progress(
                &task.id,
                10.0,
                Some(2_000_000.0),
                Some(60),
                Some(1),
                Some(10),
            );

            let snapshot = store
                .apply_progress(&task.id, 20.0, None, None, None, None)
                .unwrap();

            assert_eq!(snapshot.speed_bps, Some(2_000_000.0));
            assert_eq!(snapshot.eta_secs, Some(60));
            assert_eq!(snapshot.playlist_index, Some(1));
            assert_eq!(snapshot.playlist_count, Some(10));
        }

        #[test]
        fn destination_sets_the_output_path_and_backfills_the_title() {
            let store = TaskStore::default();
            let task = enqueue(&store);

            store.note_destination(&task.id, Path::new("abc123 - My Song.mp3"));

            let snapshot = store.snapshot(&task.id).unwrap();
            assert_eq!(
                snapshot.output_path.as_deref(),
                Some("abc123 - My Song.mp3")
            );
            assert_eq!(snapshot.title.as_deref(), Some("My Song"));
        }

        #[test]
        fn destination_keeps_a_title_that_came_from_the_preview() {
            let store = TaskStore::default();
            let mut queued = snapshot(&store.next_id());
            queued.title = Some("Preview title".into());
            let task = store.insert(queued, Vec::new());

            store.note_destination(&task.id, Path::new("abc123 - File name.mp3"));

            let snapshot = store.snapshot(&task.id).unwrap();
            assert_eq!(snapshot.title.as_deref(), Some("Preview title"));
            assert!(snapshot.output_path.is_some());
        }

        #[test]
        fn cancel_marks_a_queued_task_and_reports_no_pid() {
            let store = TaskStore::default();
            let task = enqueue(&store);

            assert_eq!(store.take_cancel(&task.id), None);
            assert!(store.is_canceled(&task.id));
            // A cancelled task no longer counts as pending work.
            assert_eq!(store.queue_state().queued, 0);
        }

        #[test]
        fn cancel_returns_the_pid_of_a_running_task() {
            let store = TaskStore::default();
            let task = enqueue(&store);
            {
                let mut inner = store.inner.lock().unwrap();
                inner.tasks.get_mut(&task.id).unwrap().pid = Some(4242);
            }

            assert_eq!(store.take_cancel(&task.id), Some(4242));
        }

        #[test]
        fn unknown_ids_are_ignored_instead_of_panicking() {
            let store = TaskStore::default();

            assert!(store.snapshot("nope").is_none());
            assert!(!store.is_canceled("nope"));
            assert_eq!(store.take_cancel("nope"), None);
            store.finish("nope", TaskState::Done, None);
            store.mark_skipped("nope");
            store.note_error("nope");
            store.note_destination("nope", Path::new("a.mp3"));
            store.finish_completed("nope");
            assert_eq!(store.all().len(), 0);
        }

        #[test]
        fn running_counter_never_underflows() {
            let store = TaskStore::default();
            store.finish_running();
            assert_eq!(store.queue_state().running, 0);
        }

        #[test]
        fn finish_clears_the_pid_of_a_finished_task() {
            let store = TaskStore::default();
            let task = enqueue(&store);
            store
                .inner
                .lock()
                .unwrap()
                .tasks
                .get_mut(&task.id)
                .unwrap()
                .pid = Some(4242);

            store.finish(&task.id, TaskState::Done, None);

            assert_eq!(store.take_cancel(&task.id), None);
            assert_eq!(store.snapshot(&task.id).unwrap().state, TaskState::Done);
        }

        #[test]
        fn remove_drops_the_task_from_the_map_and_the_queue() {
            let store = TaskStore::default();
            let task = enqueue(&store);
            store.finish(&task.id, TaskState::Done, None);

            store.remove(&task.id).unwrap();

            assert!(store.snapshot(&task.id).is_none());
            assert_eq!(store.queue_state().queued, 0);
            assert_eq!(store.all().len(), 0);
        }

        #[test]
        fn remove_rejects_a_task_that_is_still_running() {
            let store = TaskStore::default();
            let task = enqueue(&store);
            store.update(&task.id, |snapshot| {
                snapshot.state = TaskState::Running;
            });

            let error = store.remove(&task.id).unwrap_err();

            assert!(
                error.contains("отмените") || error.contains("cancel"),
                "{error}"
            );
            assert!(store.snapshot(&task.id).is_some());
        }

        #[test]
        fn remove_is_a_no_op_for_an_unknown_task() {
            let store = TaskStore::default();
            assert!(store.remove("t404").is_ok());
        }

        #[test]
        fn finished_tasks_are_capped_to_the_retention_limit() {
            let store = TaskStore::default();
            let total = MAX_FINISHED_TASKS + 5;
            for _ in 0..total {
                let task = enqueue(&store);
                store.finish(&task.id, TaskState::Done, None);
            }

            assert_eq!(store.all().len(), MAX_FINISHED_TASKS);
            // The oldest finished tasks are the ones that go away.
            assert!(store.snapshot("t1").is_none());
            assert!(store.snapshot(&format!("t{total}")).is_some());
        }

        #[test]
        fn pruning_never_touches_a_pending_task() {
            let store = TaskStore::default();
            for _ in 0..MAX_FINISHED_TASKS {
                let task = enqueue(&store);
                store.finish(&task.id, TaskState::Done, None);
            }
            let pending = enqueue(&store);
            let last = enqueue(&store);

            // One more finished task exceeds the limit and triggers the prune.
            store.finish(&last.id, TaskState::Done, None);

            assert!(store.snapshot(&pending.id).is_some());
            assert_eq!(
                store.snapshot(&pending.id).unwrap().state,
                TaskState::Queued
            );
            assert!(store.snapshot("t1").is_none());
        }
    }

    mod progress_throttle {
        use super::*;

        fn base() -> Instant {
            Instant::now()
        }

        #[test]
        fn the_first_value_is_emitted_immediately() {
            let mut throttle = ProgressThrottle::new(Duration::from_millis(100));
            assert!(throttle.allow(base()));
        }

        #[test]
        fn emits_at_most_once_per_interval() {
            let mut throttle = ProgressThrottle::new(Duration::from_millis(100));
            let start = base();

            assert!(throttle.allow(start));
            assert!(!throttle.allow(start + Duration::from_millis(99)));
            assert!(throttle.allow(start + Duration::from_millis(100)));
            assert!(!throttle.allow(start + Duration::from_millis(150)));
            assert!(throttle.allow(start + Duration::from_millis(200)));
        }

        #[test]
        fn a_burst_of_lines_becomes_a_handful_of_events() {
            let mut throttle = ProgressThrottle::new(PROGRESS_EMIT_INTERVAL);
            let start = base();

            // 50 lines over 500 ms with a 100 ms interval → 5 events.
            let emitted = (0..50u64)
                .filter(|step| throttle.allow(start + Duration::from_millis(step * 10)))
                .count();

            assert_eq!(emitted, 5);
        }

        #[test]
        fn time_going_backwards_does_not_unlock_the_throttle() {
            let mut throttle = ProgressThrottle::new(Duration::from_millis(100));
            let start = base();

            assert!(throttle.allow(start));
            assert!(!throttle.allow(start - Duration::from_millis(50)));
        }
    }

    #[test]
    fn states_serialize_to_the_wire_strings() {
        // The UI still receives plain strings: the enum must not change the
        // JSON contract while it makes the backend type-safe.
        for (state, wire) in [
            (TaskState::Queued, "queued"),
            (TaskState::Running, "running"),
            (TaskState::Done, "done"),
            (TaskState::Skipped, "skipped"),
            (TaskState::Partial, "partial"),
            (TaskState::Failed, "failed"),
            (TaskState::Canceled, "canceled"),
        ] {
            assert_eq!(
                serde_json::to_value(state).unwrap(),
                serde_json::json!(wire)
            );
        }
    }

    #[test]
    fn a_snapshot_serializes_the_state_as_a_plain_string() {
        let snapshot = TaskSnapshot {
            id: "t1".into(),
            url: "https://example.com".into(),
            title: None,
            mode: TaskMode::Audio,
            clip: false,
            state: TaskState::Running,
            percent: 12.5,
            speed_bps: None,
            eta_secs: None,
            error: None,
            playlist_index: None,
            playlist_count: None,
            output_path: None,
        };

        let value = serde_json::to_value(&snapshot).unwrap();

        assert_eq!(value["state"], serde_json::json!("running"));
        assert_eq!(value["percent"], serde_json::json!(12.5));
    }

    #[test]
    fn modes_serialize_to_the_wire_strings() {
        assert_eq!(
            serde_json::to_value(TaskMode::Audio).unwrap(),
            serde_json::json!("audio")
        );
        assert_eq!(
            serde_json::to_value(TaskMode::Video).unwrap(),
            serde_json::json!("video")
        );
    }

    #[test]
    fn only_finished_states_are_terminal() {
        for state in [
            TaskState::Done,
            TaskState::Skipped,
            TaskState::Partial,
            TaskState::Failed,
            TaskState::Canceled,
        ] {
            assert!(state.is_terminal(), "{state:?} must be terminal");
        }
        assert!(!TaskState::Queued.is_terminal());
        assert!(!TaskState::Running.is_terminal());
    }
}
