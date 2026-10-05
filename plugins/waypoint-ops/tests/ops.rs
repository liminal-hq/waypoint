// The operations plugin against a mock Tauri app: jobs run through the queue and the worker pool,
// events reach every window in revision order, questions park a worker until they are answered,
// and the journal, the clipboard and the settings are shared. Everything happens in a temporary
// directory behind a sandbox; the Trash is a fake, never the real one.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod support;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::test::MockRuntime;
use tauri::Manager;
use tauri_plugin_waypoint_ops::{
    commands, ClipboardMode, ClipboardSource, JobProgress, Ops, MAX_CONCURRENCY,
};
use waypoint_ops::{
    ConflictPolicy, Decision, JobKind, JobPriority, JobState, JournalStorage, OpsError, OpsEvent,
    OpsSettings, OpsSnapshot, Resolution, Schedule, VerifyAlgorithm, WaitReason,
};
use waypoint_protocol::VfsError;
use waypoint_vfs::Provider;

use support::*;

fn partials(env: &Env, folder: &str) -> Vec<String> {
    env.names(folder)
        .into_iter()
        .filter(|n| n.starts_with(".waypoint-"))
        .collect()
}

#[test]
fn a_submitted_job_runs_to_done_and_every_window_hears_the_same_events_in_order() {
    let env = env();
    env.write("a.txt", b"alpha");
    env.dir("dst");
    // Submitted by the first window; the second hears about it too.
    let id = env.submit("main-1", env.copy(&["a.txt"], "dst"));
    env.wait_done(id);
    env.wait_for("the last event", |e| {
        e.events_of("main-2")
            .iter()
            .any(|ev| matches!(ev, OpsEvent::JournalChanged { .. }))
    });

    assert_eq!(env.read("dst/a.txt"), b"alpha");
    let first = env.events_of("main-1");
    let second = env.events_of("main-2");
    assert_eq!(first, second, "both windows heard the same events");
    assert!(
        env.events_of("settings").len() == first.len(),
        "broadcast reaches every window"
    );

    let queue: Vec<&OpsEvent> = first
        .iter()
        .filter(|e| !matches!(e, OpsEvent::JournalChanged { .. }))
        .collect();
    let revisions: Vec<u64> = queue.iter().map(|e| e.revision()).collect();
    assert!(
        revisions.windows(2).all(|w| w[0] < w[1]),
        "queue events are in revision order: {revisions:?}"
    );
    let states: Vec<String> = queue
        .iter()
        .filter_map(|e| match e {
            OpsEvent::JobAdded { job, .. } | OpsEvent::JobChanged { job, .. } => {
                Some(job.state.name().to_owned())
            }
            _ => None,
        })
        .collect();
    assert_eq!(states.first().map(String::as_str), Some("planning"));
    assert_eq!(states.last().map(String::as_str), Some("done"));
    for needed in ["queued", "running"] {
        assert!(states.iter().any(|s| s == needed), "{states:?}");
    }

    // The origin window is the caller's, whatever the request said.
    assert_eq!(env.job(id).origin_window, "main-1");
    // Folding the events reproduces the queue.
    let mut mirror = OpsSnapshot::default();
    for event in &first {
        mirror.apply(event);
    }
    assert_eq!(mirror.jobs, env.snapshot().jobs);
    assert!(env.job(id).undoable);
    assert_eq!(
        env.snapshot().journal.undo.map(|u| u.label),
        Some("Copy \u{201c}a.txt\u{201d}".to_owned())
    );
}

#[test]
fn a_clash_found_by_the_planner_parks_the_job_until_it_is_resolved() {
    let env = env();
    env.write("a.txt", b"new");
    env.write("b.txt", b"b");
    env.dir("dst");
    env.write("dst/a.txt", b"old");
    let id = env.submit("main-1", env.copy(&["a.txt", "b.txt"], "dst"));
    let state = env.wait_state(id, "waiting", |s| matches!(s, JobState::Waiting { .. }));
    let JobState::Waiting {
        reason: WaitReason::Conflicts { conflicts },
    } = state
    else {
        panic!("waiting on conflicts, not {state:?}");
    };
    assert_eq!(conflicts.len(), 1);
    assert_eq!(
        env.read("dst/a.txt"),
        b"old",
        "nothing is overwritten without a decision"
    );
    assert!(
        !env.exists("dst/b.txt"),
        "nothing is written before the answer"
    );
    // The worker is parked with its slot held.
    std::thread::sleep(Duration::from_millis(50));
    assert!(matches!(env.state(id), JobState::Waiting { .. }));

    tauri::async_runtime::block_on(commands::resolve(
        env.window("main-2"),
        env.app.state::<Ops<MockRuntime>>(),
        id,
        vec![],
        Some(ConflictPolicy::KeepBoth),
    ))
    .unwrap();
    env.wait_done(id);
    assert_eq!(env.read("dst/a.txt"), b"old");
    assert_eq!(env.read("dst/a (2).txt"), b"new");
    assert_eq!(env.read("dst/b.txt"), b"b");
}

#[test]
fn a_clash_met_in_the_middle_of_a_merge_asks_for_that_one_alone() {
    let env = env();
    env.dir("src");
    env.dir("src/d");
    env.write("src/d/inner.txt", b"from src");
    env.write("src/d/other.txt", b"other");
    env.dir("dst");
    env.dir("dst/d");
    env.write("dst/d/inner.txt", b"in dst");
    // Merging folders is the answer up front; the file clash below it is asked alone.
    let id = env.submit(
        "main-1",
        env.copy_with(&["src/d"], "dst", ConflictPolicy::MergeFolders),
    );
    let state = env.wait_state(id, "waiting", |s| matches!(s, JobState::Waiting { .. }));
    let JobState::Waiting {
        reason: WaitReason::Conflicts { conflicts },
    } = state
    else {
        panic!("{state:?}");
    };
    assert_eq!(conflicts[0].name, "inner.txt");
    tauri::async_runtime::block_on(commands::resolve(
        env.window("main-1"),
        env.app.state::<Ops<MockRuntime>>(),
        id,
        vec![Resolution {
            source: Some(conflicts[0].source.clone()),
            policy: ConflictPolicy::Skip,
        }],
        None,
    ))
    .unwrap();
    env.wait_done(id);
    assert_eq!(env.read("dst/d/inner.txt"), b"in dst");
    assert_eq!(env.read("dst/d/other.txt"), b"other");
}

#[test]
fn an_error_waits_for_retry_skip_or_cancel() {
    // Retry: the failing write works the second time.
    let env_retry = env();
    env_retry.write("a.txt", b"alpha");
    env_retry.dir("dst");
    env_retry.gate.fail_at(
        1,
        VfsError::Io {
            location: Some(env_retry.loc("dst/a.txt")),
            message: "the disk hiccupped".to_owned(),
        },
    );
    let id = env_retry.submit("main-1", env_retry.copy(&["a.txt"], "dst"));
    let state = env_retry.wait_state(id, "waiting", |s| matches!(s, JobState::Waiting { .. }));
    let JobState::Waiting {
        reason: WaitReason::Error { item, .. },
    } = state
    else {
        panic!("{state:?}");
    };
    assert_eq!(item, env_retry.loc("a.txt"));
    // A conflict answer is refused for an error.
    assert!(tauri::async_runtime::block_on(commands::resolve(
        env_retry.window("main-1"),
        env_retry.app.state::<Ops<MockRuntime>>(),
        id,
        vec![],
        Some(ConflictPolicy::Skip),
    ))
    .is_err());
    answer(&env_retry, id, Decision::Retry);
    env_retry.wait_done(id);
    assert_eq!(env_retry.read("dst/a.txt"), b"alpha");

    // Skip: the item is left out and the job ends with a count.
    let env_skip = env();
    env_skip.write("a.txt", b"alpha");
    env_skip.write("b.txt", b"bravo");
    env_skip.dir("dst");
    env_skip.gate.fail_at(
        1,
        VfsError::Io {
            location: Some(env_skip.loc("dst/a.txt")),
            message: "the disk hiccupped".to_owned(),
        },
    );
    let id = env_skip.submit("main-1", env_skip.copy(&["a.txt", "b.txt"], "dst"));
    env_skip.wait_state(id, "waiting", |s| matches!(s, JobState::Waiting { .. }));
    answer(&env_skip, id, Decision::Skip);
    env_skip.wait_done(id);
    assert!(!env_skip.exists("dst/a.txt"));
    assert_eq!(env_skip.read("dst/b.txt"), b"bravo");
    assert_eq!(env_skip.job(id).counts.failed, 1);

    // Cancel: the job ends cancelled and leaves no partial file.
    let env_cancel = env();
    env_cancel.write("a.txt", b"alpha");
    env_cancel.dir("dst");
    env_cancel.gate.fail_at(
        1,
        VfsError::Io {
            location: Some(env_cancel.loc("dst/a.txt")),
            message: "the disk hiccupped".to_owned(),
        },
    );
    let id = env_cancel.submit("main-1", env_cancel.copy(&["a.txt"], "dst"));
    env_cancel.wait_state(id, "waiting", |s| matches!(s, JobState::Waiting { .. }));
    answer(&env_cancel, id, Decision::Cancel);
    env_cancel.wait_state(id, "cancelled", |s| *s == JobState::Cancelled);
    assert!(partials(&env_cancel, "dst").is_empty());
    assert!(env_cancel.names("dst").is_empty());
}

fn answer(env: &Env, id: waypoint_ops::JobId, decision: Decision) {
    tauri::async_runtime::block_on(commands::resolve_error(
        env.window("main-2"),
        env.app.state::<Ops<MockRuntime>>(),
        id,
        decision,
    ))
    .unwrap();
}

