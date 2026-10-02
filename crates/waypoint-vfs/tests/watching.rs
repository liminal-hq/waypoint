// Headless tests of live listings: files change under an open listing and the patches follow.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::ffi::OsString;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tempfile::TempDir;
use waypoint_path::{FilePath, VfsPath};
use waypoint_protocol::{EntryId, VfsError};
use waypoint_vfs::{
    CancelToken, Capabilities, Change, Filter, Listing, ListingEvent, ListingHandle,
    ListingOptions, ListingPhase, LocalProvider, PatchOp, Provider, ScannedEntry, SortSpec, Watch,
    WatchEvent, WatchMode, WatchOptions, WatchSink, WatchState,
};

/// Every wait is bounded, so a missed event fails a test instead of hanging CI.
const PATIENCE: Duration = Duration::from_secs(10);

type Events = Arc<Mutex<Vec<ListingEvent>>>;

fn fast() -> WatchOptions {
    WatchOptions {
        debounce: Duration::from_millis(20),
        max_wait: Duration::from_millis(200),
        rename_grace: Duration::from_millis(20),
        poll_interval: Duration::from_millis(50),
        ..WatchOptions::default()
    }
}

fn path_of(dir: &Path) -> VfsPath {
    VfsPath::File(FilePath::from_path(dir).unwrap())
}

fn open(provider: Arc<dyn Provider>, dir: &Path) -> (Arc<Listing>, Events) {
    let events: Events = Arc::default();
    let sink = {
        let events = events.clone();
        Arc::new(move |event| events.lock().unwrap().push(event))
    };
    let listing = Listing::open(
        ListingHandle(1),
        path_of(dir),
        provider,
        SortSpec::default(),
        Filter::default(),
        ListingOptions::default(),
        sink,
    )
    .unwrap();
    (listing, events)
}

fn watched(dir: &Path) -> (Arc<Listing>, Events) {
    open(Arc::new(LocalProvider::with_watch_options(fast())), dir)
}

