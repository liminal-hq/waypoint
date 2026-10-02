// Headless tests of the local provider and listings over temporary directories.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

// Several tests need Unix file names or permissions, so some imports are unused on Windows.
#![cfg_attr(windows, allow(unused_imports, dead_code))]

use std::fs::{self, File};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use tempfile::TempDir;
use waypoint_path::{FilePath, VfsPath};
use waypoint_protocol::{EntryId, VfsError};
use waypoint_vfs::{
    Change, Entry, EntryKind, Filter, GroupBy, GroupKey, GroupRun, IconGroup, KindFilter, Listing,
    ListingEvent, ListingHandle, ListingOptions, ListingPhase, LocalProvider, PatchOp, Provider,
    SizeBand, SortKey, SortSpec,
};

type Events = Arc<Mutex<Vec<ListingEvent>>>;

/// Options with watching off, so these tests see only what they cause.
fn quiet() -> ListingOptions {
    ListingOptions {
        watch: false,
        ..ListingOptions::default()
    }
}

fn folder(dir: &TempDir) -> VfsPath {
    VfsPath::File(FilePath::from_path(dir.path()).unwrap())
}

fn open_with(
    path: VfsPath,
    sort: SortSpec,
    filter: Filter,
    options: ListingOptions,
) -> (Result<Arc<Listing>, VfsError>, Events) {
    let events: Events = Arc::default();
    let sink = {
        let events = events.clone();
        Arc::new(move |event| events.lock().unwrap().push(event))
    };
    let listing = Listing::open(
        ListingHandle(1),
        path,
        Arc::new(LocalProvider::new()),
        sort,
        filter,
        options,
        sink,
    );
    (listing, events)
}

fn open(dir: &TempDir) -> Arc<Listing> {
    open_with(folder(dir), SortSpec::default(), Filter::default(), quiet())
        .0
        .unwrap()
}

fn all(listing: &Listing) -> Vec<Entry> {
    listing.get_range(0, u32::MAX)
}

fn names(listing: &Listing) -> Vec<String> {
    all(listing).into_iter().map(|e| e.name).collect()
}

fn touch(dir: &Path, name: &str, bytes: usize) {
    fs::write(dir.join(name), vec![b'x'; bytes]).unwrap();
}

fn sort(key: SortKey, descending: bool, directories_first: bool) -> SortSpec {
    SortSpec {
        key,
        descending,
        directories_first,
        ..SortSpec::default()
    }
}

#[cfg(unix)]
#[test]
fn lists_entries_with_kind_size_group_and_hidden() {
    let dir = TempDir::new().unwrap();
    touch(dir.path(), "notes.md", 12);
    touch(dir.path(), ".hidden", 1);
    touch(dir.path(), "photo.JPG", 3);
    fs::create_dir(dir.path().join("Docs")).unwrap();
    let listing = open(&dir);
    let snapshot = listing.snapshot();
    assert_eq!(snapshot.phase, ListingPhase::Ready);
    assert_eq!(snapshot.count, 3, "the hidden file is filtered out");

    let entries = all(&listing);
    let docs = &entries[0];
    assert_eq!(docs.name, "Docs");
    assert_eq!(docs.kind, EntryKind::Directory);
    assert_eq!(docs.group, IconGroup::Folder);
    assert_eq!(docs.size, None);
    let notes = &entries[1];
    assert_eq!((notes.name.as_str(), notes.size), ("notes.md", Some(12)));
    assert_eq!(notes.group, IconGroup::Document);
    assert!(notes.modified_ms.unwrap() > 1_500_000_000_000);
    assert_eq!(entries[2].group, IconGroup::Image);
    assert!(!entries.iter().any(|e| e.hidden));
}

