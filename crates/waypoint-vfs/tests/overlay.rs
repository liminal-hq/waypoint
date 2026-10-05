// An overlay decorates a live listing: marks arrive, the rows change, and closing stops it.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::HashMap;
use std::ffi::OsString;
use std::fs;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use waypoint_path::{FilePath, VfsPath};
use waypoint_vfs::{
    Filter, FolderMarks, FolderOverlay, GitChange, GitMark, Listing, ListingEvent, ListingHandle,
    ListingOptions, LocalProvider, MarkSink, OverlayGuard, PatchOp, SortKey, SortSpec,
};

/// An overlay that sends one set of marks when attached and lets the test send more.
struct Scripted {
    first: FolderMarks,
    sink: Mutex<Option<MarkSink>>,
    stopped: Arc<AtomicBool>,
}

struct Guard(Arc<AtomicBool>);

impl OverlayGuard for Guard {}

impl Drop for Guard {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

impl FolderOverlay for Scripted {
    fn attach(&self, _folder: &VfsPath, sink: MarkSink) -> Option<Box<dyn OverlayGuard>> {
        sink(self.first.clone());
        *self.sink.lock().unwrap() = Some(sink);
        Some(Box::new(Guard(Arc::clone(&self.stopped))))
    }
}

fn mark(change: GitChange) -> GitMark {
    GitMark {
        unstaged: Some(change),
        ..GitMark::default()
    }
}

fn marks(list: &[(&str, GitChange)]) -> FolderMarks {
    FolderMarks {
        default: None,
        names: list
            .iter()
            .map(|(name, change)| (OsString::from(name), mark(*change)))
            .collect::<HashMap<_, _>>(),
    }
}

fn listing(dir: &std::path::Path, sort: SortSpec) -> (Arc<Listing>, Arc<Mutex<Vec<ListingEvent>>>) {
    let events: Arc<Mutex<Vec<ListingEvent>>> = Arc::default();
    let sink = {
        let events = events.clone();
        Arc::new(move |event| events.lock().unwrap().push(event))
    };
    let listing = Listing::new(
        ListingHandle(1),
        VfsPath::File(FilePath::from_path(dir).unwrap()),
        Arc::new(LocalProvider::new()),
        sort,
        Filter::default(),
        ListingOptions {
            watch: false,
            ..ListingOptions::default()
        },
        sink,
    );
    (listing, events)
}

fn gits(listing: &Listing) -> Vec<(String, Option<GitMark>)> {
    listing
        .get_range(0, u32::MAX)
        .into_iter()
        .map(|e| (e.name, e.git))
        .collect()
}

#[test]
fn marks_sent_before_the_scan_are_on_the_rows_once_it_finishes() {
    let dir = tempfile::tempdir().unwrap();
    for name in ["a.txt", "b.txt"] {
        fs::write(dir.path().join(name), name).unwrap();
    }
    let (listing, _) = listing(dir.path(), SortSpec::default());
    let overlay = Scripted {
        first: marks(&[("b.txt", GitChange::Modified)]),
        sink: Mutex::default(),
        stopped: Arc::default(),
    };
    listing.attach_overlay(&overlay);
    listing.scan().unwrap();
    assert_eq!(
        gits(&listing),
        [
            ("a.txt".to_owned(), None),
            ("b.txt".to_owned(), Some(mark(GitChange::Modified)))
        ]
    );
}

#[test]
fn later_marks_patch_the_rows_and_a_git_sort_moves_them() {
    let dir = tempfile::tempdir().unwrap();
    for name in ["a.txt", "b.txt", "c.txt"] {
        fs::write(dir.path().join(name), name).unwrap();
    }
    let sort = SortSpec {
        key: SortKey::Git,
        ..SortSpec::default()
    };
    let (listing, events) = listing(dir.path(), sort);
    let overlay = Scripted {
        first: FolderMarks::default(),
        sink: Mutex::default(),
        stopped: Arc::default(),
    };
    listing.attach_overlay(&overlay);
    listing.scan().unwrap();
    let before = events.lock().unwrap().len();
    let send = overlay.sink.lock().unwrap().clone().unwrap();
    send(marks(&[("c.txt", GitChange::Conflicted)]));
    let names: Vec<String> = gits(&listing).into_iter().map(|(n, _)| n).collect();
    assert_eq!(
        names,
        ["c.txt", "a.txt", "b.txt"],
        "the conflict sorts first"
    );
    let events = events.lock().unwrap();
    let ListingEvent::Changed { ops, moved, .. } = &events[before] else {
        panic!("a Changed event, got {:?}", events[before]);
    };
    assert!(ops.iter().any(|op| matches!(op, PatchOp::Insert { .. })));
    assert_eq!(moved.len(), 1);
}

#[test]
fn closing_the_listing_stops_the_overlay() {
    let dir = tempfile::tempdir().unwrap();
    let (listing, _) = listing(dir.path(), SortSpec::default());
    let stopped = Arc::new(AtomicBool::new(false));
    let overlay = Scripted {
        first: FolderMarks::default(),
        sink: Mutex::default(),
        stopped: Arc::clone(&stopped),
    };
    listing.attach_overlay(&overlay);
    assert!(!stopped.load(Ordering::SeqCst));
    listing.close();
    assert!(stopped.load(Ordering::SeqCst));
    // A sink that outlives the listing does nothing.
    let send = overlay.sink.lock().unwrap().clone().unwrap();
    drop(listing);
    send(marks(&[("x", GitChange::Added)]));
}
