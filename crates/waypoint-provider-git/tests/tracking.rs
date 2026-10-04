// Watch-driven status: a change in the working tree reaches the listener within a second, only
// what changed is recomputed, and quiet places stay quiet.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod support;

use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::time::{Duration, Instant};

use support::*;
use waypoint_provider_git::{
    compute, Change, EntryStatus, HeadState, Snapshot, StatusOptions, TrackEvent, Tracker,
    TrackerOptions,
};
use waypoint_vfs::CancelToken;

struct Events(Receiver<TrackEvent>);

struct Watching {
    tracker: Tracker,
    events: Events,
}

impl std::ops::Deref for Watching {
    type Target = Events;
    fn deref(&self) -> &Events {
        &self.events
    }
}

fn watch(repo: &Repo, options: TrackerOptions) -> Watching {
    let (tx, events) = mpsc::channel();
    let tracker = Tracker::start(
        repo.path().to_path_buf(),
        options,
        Arc::new(move |event| {
            let _ = tx.send(event);
        }),
    );
    Watching {
        tracker,
        events: Events(events),
    }
}

impl Events {
    /// Waits for an update that `accept`s, returning it and how long it took.
    fn until(
        &self,
        what: &str,
        limit: Duration,
        accept: impl Fn(&Snapshot) -> bool,
    ) -> (Arc<Snapshot>, Duration) {
        let started = Instant::now();
        loop {
            let left = limit.saturating_sub(started.elapsed());
            match self.0.recv_timeout(left) {
                Ok(TrackEvent::Updated(snapshot)) if accept(&snapshot) => {
                    return (snapshot, started.elapsed())
                }
                Ok(TrackEvent::Failed { message }) => panic!("status failed: {message}"),
                Ok(_) => {}
                Err(_) => panic!("no update for: {what}"),
            }
        }
    }

    fn first(&self) -> Arc<Snapshot> {
        self.until("the first status", Duration::from_secs(10), |_| true)
            .0
    }

    /// Whether no update arrives for `quiet`.
    fn stays_quiet(&self, quiet: Duration) -> bool {
        let started = Instant::now();
        while started.elapsed() < quiet {
            if let Ok(TrackEvent::Updated(_)) =
                self.0.recv_timeout(quiet.saturating_sub(started.elapsed()))
            {
                return false;
            }
        }
        true
    }
}

const SECOND: Duration = Duration::from_secs(1);

type Step = Box<dyn Fn(&Repo)>;

fn clean_repo() -> Option<Repo> {
    let repo = repo()?;
    repo.write(".gitignore", "target/\n");
    repo.write("a.txt", "a");
    repo.write("src/lib.rs", "lib");
    repo.write("src/deep/x.rs", "x");
    repo.commit_all("base");
    Some(repo)
}

fn entry(snapshot: &Snapshot, path: &str) -> Option<EntryStatus> {
    snapshot.status.entry(path.as_bytes()).copied()
}

#[test]
fn the_first_status_arrives_by_itself_and_is_readable_without_waiting() {
    let Some(repo) = clean_repo() else { return };
    let w = watch(&repo, TrackerOptions::default());
    let first = w.first();
    assert_eq!(first.revision, 1);
    assert!(first.status.is_empty());
    assert_eq!(first.summary.head, HeadState::Branch("main".into()));
    assert_eq!(w.tracker.snapshot().unwrap().revision, 1);
}

#[test]
fn an_edit_shows_within_a_second() {
    let Some(repo) = clean_repo() else { return };
    let w = watch(&repo, TrackerOptions::default());
    w.first();
    let started = Instant::now();
    repo.write("src/lib.rs", "edited");
    let (snapshot, _) = w.until("the edit", Duration::from_secs(5), |s| {
        entry(s, "src/lib.rs").is_some()
    });
    let took = started.elapsed();
    assert!(took < SECOND, "the change took {took:?} to show");
    assert_eq!(
        entry(&snapshot, "src/lib.rs").unwrap().unstaged,
        Some(Change::Modified)
    );
    assert_eq!(snapshot.status.badge(b"src").unwrap().changed, 1);
    assert_eq!(snapshot.revision, 2);
}

#[test]
fn creating_deleting_and_reverting_files_are_all_seen() {
    let Some(repo) = clean_repo() else { return };
    let w = watch(&repo, TrackerOptions::default());
    w.first();
    repo.write("new.txt", "n");
    let (s, _) = w.until("a new file", Duration::from_secs(5), |s| {
        entry(s, "new.txt").is_some()
    });
    assert_eq!(entry(&s, "new.txt"), Some(EntryStatus::untracked()));
    repo.remove("a.txt");
    let (s, _) = w.until("a deletion", Duration::from_secs(5), |s| {
        entry(s, "a.txt").is_some()
    });
    assert_eq!(entry(&s, "a.txt").unwrap().unstaged, Some(Change::Deleted));
    repo.write("a.txt", "a");
    w.until("a restored file", Duration::from_secs(5), |s| {
        entry(s, "a.txt").is_none()
    });
}