#[test]
fn entries_take_their_ids_from_the_scan_and_resolve_to_paths() {
    let dir = TempDir::new().unwrap();
    touch(dir.path(), "a.txt", 1);
    let listing = open(&dir);
    let entry = &all(&listing)[0];
    let VfsPath::File(path) = listing.path_of(entry.id).unwrap() else {
        panic!("a local listing has local paths");
    };
    assert_eq!(path.as_path(), dir.path().join("a.txt"));
    assert!(matches!(
        listing.path_of(EntryId(99)),
        Err(VfsError::NotFound { .. })
    ));
}

#[test]
fn sorts_names_naturally_and_folders_first() {
    let dir = TempDir::new().unwrap();
    for name in [
        "img10.png",
        "img9.png",
        "img2.png",
        "Img1.png",
        "zeta",
        "Alpha",
    ] {
        touch(dir.path(), name, 1);
    }
    fs::create_dir(dir.path().join("b-dir")).unwrap();
    let listing = open(&dir);
    assert_eq!(
        names(&listing),
        [
            "b-dir",
            "Alpha",
            "Img1.png",
            "img2.png",
            "img9.png",
            "img10.png",
            "zeta"
        ]
    );

    listing.set_sort(sort(SortKey::Name, false, false));
    assert_eq!(names(&listing)[0], "Alpha");
    assert_eq!(names(&listing)[1], "b-dir");

    let snapshot = listing.set_sort(sort(SortKey::Name, true, true));
    assert_eq!(snapshot.sort, sort(SortKey::Name, true, true));
    let descending = names(&listing);
    assert_eq!(descending[0], "b-dir", "folders stay first when descending");
    assert_eq!(descending.last().unwrap(), "Alpha");
}

#[cfg(unix)]
#[test]
fn names_that_differ_only_in_case_keep_a_stable_order() {
    let dir = TempDir::new().unwrap();
    for name in ["b", "a", "B", "A"] {
        touch(dir.path(), name, 1);
    }
    let listing = open(&dir);
    // The folded names tie, so the raw bytes decide: upper case first, always the same way.
    assert_eq!(names(&listing), ["A", "a", "B", "b"]);
    listing.set_sort(sort(SortKey::Name, true, true));
    // Descending reverses the folded names; the tie within a pair still reads upper case first.
    assert_eq!(names(&listing), ["B", "b", "A", "a"]);
}

#[test]
fn sorts_by_size_modified_and_kind() {
    let dir = TempDir::new().unwrap();
    touch(dir.path(), "big.txt", 300);
    touch(dir.path(), "small.rs", 10);
    touch(dir.path(), "mid.png", 100);
    let epoch = SystemTime::UNIX_EPOCH;
    for (name, secs) in [("big.txt", 3000), ("small.rs", 1000), ("mid.png", 2000)] {
        File::options()
            .write(true)
            .open(dir.path().join(name))
            .unwrap()
            .set_modified(epoch + Duration::from_secs(secs))
            .unwrap();
    }
    let listing = open(&dir);
    listing.set_sort(sort(SortKey::Size, false, true));
    assert_eq!(names(&listing), ["small.rs", "mid.png", "big.txt"]);
    listing.set_sort(sort(SortKey::Size, true, true));
    assert_eq!(names(&listing), ["big.txt", "mid.png", "small.rs"]);
    listing.set_sort(sort(SortKey::Modified, false, true));
    assert_eq!(names(&listing), ["small.rs", "mid.png", "big.txt"]);
    listing.set_sort(sort(SortKey::Modified, true, true));
    assert_eq!(names(&listing), ["big.txt", "mid.png", "small.rs"]);
    // Kind orders by icon group (image, code, document in the enum's order), then extension.
    listing.set_sort(sort(SortKey::Kind, false, true));
    assert_eq!(names(&listing), ["mid.png", "small.rs", "big.txt"]);
}