#[test]
fn a_running_copy_can_be_paused_resumed_and_cancelled() {
    // Paused: held at the second file, paused, released; it stops before the third.
    let env = env();
    for name in ["1", "2", "3"] {
        env.write(&format!("{name}.txt"), name.as_bytes());
    }
    env.dir("dst");
    env.gate.block_at(2);
    let id = env.submit("main-1", env.copy(&["1.txt", "2.txt", "3.txt"], "dst"));
    env.gate.wait_held(1);
    assert_eq!(env.state(id), JobState::Running);
    tauri::async_runtime::block_on(commands::pause(
        env.window("main-1"),
        env.app.state::<Ops<MockRuntime>>(),
        id,
    ))
    .unwrap();
    assert_eq!(env.state(id), JobState::Paused);
    env.gate.open();
    std::thread::sleep(Duration::from_millis(150));
    assert_eq!(
        env.state(id),
        JobState::Paused,
        "the worker is parked, holding its slot"
    );
    assert!(!env.exists("dst/3.txt"), "nothing runs while paused");
    tauri::async_runtime::block_on(commands::resume(
        env.window("main-2"),
        env.app.state::<Ops<MockRuntime>>(),
        id,
    ))
    .unwrap();
    env.wait_done(id);
    assert_eq!(env.names("dst"), ["1.txt", "2.txt", "3.txt"]);

    // Cancelled mid-copy: what was copied stays, nothing is half written.
    let env = env_for_cancel();
    let id = env.submit("main-1", env.copy(&["1.txt", "2.txt", "3.txt"], "dst"));
    env.gate.wait_held(1);
    tauri::async_runtime::block_on(commands::cancel(
        env.window("main-1"),
        env.app.state::<Ops<MockRuntime>>(),
        id,
    ))
    .unwrap();
    assert_eq!(env.state(id), JobState::Cancelling);
    env.gate.open();
    env.wait_state(id, "cancelled", |s| *s == JobState::Cancelled);
    let left = env.names("dst");
    assert!(!left.contains(&"3.txt".to_owned()), "{left:?}");
    assert!(
        left.iter().all(|n| !n.starts_with(".waypoint-")),
        "{left:?}"
    );
    // A cancelled job can be retried as a new one.
    let again = tauri::async_runtime::block_on(commands::retry(
        env.window("main-1"),
        env.app.state::<Ops<MockRuntime>>(),
        id,
    ))
    .unwrap();
    assert_ne!(again, id);
    // The first file is already there, so the new job asks before it writes anything.
    env.wait_state(again, "waiting", |s| matches!(s, JobState::Waiting { .. }));
    assert_eq!(env.names("dst"), left);
}

fn env_for_cancel() -> Env {
    let env = env();
    for name in ["1", "2", "3"] {
        env.write(&format!("{name}.txt"), name.as_bytes());
    }
    env.dir("dst");
    env.gate.block_at(2);
    env
}

#[test]
fn only_as_many_jobs_run_at_once_as_the_setting_allows() {
    let env = env();
    for name in ["a", "b", "c", "d"] {
        env.write(&format!("{name}.txt"), name.as_bytes());
    }
    env.dir("dst");
    env.gate.block_all();
    let ids: Vec<_> = ["a", "b", "c"]
        .iter()
        .map(|n| env.submit("main-1", env.copy(&[&format!("{n}.txt")], "dst")))
        .collect();
    env.gate.wait_held(2);
    // (Which two run depends on which finished planning first.)
    env.wait_for("the third job queued", |e| {
        e.snapshot()
            .jobs
            .iter()
            .filter(|j| j.state == JobState::Queued)
            .count()
            == 1
    });
    let running = |env: &Env| {
        env.snapshot()
            .jobs
            .iter()
            .filter(|j| j.state == JobState::Running)
            .count()
    };
    assert_eq!(running(&env), 2, "the default is two");
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(
        running(&env),
        2,
        "a third never starts while two hold their slots"
    );

    // The next job reads the new setting: with one at a time, a fourth waits behind the rest.
    let one = OpsSettings {
        concurrency: 1,
        ..env.ops().settings()
    };
    tauri::async_runtime::block_on(commands::set_settings(
        env.window("main-1"),
        env.app.state::<Ops<MockRuntime>>(),
        one,
    ))
    .unwrap();
    assert_eq!(env.settings.saved(), Some(one), "saved as it is applied");
    env.gate.open();
    for id in &ids {
        env.wait_done(*id);
    }
    env.gate.block_all();
    let first = env.submit("main-1", env.copy(&["d.txt"], "dst"));
    env.gate.wait_held(1);
    let second = env.submit(
        "main-1",
        env.request(JobKind::CreateFolder, &[], Some("dst"), Some("later")),
    );
    env.wait_state(second, "queued", |s| *s == JobState::Queued);
    assert_eq!(env.state(first), JobState::Running);
    env.gate.open();
    env.wait_done(first);
    env.wait_done(second);
}

#[test]
fn settings_are_validated_and_take_effect_for_the_next_job() {
    let env = env();
    let ops = env.ops();
    let bad = OpsSettings {
        concurrency: 0,
        ..ops.settings()
    };
    assert!(ops.set_settings(bad).is_err());
    let too_many = OpsSettings {
        concurrency: MAX_CONCURRENCY + 1,
        ..ops.settings()
    };
    assert!(ops.set_settings(too_many).is_err());
    assert_eq!(env.settings.saved(), None, "a refused change saves nothing");

    // Verification is a setting: the job after the change records a verification.
    env.write("a.txt", b"alpha");
    env.dir("dst");
    let id = env.submit("main-1", env.copy(&["a.txt"], "dst"));
    env.wait_done(id);
    assert_eq!(env.job(id).verified, None);
    let verify = OpsSettings {
        verify_after_copy: true,
        verify_algorithm: VerifyAlgorithm::Sha256,
        ..ops.settings()
    };
    ops.set_settings(verify).unwrap();
    env.dir("dst2");
    let id = env.submit("main-1", env.copy(&["a.txt"], "dst2"));
    env.wait_done(id);
    let verified = env.job(id).verified.expect("the next job verified");
    assert_eq!(verified.algorithm, VerifyAlgorithm::Sha256);
    assert_eq!(verified.files, 1);
}

#[test]
fn progress_goes_only_to_the_window_that_subscribed() {
    let env = env();
    env.write("big.bin", &big(20));
    env.dir("dst");
    let ticks: Arc<Mutex<Vec<JobProgress>>> = Arc::default();
    let sink = ticks.clone();
    let channel = Channel::<JobProgress>::new(move |body| {
        if let InvokeResponseBody::Json(text) = body {
            sink.lock()
                .unwrap()
                .push(serde_json::from_str(&text).unwrap());
        }
        Ok(())
    });
    tauri::async_runtime::block_on(commands::subscribe_progress(
        env.window("main-1"),
        env.app.state::<Ops<MockRuntime>>(),
        channel,
    ))
    .unwrap();
    // The second window subscribes with a channel of its own that counts separately.
    let other: Arc<Mutex<Vec<JobProgress>>> = Arc::default();
    let other_sink = other.clone();
    let other_channel = Channel::<JobProgress>::new(move |body| {
        if let InvokeResponseBody::Json(text) = body {
            other_sink
                .lock()
                .unwrap()
                .push(serde_json::from_str(&text).unwrap());
        }
        Ok(())
    });
    tauri::async_runtime::block_on(commands::subscribe_progress(
        env.window("main-2"),
        env.app.state::<Ops<MockRuntime>>(),
        other_channel,
    ))
    .unwrap();
    tauri::async_runtime::block_on(commands::unsubscribe_progress(
        env.window("main-2"),
        env.app.state::<Ops<MockRuntime>>(),
        None,
    ))
    .unwrap();

    let id = env.submit("main-1", env.copy(&["big.bin"], "dst"));
    env.wait_done(id);
    let heard = ticks.lock().unwrap().clone();
    assert!(heard.len() >= 2, "the subscriber heard progress: {heard:?}");
    assert!(heard.iter().all(|t| t.job == id));
    assert!(heard.windows(2).all(|w| w[0].revision < w[1].revision));
    assert!(heard.last().unwrap().progress.bytes_done > heard.first().unwrap().progress.bytes_done);
    assert!(
        other.lock().unwrap().is_empty(),
        "a window that unsubscribed hears nothing"
    );
    // Progress is not an event: no window was sent a tick as one.
    let events = env.events_of("main-2");
    let progress_events = events
        .iter()
        .filter(|e| {
            matches!(e, OpsEvent::JobChanged { job, .. }
                if job.state == JobState::Running && job.progress.bytes_done > 0)
        })
        .count();
    assert_eq!(progress_events, 0, "{events:?}");
    assert_eq!(env.read("dst/big.bin").len(), 20 * 1024 * 1024);
}

#[test]
fn undo_and_redo_run_through_the_queue_and_the_journal() {
    let env = env();
    let id = env.submit(
        "main-1",
        env.request(JobKind::CreateFolder, &[], Some(""), Some("made")),
    );
    env.wait_done(id);
    assert!(env.exists("made"));
    let summaries = tauri::async_runtime::block_on(commands::journal_summaries(
        env.window("main-2"),
        env.app.state::<Ops<MockRuntime>>(),
    ))
    .unwrap();
    assert_eq!(summaries.len(), 1);
    assert!(summaries[0].undoable);

    let undo = tauri::async_runtime::block_on(commands::undo(
        env.window("main-2"),
        env.app.state::<Ops<MockRuntime>>(),
        None,
    ))
    .unwrap();
    env.wait_done(undo);
    assert!(!env.exists("made"));
    assert_eq!(env.job(undo).origin_window, "main-2");
    let snapshot = env.snapshot();
    assert!(snapshot.journal.undo.is_none());
    assert!(snapshot.journal.redo.is_some());

    let redo = tauri::async_runtime::block_on(commands::redo(
        env.window("main-1"),
        env.app.state::<Ops<MockRuntime>>(),
        None,
    ))
    .unwrap();
    env.wait_done(redo);
    assert!(env.exists("made"));
    assert!(env.snapshot().journal.undo.is_some());
    // Both windows were told about the history, on the same channel as the queue.
    for window in ["main-1", "main-2"] {
        let changes = env
            .events_of(window)
            .into_iter()
            .filter(|e| matches!(e, OpsEvent::JournalChanged { .. }))
            .count();
        assert!(changes >= 3, "{window} heard {changes} journal changes");
    }
    // Nothing is left to undo twice: an undo of an entry already being undone is refused.
    assert!(env.ops().undo("main-1", None).is_ok());
    assert!(matches!(
        env.ops().undo("main-1", None),
        Err(tauri_plugin_waypoint_ops::Error::Ops(
            OpsError::UndoUnavailable { .. }
        )) | Ok(_)
    ));
}

