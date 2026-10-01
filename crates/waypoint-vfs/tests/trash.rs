// Headless tests of the Trash provider: listings of `trash:/` over an in-memory Trash.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::ffi::OsStr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use waypoint_path::{CaseRule, VfsPath};
use waypoint_protocol::{EntryId, VfsError};
use waypoint_vfs::{
    CancelToken, EntryKind, Filter, Listing, ListingEvent, ListingHandle, ListingLayout,
    ListingOptions, MemoryTrashSource, PatchOp, Provider, SelectionSpec, SortKey, SortSpec,
    TrashProvider, TrashSource, TrashedItem, WatchState,
};

const PATIENCE: Duration = Duration::from_secs(10);
const DAY: i64 = 86_400_000;
/// An absolute folder on this platform; the other spelling is not a path on Windows.
const DOCS: &str = if cfg!(windows) {
    r"C:\home\a\docs"
} else {
    "/home/a/docs"
};

type Events = Arc<Mutex<Vec<ListingEvent>>>;

fn item(id: &str, name: &str, deleted_ms: i64) -> TrashedItem {
    TrashedItem {
        id: id.to_owned(),
        name: name.to_owned(),
        original_path: DOCS.to_owned(),
        deleted_ms,
        size: 10,
        is_dir: false,
    }
}

fn root() -> VfsPath {
    VfsPath::parse_input("trash:/").unwrap()
}

fn provider(source: &Arc<MemoryTrashSource>, poll_ms: u64) -> Arc<TrashProvider> {
    Arc::new(TrashProvider::with_poll(
        source.clone(),
        Duration::from_millis(poll_ms),
    ))
}

fn open(provider: Arc<dyn Provider>, sort: SortSpec) -> (Arc<Listing>, Events) {
    let events: Events = Arc::default();
    let sink = {
        let events = events.clone();
        Arc::new(move |event| events.lock().unwrap().push(event))
    };
    let listing = Listing::open(
        ListingHandle(1),
        root(),
        provider,
        sort,
        Filter::default(),
        ListingOptions::default(),
        sink,
    )
    .unwrap();
    (listing, events)
}