#[cfg(unix)]
#[test]
fn filters_hidden_entries_and_kinds() {
    let dir = TempDir::new().unwrap();
    touch(dir.path(), "file", 1);
    touch(dir.path(), ".dotfile", 1);
    fs::create_dir(dir.path().join("folder")).unwrap();
    fs::create_dir(dir.path().join(".dotfolder")).unwrap();
    let listing = open(&dir);
    assert_eq!(names(&listing), ["folder", "file"]);

    let shown = listing.set_filter(Filter {
        show_hidden: true,
        only: None,
    });
    assert_eq!(shown.count, 4);
    assert_eq!(
        names(&listing),
        [".dotfolder", "folder", ".dotfile", "file"]
    );

    let folders = listing.set_filter(Filter {
        show_hidden: false,
        only: Some(KindFilter::Directories),
    });
    assert_eq!(folders.count, 1);
    assert_eq!(names(&listing), ["folder"]);
    assert_eq!(listing.get_range(5, 10), Vec::<Entry>::new());
}

#[test]
fn range_fetch_clamps_and_pages_add_up_to_the_whole() {
    let dir = TempDir::new().unwrap();
    for i in 0..25 {
        touch(dir.path(), &format!("f{i:02}"), 1);
    }
    let listing = open(&dir);
    let whole = all(&listing);
    assert_eq!(whole.len(), 25);
    let mut paged = Vec::new();
    for start in (0..30).step_by(10) {
        paged.extend(listing.get_range(start, 10));
    }
    assert_eq!(paged, whole);
    assert_eq!(listing.get_range(20, 100).len(), 5);
    assert!(listing.get_range(u32::MAX, u32::MAX).is_empty());
}

#[test]
fn revisions_only_ever_increase_and_no_op_changes_keep_them() {
    let dir = TempDir::new().unwrap();
    touch(dir.path(), "a", 1);
    let (listing, events) = open_with(
        folder(&dir),
        SortSpec::default(),
        Filter::default(),
        quiet(),
    );
    let listing = listing.unwrap();
    let scanned = listing.snapshot().revision;
    assert!(
        scanned > 1,
        "the first scan moves past the initial revision"
    );

    assert_eq!(listing.set_sort(SortSpec::default()).revision, scanned);
    let sorted = listing.set_sort(sort(SortKey::Size, false, true)).revision;
    let filtered = listing
        .set_filter(Filter {
            show_hidden: true,
            only: None,
        })
        .revision;
    assert!(scanned < sorted && sorted < filtered);

    let ops = listing.apply_changes(vec![Change::Remove("nothing".into())]);
    assert!(ops.is_empty());
    assert_eq!(listing.snapshot().revision, filtered);

    let ops = listing.apply_changes(vec![Change::Remove("a".into())]);
    assert_eq!(ops, [PatchOp::Remove { at: 0, count: 1 }]);
    let changed = listing.snapshot().revision;
    assert!(changed > filtered);

    let seen = events.lock().unwrap().clone();
    let mut last = 0;
    for event in &seen {
        let revision = match event {
            ListingEvent::Progress { revision, .. } | ListingEvent::Changed { revision, .. } => {
                *revision
            }
            ListingEvent::Failed { .. } => unreachable!(),
        };
        assert!(revision >= last, "events leave in revision order");
        last = revision;
    }
    assert_eq!(last, changed);
}

#[test]
fn a_scan_ends_with_one_ready_event_that_carries_the_final_count() {
    let dir = TempDir::new().unwrap();
    for i in 0..5 {
        touch(dir.path(), &format!("f{i}"), 1);
    }
    let (listing, events) = open_with(
        folder(&dir),
        SortSpec::default(),
        Filter::default(),
        quiet(),
    );
    let snapshot = listing.unwrap().snapshot();
    let seen = events.lock().unwrap().clone();
    let Some(ListingEvent::Progress {
        phase,
        scanned,
        count,
        revision,
        ..
    }) = seen.last()
    else {
        panic!("expected a progress event, got {seen:?}");
    };
    assert_eq!(*phase, ListingPhase::Ready);
    assert_eq!((*scanned, *count, *revision), (5, 5, snapshot.revision));
    let ready = seen
        .iter()
        .filter(|e| {
            matches!(
                e,
                ListingEvent::Progress {
                    phase: ListingPhase::Ready,
                    ..
                }
            )
        })
        .count();
    assert_eq!(ready, 1);
}