#[test]
fn a_folder_made_after_the_start_is_watched_too() {
    let Some(repo) = clean_repo() else { return };
    let w = watch(&repo, TrackerOptions::default());
    w.first();
    repo.write("fresh/inner.txt", "1");
    w.until("the new folder", Duration::from_secs(5), |s| {
        entry(s, "fresh") == Some(EntryStatus::untracked())
    });
    // Once the folder is tracked, a change inside it is seen by the watch added for it.
    repo.git(&["add", "fresh"]);
    repo.commit_all("fresh");
    w.until("the commit", Duration::from_secs(5), |s| {
        s.status.is_empty()
    });
    repo.write("fresh/inner.txt", "2");
    w.until("an edit in the new folder", Duration::from_secs(5), |s| {
        entry(s, "fresh/inner.txt").is_some()
    });
}

#[test]
fn staging_committing_and_switching_branches_follow_the_index_and_head() {
    let Some(repo) = clean_repo() else { return };
    let w = watch(&repo, TrackerOptions::default());
    w.first();
    repo.write("a.txt", "changed");
    w.until("the edit", Duration::from_secs(5), |s| {
        entry(s, "a.txt").is_some()
    });
    repo.git(&["add", "a.txt"]);
    let (s, _) = w.until("the staging", Duration::from_secs(5), |s| {
        entry(s, "a.txt").is_some_and(|e| e.staged == Some(Change::Modified))
    });
    assert_eq!(entry(&s, "a.txt").unwrap().unstaged, None);
    repo.git(&["commit", "-q", "-m", "change"]);
    let (s, _) = w.until("the commit", Duration::from_secs(5), |s| {
        s.status.is_empty()
    });
    assert!(!s.summary.is_dirty());
    repo.git(&["checkout", "-q", "-b", "feature"]);
    let (s, _) = w.until("the branch switch", Duration::from_secs(5), |s| {
        s.summary.head == HeadState::Branch("feature".into())
    });
    assert!(s.status.is_empty());
}

#[test]
fn changes_in_an_ignored_folder_make_no_noise() {
    let Some(repo) = clean_repo() else { return };
    std::fs::create_dir_all(repo.path().join("target/debug")).unwrap();
    repo.write("target/debug/old.o", "o");
    let w = watch(&repo, TrackerOptions::default());
    let first = w.first();
    assert_eq!(
        first.status.entry(b"target"),
        Some(&EntryStatus::ignored()),
        "the ignored folder is one entry"
    );
    for n in 0..20 {
        repo.write(&format!("target/debug/build{n}.o"), "built");
    }
    assert!(
        w.stays_quiet(Duration::from_millis(700)),
        "an ignored folder's changes must not recompute anything"
    );
    // A folder that becomes ignored later is pruned from the watch after the next status.
    repo.write("other.txt", "o");
    w.until("a normal change still works", Duration::from_secs(5), |s| {
        entry(s, "other.txt").is_some()
    });
}

#[test]
fn a_gitignore_change_reclassifies_files_at_once() {
    let Some(repo) = clean_repo() else { return };
    repo.write("junk.tmp", "j");
    let w = watch(&repo, TrackerOptions::default());
    let first = w.first();
    assert_eq!(entry(&first, "junk.tmp"), Some(EntryStatus::untracked()));
    repo.write(".gitignore", "target/\n*.tmp\n");
    w.until("the ignore rule", Duration::from_secs(5), |s| {
        entry(s, "junk.tmp") == Some(EntryStatus::ignored())
    });
}

#[test]
fn a_burst_of_changes_is_one_update() {
    let Some(repo) = clean_repo() else { return };
    let w = watch(&repo, TrackerOptions::default());
    w.first();
    for n in 0..50 {
        repo.write(&format!("burst/f{n}.txt"), "b");
    }
    let (s, _) = w.until("the burst", Duration::from_secs(5), |s| {
        entry(s, "burst").is_some()
    });
    assert_eq!(s.revision, 2, "50 writes became one recompute, not 50");
    assert!(w.stays_quiet(Duration::from_millis(500)));
}