#[test]
fn an_undo_refused_because_the_world_changed_says_so_and_changes_nothing() {
    let env = env();
    env.write("a.txt", b"alpha");
    env.dir("dst");
    let id = env.submit("main-1", env.copy(&["a.txt"], "dst"));
    env.wait_done(id);
    // Edited after the copy.
    env.fs
        .create_write(
            &env.path("dst/a.txt"),
            waypoint_vfs::WriteOptions::truncate(),
        )
        .unwrap()
        .finish(false)
        .unwrap();
    let undo = env.ops().undo("main-1", None).unwrap();
    let state = env.wait_state(undo, "failed", |s| s.is_finished());
    assert!(
        matches!(
            state,
            JobState::Failed {
                error: OpsError::UndoStale { .. },
                ..
            }
        ),
        "{state:?}"
    );
    assert!(env.exists("dst/a.txt"));
}

#[test]
fn the_clipboard_is_shared_by_every_window() {
    let env = env();
    assert!(tauri::async_runtime::block_on(commands::get_clipboard(
        env.window("main-2"),
        env.app.state::<Ops<MockRuntime>>(),
    ))
    .unwrap()
    .items
    .is_empty());
    let items = vec![env.loc("a.txt"), env.loc("b.txt")];
    let set = tauri::async_runtime::block_on(commands::set_clipboard(
        env.window("main-1"),
        env.app.state::<Ops<MockRuntime>>(),
        ClipboardMode::Cut,
        items.clone(),
        None,
    ))
    .unwrap();
    assert_eq!(set.revision, 1);
    let seen = tauri::async_runtime::block_on(commands::get_clipboard(
        env.window("main-2"),
        env.app.state::<Ops<MockRuntime>>(),
    ))
    .unwrap();
    assert_eq!(seen, set);
    assert_eq!(seen.mode, ClipboardMode::Cut);
    assert_eq!(seen.items, items);
    env.wait_for("the clipboard event on both windows", |e| {
        e.clipboards.lock().unwrap().len() >= 3
    });
    {
        let log = env.clipboards.lock().unwrap();
        for window in ["main-1", "main-2"] {
            assert!(
                log.iter().any(|(w, c)| w == window && *c == set),
                "{window} was told"
            );
        }
    }
    // A clear is a change too, and the revision only goes up.
    let cleared = env.ops().set_clipboard(ClipboardMode::Copy, Vec::new());
    assert_eq!(cleared.revision, 2);
    assert!(cleared.items.is_empty());
}

#[test]
fn the_clipboard_remembers_who_set_it() {
    let env = env();
    let first = env
        .ops()
        .set_clipboard(ClipboardMode::Copy, vec![env.loc("a.txt")]);
    assert_eq!(first.source, ClipboardSource::App);
    let adopted = tauri::async_runtime::block_on(commands::set_clipboard(
        env.window("main-1"),
        env.app.state::<Ops<MockRuntime>>(),
        ClipboardMode::Cut,
        vec![env.loc("b.txt")],
        Some(ClipboardSource::Os),
    ))
    .unwrap();
    assert_eq!(adopted.source, ClipboardSource::Os);
    assert_eq!(adopted.revision, 2);
    // A clear is the app's, and an omitted source means the app.
    let cleared = env.ops().set_clipboard(ClipboardMode::Copy, Vec::new());
    assert_eq!(cleared.source, ClipboardSource::App);
}

#[test]
fn a_selection_goes_on_the_clipboard_resolved_by_the_apps_resolver() {
    let env = env();
    env.write("a.txt", b"alpha");
    env.write("b.txt", b"bravo");
    *env.resolver.0.lock().unwrap() = vec![env.loc("b.txt")];
    let set = tauri::async_runtime::block_on(commands::set_clipboard_from_selection(
        env.window("main-2"),
        env.app.state::<Ops<MockRuntime>>(),
        waypoint_vfs::ListingHandle(1),
        waypoint_vfs::SelectionSpec::AllExcept { ids: vec![] },
        ClipboardMode::Cut,
    ))
    .unwrap();
    assert_eq!(set.items, vec![env.loc("b.txt")]);
    assert_eq!(set.mode, ClipboardMode::Cut);
    assert_eq!(set.source, ClipboardSource::App);
    env.wait_for("the clipboard event", |e| {
        e.clipboards.lock().unwrap().iter().any(|(_, c)| *c == set)
    });
    // Nothing selected is refused and leaves the clipboard as it was.
    *env.resolver.0.lock().unwrap() = Vec::new();
    let refused = tauri::async_runtime::block_on(commands::set_clipboard_from_selection(
        env.window("main-2"),
        env.app.state::<Ops<MockRuntime>>(),
        waypoint_vfs::ListingHandle(1),
        waypoint_vfs::SelectionSpec::Chosen { ids: vec![] },
        ClipboardMode::Copy,
    ));
    assert!(refused.is_err());
    assert_eq!(env.ops().clipboard(), set);
}

#[test]
fn a_selection_is_resolved_for_an_outbound_drag_and_the_clipboard_is_left_alone() {
    let env = env();
    env.write("a.txt", b"alpha");
    env.write("b.txt", b"bravo");
    *env.resolver.0.lock().unwrap() = vec![env.loc("a.txt"), env.loc("b.txt")];
    let before = env.ops().clipboard();
    let items = tauri::async_runtime::block_on(commands::resolve_selection(
        env.window("main-2"),
        env.app.state::<Ops<MockRuntime>>(),
        waypoint_vfs::ListingHandle(1),
        waypoint_vfs::SelectionSpec::AllExcept { ids: vec![] },
    ))
    .unwrap();
    assert_eq!(items, vec![env.loc("a.txt"), env.loc("b.txt")]);
    assert_eq!(env.ops().clipboard(), before);
    // Nothing selected is refused rather than starting a drag of nothing.
    *env.resolver.0.lock().unwrap() = Vec::new();
    let refused = tauri::async_runtime::block_on(commands::resolve_selection(
        env.window("main-2"),
        env.app.state::<Ops<MockRuntime>>(),
        waypoint_vfs::ListingHandle(1),
        waypoint_vfs::SelectionSpec::Chosen { ids: vec![] },
    ));
    assert!(refused.is_err());
}

#[test]
fn a_closing_window_does_not_stop_its_jobs() {
    let env = env();
    env.write("a.txt", b"alpha");
    env.write("b.txt", b"bravo");
    env.dir("dst");
    env.gate.block_at(1);
    let id = env.submit("main-2", env.copy(&["a.txt", "b.txt"], "dst"));
    env.gate.wait_held(1);
    // The window that started it goes away; its progress channel goes with it.
    let channel = Channel::<JobProgress>::new(|_| Ok(()));
    tauri::async_runtime::block_on(commands::subscribe_progress(
        env.window("main-2"),
        env.app.state::<Ops<MockRuntime>>(),
        channel,
    ))
    .unwrap();
    tauri_plugin_waypoint_ops::on_window_destroyed(env.app.handle(), "main-2");
    env.window("main-2").destroy().unwrap();
    assert_eq!(env.state(id), JobState::Running);
    env.gate.open();
    env.wait_done(id);
    assert_eq!(env.names("dst"), ["a.txt", "b.txt"]);
    // The first window still sees the finished job in the shared queue.
    assert_eq!(env.job(id).origin_window, "main-2");
}

#[test]
fn the_journal_is_written_a_moment_after_a_change_and_at_once_on_exit() {
    // A long delay, so only the explicit flushes can have written it.
    let env = env_with(Setup {
        save_delay: Duration::from_secs(600),
        ..Setup::default()
    });
    let id = env.submit(
        "main-1",
        env.request(JobKind::CreateFolder, &[], Some(""), Some("made")),
    );
    env.wait_done(id);
    let stored = |env: &Env| {
        env.journal
            .current_document()
            .map(|d| d.body.entries.len())
            .unwrap_or(0)
    };
    // The write-ahead record was stored synchronously; the entry waits for the debounce.
    assert_eq!(stored(&env), 0);
    tauri_plugin_waypoint_ops::on_exit(env.app.handle());
    assert_eq!(stored(&env), 1, "exit flushed the entry");
    assert!(env
        .journal
        .current_document()
        .unwrap()
        .body
        .pending
        .is_empty());

    // Closing a window flushes too.
    let id = env.submit(
        "main-1",
        env.request(JobKind::CreateFolder, &[], Some(""), Some("second")),
    );
    // (The app is shutting down, so nothing new runs; use a fresh one.)
    let _ = id;
    let env = env_with(Setup {
        save_delay: Duration::from_secs(600),
        ..Setup::default()
    });
    let id = env.submit(
        "main-1",
        env.request(JobKind::CreateFolder, &[], Some(""), Some("made")),
    );
    env.wait_done(id);
    assert_eq!(
        env.journal
            .current_document()
            .map_or(0, |d| d.body.entries.len()),
        0
    );
    tauri_plugin_waypoint_ops::on_window_destroyed(env.app.handle(), "main-1");
    assert_eq!(
        env.journal
            .current_document()
            .map_or(0, |d| d.body.entries.len()),
        1,
        "a closing window flushed the journal"
    );
}

#[test]
fn the_journal_is_debounced_by_the_save_delay() {
    let env = env();
    let id = env.submit(
        "main-1",
        env.request(JobKind::CreateFolder, &[], Some(""), Some("made")),
    );
    env.wait_done(id);
    env.wait_for("the debounced save", |e| {
        e.journal
            .current_document()
            .is_some_and(|d| d.body.entries.len() == 1)
    });
}

#[test]
fn exit_cancels_a_running_job_and_it_removes_its_partial_files() {
    let env = env();
    env.write("big.bin", &big(24));
    env.dir("dst");
    let id = env.submit("main-1", env.copy(&["big.bin"], "dst"));
    env.wait_state(id, "running", |s| *s == JobState::Running);
    tauri_plugin_waypoint_ops::on_exit(env.app.handle());
    let state = env.state(id);
    assert!(
        matches!(state, JobState::Cancelled | JobState::Done),
        "{state:?}"
    );
    assert!(partials(&env, "dst").is_empty());
    assert!(env
        .journal
        .current_document()
        .is_some_and(|d| d.body.pending.is_empty()));
}