#[test]
fn a_missing_folder_a_file_and_a_denied_folder_are_typed_errors() {
    let dir = TempDir::new().unwrap();
    let missing = VfsPath::File(FilePath::from_path(dir.path().join("nope")).unwrap());
    let (result, _) = open_with(missing, SortSpec::default(), Filter::default(), quiet());
    assert!(matches!(result, Err(VfsError::NotFound { .. })));

    touch(dir.path(), "plain", 1);
    let file = VfsPath::File(FilePath::from_path(dir.path().join("plain")).unwrap());
    let (result, _) = open_with(file, SortSpec::default(), Filter::default(), quiet());
    assert!(matches!(result, Err(VfsError::NotADirectory { .. })));
}

#[cfg(unix)]
#[test]
fn a_folder_that_may_not_be_read_is_permission_denied() {
    use std::os::unix::fs::PermissionsExt;
    let dir = TempDir::new().unwrap();
    let locked = dir.path().join("locked");
    fs::create_dir(&locked).unwrap();
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
    // Root reads anything, so the test only means something for an ordinary user.
    let root = fs::read_dir(&locked).is_ok();
    let (result, _) = open_with(
        VfsPath::File(FilePath::from_path(&locked).unwrap()),
        SortSpec::default(),
        Filter::default(),
        quiet(),
    );
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
    if !root {
        assert!(matches!(result, Err(VfsError::PermissionDenied { .. })));
    }
}

#[cfg(unix)]
#[test]
fn names_that_are_not_utf8_sort_deterministically_and_resolve_to_their_real_path() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;
    let dir = TempDir::new().unwrap();
    let raw = OsStr::from_bytes(b"caf\xe9.txt");
    File::create(dir.path().join(raw)).unwrap();
    touch(dir.path(), "cafe.txt", 1);
    touch(dir.path(), "caff.txt", 1);
    let listing = open(&dir);
    let entries = all(&listing);
    assert_eq!(entries.len(), 3);
    // 0xE9 sorts after ASCII letters, so the name that is not UTF-8 comes last, every time.
    assert_eq!(entries[2].name, "caf\u{fffd}.txt");
    assert_eq!(entries[2].group, IconGroup::Document);
    let path = listing.path_of(entries[2].id).unwrap();
    let VfsPath::File(file) = path else {
        panic!("a local listing has local paths");
    };
    assert_eq!(
        file.as_path().file_name().unwrap().as_bytes(),
        raw.as_bytes()
    );
    assert!(file.as_path().exists());
}

#[cfg(unix)]
mod symlinks {
    use super::*;
    use std::os::unix::fs::symlink;

    fn fixture() -> TempDir {
        let dir = TempDir::new().unwrap();
        touch(dir.path(), "real.txt", 42);
        fs::create_dir(dir.path().join("realdir")).unwrap();
        symlink("real.txt", dir.path().join("to-file")).unwrap();
        symlink("realdir", dir.path().join("to-dir")).unwrap();
        symlink("missing", dir.path().join("broken")).unwrap();
        symlink("loop-b", dir.path().join("loop-a")).unwrap();
        symlink("loop-a", dir.path().join("loop-b")).unwrap();
        dir
    }

    fn entry(listing: &Listing, name: &str) -> Entry {
        all(listing).into_iter().find(|e| e.name == name).unwrap()
    }