#[test]
fn a_scoped_refresh_gives_what_a_full_one_would() {
    let Some(repo) = clean_repo() else { return };
    let w = watch(&repo, TrackerOptions::default());
    w.first();
    // A mix of operations, applied in small steps so each is a scoped refresh.
    let steps: Vec<Step> = vec![
        Box::new(|r| r.write("src/lib.rs", "one")),
        Box::new(|r| r.write("src/new/file.txt", "n")),
        Box::new(|r| r.write("src/new/more/file.txt", "n")),
        Box::new(|r| r.remove("src/deep/x.rs")),
        Box::new(|r| r.write("top.txt", "t")),
        Box::new(|r| std::fs::remove_dir_all(r.path().join("src/new")).unwrap()),
        Box::new(|r| r.write("src/deep/y.rs", "y")),
        Box::new(|r| r.write("a.txt", "changed")),
        Box::new(|r| r.write("target/out.o", "ignored")),
        Box::new(|r| r.remove("top.txt")),
    ];
    for (n, step) in steps.iter().enumerate() {
        step(&repo);
        // Let the burst settle, then compare with a status computed from scratch.
        std::thread::sleep(Duration::from_millis(600));
        let full = compute(
            repo.path(),
            &StatusOptions::default(),
            &CancelToken::new(),
            None,
        )
        .unwrap();
        let seen = w.tracker.snapshot().unwrap();
        assert_eq!(
            *seen.status, full,
            "after step {n} the scoped refresh and a full status disagree"
        );
    }
}

#[test]
fn polling_is_the_fallback_when_watching_is_off() {
    let Some(repo) = clean_repo() else { return };
    let w = watch(
        &repo,
        TrackerOptions {
            watch: false,
            poll_interval: Duration::from_millis(200),
            ..TrackerOptions::default()
        },
    );
    w.first();
    repo.write("a.txt", "edited");
    w.until("a polled change", Duration::from_secs(5), |s| {
        entry(s, "a.txt").is_some()
    });
}

#[test]
fn dropping_the_tracker_stops_it() {
    let Some(repo) = clean_repo() else { return };
    let w = watch(&repo, TrackerOptions::default());
    w.first();
    let Watching { tracker, events } = w;
    drop(tracker);
    let events = events.0;
    repo.write("a.txt", "edited");
    // The sender is dropped with the thread, so the channel ends rather than delivering.
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        match events.recv_timeout(Duration::from_millis(200)) {
            Ok(TrackEvent::Updated(_)) => panic!("an update after the tracker was dropped"),
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
            _ => {}
        }
        assert!(Instant::now() < deadline, "the tracker thread did not stop");
    }
}

#[test]
fn a_refresh_on_request_recomputes_everything() {
    let Some(repo) = clean_repo() else { return };
    let w = watch(
        &repo,
        TrackerOptions {
            watch: false,
            poll_interval: Duration::from_secs(3600),
            ..TrackerOptions::default()
        },
    );
    w.first();
    repo.write("a.txt", "edited");
    assert!(
        w.stays_quiet(Duration::from_millis(300)),
        "not watched, not polled"
    );
    w.tracker.refresh();
    w.until("the refresh", Duration::from_secs(5), |s| {
        entry(s, "a.txt").is_some()
    });
}

#[test]
fn many_listeners_share_one_tracker_and_the_last_one_leaving_stops_it() {
    use waypoint_provider_git::StatusService;
    let Some(repo) = clean_repo() else { return };
    let service = StatusService::new(TrackerOptions::default());
    assert_eq!(
        service
            .repository_of(&repo.path().join("src/deep"))
            .as_deref(),
        Some(repo.path())
    );
    let listen = || {
        let (tx, rx) = mpsc::channel();
        let sink: waypoint_provider_git::TrackSink = Arc::new(move |event| {
            let _ = tx.send(event);
        });
        (sink, Events(rx))
    };
    let (sink_one, one_events) = listen();
    let one = service.subscribe(repo.path(), sink_one);
    one_events.until("the first status", Duration::from_secs(10), |_| true);

    // A second listener gets the snapshot at once, and both hear the next change.
    let (sink_two, two_events) = listen();
    let two = service.subscribe(repo.path(), sink_two);
    let (joined, _) = two_events.until("the current snapshot", Duration::from_secs(2), |_| true);
    assert_eq!(joined.revision, 1);
    assert_eq!(service.tracked(), 1);
    repo.write("a.txt", "edited");
    one_events.until("one hears the change", Duration::from_secs(5), |s| {
        entry(s, "a.txt").is_some()
    });
    two_events.until("two hears the change", Duration::from_secs(5), |s| {
        entry(s, "a.txt").is_some()
    });
    assert!(one.snapshot().is_some());
    drop(one);
    assert_eq!(service.tracked(), 1, "one listener is left");
    drop(two);
    assert_eq!(service.tracked(), 0, "the last listener stops the tracker");
}
