// A remote listing streams: rows that have arrived can be fetched, in order, while the rest of a
// slow folder is still on its way (A83, A115).
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::ffi::OsString;
use std::sync::mpsc;
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::time::{Duration, Instant};

use waypoint_path::{CaseRule, VfsPath};
use waypoint_protocol::VfsError;
use waypoint_vfs::{
    CancelToken, Capabilities, EntryKind, EventSink, FakeRemoteProvider, Filter, IconGroup,
    Listing, ListingEvent, ListingHandle, ListingOptions, ListingPhase, Provider, ScannedEntry,
    SortKey, SortSpec,
};

fn file(name: &str) -> ScannedEntry {
    ScannedEntry {
        name: OsString::from(name),
        kind: EntryKind::File,
        link_target: None,
        link_pending: false,
        group: IconGroup::Other,
        special: None,
        size: Some(name.len() as u64),
        modified_ms: Some(0),
        hidden: false,
        trashed: None,
        attributes: None,
    }
}

/// A server that hands over one batch, then waits for the test to let the next one go, so a test
/// can look at the listing between batches without timing anything.
struct GatedServer {
    batches: Vec<Vec<ScannedEntry>>,
    gate: Mutex<mpsc::Receiver<()>>,
}

impl Provider for GatedServer {
    fn scheme(&self) -> &'static str {
        "sftp"
    }

    fn capabilities(&self) -> Capabilities {
        let mut capabilities = Capabilities::new(CaseRule::Sensitive);
        capabilities.remote = true;
        capabilities
    }

    fn stat(&self, path: &VfsPath) -> Result<ScannedEntry, VfsError> {
        Err(VfsError::NotFound {
            location: path.to_location(),
        })
    }

    fn list(
        &self,
        path: &VfsPath,
        cancel: &CancelToken,
        budget: usize,
        progress: &mut dyn FnMut(u32),
    ) -> Result<Vec<ScannedEntry>, VfsError> {
        let mut all = Vec::new();
        self.list_batches(path, cancel, budget, &mut |batch| {
            all.extend(batch);
            progress(all.len() as u32);
        })?;
        Ok(all)
    }

    fn list_batches(
        &self,
        _: &VfsPath,
        cancel: &CancelToken,
        _: usize,
        sink: &mut dyn FnMut(Vec<ScannedEntry>),
    ) -> Result<(), VfsError> {
        for (at, batch) in self.batches.iter().enumerate() {
            if at > 0 {
                self.gate
                    .lock()
                    .unwrap()
                    .recv_timeout(Duration::from_secs(10))
                    .map_err(|_| VfsError::Cancelled)?;
            }
            if cancel.is_cancelled() {
                return Err(VfsError::Cancelled);
            }
            sink(batch.clone());
        }
        Ok(())
    }

    fn resolve_link(&self, _: &VfsPath, entry: &ScannedEntry) -> Result<ScannedEntry, VfsError> {
        Ok(entry.clone())
    }
}

/// What a reader saw at one `Progress` event: its phase and count, and the names `get_range` gave.
#[derive(Debug, Clone)]
struct Seen {
    phase: ListingPhase,
    revision: u32,
    count: u32,
    names: Vec<String>,
}

/// Where a test's sink finds the listing it reads back, once the listing exists.
type Slot = Arc<OnceLock<Weak<Listing>>>;

/// A sink that reads the whole view back at every progress event, as the page would.
fn reading_sink() -> (EventSink, Slot, Arc<Mutex<Vec<Seen>>>) {
    let listing: Slot = Arc::new(OnceLock::new());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let sink: EventSink = {
        let (listing, seen) = (listing.clone(), seen.clone());
        Arc::new(move |event| {
            let ListingEvent::Progress {
                phase,
                revision,
                count,
                ..
            } = event
            else {
                return;
            };
            let Some(listing) = listing.get().and_then(Weak::upgrade) else {
                return;
            };
            let names = listing
                .get_range(0, u32::MAX)
                .into_iter()
                .map(|entry| entry.name)
                .collect();
            seen.lock().unwrap().push(Seen {
                phase,
                revision,
                count,
                names,
            });
        })
    };
    (sink, listing, seen)
}