    #[test]
    fn resolves_each_kind_of_link_at_scan_time() {
        let dir = fixture();
        let listing = open(&dir);
        let to_file = entry(&listing, "to-file");
        assert_eq!(to_file.kind, EntryKind::Symlink);
        assert_eq!(to_file.link_target, Some(EntryKind::File));
        assert_eq!(
            to_file.size,
            Some(42),
            "a link to a file shows the file's size"
        );
        let to_dir = entry(&listing, "to-dir");
        assert_eq!(to_dir.link_target, Some(EntryKind::Directory));
        assert_eq!(to_dir.group, IconGroup::Folder);
        assert_eq!(entry(&listing, "broken").link_target, None);
        assert_eq!(entry(&listing, "loop-a").link_target, None);
    }

    #[test]
    fn a_link_to_a_folder_sorts_with_the_folders() {
        let dir = fixture();
        let listing = open(&dir);
        let order = names(&listing);
        let first_file = order.iter().position(|n| n == "broken").unwrap();
        for folder in ["realdir", "to-dir"] {
            assert!(order.iter().position(|n| n == folder).unwrap() < first_file);
        }
    }

    #[test]
    fn a_link_removed_before_the_background_pass_is_not_resurrected() {
        let dir = fixture();
        let (listing, _events) = open_with(
            folder(&dir),
            SortSpec::default(),
            Filter::default(),
            ListingOptions {
                inline_link_budget: 0,
                ..ListingOptions::default()
            },
        );
        let listing = listing.unwrap();
        let kind = entry(&listing, "broken").kind;
        fs::remove_file(dir.path().join("broken")).unwrap();
        assert_eq!(listing.resolve_pending_links().unwrap(), 4);
        assert_eq!(entry(&listing, "broken").kind, kind);
    }

    #[test]
    fn past_the_inline_budget_links_resolve_in_a_background_pass_that_patches_the_view() {
        let dir = fixture();
        let (listing, events) = open_with(
            folder(&dir),
            SortSpec::default(),
            Filter::default(),
            ListingOptions {
                inline_link_budget: 0,
                ..quiet()
            },
        );
        let listing = listing.unwrap();
        // Unresolved links look like files, so the folder link sorts among them.
        let before = names(&listing);
        assert!(
            before.iter().position(|n| n == "to-dir") > before.iter().position(|n| n == "realdir")
        );
        assert_eq!(entry(&listing, "to-dir").link_target, None);

        let revision = listing.snapshot().revision;
        assert_eq!(listing.resolve_pending_links().unwrap(), 5);
        assert!(listing.snapshot().revision > revision);
        assert_eq!(
            entry(&listing, "to-dir").link_target,
            Some(EntryKind::Directory)
        );
        assert_eq!(
            entry(&listing, "to-file").link_target,
            Some(EntryKind::File)
        );
        let order = names(&listing);
        assert!(
            order.iter().position(|n| n == "to-dir") < order.iter().position(|n| n == "broken")
        );
        assert!(events
            .lock()
            .unwrap()
            .iter()
            .any(|e| matches!(e, ListingEvent::Changed { .. })));
        // Nothing is left to do the second time.
        assert_eq!(listing.resolve_pending_links().unwrap(), 0);
    }
}

#[test]
fn cancelling_before_the_scan_returns_cancelled() {
    let dir = TempDir::new().unwrap();
    touch(dir.path(), "a", 1);
    let listing = Listing::new(
        ListingHandle(1),
        folder(&dir),
        Arc::new(LocalProvider::new()),
        SortSpec::default(),
        Filter::default(),
        quiet(),
        Arc::new(|_| {}),
    );
    listing.close();
    assert_eq!(listing.scan().map(|_| ()), Err(VfsError::Cancelled));
    assert_eq!(listing.snapshot().phase, ListingPhase::Failed);
}