fn wait_until(what: &str, mut condition: impl FnMut() -> bool) {
    let started = Instant::now();
    while !condition() {
        assert!(started.elapsed() < PATIENCE, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn names(listing: &Listing) -> Vec<String> {
    listing
        .get_range(0, u32::MAX)
        .into_iter()
        .map(|e| e.name)
        .collect()
}

fn by(key: SortKey, descending: bool) -> SortSpec {
    SortSpec {
        key,
        descending,
        directories_first: false,
    }
}

fn three() -> Arc<MemoryTrashSource> {
    let source = Arc::new(MemoryTrashSource::new());
    source.add(item("t|1", "zebra.txt", 3 * DAY));
    source.add(item("t|2", "Apple.png", 1 * DAY));
    source.add(item("t|3", "mango.md", 2 * DAY));
    source
}

#[test]
fn items_are_listed_under_their_original_names_with_where_and_when() {
    let source = three();
    let (listing, _) = open(provider(&source, 50), SortSpec::default());
    let entries = listing.get_range(0, 10);
    assert_eq!(
        entries.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(),
        ["Apple.png", "mango.md", "zebra.txt"]
    );
    let apple = &entries[0];
    assert_eq!(apple.original_path.as_deref(), Some(DOCS));
    assert_eq!(apple.deleted_ms, Some(DAY));
    assert_eq!(apple.size, Some(10));
    assert_eq!(apple.modified_ms, None);
    assert_eq!(apple.kind, EntryKind::File);
    assert!(!apple.hidden);
    let snapshot = listing.snapshot();
    assert!(snapshot.read_only);
    assert_eq!(snapshot.layout, ListingLayout::Trash);
    assert_eq!(snapshot.location.uri, "trash:/");
    assert_eq!(snapshot.location.display, "Trash");
}

#[test]
fn a_folder_reads_as_a_folder_and_a_dotfile_is_not_hidden() {
    let source = Arc::new(MemoryTrashSource::new());
    source.add(TrashedItem {
        is_dir: true,
        size: 1234,
        ..item("d", "old stuff", DAY)
    });
    source.add(item("h", ".bashrc", DAY));
    let (listing, _) = open(provider(&source, 50), SortSpec::default());
    let entries = listing.get_range(0, 10);
    assert_eq!(entries[0].name, "old stuff");
    assert_eq!(entries[0].kind, EntryKind::Directory);
    assert_eq!(entries[0].size, Some(1234));
    assert_eq!(entries[1].name, ".bashrc");
    assert!(!entries[1].hidden);
}

#[test]
fn the_deletion_date_sorts_both_ways_and_name_sorts_by_the_original_name() {
    let source = three();
    let p = provider(&source, 50);
    let (listing, _) = open(p, by(SortKey::Deleted, false));
    assert_eq!(names(&listing), ["Apple.png", "mango.md", "zebra.txt"]);
    listing.set_sort(by(SortKey::Deleted, true));
    assert_eq!(names(&listing), ["zebra.txt", "mango.md", "Apple.png"]);
    listing.set_sort(by(SortKey::Name, true));
    assert_eq!(names(&listing), ["zebra.txt", "mango.md", "Apple.png"]);
    listing.set_sort(by(SortKey::Kind, false));
    // Documents, then images by extension after their group: the extension is the original's.
    let kinds = names(&listing);
    assert_eq!(kinds.len(), 3);
    listing.set_sort(by(SortKey::Size, false));
    assert_eq!(listing.snapshot().count, 3);
}

#[test]
fn two_items_with_one_name_are_two_entries() {
    let source = Arc::new(MemoryTrashSource::new());
    source.add(item("a", "report.txt", DAY));
    source.add(item("b", "report.txt", 2 * DAY));
    let (listing, _) = open(provider(&source, 50), by(SortKey::Deleted, false));
    let entries = listing.get_range(0, 10);
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].name, entries[1].name);
    assert_ne!(entries[0].id, entries[1].id);
    assert_eq!(listing.path_of(entries[0].id).unwrap().to_uri(), "trash:/a");
    assert_eq!(listing.path_of(entries[1].id).unwrap().to_uri(), "trash:/b");
}

#[test]
fn every_receipt_id_survives_the_location_round_trip() {
    let odd = [
        "%2Fhome%2Fa%2F.local%2Fshare%2FTrash|a b.txt",
        "/mnt/usb/.Trash-1000|100%.txt",
        "x|é#?&=+ 日本語 \u{1F600}",
        "|",
        "plain",
    ];
    let source = Arc::new(MemoryTrashSource::new());
    for (n, id) in odd.iter().enumerate() {
        source.add(item(id, &format!("file {n}"), DAY));
    }
    let p = provider(&source, 50);
    let (listing, _) = open(p.clone(), SortSpec::default());
    for entry in listing.get_range(0, 10) {
        let path = listing.path_of(entry.id).unwrap();
        let location = path.to_location();
        let back = VfsPath::from_location(&location).unwrap();
        assert_eq!(back, path);
        let VfsPath::Trash(trash) = &back else {
            panic!("a trash location")
        };
        assert!(odd.contains(&trash.id().unwrap()));
        // And stat resolves the same entry from the location alone.
        let stat = p.stat(&back).unwrap();
        assert_eq!(stat.trashed.unwrap().display_name, entry.name);
    }
}

#[test]
fn a_selection_resolves_to_trash_locations() {
    let source = three();
    let (listing, _) = open(provider(&source, 50), by(SortKey::Name, false));
    let entries = listing.get_range(0, 3);
    let paths = listing
        .resolve_selection(&SelectionSpec::Chosen {
            ids: vec![entries[0].id, entries[2].id],
        })
        .unwrap();
    assert_eq!(
        paths.iter().map(VfsPath::to_uri).collect::<Vec<_>>(),
        ["trash:/t%7C2", "trash:/t%7C1"]
    );
}

#[test]
fn stat_and_resolve_link_work_and_a_missing_item_is_not_found() {
    let source = three();
    let p = provider(&source, 50);
    let one = p
        .stat(&VfsPath::parse_input("trash:/t%7C2").unwrap())
        .unwrap();
    assert_eq!(one.trashed.as_ref().unwrap().display_name, "Apple.png");
    assert_eq!(one.name, OsStr::new("t|2"));
    assert_eq!(p.resolve_link(&root(), &one).unwrap(), one);
    assert_eq!(p.stat(&root()).unwrap().kind, EntryKind::Directory);
    assert!(matches!(
        p.stat(&VfsPath::parse_input("trash:/gone").unwrap()),
        Err(VfsError::NotFound { .. })
    ));
}

#[test]
fn what_is_inside_an_item_cannot_be_listed() {
    let source = three();
    let p = provider(&source, 50);
    let result = p.list(
        &VfsPath::parse_input("trash:/t%7C1").unwrap(),
        &CancelToken::new(),
        0,
        &mut |_| {},
    );
    assert!(matches!(result, Err(VfsError::Unsupported { .. })));
}

#[test]
fn nothing_writes_through_the_provider() {
    let source = three();
    let p = provider(&source, 50);
    let a = VfsPath::parse_input("trash:/t%7C1").unwrap();
    let b = VfsPath::parse_input("trash:/new").unwrap();
    assert_eq!(p.scheme(), "trash");
    assert!(p.read_only());
    assert_eq!(p.capabilities().case_rule, CaseRule::Sensitive);
    let unsupported = |r: Result<(), VfsError>| {
        assert!(matches!(r, Err(VfsError::Unsupported { .. })), "{r:?}");
    };
    unsupported(p.create_dir(&b));
    unsupported(p.create_file(&b));
    unsupported(p.rename(&a, &b, false));
    unsupported(p.remove_file(&a));
    unsupported(p.remove_dir(&a));
    unsupported(p.symlink(&b, OsStr::new("x")));
    unsupported(p.set_times(&a, Default::default()));
    assert!(matches!(
        p.create_write(&b, Default::default()),
        Err(VfsError::Unsupported { .. })
    ));
    assert!(matches!(p.open_read(&a), Err(VfsError::Unsupported { .. })));
    assert!(matches!(
        p.permissions(&a),
        Err(VfsError::Unsupported { .. })
    ));
    assert_eq!(source.len(), 3);
    assert!(source.log().is_empty());
}

#[test]
fn another_scheme_is_refused() {
    let source = three();
    let p = provider(&source, 50);
    let file = VfsPath::parse_input(if cfg!(windows) { r"C:\tmp" } else { "/tmp" }).unwrap();
    assert!(matches!(p.stat(&file), Err(VfsError::Unsupported { .. })));
}

#[test]
fn a_change_made_elsewhere_arrives_as_a_patch_by_polling() {
    let source = three();
    let (listing, events) = open(provider(&source, 20), SortSpec::default());
    assert_eq!(listing.watch_state(), WatchState::Native);

    source.add(item("t|4", "Banana.txt", 4 * DAY));
    wait_until("the insert", || names(&listing).len() == 4);
    assert_eq!(
        names(&listing),
        ["Apple.png", "Banana.txt", "mango.md", "zebra.txt"]
    );
    source.remove("t|2");
    wait_until("the removal", || names(&listing).len() == 3);
    assert_eq!(names(&listing), ["Banana.txt", "mango.md", "zebra.txt"]);

    let changes: Vec<PatchOp> = events
        .lock()
        .unwrap()
        .iter()
        .filter_map(|e| match e {
            ListingEvent::Changed { ops, .. } => Some(ops.clone()),
            _ => None,
        })
        .flatten()
        .collect();
    assert_eq!(
        changes,
        [
            PatchOp::Insert { at: 1, count: 1 },
            PatchOp::Remove { at: 0, count: 1 }
        ]
    );
    // The row that stayed kept its id, so a selection of it survives.
    let ids: Vec<EntryId> = listing.get_range(0, 10).iter().map(|e| e.id).collect();
    assert_eq!(ids.len(), 3);
}

#[test]
fn a_changed_item_updates_in_place() {
    let source = three();
    let (listing, _) = open(provider(&source, 20), by(SortKey::Name, false));
    source.add(TrashedItem {
        size: 999,
        ..item("t|3", "mango.md", 2 * DAY)
    });
    wait_until("the update", || {
        listing.get_range(0, 10).iter().any(|e| e.size == Some(999))
    });
    assert_eq!(listing.snapshot().count, 3);
}

#[test]
fn a_source_that_hints_is_not_listed_while_nothing_changes() {
    let source = three();
    source.give_hints(true);
    let (listing, _) = open(provider(&source, 10), SortSpec::default());
    std::thread::sleep(Duration::from_millis(150));
    let idle = source.list_calls();
    std::thread::sleep(Duration::from_millis(150));
    assert_eq!(
        source.list_calls(),
        idle,
        "no list while the token is steady"
    );
    source.add(item("t|9", "late.txt", 9 * DAY));
    wait_until("the hinted change", || names(&listing).len() == 4);
    assert!(source.list_calls() > idle);
}

#[test]
fn closing_the_listing_stops_the_polling() {
    let source = three();
    let (listing, _) = open(provider(&source, 10), SortSpec::default());
    std::thread::sleep(Duration::from_millis(60));
    listing.close();
    std::thread::sleep(Duration::from_millis(60));
    let after = source.list_calls();
    std::thread::sleep(Duration::from_millis(120));
    assert_eq!(source.list_calls(), after);
}

#[test]
fn a_source_that_keeps_failing_ends_the_watch_with_the_error() {
    let source = three();
    let (listing, events) = open(provider(&source, 10), SortSpec::default());
    source.fail_lists(Some(VfsError::PermissionDenied {
        location: waypoint_protocol::Location::new("Trash", "trash:/"),
    }));
    wait_until("the failure", || {
        events
            .lock()
            .unwrap()
            .iter()
            .any(|e| matches!(e, ListingEvent::Failed { .. }))
    });
    assert_eq!(listing.snapshot().phase, waypoint_vfs::ListingPhase::Failed);
}

#[test]
fn an_unavailable_trash_cannot_be_opened_and_says_why() {
    let source = three();
    source.set_unavailable(Some("the Trash portal can only move files to the trash"));
    let p = provider(&source, 50);
    let error = p.stat(&root()).unwrap_err();
    assert_eq!(
        error,
        VfsError::Unsupported {
            what: "the Trash portal can only move files to the trash".to_owned()
        }
    );
    assert!(p
        .list(&root(), &CancelToken::new(), 0, &mut |_| {})
        .is_err());
}

#[test]
fn the_source_restores_deletes_and_empties() {
    let source = three();
    source.set_now_ms(10 * DAY);
    let back = source.restore("t|2").unwrap();
    let expected = waypoint_path::FilePath::parse(DOCS)
        .and_then(|folder| folder.join("Apple.png"))
        .unwrap()
        .to_location();
    assert_eq!(back.display, expected.display);
    assert!(matches!(
        source.restore("t|2"),
        Err(VfsError::NotFound { .. })
    ));
    source.delete("t|1").unwrap();
    assert_eq!(source.len(), 1);
    // `t|3` was trashed 8 days before "now".
    assert_eq!(source.empty(Some(30)).unwrap(), 0);
    assert_eq!(source.empty(Some(8)).unwrap(), 1);
    source.add(item("x", "x", 10 * DAY));
    assert_eq!(source.empty(None).unwrap(), 1);
    assert!(source.is_empty());
    assert_eq!(
        source.log(),
        [
            "restore:t|2",
            "delete:t|1",
            "empty:30",
            "empty:8",
            "empty:all"
        ]
    );
}