fn streaming_options() -> ListingOptions {
    ListingOptions {
        watch: false,
        progress_interval: Duration::ZERO,
        ..ListingOptions::default()
    }
}

#[test]
fn rows_that_have_arrived_are_fetchable_and_sorted_before_the_listing_completes() {
    let (release, gate) = mpsc::channel();
    let server = Arc::new(GatedServer {
        batches: vec![
            vec![file("m"), file("c"), file("x")],
            vec![file("a"), file("q")],
            vec![file("z"), file("b")],
        ],
        gate: Mutex::new(gate),
    });
    let (sink, slot, seen) = reading_sink();
    let listing = Listing::new(
        ListingHandle(1),
        FakeRemoteProvider::sftp().root("h"),
        server,
        SortSpec::default(),
        Filter::default(),
        streaming_options(),
        sink,
    );
    slot.set(Arc::downgrade(&listing)).unwrap();
    let scanning = {
        let listing = listing.clone();
        std::thread::spawn(move || listing.scan())
    };

    // The first batch is drawn while the server holds the rest back.
    let wait_for = |count: usize| {
        let deadline = Instant::now() + Duration::from_secs(10);
        while seen.lock().unwrap().len() < count {
            assert!(Instant::now() < deadline, "no progress event arrived");
            std::thread::sleep(Duration::from_millis(1));
        }
    };
    wait_for(1);
    assert_eq!(listing.get_range(0, 10).len(), 3);
    assert_eq!(listing.snapshot().phase, ListingPhase::Scanning);
    release.send(()).unwrap();
    wait_for(2);
    release.send(()).unwrap();
    scanning.join().unwrap().unwrap();

    let seen = seen.lock().unwrap().clone();
    let shapes: Vec<(ListingPhase, u32, Vec<&str>)> = seen
        .iter()
        .map(|s| {
            (
                s.phase,
                s.count,
                s.names.iter().map(String::as_str).collect(),
            )
        })
        .collect();
    assert_eq!(
        shapes,
        vec![
            (ListingPhase::Scanning, 3, vec!["c", "m", "x"]),
            (ListingPhase::Scanning, 5, vec!["a", "c", "m", "q", "x"]),
            (
                ListingPhase::Scanning,
                7,
                vec!["a", "b", "c", "m", "q", "x", "z"]
            ),
            (
                ListingPhase::Ready,
                7,
                vec!["a", "b", "c", "m", "q", "x", "z"]
            ),
        ]
    );
    // Each publish is a new revision, so a page fetched for an older one is never merged.
    assert!(seen.windows(2).all(|w| w[0].revision < w[1].revision));
    // An id stays with its entry through the scan: the first row to arrive is still id 0.
    let m = listing
        .get_range(0, 10)
        .into_iter()
        .find(|e| e.name == "m")
        .unwrap();
    assert_eq!(m.id.0, 0);
}

#[test]
fn a_slow_server_s_first_rows_are_drawn_long_before_the_last_arrive() {
    let fake = FakeRemoteProvider::sftp();
    let folder = fake.root("h").join("big").unwrap();
    fake.put_dir(&folder);
    for i in 0..400 {
        fake.put_file(&folder.join(format!("f{i:04}")).unwrap(), b"");
    }
    fake.set_batch(50);
    fake.set_latency(Duration::from_millis(15));
    let (sink, slot, seen) = reading_sink();
    let listing = Listing::new(
        ListingHandle(2),
        folder,
        Arc::new(fake),
        SortSpec {
            key: SortKey::Name,
            descending: true,
            ..SortSpec::default()
        },
        Filter::default(),
        ListingOptions {
            progress_interval: Duration::from_millis(30),
            ..streaming_options()
        },
        sink,
    );
    slot.set(Arc::downgrade(&listing)).unwrap();
    listing.scan().unwrap();

    let seen = seen.lock().unwrap().clone();
    let (last, during) = seen.split_last().unwrap();
    assert_eq!(last.phase, ListingPhase::Ready);
    assert_eq!(last.count, 400);
    // Batches are coalesced by the interval, so there are fewer publishes than batches, but more
    // than one, and every one of them could be read back whole and in order.
    assert!(!during.is_empty(), "nothing was published mid-scan");
    assert!(during.len() < 8, "{} publishes for 8 batches", during.len());
    for s in during {
        assert_eq!(s.phase, ListingPhase::Scanning);
        assert!(s.count > 0 && s.count < 400);
        assert_eq!(s.names.len() as u32, s.count);
        assert!(s.names.windows(2).all(|w| w[0] > w[1]), "{:?}", s.names);
    }
    assert_eq!(last.names.first().map(String::as_str), Some("f0399"));
}