#[test]
fn cancelling_an_in_flight_scan_stops_it() {
    let dir = TempDir::new().unwrap();
    for i in 0..4000 {
        touch(dir.path(), &format!("f{i}"), 0);
    }
    let cell: Arc<Mutex<Option<Arc<Listing>>>> = Arc::default();
    let sink = {
        let cell = cell.clone();
        Arc::new(move |event: ListingEvent| {
            // The first progress report, about a quarter of the way in, cancels the scan.
            if matches!(event, ListingEvent::Progress { .. }) {
                if let Some(listing) = cell.lock().unwrap().as_ref() {
                    listing.close();
                }
            }
        })
    };
    let listing = Listing::new(
        ListingHandle(1),
        folder(&dir),
        Arc::new(LocalProvider::new()),
        SortSpec::default(),
        Filter::default(),
        ListingOptions {
            progress_interval: Duration::ZERO,
            ..quiet()
        },
        sink,
    );
    *cell.lock().unwrap() = Some(listing.clone());
    assert_eq!(listing.scan().map(|_| ()), Err(VfsError::Cancelled));
    assert_eq!(
        listing.snapshot().count,
        0,
        "a cancelled scan publishes nothing"
    );
    *cell.lock().unwrap() = None;
}

#[test]
fn stat_describes_one_entry() {
    let dir = TempDir::new().unwrap();
    touch(dir.path(), "a.zip", 7);
    let path = VfsPath::File(FilePath::from_path(dir.path().join("a.zip")).unwrap());
    let entry = LocalProvider::new().stat(&path).unwrap();
    assert_eq!(entry.name, "a.zip");
    assert_eq!(entry.size, Some(7));
    assert_eq!(entry.group, IconGroup::Archive);
    let missing = VfsPath::File(FilePath::from_path(dir.path().join("x")).unwrap());
    assert!(matches!(
        LocalProvider::new().stat(&missing),
        Err(VfsError::NotFound { .. })
    ));
}

/// Windows hides files by attribute, not by a leading dot.
#[cfg(windows)]
#[test]
fn a_file_with_the_hidden_attribute_is_hidden_and_a_dotfile_is_not() {
    let dir = TempDir::new().unwrap();
    touch(dir.path(), "visible.txt", 1);
    touch(dir.path(), ".dotfile", 1);
    touch(dir.path(), "secret.txt", 1);
    let status = std::process::Command::new("attrib")
        .arg("+h")
        .arg(dir.path().join("secret.txt"))
        .status()
        .unwrap();
    assert!(status.success());
    let listing = open(&dir);
    assert_eq!(names(&listing), [".dotfile", "visible.txt"]);
}

#[cfg(unix)]
mod selection {
    use super::*;
    use waypoint_vfs::{SelectionSpec, SelectionSummary};

    /// `a` (1 byte), `bb` (2), `ccc` (3), a hidden `.d` (4) and a folder `dir`.
    fn sized() -> (TempDir, Arc<Listing>, Vec<Entry>) {
        let dir = TempDir::new().unwrap();
        for (name, size) in [("a", 1), ("bb", 2), ("ccc", 3), (".d", 4)] {
            fs::write(dir.path().join(name), vec![0u8; size]).unwrap();
        }
        fs::create_dir(dir.path().join("dir")).unwrap();
        let listing = open(&dir);
        let entries = listing.get_range(0, 100);
        (dir, listing, entries)
    }

    fn id(entries: &[Entry], name: &str) -> EntryId {
        entries.iter().find(|e| e.name == name).unwrap().id
    }

    #[test]
    fn chosen_entries_add_up_and_folders_add_nothing() {
        let (_dir, listing, entries) = sized();
        let summary = listing.summarise_selection(&SelectionSpec::Chosen {
            ids: vec![id(&entries, "bb"), id(&entries, "ccc"), id(&entries, "dir")],
        });
        assert_eq!(
            summary,
            SelectionSummary {
                count: 3,
                total_size: 5
            }
        );
    }