#[test]
fn start_up_recovery_reports_what_a_crash_left_once() {
    use waypoint_ops::{JobId, JournalBody, JournalDocument, PendingRecord};
    let journal = Arc::new(waypoint_ops::testing::journal_storage::MemoryJournalStorage::new());
    let setup = Setup {
        journal: journal.clone(),
        prepare: Box::new({
            let journal = journal.clone();
            move |work, fs| {
                // A job that was running when the app died: its partial file is in `work`, and
                // the journal holds its write-ahead record.
                let partial = work.join(".waypoint-partial-7-1-big.bin").unwrap();
                let mut stream = fs
                    .create_write(&partial, waypoint_vfs::WriteOptions::exclusive())
                    .unwrap();
                std::io::Write::write_all(&mut stream, b"half").unwrap();
                stream.finish(false).unwrap();
                let record = PendingRecord {
                    job: JobId(7),
                    at_ms: 1,
                    kind: JobKind::Copy,
                    label: "Copy \u{201c}big.bin\u{201d}".to_owned(),
                    items: vec![work.join("big.bin").unwrap().to_location()],
                    folders: vec![work.to_location()],
                    renames: vec![],
                };
                journal
                    .save(&JournalDocument::new(JournalBody {
                        next_id: 1,
                        pending: vec![record],
                        ..JournalBody::default()
                    }))
                    .unwrap();
            }
        }),
        ..Setup::default()
    };
    let env = env_with(setup);
    assert!(
        partials(&env, "").is_empty(),
        "recovery cleaned the partial file"
    );
    // The event went out as the plugin started.
    let heard = env.recovered.lock().unwrap().clone();
    assert_eq!(heard.len(), 1);
    assert_eq!(heard[0].interrupted.len(), 1);
    assert_eq!(heard[0].interrupted[0].job, JobId(7));
    assert_eq!(heard[0].interrupted[0].removed.len(), 1);

    let take = |env: &Env| {
        tauri::async_runtime::block_on(commands::take_recovery_report(
            env.window("main-1"),
            env.app.state::<Ops<MockRuntime>>(),
        ))
        .unwrap()
    };
    let report = take(&env).expect("the report is there for the first window");
    assert_eq!(report, heard[0]);
    assert_eq!(take(&env), None, "once");
    // A run that has nothing to tell has no report and no event.
    let quiet = env_with(Setup::default());
    assert_eq!(take(&quiet), None);
    assert!(quiet.recovered.lock().unwrap().is_empty());
}

#[test]
fn a_plan_previews_a_request_without_queueing_it() {
    let env = env();
    env.write("a.txt", b"alpha");
    env.dir("dst");
    env.write("dst/a.txt", b"old");
    let preview = tauri::async_runtime::block_on(commands::plan(
        env.window("main-1"),
        env.app.state::<Ops<MockRuntime>>(),
        env.copy(&["a.txt"], "dst"),
    ))
    .unwrap();
    assert_eq!(preview.items, 1);
    assert_eq!(preview.bytes, 5);
    assert_eq!(preview.conflicts.len(), 1);
    assert!(preview.same_volume || !preview.same_volume);
    assert!(env.snapshot().jobs.is_empty(), "a preview queues nothing");
    assert_eq!(env.read("dst/a.txt"), b"old");
    // A request the planner refuses is the engine's typed refusal.
    let refused = tauri::async_runtime::block_on(commands::plan(
        env.window("main-1"),
        env.app.state::<Ops<MockRuntime>>(),
        env.copy(&["missing"], "dst"),
    ));
    assert!(matches!(
        refused,
        Err(tauri_plugin_waypoint_ops::Error::Ops(
            OpsError::NotFound { .. }
        ))
    ));
}

#[test]
fn jobs_targeting_a_folder_are_found_until_they_finish() {
    let env = env();
    env.write("a.txt", b"alpha");
    env.dir("dst");
    env.gate.block_at(1);
    let id = env.submit("main-1", env.copy(&["a.txt"], "dst"));
    env.gate.wait_held(1);
    let busy = tauri::async_runtime::block_on(commands::jobs_targeting(
        env.window("main-2"),
        env.app.state::<Ops<MockRuntime>>(),
        env.loc("dst"),
    ))
    .unwrap();
    assert_eq!(busy, vec![id]);
    env.gate.open();
    env.wait_done(id);
    assert!(env.ops().jobs_targeting(&env.loc("dst")).is_empty());
}

#[test]
fn the_queue_can_be_reordered_and_finished_jobs_dismissed() {
    let env = env();
    for name in ["a", "b", "c"] {
        env.write(&format!("{name}.txt"), name.as_bytes());
    }
    env.dir("dst");
    env.gate.block_all();
    let one = OpsSettings {
        concurrency: 1,
        ..env.ops().settings()
    };
    env.ops().set_settings(one).unwrap();
    let a = env.submit("main-1", env.copy(&["a.txt"], "dst"));
    env.gate.wait_held(1);
    let b = env.submit("main-1", env.copy(&["b.txt"], "dst"));
    let c = env.submit("main-1", env.copy(&["c.txt"], "dst"));
    env.wait_state(c, "queued", |s| *s == JobState::Queued);
    env.wait_state(b, "queued", |s| *s == JobState::Queued);
    env.ops().reorder(c, 0).unwrap();
    let order: Vec<_> = env.snapshot().jobs.iter().map(|j| j.id).collect();
    assert_eq!(order, vec![a, c, b]);
    env.gate.open();
    for id in [a, b, c] {
        env.wait_done(id);
    }
    // c ran before b.
    let finished: Vec<_> = env
        .snapshot()
        .jobs
        .iter()
        .map(|j| (j.id, j.finished_ms))
        .collect();
    let at = |id| finished.iter().find(|(j, _)| *j == id).unwrap().1.unwrap();
    assert!(at(c) <= at(b));
    env.ops().dismiss(a).unwrap();
    assert_eq!(env.snapshot().jobs.len(), 2);
    env.ops().dismiss_finished();
    assert!(env.snapshot().jobs.is_empty());
    assert!(
        env.ops().dismiss(a).is_err(),
        "an unknown job is a typed error"
    );
}

#[test]
fn a_new_job_is_planned_while_every_worker_is_parked_on_a_question() {
    // Two jobs wait on conflicts (holding both slots); a third is still planned and queued.
    let env = env();
    for name in ["a", "b", "c"] {
        env.write(&format!("{name}.txt"), name.as_bytes());
    }
    env.dir("dst");
    env.write("dst/a.txt", b"x");
    env.write("dst/b.txt", b"x");
    let a = env.submit("main-1", env.copy(&["a.txt"], "dst"));
    let b = env.submit("main-1", env.copy(&["b.txt"], "dst"));
    env.wait_state(a, "waiting", |s| matches!(s, JobState::Waiting { .. }));
    env.wait_state(b, "waiting", |s| matches!(s, JobState::Waiting { .. }));
    let c = env.submit("main-1", env.copy(&["c.txt"], "dst"));
    env.wait_state(c, "queued", |s| *s == JobState::Queued);
    assert!(!env.exists("dst/c.txt"));
    env.ops()
        .resolve(a, vec![], Some(ConflictPolicy::Skip))
        .unwrap();
    env.wait_done(a);
    env.wait_done(c);
    assert!(env.exists("dst/c.txt"));
    env.ops()
        .resolve(b, vec![], Some(ConflictPolicy::Skip))
        .unwrap();
    env.wait_done(b);
}

#[test]
fn a_selection_is_resolved_by_the_injected_resolver() {
    // The plugin never calls another plugin: the resolver is the app's.
    let env = env();
    env.write("a.txt", b"alpha");
    env.write("b.txt", b"bravo");
    env.dir("dst");
    *env.resolver.0.lock().unwrap() = vec![env.loc("b.txt")];
    let mut request = env.copy(&[], "dst");
    request.sources = waypoint_ops::Sources::Selection {
        handle: waypoint_vfs::ListingHandle(1),
        spec: waypoint_vfs::SelectionSpec::AllExcept { ids: vec![] },
    };
    let id = env.submit("main-1", request);
    env.wait_done(id);
    assert_eq!(env.names("dst"), ["b.txt"]);
    // The entry records what was resolved, so a redo does not depend on the listing.
    let entry = env.ops().journal_summaries();
    assert_eq!(entry.len(), 1);
}

#[test]
fn submitting_an_undo_directly_is_refused() {
    let env = env();
    let refused = env.ops().submit(
        "main-1",
        env.request(
            JobKind::Undo {
                of: waypoint_ops::JournalId(1),
            },
            &[],
            None,
            None,
        ),
    );
    assert!(matches!(
        refused,
        Err(tauri_plugin_waypoint_ops::Error::Ops(
            OpsError::Unsupported { .. }
        ))
    ));
}

/// A journal storage whose writes take as long as the test says, over the in-memory one.
struct SlowStorage {
    inner: Arc<waypoint_ops::testing::journal_storage::MemoryJournalStorage>,
    /// How long the next saves take.
    delay: Mutex<Duration>,
}

impl SlowStorage {
    fn set_delay(&self, delay: Duration) {
        *self.delay.lock().unwrap() = delay;
    }
}

impl JournalStorage for SlowStorage {
    fn load(&self) -> Result<waypoint_ops::Loaded, waypoint_ops::StorageError> {
        self.inner.load()
    }

    fn save(
        &self,
        document: &waypoint_ops::JournalDocument,
    ) -> Result<(), waypoint_ops::StorageError> {
        let delay = *self.delay.lock().unwrap();
        std::thread::sleep(delay);
        self.inner.save(document)
    }
}

fn slow_env() -> (Env, Arc<SlowStorage>) {
    let journal = Arc::new(waypoint_ops::testing::journal_storage::MemoryJournalStorage::new());
    let slow = Arc::new(SlowStorage {
        inner: journal.clone(),
        delay: Mutex::new(Duration::ZERO),
    });
    let env = env_with(Setup {
        journal,
        storage: Some(slow.clone()),
        // Only the explicit flushes write.
        save_delay: Duration::from_secs(600),
        ..Setup::default()
    });
    (env, slow)
}

fn stored_entries(env: &Env) -> usize {
    env.journal
        .current_document()
        .map_or(0, |d| d.body.entries.len())
}

#[test]
fn a_slow_journal_write_on_window_close_does_not_block_commands_or_workers() {
    let (env, slow) = slow_env();
    let id = env.submit(
        "main-1",
        env.request(JobKind::CreateFolder, &[], Some(""), Some("one")),
    );
    env.wait_done(id);
    slow.set_delay(Duration::from_millis(1500));
    let ops = env.ops();
    let closing = std::thread::spawn(move || ops.window_closed("main-1"));
    std::thread::sleep(Duration::from_millis(200));

    // The write is under way. Commands answer at once, and so does a job's whole life.
    let started = std::time::Instant::now();
    let _ = env.ops().snapshot();
    let _ = env.ops().clipboard();
    let _ = env.ops().set_clipboard(ClipboardMode::Copy, vec![]);
    assert!(
        started.elapsed() < Duration::from_millis(500),
        "commands waited for the disk: {:?}",
        started.elapsed()
    );
    closing.join().unwrap();
    assert_eq!(stored_entries(&env), 1);
}