fn wait_until(what: &str, mut condition: impl FnMut() -> bool) {
    let started = Instant::now();
    while !condition() {
        assert!(started.elapsed() < PATIENCE, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn names(listing: &Listing) -> Vec<String> {
    listing
        .get_range(0, u32::MAX)
        .into_iter()
        .map(|e| e.name)
        .collect()
}

fn changed(events: &Events) -> Vec<Vec<PatchOp>> {
    events
        .lock()
        .unwrap()
        .iter()
        .filter_map(|e| match e {
            ListingEvent::Changed { ops, .. } => Some(ops.clone()),
            _ => None,
        })
        .collect()
}

fn touch(dir: &Path, name: &str, bytes: usize) {
    fs::write(dir.join(name), vec![b'x'; bytes]).unwrap();
}

#[test]
fn a_created_file_is_inserted_at_its_sorted_position() {
    let dir = TempDir::new().unwrap();
    touch(dir.path(), "a", 1);
    touch(dir.path(), "c", 1);
    let (listing, events) = watched(dir.path());
    assert_eq!(listing.watch_state(), WatchState::Native);

    touch(dir.path(), "b", 1);
    wait_until("the insert", || names(&listing).len() == 3);
    // The listing is updated before the sink is told, so the event can trail the state.
    wait_until("the insert's event", || !changed(&events).is_empty());
    assert_eq!(names(&listing), ["a", "b", "c"]);
    assert_eq!(
        changed(&events).concat(),
        [PatchOp::Insert { at: 1, count: 1 }]
    );
    let event = events.lock().unwrap().last().cloned().unwrap();
    let ListingEvent::Changed {
        revision, count, ..
    } = event
    else {
        panic!("expected a change, got {event:?}");
    };
    assert_eq!((revision, count), (listing.snapshot().revision, 3));
}

#[test]
fn a_deleted_file_is_removed_from_its_position() {
    let dir = TempDir::new().unwrap();
    for name in ["a", "b", "c"] {
        touch(dir.path(), name, 1);
    }
    let (listing, events) = watched(dir.path());
    fs::remove_file(dir.path().join("b")).unwrap();
    wait_until("the removal", || names(&listing).len() == 2);
    wait_until("the removal's event", || !changed(&events).is_empty());
    assert_eq!(names(&listing), ["a", "c"]);
    assert_eq!(
        changed(&events).concat(),
        [PatchOp::Remove { at: 1, count: 1 }]
    );
}

#[test]
fn a_changed_file_is_updated_in_place() {
    let dir = TempDir::new().unwrap();
    touch(dir.path(), "a", 1);
    touch(dir.path(), "b", 1);
    let (listing, events) = watched(dir.path());
    touch(dir.path(), "b", 500);
    wait_until("the update", || {
        listing.get_range(1, 1).first().and_then(|e| e.size) == Some(500)
    });
    wait_until("the update's event", || !changed(&events).is_empty());
    assert_eq!(
        changed(&events).concat(),
        [PatchOp::Update { at: 1, count: 1 }]
    );
}

#[test]
fn a_renamed_file_keeps_its_entry_id() {
    let dir = TempDir::new().unwrap();
    for name in ["a", "b", "c"] {
        touch(dir.path(), name, 1);
    }
    let (listing, events) = watched(dir.path());
    let before = listing.id_of(&OsString::from("a")).unwrap();

    fs::rename(dir.path().join("a"), dir.path().join("z")).unwrap();
    wait_until("the rename", || names(&listing) == ["b", "c", "z"]);
    wait_until("the rename's events", || {
        changed(&events).concat().len() >= 2
    });
    assert_eq!(listing.id_of(&OsString::from("z")), Some(before));
    assert_eq!(listing.id_of(&OsString::from("a")), None);
    assert_eq!(listing.position_of(before), Some(2));
    // Moving in the order is a removal and an insertion of the same id, not a reset.
    assert_eq!(
        changed(&events).concat(),
        [
            PatchOp::Remove { at: 0, count: 1 },
            PatchOp::Insert { at: 2, count: 1 },
        ]
    );
    // The event says the entry moved, so a selection keyed by its id survives.
    assert!(events.lock().unwrap().iter().any(|e| matches!(
        e,
        ListingEvent::Changed { moved, .. } if moved == &[before]
    )));
    let entry = &listing.get_range(2, 1)[0];
    assert_eq!((entry.id, entry.name.as_str()), (before, "z"));
}

#[test]
fn a_file_moved_out_is_removed_and_one_moved_in_is_added() {
    let dir = TempDir::new().unwrap();
    let elsewhere = TempDir::new().unwrap();
    touch(dir.path(), "leaving", 1);
    touch(elsewhere.path(), "arriving", 1);
    let (listing, _events) = watched(dir.path());

    fs::rename(dir.path().join("leaving"), elsewhere.path().join("leaving")).unwrap();
    fs::rename(
        elsewhere.path().join("arriving"),
        dir.path().join("arriving"),
    )
    .unwrap();
    wait_until("both moves", || names(&listing) == ["arriving"]);
}

#[test]
fn a_burst_of_files_is_coalesced_into_few_patches() {
    let dir = TempDir::new().unwrap();
    let (listing, events) = watched(dir.path());
    for i in 0..300 {
        touch(dir.path(), &format!("f{i:03}"), 1);
    }
    wait_until("every file", || listing.snapshot().count == 300);
    let batches = changed(&events);
    assert!(
        batches.len() < 60,
        "{} patches for 300 files",
        batches.len()
    );
    let names = names(&listing);
    let mut sorted = names.clone();
    sorted.sort();
    assert_eq!(names, sorted);
}

#[test]
fn removing_the_watched_folder_fails_the_listing() {
    let dir = TempDir::new().unwrap();
    let watched_dir = dir.path().join("sub");
    fs::create_dir(&watched_dir).unwrap();
    touch(&watched_dir, "a", 1);
    let (listing, events) = watched(&watched_dir);

    fs::remove_dir_all(&watched_dir).unwrap();
    wait_until("the failure", || {
        listing.snapshot().phase == ListingPhase::Failed
    });
    let failure = events
        .lock()
        .unwrap()
        .iter()
        .find_map(|e| match e {
            ListingEvent::Failed { error, .. } => Some(error.clone()),
            _ => None,
        })
        .expect("a Failed event");
    assert!(matches!(failure, VfsError::NotFound { .. }), "{failure:?}");
}

#[cfg(unix)]
#[test]
fn a_watched_folder_that_becomes_unreadable_fails_with_permission_denied() {
    use std::os::unix::fs::PermissionsExt;
    let dir = TempDir::new().unwrap();
    let locked = dir.path().join("locked");
    fs::create_dir(&locked).unwrap();
    touch(&locked, "a", 1);
    let (listing, events) = watched(&locked);

    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
    // Root reads anything, so there is nothing to observe for root.
    let root = fs::read_dir(&locked).is_ok();
    if !root {
        wait_until("the failure", || {
            listing.snapshot().phase == ListingPhase::Failed
        });
        assert!(events.lock().unwrap().iter().any(|e| matches!(
            e,
            ListingEvent::Failed {
                error: VfsError::PermissionDenied { .. },
                ..
            }
        )));
    }
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn the_polling_fallback_follows_changes_and_reports_why_it_is_polling() {
    let dir = TempDir::new().unwrap();
    touch(dir.path(), "a", 1);
    let provider = LocalProvider::with_watch_options(WatchOptions {
        mode: WatchMode::Poll,
        ..fast()
    });
    let (listing, _events) = open(Arc::new(provider), dir.path());
    let WatchState::Polling { reason } = listing.watch_state() else {
        panic!("expected polling, got {:?}", listing.watch_state());
    };
    assert!(reason.contains("polling was requested"), "{reason}");

    touch(dir.path(), "b", 1);
    wait_until("the new file", || names(&listing) == ["a", "b"]);
    fs::remove_file(dir.path().join("a")).unwrap();
    wait_until("the removal", || names(&listing) == ["b"]);

    fs::remove_file(dir.path().join("b")).unwrap();
    fs::remove_dir(dir.path()).unwrap();
    wait_until("the failure", || {
        listing.snapshot().phase == ListingPhase::Failed
    });
}

/// Wraps the local provider so a test can inject watcher events and see when the watch is dropped.
struct Capture {
    inner: LocalProvider,
    sink: Arc<Mutex<Option<WatchSink>>>,
    dropped: Arc<AtomicBool>,
    /// Sent through the sink from inside `list`, like a change that lands during the scan.
    during_scan: Mutex<Vec<WatchEvent>>,
}

struct DropFlag(Arc<AtomicBool>);

impl Watch for DropFlag {}

impl Drop for DropFlag {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

impl Capture {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            inner: LocalProvider::new(),
            sink: Arc::default(),
            dropped: Arc::default(),
            during_scan: Mutex::default(),
        })
    }

    fn send(&self, event: WatchEvent) {
        let sink = self.sink.lock().unwrap().clone().expect("a watch");
        sink(event);
    }
}

impl Provider for Capture {
    fn scheme(&self) -> &'static str {
        "file"
    }

    fn capabilities(&self) -> Capabilities {
        self.inner.capabilities()
    }

    fn stat(&self, path: &VfsPath) -> Result<ScannedEntry, VfsError> {
        self.inner.stat(path)
    }

    fn list(
        &self,
        path: &VfsPath,
        cancel: &CancelToken,
        budget: usize,
        progress: &mut dyn FnMut(u32),
    ) -> Result<Vec<ScannedEntry>, VfsError> {
        let entries = self.inner.list(path, cancel, budget, progress);
        for event in self.during_scan.lock().unwrap().drain(..) {
            self.send(event);
        }
        entries
    }

    fn resolve_link(
        &self,
        folder: &VfsPath,
        entry: &ScannedEntry,
    ) -> Result<ScannedEntry, VfsError> {
        self.inner.resolve_link(folder, entry)
    }

    fn watch(&self, _path: &VfsPath, sink: WatchSink) -> Result<Box<dyn Watch>, VfsError> {
        *self.sink.lock().unwrap() = Some(sink);
        Ok(Box::new(DropFlag(self.dropped.clone())))
    }
}

/// The event kinds in order, without the scan's own progress reports and with repeats folded.
fn kinds(events: &Events) -> Vec<String> {
    let mut out: Vec<String> = events
        .lock()
        .unwrap()
        .iter()
        .map(|e| match e {
            ListingEvent::Progress { phase, .. } => format!("progress {phase:?}"),
            ListingEvent::Changed { .. } => "changed".to_owned(),
            ListingEvent::Failed { .. } => "failed".to_owned(),
        })
        .filter(|kind| kind != "progress Scanning")
        .collect();
    out.dedup();
    out
}

#[test]
fn an_overflow_triggers_a_rescan_that_patches_the_view_and_keeps_ids() {
    let dir = TempDir::new().unwrap();
    for name in ["a", "b", "c"] {
        touch(dir.path(), name, 1);
    }
    let capture = Capture::new();
    let (listing, events) = open(capture.clone(), dir.path());
    let b = listing.id_of(&OsString::from("b")).unwrap();
    events.lock().unwrap().clear();
    let revision = listing.snapshot().revision;

    // Changes the watcher never heard about.
    fs::remove_file(dir.path().join("a")).unwrap();
    touch(dir.path(), "d", 1);
    touch(dir.path(), "c", 77);
    capture.send(WatchEvent::Rescan(waypoint_vfs::RescanReason::Overflow));

    assert_eq!(names(&listing), ["b", "c", "d"]);
    assert_eq!(listing.id_of(&OsString::from("b")), Some(b));
    assert_eq!(listing.snapshot().phase, ListingPhase::Ready);
    assert_eq!(
        kinds(&events),
        ["progress Rescanning", "changed", "progress Ready"]
    );
    assert!(listing.snapshot().revision > revision + 2);
    assert_eq!(
        changed(&events).concat(),
        [
            PatchOp::Remove { at: 0, count: 1 },
            PatchOp::Insert { at: 2, count: 1 },
            PatchOp::Update { at: 1, count: 1 },
        ]
    );
}

#[test]
fn a_rescan_of_an_unchanged_folder_raises_no_patch() {
    let dir = TempDir::new().unwrap();
    touch(dir.path(), "a", 1);
    let capture = Capture::new();
    let (listing, events) = open(capture.clone(), dir.path());
    events.lock().unwrap().clear();
    listing.rescan().unwrap();
    assert_eq!(kinds(&events), ["progress Rescanning", "progress Ready"]);
}

#[test]
fn a_rescan_of_a_vanished_folder_fails_the_listing() {
    let dir = TempDir::new().unwrap();
    let sub = dir.path().join("sub");
    fs::create_dir(&sub).unwrap();
    let capture = Capture::new();
    let (listing, events) = open(capture.clone(), &sub);
    fs::remove_dir(&sub).unwrap();
    assert!(matches!(listing.rescan(), Err(VfsError::NotFound { .. })));
    assert_eq!(listing.snapshot().phase, ListingPhase::Failed);
    assert!(kinds(&events).contains(&"failed".to_owned()));
}

#[test]
fn a_degraded_watcher_is_reported_in_the_watch_state() {
    let dir = TempDir::new().unwrap();
    let capture = Capture::new();
    let (listing, _events) = open(capture.clone(), dir.path());
    assert_eq!(listing.watch_state(), WatchState::Native);
    capture.send(WatchEvent::Degraded {
        reason: "inotify watch limit reached".to_owned(),
    });
    assert_eq!(
        listing.watch_state(),
        WatchState::Polling {
            reason: "inotify watch limit reached".to_owned()
        }
    );
}

#[test]
fn a_lost_watch_fails_the_listing_and_later_changes_are_ignored() {
    let dir = TempDir::new().unwrap();
    touch(dir.path(), "a", 1);
    let capture = Capture::new();
    let (listing, events) = open(capture.clone(), dir.path());
    let location = listing.snapshot().location;
    capture.send(WatchEvent::Lost(VfsError::NotFound {
        location: location.clone(),
    }));
    let snapshot = listing.snapshot();
    assert_eq!(snapshot.phase, ListingPhase::Failed);
    assert!(matches!(
        events.lock().unwrap().last(),
        Some(ListingEvent::Failed {
            error: VfsError::NotFound { .. },
            ..
        })
    ));
    capture.send(WatchEvent::Changes(vec![Change::Remove("a".into())]));
    assert_eq!(listing.snapshot().revision, snapshot.revision);
    assert_eq!(names(&listing), ["a"]);
}

#[test]
fn a_watch_lost_during_the_scan_keeps_the_listing_failed() {
    let dir = TempDir::new().unwrap();
    touch(dir.path(), "a", 1);
    let capture = Capture::new();
    let location = path_of(dir.path()).to_location();
    capture
        .during_scan
        .lock()
        .unwrap()
        .push(WatchEvent::Lost(VfsError::NotFound { location }));
    let events: Events = Arc::default();
    let sink = {
        let events = events.clone();
        Arc::new(move |event| events.lock().unwrap().push(event))
    };
    let opened = Listing::open(
        ListingHandle(1),
        path_of(dir.path()),
        capture.clone(),
        SortSpec::default(),
        Filter::default(),
        ListingOptions::default(),
        sink,
    );
    assert!(opened.is_err());
    assert!(!events.lock().unwrap().iter().any(|e| matches!(
        e,
        ListingEvent::Progress {
            phase: ListingPhase::Ready,
            ..
        }
    )));
}

#[test]
fn changes_that_arrive_during_the_scan_are_applied_after_it() {
    let dir = TempDir::new().unwrap();
    touch(dir.path(), "a", 1);
    touch(dir.path(), "b", 1);
    let capture = Capture::new();
    let added = ScannedEntry {
        name: "c".into(),
        ..LocalProvider::new()
            .stat(&path_of(&dir.path().join("a")))
            .unwrap()
    };
    capture.during_scan.lock().unwrap().extend([
        WatchEvent::Changes(vec![Change::Upsert(added)]),
        WatchEvent::Changes(vec![Change::Remove("b".into())]),
    ]);
    let (listing, events) = open(capture, dir.path());
    assert_eq!(names(&listing), ["a", "c"]);
    assert_eq!(
        kinds(&events),
        ["progress Ready", "changed"],
        "one Ready, then the held changes as one patch"
    );
}

#[test]
fn an_overflow_during_the_scan_rescans_afterwards() {
    let dir = TempDir::new().unwrap();
    touch(dir.path(), "a", 1);
    let capture = Capture::new();
    capture
        .during_scan
        .lock()
        .unwrap()
        .push(WatchEvent::Rescan(waypoint_vfs::RescanReason::Overflow));
    let (_listing, events) = open(capture, dir.path());
    assert_eq!(
        kinds(&events),
        ["progress Ready", "progress Rescanning", "progress Ready"]
    );
}

#[test]
fn closing_a_listing_stops_its_watch() {
    let dir = TempDir::new().unwrap();
    let capture = Capture::new();
    let (listing, _events) = open(capture.clone(), dir.path());
    assert!(!capture.dropped.load(Ordering::SeqCst));
    listing.close();
    assert!(capture.dropped.load(Ordering::SeqCst));
    assert_eq!(listing.watch_state(), WatchState::Off);
}

#[test]
fn a_provider_that_cannot_watch_says_so() {
    struct Silent(LocalProvider);
    impl Provider for Silent {
        fn scheme(&self) -> &'static str {
            "file"
        }
        fn capabilities(&self) -> Capabilities {
            Capabilities {
                watch: false,
                ..self.0.capabilities()
            }
        }
        fn stat(&self, path: &VfsPath) -> Result<ScannedEntry, VfsError> {
            self.0.stat(path)
        }
        fn list(
            &self,
            path: &VfsPath,
            cancel: &CancelToken,
            budget: usize,
            progress: &mut dyn FnMut(u32),
        ) -> Result<Vec<ScannedEntry>, VfsError> {
            self.0.list(path, cancel, budget, progress)
        }
        fn resolve_link(&self, f: &VfsPath, e: &ScannedEntry) -> Result<ScannedEntry, VfsError> {
            self.0.resolve_link(f, e)
        }
    }
    let dir = TempDir::new().unwrap();
    let (listing, _events) = open(Arc::new(Silent(LocalProvider::new())), dir.path());
    assert!(matches!(
        listing.watch_state(),
        WatchState::Unavailable { .. }
    ));
    // The owner can still refresh it by hand.
    touch(dir.path(), "new", 1);
    listing.rescan().unwrap();
    assert_eq!(names(&listing), ["new"]);
    let _ = EntryId(0);
}

#[test]
fn dropping_a_watch_does_not_wait_for_a_busy_worker() {
    let dir = TempDir::new().unwrap();
    let entered = Arc::new(AtomicBool::new(false));
    let release = Arc::new(AtomicBool::new(false));
    let sink: WatchSink = {
        let (entered, release) = (entered.clone(), release.clone());
        Arc::new(move |_| {
            // Stands in for a worker stuck in a long scan.
            entered.store(true, Ordering::SeqCst);
            while !release.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(5));
            }
        })
    };
    let provider = LocalProvider::with_watch_options(fast());
    let watch = provider.watch(&path_of(dir.path()), sink).unwrap();
    touch(dir.path(), "a", 1);
    wait_until("the worker to be busy", || entered.load(Ordering::SeqCst));
    let (done_tx, done_rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        drop(watch);
        let _ = done_tx.send(());
    });
    let returned = done_rx.recv_timeout(Duration::from_secs(2)).is_ok();
    release.store(true, Ordering::SeqCst);
    assert!(returned, "dropping the watch blocked on its worker");
}