#[test]
fn a_local_listing_is_still_read_whole() {
    let dir = tempfile::tempdir().unwrap();
    for name in ["b", "a"] {
        std::fs::write(dir.path().join(name), b"").unwrap();
    }
    let (sink, slot, seen) = reading_sink();
    let listing = Listing::new(
        ListingHandle(3),
        VfsPath::File(waypoint_path::FilePath::from_path(dir.path()).unwrap()),
        Arc::new(waypoint_vfs::LocalProvider::new()),
        SortSpec::default(),
        Filter::default(),
        streaming_options(),
        sink,
    );
    slot.set(Arc::downgrade(&listing)).unwrap();
    listing.scan().unwrap();
    let seen = seen.lock().unwrap();
    // Progress while it reads counts entries without holding them; the rows come with Ready.
    for s in seen.iter().filter(|s| s.phase == ListingPhase::Scanning) {
        assert!(s.names.is_empty());
    }
    assert_eq!(seen.last().unwrap().names, ["a", "b"]);
}

fn names_of(listing: &Listing) -> Vec<String> {
    let count = listing.snapshot().count;
    listing
        .get_range(0, count)
        .into_iter()
        .map(|entry| entry.name)
        .collect()
}

#[test]
fn a_folder_that_is_not_watched_is_read_again_by_the_refresh_rule() {
    let server = FakeRemoteProvider::sftp();
    let root = server.root("me@fake.test");
    server.put_file(&root.join("old.txt").unwrap(), b"old");
    let listing = Listing::new(
        ListingHandle(9),
        root.clone(),
        Arc::new(server.clone()),
        SortSpec::default(),
        Filter::default(),
        streaming_options(),
        Arc::new(|_| {}),
    );
    listing.scan().unwrap();
    assert!(
        !listing.snapshot().watched,
        "a server's folder is not watched"
    );
    assert_eq!(names_of(&listing), ["old.txt"]);

    // Changed behind Waypoint's back: nothing tells the listing.
    server.put_file(&root.join("new.txt").unwrap(), b"new");
    assert_eq!(names_of(&listing), ["old.txt"]);

    // A refresh asked for less than ten seconds after the read does nothing...
    assert!(!listing.refresh(true, Duration::from_secs(10)).unwrap());
    assert_eq!(names_of(&listing), ["old.txt"]);
    // ...and one that is due reads the folder again and patches the view.
    assert!(listing.refresh(true, Duration::ZERO).unwrap());
    assert_eq!(names_of(&listing), ["new.txt", "old.txt"]);
    assert_eq!(listing.snapshot().phase, ListingPhase::Ready);
}

#[test]
fn a_watched_folder_is_left_alone_by_a_refresh_that_only_wants_unwatched_ones() {
    let dir = tempfile::tempdir().unwrap();
    let listing = Listing::new(
        ListingHandle(10),
        VfsPath::File(waypoint_path::FilePath::from_path(dir.path()).unwrap()),
        Arc::new(waypoint_vfs::LocalProvider::new()),
        SortSpec::default(),
        Filter::default(),
        streaming_options(),
        Arc::new(|_| {}),
    );
    listing.scan().unwrap();
    assert!(listing.snapshot().watched);
    assert!(!listing.refresh(true, Duration::ZERO).unwrap());
    // F5 forces one.
    assert!(listing.refresh(false, Duration::ZERO).unwrap());
}