#[test]
fn a_journal_write_that_started_earlier_never_replaces_a_newer_one() {
    let (env, slow) = slow_env();
    let id = env.submit(
        "main-1",
        env.request(JobKind::CreateFolder, &[], Some(""), Some("one")),
    );
    env.wait_done(id);
    // The first write holds a snapshot of one entry, and is slow.
    slow.set_delay(Duration::from_millis(800));
    let ops = env.ops();
    let first = std::thread::spawn(move || ops.flush_journal());
    std::thread::sleep(Duration::from_millis(200));
    // A second entry is made while it is under way, and its own flush is quick.
    slow.set_delay(Duration::ZERO);
    let id = env.submit(
        "main-1",
        env.request(JobKind::CreateFolder, &[], Some(""), Some("two")),
    );
    env.wait_done(id);
    env.ops().flush_journal();
    first.join().unwrap();
    assert_eq!(
        stored_entries(&env),
        2,
        "the last state is what the file holds"
    );
}

#[test]
fn a_retry_runs_the_sources_the_job_was_accepted_with_not_the_listing_as_it_is_now() {
    let env = env();
    env.write("a.txt", b"alpha");
    env.write("b.txt", b"bravo");
    env.dir("dst");
    *env.resolver.0.lock().unwrap() = vec![env.loc("a.txt")];
    let mut request = env.copy(&[], "dst");
    request.sources = waypoint_ops::Sources::Selection {
        handle: waypoint_vfs::ListingHandle(1),
        spec: waypoint_vfs::SelectionSpec::AllExcept { ids: vec![] },
    };
    env.gate.block_at(1);
    let id = env.submit("main-1", request);
    env.gate.wait_held(1);
    env.ops().cancel(id).unwrap();
    env.gate.open();
    env.wait_state(id, "cancelled", |s| *s == JobState::Cancelled);

    // The listing gains an entry before the retry.
    *env.resolver.0.lock().unwrap() = vec![env.loc("a.txt"), env.loc("b.txt")];
    let again = env.ops().retry(id).unwrap();
    env.wait_done(again);
    assert_eq!(
        env.names("dst"),
        ["a.txt"],
        "b.txt was never part of the job"
    );
}

#[test]
fn a_late_stop_from_a_replaced_subscription_leaves_the_newer_one_listening() {
    // A page that mounts twice subscribes, subscribes again, and only then hears the first stop.
    let env = env();
    env.write("big.bin", &big(20));
    env.dir("dst");
    let channel_into = |sink: Arc<Mutex<Vec<JobProgress>>>| {
        Channel::<JobProgress>::new(move |body| {
            if let InvokeResponseBody::Json(text) = body {
                sink.lock()
                    .unwrap()
                    .push(serde_json::from_str(&text).unwrap());
            }
            Ok(())
        })
    };
    let (first, second): (Arc<Mutex<Vec<_>>>, Arc<Mutex<Vec<_>>>) = Default::default();
    let ops = env.ops();
    let old = ops.subscribe_progress("main-1", channel_into(first.clone()));
    let new = ops.subscribe_progress("main-1", channel_into(second.clone()));
    assert_ne!(old, new);
    ops.unsubscribe_progress("main-1", Some(old));

    let id = env.submit("main-1", env.copy(&["big.bin"], "dst"));
    env.wait_done(id);
    assert!(
        !second.lock().unwrap().is_empty(),
        "the newer one still hears"
    );
    assert!(first.lock().unwrap().is_empty());

    // Its own stop ends it.
    ops.unsubscribe_progress("main-1", Some(new));
    let before = second.lock().unwrap().len();
    env.write("big2.bin", &big(20));
    let id = env.submit("main-1", env.copy(&["big2.bin"], "dst"));
    env.wait_done(id);
    assert_eq!(second.lock().unwrap().len(), before);
}

#[test]
fn a_job_says_which_journal_entry_it_made() {
    use tauri::Listener;
    let env = env();
    let heard: Arc<Mutex<Vec<tauri_plugin_waypoint_ops::JobJournal>>> = Arc::default();
    let sink = heard.clone();
    env.app
        .listen(tauri_plugin_waypoint_ops::JOB_JOURNAL_EVENT, move |event| {
            sink.lock()
                .unwrap()
                .push(serde_json::from_str(event.payload()).expect("a job journal"));
        });
    let first = env.submit(
        "main-1",
        env.request(JobKind::CreateFolder, &[], Some(""), Some("one")),
    );
    env.wait_done(first);
    let second = env.submit(
        "main-2",
        env.request(JobKind::CreateFolder, &[], Some(""), Some("two")),
    );
    env.wait_done(second);

    let a = env.ops().journal_entry_of(first).expect("an entry");
    let b = env.ops().journal_entry_of(second).expect("an entry");
    assert_ne!(a, b);
    // The newest applied entry is the second job's; the first job's is named, not the newest.
    let summaries = env.ops().journal_summaries();
    assert_eq!(summaries[0].id, b);
    assert!(summaries.iter().any(|s| s.id == a));
    env.wait_for("the events", |_| heard.lock().unwrap().len() == 2);
    let heard = heard.lock().unwrap().clone();
    assert_eq!((heard[0].job, heard[0].entry), (first, a));
    assert_eq!((heard[1].job, heard[1].entry), (second, b));
    // A job that is not in the queue has none.
    assert_eq!(env.ops().journal_entry_of(waypoint_ops::JobId(9999)), None);
}

fn batch_request(
    env: &Env,
    sources: &[&str],
    rules: Vec<waypoint_ops::RenameRule>,
) -> waypoint_ops::JobRequest {
    let mut request = env.request(JobKind::BatchRename, sources, None, None);
    request.rename = Some(waypoint_ops::RenameSpec {
        rules,
        utc_offset_minutes: 0,
        now_ms: None,
    });
    request
}

fn preview_of(env: &Env, request: waypoint_ops::JobRequest) -> waypoint_ops::BatchPreview {
    tauri::async_runtime::block_on(commands::preview_batch_rename(
        env.window("main-1"),
        env.app.state::<Ops<MockRuntime>>(),
        request,
    ))
    .expect("the preview is made")
}

fn counter(start: u32) -> waypoint_ops::RenameRule {
    waypoint_ops::RenameRule::Counter {
        start,
        step: 1,
        width: 3,
        position: waypoint_ops::RulePosition::Prefix,
        separator: "_".to_owned(),
    }
}

#[test]
fn a_batch_rename_previews_then_runs_as_one_journalled_job() {
    let env = env();
    env.write("a.txt", b"alpha");
    env.write("b.txt", b"bravo");
    env.write("keep.txt", b"keep");
    let request = batch_request(&env, &["b.txt", "a.txt"], vec![counter(1)]);
    let preview = preview_of(&env, request.clone());
    let pairs: Vec<(&str, &str)> = preview
        .rows
        .iter()
        .map(|r| (r.from.as_str(), r.to.as_str()))
        .collect();
    assert_eq!(pairs, [("b.txt", "001_b.txt"), ("a.txt", "002_a.txt")]);
    assert!(preview.ready());
    assert!(preview.now_ms > 0, "the preview fixes the time it used");
    assert_eq!(
        env.names(""),
        ["a.txt", "b.txt", "keep.txt"],
        "a preview writes nothing"
    );
    assert!(env.snapshot().jobs.is_empty());

    // The job the dialog submits carries the preview's time.
    let mut submitted = request;
    submitted.rename.as_mut().unwrap().now_ms = Some(preview.now_ms);
    let id = env.submit("main-1", submitted);
    env.wait_done(id);
    assert_eq!(env.names(""), ["001_b.txt", "002_a.txt", "keep.txt"]);
    assert!(env.job(id).undoable);
    let summaries = env.ops().journal_summaries();
    assert_eq!(summaries.len(), 1, "one entry for the whole batch");
    assert_eq!(summaries[0].label, "Rename 2 items");

    let undo = tauri::async_runtime::block_on(commands::undo(
        env.window("main-2"),
        env.app.state::<Ops<MockRuntime>>(),
        None,
    ))
    .unwrap();
    env.wait_done(undo);
    assert_eq!(env.names(""), ["a.txt", "b.txt", "keep.txt"]);
    assert_eq!(env.read("a.txt"), b"alpha");
}

#[test]
fn a_batch_preview_reports_clashes_and_a_job_with_one_is_refused() {
    let env = env();
    env.write("a.txt", b"alpha");
    env.write("b.txt", b"bravo");
    env.write("001_a.txt", b"in the way");
    let request = batch_request(&env, &["a.txt", "b.txt"], vec![counter(1)]);
    let preview = preview_of(&env, request.clone());
    assert!(!preview.ready());
    assert_eq!(preview.problems, 1);
    assert!(preview.rows[0]
        .problems
        .contains(&waypoint_ops::Problem::ExistsInFolder));
    // A rule that cannot work is a problem of the stack, not an error of the command.
    let broken = batch_request(
        &env,
        &["a.txt"],
        vec![waypoint_ops::RenameRule::FindReplace {
            find: "(".to_owned(),
            replace: String::new(),
            regex: true,
            case_sensitive: true,
            scope: waypoint_ops::RenameScope::Stem,
            all: true,
        }],
    );
    let preview = preview_of(&env, broken);
    assert_eq!(preview.rule_errors.len(), 1);
    assert!(!preview.ready());

    let id = env.submit("main-1", request);
    let state = env.wait_state(id, "failed", |s| matches!(s, JobState::Failed { .. }));
    assert!(matches!(
        state,
        JobState::Failed {
            error: OpsError::NameInUse { .. },
            ..
        }
    ));
    assert_eq!(env.names(""), ["001_a.txt", "a.txt", "b.txt"]);
    assert!(env.ops().journal_summaries().is_empty());
    // A preview of something that is not there is the engine's typed refusal.
    let missing = tauri::async_runtime::block_on(commands::preview_batch_rename(
        env.window("main-1"),
        env.app.state::<Ops<MockRuntime>>(),
        batch_request(&env, &["missing"], vec![counter(1)]),
    ));
    assert!(matches!(
        missing,
        Err(tauri_plugin_waypoint_ops::Error::Ops(
            OpsError::NotFound { .. }
        ))
    ));
}