    #[test]
    fn all_except_covers_the_rest_of_the_view() {
        let (_dir, listing, entries) = sized();
        // The hidden file is not in the view, so it is neither counted nor summed.
        let summary = listing.summarise_selection(&SelectionSpec::AllExcept {
            ids: vec![id(&entries, "a")],
        });
        assert_eq!(
            summary,
            SelectionSummary {
                count: 3,
                total_size: 5
            }
        );
        let all = listing.summarise_selection(&SelectionSpec::AllExcept { ids: vec![] });
        assert_eq!(
            all,
            SelectionSummary {
                count: 4,
                total_size: 6
            }
        );
    }

    #[test]
    fn unknown_filtered_and_repeated_ids_do_not_count_twice_or_at_all() {
        let (_dir, listing, entries) = sized();
        let a = id(&entries, "a");
        let hidden = listing.get_range(0, 100).len();
        assert_eq!(hidden, 4);
        let hidden_id = {
            listing.set_filter(Filter {
                show_hidden: true,
                only: None,
            });
            let all = listing.get_range(0, 100);
            let id = all.iter().find(|e| e.name == ".d").unwrap().id;
            listing.set_filter(Filter::default());
            id
        };
        let chosen = listing.summarise_selection(&SelectionSpec::Chosen {
            ids: vec![a, a, hidden_id, EntryId(9999)],
        });
        assert_eq!(
            chosen,
            SelectionSummary {
                count: 1,
                total_size: 1
            }
        );
        let except = listing.summarise_selection(&SelectionSpec::AllExcept {
            ids: vec![a, a, hidden_id, EntryId(9999)],
        });
        assert_eq!(
            except,
            SelectionSummary {
                count: 3,
                total_size: 5
            }
        );
    }
}

#[test]
fn a_grouped_listing_carries_its_groups_in_the_snapshot_and_every_event() {
    let dir = TempDir::new().unwrap();
    touch(dir.path(), "small.txt", 5);
    touch(dir.path(), "large.bin", 2_000_000);
    let by_size = SortSpec {
        group_by: GroupBy::Size,
        ..SortSpec::default()
    };
    let (listing, events) = open_with(folder(&dir), by_size, Filter::default(), quiet());
    let listing = listing.unwrap();
    let bands = |groups: &[GroupRun]| -> Vec<(GroupKey, u32, u32)> {
        groups
            .iter()
            .map(|g| (g.key.clone(), g.start, g.count))
            .collect()
    };
    let size = |band| GroupKey::Size { band };
    let expected = vec![(size(SizeBand::Tiny), 0, 1), (size(SizeBand::Large), 1, 1)];
    assert_eq!(bands(&listing.snapshot().groups), expected);
    let ready = events
        .lock()
        .unwrap()
        .iter()
        .find_map(|e| match e {
            ListingEvent::Progress {
                phase: ListingPhase::Ready,
                groups,
                ..
            } => groups.clone(),
            _ => None,
        })
        .expect("the scan's last event carries the groups");
    assert_eq!(bands(&ready), expected);

    // A row that moves to another group arrives with the new boundaries.
    touch(dir.path(), "small.txt", 3_000_000);
    let mut grown =
        listing
            .provider()
            .list(listing.path(), listing.cancel_token(), 10, &mut |_| {});
    let entry = grown
        .as_mut()
        .unwrap()
        .iter()
        .find(|e| e.name == "small.txt")
        .unwrap()
        .clone();
    listing.apply_changes(vec![Change::Upsert(entry)]);
    let changed = events
        .lock()
        .unwrap()
        .iter()
        .rev()
        .find_map(|e| match e {
            ListingEvent::Changed { groups, .. } => groups.clone(),
            _ => None,
        })
        .expect("a change carries the groups");
    assert_eq!(bands(&changed), vec![(size(SizeBand::Large), 0, 2)]);

    // Re-sorting without a group sends none.
    let plain = listing.set_sort(SortSpec::default());
    assert!(plain.groups.is_empty());
    assert_eq!(plain.sort.group_by, GroupBy::None);
}