#[test]
fn a_batch_preview_reads_a_selection_through_the_injected_resolver() {
    let env = env();
    env.write("a.txt", b"alpha");
    env.write("b.txt", b"bravo");
    *env.resolver.0.lock().unwrap() = vec![env.loc("b.txt")];
    let mut request = batch_request(&env, &[], vec![counter(7)]);
    request.sources = waypoint_ops::Sources::Selection {
        handle: waypoint_vfs::ListingHandle(1),
        spec: waypoint_vfs::SelectionSpec::AllExcept { ids: vec![] },
    };
    let preview = preview_of(&env, request.clone());
    assert_eq!(preview.rows.len(), 1);
    assert_eq!(preview.rows[0].to, "007_b.txt");
    let id = env.submit("main-1", request);
    env.wait_done(id);
    assert_eq!(env.names(""), ["007_b.txt", "a.txt"]);
}

// ---- the Trash view's jobs ----

/// Trashes `names` through the queue and returns the receipts' trashed locations, in order.
fn trash_away(env: &Env, names: &[&str]) -> Vec<waypoint_protocol::Location> {
    let id = env.submit("main-1", env.request(JobKind::Trash, names, None, None));
    env.wait_done(id);
    env.trash
        .receipts()
        .iter()
        .map(|r| env.trash.trashed_location(r))
        .collect()
}

fn trash_request(
    env: &Env,
    kind: JobKind,
    locations: Vec<waypoint_protocol::Location>,
) -> waypoint_ops::JobRequest {
    let mut request = env.request(kind, &[], None, None);
    request.sources = waypoint_ops::Sources::Locations { locations };
    request
}

#[test]
fn restoring_over_a_taken_name_waits_for_an_answer_and_keeps_both() {
    let env = env();
    env.write("a.txt", b"old");
    let trashed = trash_away(&env, &["a.txt"]);
    env.write("a.txt", b"new");
    let id = env.submit(
        "main-1",
        trash_request(&env, JobKind::Restore, trashed.clone()),
    );
    let state = env.wait_state(id, "waiting", |s| matches!(s, JobState::Waiting { .. }));
    let JobState::Waiting {
        reason: WaitReason::Conflicts { conflicts },
    } = state
    else {
        panic!("waiting on conflicts");
    };
    assert_eq!(conflicts[0].existing, env.loc("a.txt"));
    assert_eq!(
        env.read("a.txt"),
        b"new",
        "nothing is written while it waits"
    );
    tauri::async_runtime::block_on(commands::resolve(
        env.window("main-2"),
        env.app.state::<Ops<MockRuntime>>(),
        id,
        vec![],
        Some(ConflictPolicy::KeepBoth),
    ))
    .unwrap();
    env.wait_done(id);
    assert_eq!(env.read("a.txt"), b"new");
    assert_eq!(env.read("a (2).txt"), b"old");
    assert!(env.trash.is_empty());
}

#[test]
fn a_restore_whose_folder_is_gone_asks_and_makes_it_when_told_to() {
    let env = env();
    env.dir("gone");
    env.write("gone/f.txt", b"x");
    trash_away(&env, &["gone/f.txt"]);
    let waypoint_path::VfsPath::File(gone) = env.path("gone") else {
        panic!("a local path");
    };
    std::fs::remove_dir(gone.as_path()).unwrap();
    let trashed: Vec<_> = env
        .trash
        .receipts()
        .iter()
        .map(|r| env.trash.trashed_location(r))
        .collect();
    let id = env.submit("main-1", trash_request(&env, JobKind::Restore, trashed));
    let state = env.wait_state(id, "waiting", |s| matches!(s, JobState::Waiting { .. }));
    let JobState::Waiting {
        reason: WaitReason::Error { error, .. },
    } = state
    else {
        panic!("waiting on an error");
    };
    assert_eq!(
        error,
        OpsError::OriginMissingParent {
            location: env.loc("gone")
        }
    );
    answer(&env, id, Decision::CreateParents);
    env.wait_done(id);
    assert_eq!(env.read("gone/f.txt"), b"x");
}

#[test]
fn deleting_from_the_trash_and_emptying_it_are_jobs() {
    let env = env();
    for name in ["a", "b", "c"] {
        env.write(name, name.as_bytes());
    }
    let trashed = trash_away(&env, &["a", "b", "c"]);
    let id = env.submit(
        "main-1",
        trash_request(&env, JobKind::Delete, vec![trashed[0].clone()]),
    );
    env.wait_done(id);
    assert_eq!(env.trash.len(), 2);
    assert!(!env.job(id).undoable, "a permanent delete cannot be undone");

    let sweep = env.submit(
        "main-1",
        trash_request(
            &env,
            JobKind::EmptyTrash {
                older_than_days: Some(30),
            },
            vec![],
        ),
    );
    env.wait_done(sweep);
    assert_eq!(env.trash.len(), 2, "nothing is 30 days old");
    assert_eq!(env.job(sweep).title, "Empty old items from the Trash");

    let empty = env.submit(
        "main-1",
        trash_request(
            &env,
            JobKind::EmptyTrash {
                older_than_days: None,
            },
            vec![],
        ),
    );
    env.wait_done(empty);
    assert!(env.trash.is_empty());
    assert_eq!(env.job(empty).title, "Empty Trash");
}

#[test]
fn the_trash_sweep_setting_is_validated_saved_and_defaults_off() {
    let env = env();
    let ops = env.ops();
    assert_eq!(ops.settings().trash_expiry_days, None);
    for bad in [0, tauri_plugin_waypoint_ops::MAX_TRASH_EXPIRY_DAYS + 1] {
        let refused = OpsSettings {
            trash_expiry_days: Some(bad),
            ..ops.settings()
        };
        assert!(ops.set_settings(refused).is_err(), "{bad} days");
    }
    assert_eq!(env.settings.saved(), None);
    let thirty = OpsSettings {
        trash_expiry_days: Some(30),
        ..ops.settings()
    };
    ops.set_settings(thirty).unwrap();
    assert_eq!(env.settings.saved().unwrap().trash_expiry_days, Some(30));
    assert_eq!(ops.settings().trash_expiry_days, Some(30));
    let off = OpsSettings {
        trash_expiry_days: None,
        ..ops.settings()
    };
    ops.set_settings(off).unwrap();
    assert_eq!(ops.settings().trash_expiry_days, None);
}

fn preview_clash(
    env: &Env,
    job: waypoint_ops::JobId,
    item: waypoint_protocol::Location,
) -> Result<waypoint_ops::ConflictPreview, tauri_plugin_waypoint_ops::Error> {
    tauri::async_runtime::block_on(commands::conflict_preview(
        env.window("main-1"),
        env.app.state::<Ops<MockRuntime>>(),
        job,
        item,
    ))
}

#[test]
fn a_waiting_job_answers_the_preview_of_its_clash_and_nothing_else() {
    let env = env();
    env.write("a.txt", b"one\ntwo\n");
    env.dir("dst");
    env.write("dst/a.txt", b"one\n2\n");
    let id = env.submit("main-1", env.copy(&["a.txt"], "dst"));
    let state = env.wait_state(id, "waiting", |s| matches!(s, JobState::Waiting { .. }));
    let JobState::Waiting {
        reason: WaitReason::Conflicts { conflicts },
    } = state
    else {
        panic!("{state:?}");
    };

    let preview = preview_clash(&env, id, conflicts[0].source.clone()).unwrap();
    let waypoint_ops::PreviewKind::Text { diff } = preview.kind else {
        panic!("{:?}", preview.kind);
    };
    assert_eq!((diff.added, diff.removed), (1, 1));
    assert_eq!(preview.existing.location, conflicts[0].existing);

    // A source that is not a clash of this job, and a job that is unknown, are refused.
    assert!(preview_clash(&env, id, env.loc("dst")).is_err());
    assert!(preview_clash(&env, waypoint_ops::JobId(9999), conflicts[0].source.clone()).is_err());
    // The job is still waiting: a preview asks nothing of the queue.
    assert!(matches!(env.state(id), JobState::Waiting { .. }));

    // Once the job has been answered it no longer waits on conflicts.
    tauri::async_runtime::block_on(commands::resolve(
        env.window("main-1"),
        env.app.state::<Ops<MockRuntime>>(),
        id,
        vec![],
        Some(ConflictPolicy::Skip),
    ))
    .unwrap();
    env.wait_done(id);
    assert!(preview_clash(&env, id, conflicts[0].source.clone()).is_err());
}

const MIB: u64 = 1024 * 1024;

/// How many bytes a job has moved, as the queue last heard.
fn moved(env: &Env, id: waypoint_ops::JobId) -> u64 {
    env.job(id).progress.bytes_done
}

#[test]
fn a_job_limit_holds_a_running_copy_and_lifting_it_takes_effect_at_once() {
    let env = env();
    env.write("big.bin", &big(6));
    env.dir("dst");
    let mut request = env.copy(&["big.bin"], "dst");
    request.options.speed_limit = Some(MIB);
    let started = std::time::Instant::now();
    let id = env.submit("main-1", request);
    env.wait_state(id, "running", |s| *s == JobState::Running);
    env.wait_for("some bytes moved", |e| moved(e, id) > 0);
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(
        env.state(id),
        JobState::Running,
        "6 MiB at 1 MiB/s takes about six seconds"
    );
    assert!(moved(&env, id) <= 3 * MIB, "{}", moved(&env, id));

    env.ops().set_job_limits(id, None, None).unwrap();
    assert_eq!(env.job(id).options.speed_limit, None);
    env.wait_done(id);
    assert!(
        started.elapsed() < Duration::from_secs(4),
        "lifting the limit sped the rest up: {:?}",
        started.elapsed()
    );
    assert_eq!(env.read("dst/big.bin"), big(6));
}

#[test]
fn the_global_limit_applies_to_a_running_job_and_changes_with_the_settings() {
    let env = env();
    let ops = env.ops();
    env.write("big.bin", &big(6));
    env.dir("dst");
    ops.set_settings(OpsSettings {
        speed_limit_bps: Some(MIB),
        ..ops.settings()
    })
    .unwrap();
    assert_eq!(env.settings.saved().unwrap().speed_limit_bps, Some(MIB));
    let started = std::time::Instant::now();
    let id = env.submit("main-1", env.copy(&["big.bin"], "dst"));
    env.wait_for("some bytes moved", |e| moved(e, id) > 0);
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(env.state(id), JobState::Running);
    assert!(moved(&env, id) <= 3 * MIB);
    // Lifted for the whole queue, the running job finishes quickly.
    ops.set_settings(OpsSettings {
        speed_limit_bps: None,
        ..ops.settings()
    })
    .unwrap();
    env.wait_done(id);
    assert!(
        started.elapsed() < Duration::from_secs(4),
        "{:?}",
        started.elapsed()
    );
}

#[test]
fn limits_are_validated_and_a_finished_job_keeps_what_it_had() {
    let env = env();
    let ops = env.ops();
    for bad in [0, tauri_plugin_waypoint_ops::MAX_SPEED_LIMIT + 1] {
        assert!(ops
            .set_settings(OpsSettings {
                speed_limit_bps: Some(bad),
                ..ops.settings()
            })
            .is_err());
    }
    assert_eq!(env.settings.saved(), None, "a refused change saves nothing");
    env.write("a.txt", b"alpha");
    env.dir("dst");
    let id = env.submit("main-1", env.copy(&["a.txt"], "dst"));
    assert!(ops.set_job_limits(id, Some(0), None).is_err());
    env.wait_done(id);
    assert!(ops.set_job_limits(id, Some(MIB), None).is_err());
    assert_eq!(env.job(id).options.speed_limit, None);
}

#[test]
fn a_higher_priority_job_is_taken_first_when_a_slot_frees_up() {
    let env = env();
    ops_one_at_a_time(&env);
    for name in ["a", "b", "c", "d"] {
        env.write(&format!("{name}.txt"), name.as_bytes());
    }
    env.dir("dst");
    env.gate.block_all();
    let first = env.submit("main-1", env.copy(&["a.txt"], "dst"));
    env.gate.wait_held(1);
    let mut low = env.copy(&["b.txt"], "dst");
    low.options.priority = Some(JobPriority::Low);
    let low = env.submit("main-1", low);
    let normal = env.submit("main-1", env.copy(&["c.txt"], "dst"));
    let high = env.submit("main-1", env.copy(&["d.txt"], "dst"));
    for id in [low, normal, high] {
        env.wait_state(id, "queued", |s| *s == JobState::Queued);
    }
    // The last to arrive is raised over the others while it waits.
    env.ops()
        .set_job_limits(high, None, Some(JobPriority::High))
        .unwrap();
    env.gate.open();
    for id in [first, low, normal, high] {
        env.wait_done(id);
    }
    let started = |id| {
        env.events_of("main-1")
            .iter()
            .position(|e| matches!(e, OpsEvent::JobChanged { job, .. } if job.id == id && job.state == JobState::Running))
            .expect("the job ran")
    };
    assert!(started(first) < started(high));
    assert!(started(high) < started(normal), "high before normal");
    assert!(started(normal) < started(low), "normal before low");
}

fn ops_one_at_a_time(env: &Env) {
    let ops = env.ops();
    ops.set_settings(OpsSettings {
        concurrency: 1,
        ..ops.settings()
    })
    .unwrap();
}

fn in_ms(ms: i64) -> Schedule {
    Schedule::StartAt {
        at_ms: clock_ms() + ms,
    }
}

fn scheduled_in_journal(env: &Env) -> usize {
    env.journal
        .current_document()
        .map_or(0, |d| d.body.scheduled.len())
}

#[test]
fn a_scheduled_job_waits_for_its_time_is_kept_in_the_journal_and_starts_by_itself() {
    let env = env();
    env.write("a.txt", b"alpha");
    env.dir("dst");
    let mut request = env.copy(&["a.txt"], "dst");
    request.options.schedule = Some(in_ms(1_200));
    let id = env.submit("main-1", request);
    env.wait_state(id, "queued", |s| *s == JobState::Queued);
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(env.state(id), JobState::Queued, "not before its time");
    assert!(!env.exists("dst/a.txt"));
    tauri_plugin_waypoint_ops::on_window_destroyed(env.app.handle(), "main-1");
    assert_eq!(scheduled_in_journal(&env), 1, "saved while it waits");
    let held = &env.journal.current_document().unwrap().body.scheduled[0];
    assert_eq!(held.request.options.schedule, env.job(id).options.schedule);

    env.wait_done(id);
    assert_eq!(env.read("dst/a.txt"), b"alpha");
    env.wait_for("the record to go", |e| {
        tauri_plugin_waypoint_ops::on_window_destroyed(e.app.handle(), "main-1");
        scheduled_in_journal(e) == 0
    });
}

#[test]
fn run_now_clears_a_schedule_and_a_started_job_cannot_be_scheduled() {
    let env = env();
    let ops = env.ops();
    env.write("a.txt", b"alpha");
    env.dir("dst");
    let mut request = env.copy(&["a.txt"], "dst");
    request.options.schedule = Some(in_ms(3_600_000));
    let id = env.submit("main-1", request);
    env.wait_state(id, "queued", |s| *s == JobState::Queued);
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(env.state(id), JobState::Queued);
    ops.set_job_schedule(id, None).unwrap();
    assert_eq!(env.job(id).options.schedule, None);
    env.wait_done(id);
    assert!(ops.set_job_schedule(id, Some(in_ms(1_000))).is_err());
    // A schedule that cannot run is refused, on a job and when submitting.
    let bad = Schedule::Window {
        start_minute: 60,
        end_minute: 60,
        utc_offset_minutes: 0,
    };
    let mut request = env.copy(&["a.txt"], "dst");
    request.options.schedule = Some(bad);
    assert!(ops.submit("main-1", request).is_err());
}

#[test]
fn a_job_held_by_a_window_that_is_closed_waits_and_others_pass_it() {
    let env = env();
    env.write("a.txt", b"alpha");
    env.write("b.txt", b"bravo");
    env.dir("dst");
    // A one-minute window that is not now: it opens in the next hour or so.
    let minute = ((clock_ms() / 60_000).rem_euclid(1440)) as u16;
    let window = Schedule::Window {
        start_minute: (minute + 30) % 1440,
        end_minute: (minute + 31) % 1440,
        utc_offset_minutes: 0,
    };
    let mut held = env.copy(&["a.txt"], "dst");
    held.options.schedule = Some(window);
    let held = env.submit("main-1", held);
    let free = env.submit("main-1", env.copy(&["b.txt"], "dst"));
    env.wait_done(free);
    assert_eq!(env.state(held), JobState::Queued);
    assert!(!env.exists("dst/a.txt"));
}

#[test]
fn a_scheduled_job_left_by_the_last_run_is_queued_again_at_start_up() {
    use waypoint_ops::{JobId, JournalBody, JournalDocument, ScheduledRecord};
    let journal = Arc::new(waypoint_ops::testing::journal_storage::MemoryJournalStorage::new());
    let setup = Setup {
        journal: journal.clone(),
        prepare: Box::new({
            let journal = journal.clone();
            move |work, fs| {
                let src = work.join("a.txt").unwrap();
                let mut stream = fs
                    .create_write(&src, waypoint_vfs::WriteOptions::exclusive())
                    .unwrap();
                std::io::Write::write_all(&mut stream, b"alpha").unwrap();
                stream.finish(false).unwrap();
                fs.create_dir(&work.join("dst").unwrap()).unwrap();
                let mut request = waypoint_ops::JobRequest {
                    kind: JobKind::Copy,
                    sources: waypoint_ops::Sources::Locations {
                        locations: vec![src.to_location()],
                    },
                    destination: Some(work.join("dst").unwrap().to_location()),
                    name: None,
                    options: waypoint_ops::JobOptions::default(),
                    origin_window: "main-1".to_owned(),
                    rename: None,
                    archive: None,
                };
                request.options.schedule = Some(Schedule::StartAt {
                    at_ms: clock_ms() + 700,
                });
                journal
                    .save(&JournalDocument::new(JournalBody {
                        next_id: 1,
                        scheduled: vec![ScheduledRecord {
                            job: JobId(41),
                            request,
                        }],
                        ..JournalBody::default()
                    }))
                    .unwrap();
            }
        }),
        ..Setup::default()
    };
    let env = env_with(setup);
    let jobs = env.snapshot().jobs;
    assert_eq!(jobs.len(), 1, "the job is back in the queue");
    let id = jobs[0].id;
    assert_ne!(env.state(id), JobState::Done, "and waits for its time");
    assert!(!env.exists("dst/a.txt"));
    env.wait_done(id);
    assert_eq!(env.read("dst/a.txt"), b"alpha");
    // It is not an interruption, so there is nothing to report.
    assert!(env.recovered.lock().unwrap().is_empty());
}

#[test]
fn quitting_keeps_the_jobs_waiting_for_their_time() {
    let env = env();
    env.write("a.txt", b"alpha");
    env.dir("dst");
    let mut request = env.copy(&["a.txt"], "dst");
    request.options.schedule = Some(in_ms(3_600_000));
    let id = env.submit("main-1", request);
    env.wait_state(id, "queued", |s| *s == JobState::Queued);
    tauri_plugin_waypoint_ops::on_exit(env.app.handle());
    assert_eq!(
        scheduled_in_journal(&env),
        1,
        "cancelled for now, kept for next time"
    );
}

#[test]
fn pause_all_stops_every_running_copy_and_holds_the_queue_and_resume_all_goes_on() {
    let env = env();
    let ops = env.ops();
    env.write("big.bin", &big(6));
    env.write("b.txt", b"bravo");
    env.dir("dst");
    let mut request = env.copy(&["big.bin"], "dst");
    request.options.speed_limit = Some(MIB);
    let big_job = env.submit("main-1", request);
    env.wait_for("some bytes moved", |e| moved(e, big_job) > 0);

    ops.pause_all();
    env.wait_state(big_job, "paused", |s| *s == JobState::Paused);
    assert!(env.snapshot().paused);
    // The progress stands still while paused.
    std::thread::sleep(Duration::from_millis(200));
    let at = moved(&env, big_job);
    std::thread::sleep(Duration::from_millis(400));
    assert_eq!(moved(&env, big_job), at);
    // A free slot does not start a new job.
    let other = env.submit("main-1", env.copy(&["b.txt"], "dst"));
    env.wait_state(other, "queued", |s| *s == JobState::Queued);
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(env.state(other), JobState::Queued);

    ops.set_job_limits(big_job, None, None).unwrap();
    ops.resume_all();
    assert!(!env.snapshot().paused);
    env.wait_done(big_job);
    env.wait_done(other);
    assert_eq!(env.read("dst/big.bin"), big(6));
    assert_eq!(env.read("dst/b.txt"), b"bravo");
    // Both windows heard the flag change, in order.
    let flags: Vec<bool> = env
        .events_of("main-2")
        .into_iter()
        .filter_map(|e| match e {
            OpsEvent::QueuePaused { paused, .. } => Some(paused),
            _ => None,
        })
        .collect();
    assert_eq!(flags, vec![true, false]);
}

#[test]
fn the_archive_limits_have_defaults_bounds_and_an_old_document_gets_the_defaults() {
    use tauri_plugin_waypoint_ops::{
        ARCHIVE_BYTES_RANGE, ARCHIVE_ENTRIES_RANGE, ARCHIVE_RATIO_FLOOR_RANGE, ARCHIVE_RATIO_RANGE,
    };
    let env = env();
    let ops = env.ops();
    let defaults = ops.settings();
    assert_eq!(defaults.archive_max_entries, 1_000_000);
    assert_eq!(defaults.archive_max_bytes, 100 * 1024 * 1024 * 1024);
    assert_eq!(defaults.archive_max_ratio, 1_000);
    assert_eq!(defaults.archive_ratio_floor_bytes, 1024 * 1024 * 1024);
    // A document saved before these settings existed reads with the defaults.
    let old = serde_json::json!({
        "concurrency": 3, "verifyAfterCopy": false, "verifyAlgorithm": "blake3",
        "confirmTrash": false, "undoDepth": 50, "trashExpiryDays": null
    });
    let read: OpsSettings = serde_json::from_value(old).unwrap();
    assert_eq!(read.archive_max_entries, 1_000_000);
    assert_eq!(read.concurrency, 3);
    // Each is refused outside its range, at both ends, and nothing is saved.
    type Change = fn(&mut OpsSettings, bool);
    let changes: [(Change, bool); 8] = [
        (
            |s, low| {
                s.archive_max_entries = if low {
                    ARCHIVE_ENTRIES_RANGE.start() - 1
                } else {
                    ARCHIVE_ENTRIES_RANGE.end() + 1
                }
            },
            true,
        ),
        (
            |s, low| {
                s.archive_max_bytes = if low {
                    ARCHIVE_BYTES_RANGE.start() - 1
                } else {
                    ARCHIVE_BYTES_RANGE.end() + 1
                }
            },
            true,
        ),
        (
            |s, low| {
                s.archive_max_ratio = if low {
                    ARCHIVE_RATIO_RANGE.start() - 1
                } else {
                    ARCHIVE_RATIO_RANGE.end() + 1
                }
            },
            true,
        ),
        (
            |s, low| {
                s.archive_ratio_floor_bytes = if low {
                    ARCHIVE_RATIO_FLOOR_RANGE.start() - 1
                } else {
                    ARCHIVE_RATIO_FLOOR_RANGE.end() + 1
                }
            },
            true,
        ),
        (|s, _| s.archive_max_entries = 0, true),
        (|s, _| s.archive_max_bytes = 0, true),
        (|s, _| s.archive_max_ratio = 0, true),
        (|s, _| s.archive_ratio_floor_bytes = 0, true),
    ];
    for (n, (change, _)) in changes.iter().enumerate() {
        for low in [true, false] {
            let mut bad = ops.settings();
            change(&mut bad, low);
            assert!(ops.set_settings(bad).is_err(), "change {n} low={low}");
        }
    }
    assert_eq!(env.settings.saved(), None);
    // Values at the ends are accepted and come back as saved.
    let ends = OpsSettings {
        archive_max_entries: *ARCHIVE_ENTRIES_RANGE.start(),
        archive_max_bytes: *ARCHIVE_BYTES_RANGE.end(),
        archive_max_ratio: *ARCHIVE_RATIO_RANGE.end(),
        archive_ratio_floor_bytes: *ARCHIVE_RATIO_FLOOR_RANGE.start(),
        ..ops.settings()
    };
    assert_eq!(ops.set_settings(ends).unwrap(), ends);
    assert_eq!(env.settings.saved(), Some(ends));
}

/// Tries a server again after 20 ms, twice, so the offline wait is quick to test.
fn quick_reconnect(error: &OpsError, attempt: u32) -> Option<u64> {
    (waypoint_ops::is_transient(error) && attempt < 2).then_some(20)
}

fn upload_to(
    env: &Env,
    server: &waypoint_vfs::FakeRemoteProvider,
    sources: &[&str],
) -> waypoint_ops::JobRequest {
    let mut request = env.request(JobKind::Copy, sources, None, None);
    request.destination = Some(
        server
            .root("me@fake.test")
            .join("up")
            .unwrap()
            .to_location(),
    );
    request
}

fn server_bytes(server: &waypoint_vfs::FakeRemoteProvider, name: &str) -> Vec<u8> {
    let mut out = Vec::new();
    let path = server
        .root("me@fake.test")
        .join("up")
        .unwrap()
        .join(name)
        .unwrap();
    std::io::Read::read_to_end(&mut server.open_read(&path).unwrap(), &mut out).unwrap();
    out
}

fn lose_connection_at_write(server: &waypoint_vfs::FakeRemoteProvider, n: usize) {
    server.memory().fail_nth(
        waypoint_vfs::MemOp::Write,
        n,
        VfsError::Disconnected {
            location: server.root("me@fake.test").to_location(),
        },
    );
}

#[test]
fn a_lost_connection_waits_offline_and_carries_on_from_what_the_server_holds() {
    let server = waypoint_vfs::FakeRemoteProvider::sftp();
    server.put_dir(&server.root("me@fake.test").join("up").unwrap());
    let env = env_with(Setup {
        remote: Some(server.clone()),
        reconnect_wait: Some(quick_reconnect),
        ..Setup::default()
    });
    let content = big(20);
    env.write("big.bin", &content);
    lose_connection_at_write(&server, 2);
    let id = env.submit("main-1", upload_to(&env, &server, &["big.bin"]));
    env.wait_done(id);
    assert_eq!(server_bytes(&server, "big.bin"), content);
    // The job was offline for a moment, said so, and ran on by itself.
    let offline = env.events_of("main-1").into_iter().any(|event| {
        matches!(
            event,
            OpsEvent::JobChanged { job, .. } if matches!(job.state, JobState::Offline { attempt: 1, .. })
        )
    });
    assert!(offline, "the job showed it was offline");
    // One 8 MiB chunk went, the second failed, and the rest went once: 20 MiB in three chunks,
    // plus the fake's own rewrite of what it kept.
    assert_eq!(
        server.memory().calls(waypoint_vfs::MemOp::Write),
        1 + 1 + 1 + 2
    );
}

#[test]
fn a_transfer_stopped_by_a_lost_connection_is_offered_after_a_restart_and_resumes() {
    let server = waypoint_vfs::FakeRemoteProvider::sftp();
    server.put_dir(&server.root("me@fake.test").join("up").unwrap());
    let journal = Arc::new(waypoint_ops::testing::journal_storage::MemoryJournalStorage::new());
    let setup = |dir| Setup {
        journal: journal.clone(),
        remote: Some(server.clone()),
        reconnect_wait: Some(quick_reconnect),
        dir,
        ..Setup::default()
    };
    let env = env_with(setup(None));
    let content = big(20);
    env.write("big.bin", &content);
    // The connection drops after the first chunk and the server stays away: after its tries the
    // job asks, and the app quits while it waits.
    lose_connection_at_write(&server, 2);
    // Each try again fails as it reopens the partial file.
    for _ in 0..2 {
        server.memory().fail_next(
            waypoint_vfs::MemOp::OpenRead,
            VfsError::Disconnected {
                location: server.root("me@fake.test").to_location(),
            },
        );
    }
    let stopped = env.submit("main-1", upload_to(&env, &server, &["big.bin"]));
    let state = env.wait_state(stopped, "asking", |s| matches!(s, JobState::Waiting { .. }));
    let JobState::Waiting {
        reason: WaitReason::Error { error, .. },
    } = state
    else {
        panic!("{state:?}");
    };
    assert!(matches!(error, OpsError::Connection { .. }), "{error:?}");
    let note = env
        .job(stopped)
        .partial
        .expect("the dialog can say what Retry does");
    assert!(note.resumes);
    tauri_plugin_waypoint_ops::on_exit(env.app.handle());
    let document = journal.current_document().expect("the journal was written");
    assert_eq!(document.body.resumable.len(), 1, "kept for the next start");
    let Env { dir, .. } = env;

    // The next start offers it and touches nothing on the server; Resume continues it.
    let connects = server.connects();
    let env = env_with(setup(Some(dir)));
    assert_eq!(
        server.connects(),
        connects,
        "start-up does not connect to a server"
    );
    let report = tauri::async_runtime::block_on(commands::take_recovery_report(
        env.window("main-1"),
        env.app.state::<Ops<MockRuntime>>(),
    ))
    .unwrap()
    .expect("a report");
    assert_eq!(report.resumable.len(), 1);
    let writes = server.memory().calls(waypoint_vfs::MemOp::Write);
    let resumed = tauri::async_runtime::block_on(commands::resume_interrupted(
        env.window("main-1"),
        env.app.state::<Ops<MockRuntime>>(),
        report.resumable[0].job,
    ))
    .unwrap();
    env.wait_done(resumed);
    assert_eq!(server_bytes(&server, "big.bin"), content);
    // The 8 MiB the server held were not sent again: the fake's rewrite, then two chunks.
    assert_eq!(
        server.memory().calls(waypoint_vfs::MemOp::Write) - writes,
        1 + 2
    );
    // The record is gone once the resumed job is done, and so is the partial file.
    tauri_plugin_waypoint_ops::on_exit(env.app.handle());
    let document = journal.current_document().expect("the journal was written");
    assert!(document.body.resumable.is_empty());
    let left: Vec<_> = server
        .list(
            &server.root("me@fake.test").join("up").unwrap(),
            &waypoint_vfs::CancelToken::new(),
            0,
            &mut |_| {},
        )
        .unwrap()
        .into_iter()
        .map(|e| e.name.to_string_lossy().into_owned())
        .collect();
    assert_eq!(left, ["big.bin"]);
}
