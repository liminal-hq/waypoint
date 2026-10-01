// The Trash adapter and the Trash view over the Trash plugin's own Linux implementation, in a
// temporary freedesktop environment. Nothing here reads or writes the real Trash.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tauri::test::{mock_builder, mock_context, noop_assets, MockRuntime};
use tauri::Manager;
use tauri_plugin_trash::freedesktop::{system_clock, MountInfo, TrashEnv};
use tauri_plugin_waypoint_ops::{MemorySettings, Ops};
use waypoint_ops::{
    ConflictPolicy, JobKind, JobState, OpsError, OpsSettings, Protected, Resolution, Trash,
    TrashReceipt, WaitReason,
};
use waypoint_path::{FilePath, TrashPath, VfsPath};
use waypoint_protocol::{Location, VfsError};
use waypoint_vfs::{
    Filter, Listing, ListingHandle, ListingOptions, SortKey, SortSpec, TrashInfo, TrashSource,
};

use super::*;

struct Fixture {
    _tmp: tempfile::TempDir,
    root: PathBuf,
    app: tauri::App<MockRuntime>,
    settings: Arc<MemorySettings>,
}

fn fixture() -> Fixture {
    let tmp = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(tmp.path()).unwrap();
    std::fs::create_dir_all(root.join("home")).unwrap();
    std::fs::create_dir_all(root.join("work")).unwrap();
    let env = TrashEnv {
        data_home: root.join("home/.local/share"),
        home_dir: root.join("home"),
        uid: 4242,
        // The temporary directory is its own volume, so the home trash is the one used.
        mounts: vec![MountInfo {
            mount_point: root.clone(),
            device_id: 1,
            is_network: false,
            is_removable: false,
        }],
        now: system_clock(),
    };
    let settings = Arc::new(MemorySettings::default());
    let for_ops = settings.clone();
    let app = mock_builder()
        .plugin(tauri_plugin_trash::init_with_env(env))
        .plugin(tauri_plugin_waypoint_vfs::init())
        .plugin(tauri_plugin_waypoint_ops::init_with(move |app| {
            compose(
                app,
                Arc::new(JournalPersistence::new(MemoryKv::default())),
                for_ops,
                Protected::default(),
            )
        }))
        .build(mock_context(noop_assets()))
        .expect("the plugins initialise");
    Fixture {
        _tmp: tmp,
        root,
        app,
        settings,
    }
}

impl Fixture {
    fn adapter(&self) -> TrashAdapter<MockRuntime> {
        TrashAdapter::new(self.app.handle().clone())
    }

    fn ops(&self) -> Ops<MockRuntime> {
        self.app.state::<Ops<MockRuntime>>().inner().clone()
    }

    fn at(&self, relative: &str) -> PathBuf {
        self.root.join("work").join(relative)
    }

    fn write(&self, relative: &str, text: &str) -> Location {
        let path = self.at(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, text).unwrap();
        loc(&path)
    }

    /// Trashes the file at `relative` through the adapter and returns its receipt.
    fn trash(&self, relative: &str) -> TrashReceipt {
        let adapter = self.adapter();
        let location = loc(&self.at(relative));
        adapter
            .trash(&[location])
            .pop()
            .unwrap()
            .expect("the file is trashed")
    }

    fn wait(&self, what: &str, id: waypoint_ops::JobId) -> JobState {
        let started = Instant::now();
        loop {
            let state = self
                .ops()
                .snapshot()
                .jobs
                .into_iter()
                .find(|j| j.id == id)
                .map(|j| j.state);
            match state {
                Some(state) if state.is_finished() => return state,
                Some(JobState::Waiting { reason }) if what == "waiting" => {
                    return JobState::Waiting { reason }
                }
                _ => {}
            }
            assert!(
                started.elapsed() < Duration::from_secs(10),
                "timed out: {what}"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

fn loc(path: &Path) -> Location {
    FilePath::from_path(path).unwrap().to_location()
}

fn listing(fx: &Fixture) -> Arc<Listing> {
    let vfs = fx.app.state::<Vfs>();
    let provider = vfs
        .trash_provider()
        .expect("the app gave the Trash to the plugin");
    Listing::open(
        ListingHandle(1),
        VfsPath::Trash(TrashPath::Root),
        provider,
        SortSpec::default(),
        Filter::default(),
        ListingOptions::default(),
        Arc::new(|_| {}),
    )
    .expect("the Trash opens")
}

fn names(listing: &Listing) -> Vec<String> {
    listing
        .get_range(0, u32::MAX)
        .into_iter()
        .map(|e| e.name)
        .collect()
}

#[test]
fn the_view_lists_what_the_plugin_trashed_under_the_original_names() {
    let fx = fixture();
    fx.write("docs/report.txt", "twelve bytes");
    fx.write("docs/sub/notes.md", "x");
    std::fs::create_dir_all(fx.at("old stuff")).unwrap();
    std::fs::write(fx.at("old stuff/in.txt"), "123456").unwrap();
    fx.trash("docs/report.txt");
    fx.trash("docs/sub/notes.md");
    fx.trash("old stuff");

    let items = TrashSource::list(&fx.adapter()).unwrap();
    assert_eq!(items.len(), 3);
    let report = items.iter().find(|i| i.name == "report.txt").unwrap();
    assert_eq!(report.original_path, fx.at("docs").to_string_lossy());
    assert_eq!(report.size, 12);
    assert!(!report.is_dir);
    assert!(report.deleted_ms > 1_577_836_800_000);
    let folder = items.iter().find(|i| i.name == "old stuff").unwrap();
    assert!(folder.is_dir);
    assert_eq!(folder.size, 6);

    // The same through the provider the plugin serves.
    let listing = listing(&fx);
    assert_eq!(names(&listing), ["old stuff", "notes.md", "report.txt"]);
    let entries = listing.get_range(0, 10);
    assert_eq!(
        entries[2].original_path.as_deref(),
        Some(fx.at("docs").to_string_lossy().as_ref())
    );
    assert!(listing.snapshot().read_only);
}

#[test]
fn an_item_resolves_from_its_location_to_the_receipt_it_was_given() {
    let fx = fixture();
    fx.write("a b%c|d.txt", "x");
    fx.write("plain.txt", "y");
    let odd = fx.trash("a b%c|d.txt");
    let plain = fx.trash("plain.txt");

    let listing = listing(&fx);
    let adapter = fx.adapter();
    for entry in listing.get_range(0, 10) {
        let at = listing.path_of(entry.id).unwrap().to_location();
        assert!(at.uri.starts_with("trash:/"), "{}", at.uri);
        assert!(adapter.is_trashed(&at));
        let receipt = adapter.receipt_for(&at).unwrap();
        let known = [&odd, &plain];
        let original = known
            .iter()
            .find(|r| r.id == receipt.id)
            .expect("a receipt the Trash gave");
        // The receipt read back carries the real original place and date.
        assert_eq!(receipt.original, original.original);
        assert_eq!(receipt.deleted_at / 1000, original.deleted_at / 1000);
    }
    // Not an item: a file, the root, an id that is not there.
    for location in [
        loc(&fx.at("plain.txt")),
        Location::new("Trash", "trash:/"),
        Location::new("x", "trash:/nothing%7Chere"),
    ] {
        assert!(!adapter.is_trashed(&location) || location.uri.contains("nothing"));
        assert!(matches!(
            adapter.receipt_for(&location),
            Err(OpsError::NotFound { .. })
        ));
    }
}

#[test]
fn restore_puts_the_file_back_and_a_taken_name_is_refused_then_kept_both() {
    let fx = fixture();
    fx.write("a.txt", "old");
    let receipt = fx.trash("a.txt");
    fx.write("a.txt", "new");
    let adapter = fx.adapter();
    assert!(matches!(
        Trash::restore(&adapter, &receipt),
        Err(OpsError::NameInUse { .. })
    ));
    assert_eq!(std::fs::read_to_string(fx.at("a.txt")).unwrap(), "new");
    // To a name of the caller's choosing.
    let target = loc(&fx.at("a (2).txt"));
    let back = adapter.restore_to(&receipt, &target).unwrap();
    assert_eq!(back, target);
    assert_eq!(std::fs::read_to_string(fx.at("a (2).txt")).unwrap(), "old");
    assert!(TrashSource::list(&adapter).unwrap().is_empty());
}

#[test]
fn a_missing_folder_is_the_typed_error_naming_it() {
    let fx = fixture();
    fx.write("gone/f.txt", "x");
    let receipt = fx.trash("gone/f.txt");
    std::fs::remove_dir(fx.at("gone")).unwrap();
    let error = Trash::restore(&fx.adapter(), &receipt).unwrap_err();
    assert_eq!(
        error,
        OpsError::OriginMissingParent {
            location: loc(&fx.at("gone"))
        }
    );
}

#[test]
fn delete_and_empty_remove_items_and_the_list_notices() {
    let fx = fixture();
    for name in ["a", "b", "c"] {
        fx.write(name, name);
    }
    let a = fx.trash("a");
    fx.trash("b");
    fx.trash("c");
    let adapter = fx.adapter();
    assert_eq!(TrashSource::list(&adapter).unwrap().len(), 3);
    Trash::delete(&adapter, &a).unwrap();
    assert_eq!(
        TrashSource::list(&adapter).unwrap().len(),
        2,
        "the list was dropped"
    );
    assert!(!adapter.contains(&a).unwrap());
    assert!(matches!(
        Trash::delete(&adapter, &a),
        Err(OpsError::NotFound { .. })
    ));
    // Nothing is a day old yet.
    assert_eq!(Trash::empty(&adapter, Some(1)).unwrap(), 0);
    assert_eq!(Trash::empty(&adapter, None).unwrap(), 2);
    assert_eq!(TrashInfo::of(&adapter).count, 0);
}

#[test]
fn the_trash_info_counts_and_says_it_is_available() {
    let fx = fixture();
    assert_eq!(
        TrashInfo::of(&fx.adapter()),
        TrashInfo {
            available: true,
            reason: None,
            count: 0
        }
    );
    fx.write("a", "a");
    fx.trash("a");
    assert_eq!(TrashInfo::of(&fx.adapter()).count, 1);
    assert!(Trash::available(&fx.adapter()).is_ok());
    assert!(TrashSource::available(&fx.adapter()).is_ok());
}

#[test]
fn an_open_view_follows_the_trash_as_other_programs_change_it() {
    let fx = fixture();
    fx.write("a.txt", "a");
    fx.trash("a.txt");
    let listing = listing(&fx);
    assert_eq!(names(&listing), ["a.txt"]);
    // Another program trashes a file. The view polls every two seconds, and the adapter's list is
    // a second old at most, so the open listing shows it within a few.
    fx.write("b.txt", "b");
    fx.trash("b.txt");
    let started = Instant::now();
    while names(&listing).len() < 2 {
        assert!(
            started.elapsed() < Duration::from_secs(8),
            "the open view never saw the new item"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(names(&listing), ["a.txt", "b.txt"]);
}

#[test]
fn restore_and_delete_jobs_work_on_the_locations_the_view_gives() {
    let fx = fixture();
    fx.write("keep.txt", "k");
    fx.write("lose.txt", "l");
    fx.trash("keep.txt");
    fx.trash("lose.txt");
    let listing = listing(&fx);
    let by_name = |name: &str| {
        let entry = listing
            .get_range(0, 10)
            .into_iter()
            .find(|e| e.name == name)
            .unwrap();
        listing.path_of(entry.id).unwrap().to_location()
    };
    let request = |kind, locations| waypoint_ops::JobRequest {
        kind,
        sources: waypoint_ops::Sources::Locations { locations },
        destination: None,
        name: None,
        options: waypoint_ops::JobOptions::default(),
        origin_window: "main-1".to_owned(),
    };
    let ops = fx.ops();
    let restore = ops
        .submit(
            "main-1",
            request(JobKind::Restore, vec![by_name("keep.txt")]),
        )
        .unwrap();
    assert_eq!(fx.wait("restore", restore), JobState::Done);
    assert_eq!(std::fs::read_to_string(fx.at("keep.txt")).unwrap(), "k");
    let delete = ops
        .submit(
            "main-1",
            request(JobKind::Delete, vec![by_name("lose.txt")]),
        )
        .unwrap();
    assert_eq!(fx.wait("delete", delete), JobState::Done);
    assert!(!fx.at("lose.txt").exists());
    assert_eq!(TrashInfo::of(&fx.adapter()).count, 0);
}

#[test]
fn a_restore_over_a_taken_name_waits_and_keep_both_restores_beside_it() {
    let fx = fixture();
    fx.write("a.txt", "old");
    fx.trash("a.txt");
    fx.write("a.txt", "new");
    let listing = listing(&fx);
    let at = listing
        .path_of(listing.get_range(0, 1)[0].id)
        .unwrap()
        .to_location();
    let ops = fx.ops();
    let id = ops
        .submit(
            "main-1",
            waypoint_ops::JobRequest {
                kind: JobKind::Restore,
                sources: waypoint_ops::Sources::Locations {
                    locations: vec![at.clone()],
                },
                destination: None,
                name: None,
                options: waypoint_ops::JobOptions::default(),
                origin_window: "main-1".to_owned(),
            },
        )
        .unwrap();
    let JobState::Waiting {
        reason: WaitReason::Conflicts { conflicts },
    } = fx.wait("waiting", id)
    else {
        panic!("the job waits on the clash");
    };
    assert_eq!(conflicts[0].source, at);
    assert_eq!(conflicts[0].existing, loc(&fx.at("a.txt")));
    ops.resolve(
        id,
        vec![Resolution {
            source: None,
            policy: ConflictPolicy::KeepBoth,
        }],
        None,
    )
    .unwrap();
    assert_eq!(fx.wait("restore", id), JobState::Done);
    assert_eq!(std::fs::read_to_string(fx.at("a.txt")).unwrap(), "new");
    assert_eq!(std::fs::read_to_string(fx.at("a (2).txt")).unwrap(), "old");
}

#[test]
fn the_sweep_is_off_by_default_and_a_job_when_set() {
    let fx = fixture();
    fx.write("a", "a");
    fx.trash("a");
    let ops = fx.ops();
    assert_eq!(ops.settings().trash_expiry_days, None);
    assert_eq!(submit_sweep(&ops), None, "off by default");

    ops.set_settings(OpsSettings {
        trash_expiry_days: Some(30),
        ..ops.settings()
    })
    .unwrap();
    assert_eq!(fx.settings.saved().unwrap().trash_expiry_days, Some(30));
    let id = submit_sweep(&ops).expect("on: a job is queued");
    assert_eq!(fx.wait("sweep", id), JobState::Done);
    let job = ops
        .snapshot()
        .jobs
        .into_iter()
        .find(|j| j.id == id)
        .unwrap();
    assert_eq!(
        job.kind,
        JobKind::EmptyTrash {
            older_than_days: Some(30)
        }
    );
    // Nothing is thirty days old: the item stays.
    assert_eq!(TrashInfo::of(&fx.adapter()).count, 1);
    assert_eq!(
        sweep_request(7).kind,
        JobKind::EmptyTrash {
            older_than_days: Some(7)
        }
    );
}

#[test]
fn the_plugins_refusals_become_the_views() {
    let at = Location::new("Trash", "trash:/x");
    assert_eq!(
        vfs_error(TrashError::NotFound, &at),
        VfsError::NotFound {
            location: at.clone()
        }
    );
    assert!(matches!(
        vfs_error(TrashError::Unsupported, &at),
        VfsError::Unsupported { .. }
    ));
    assert!(matches!(
        vfs_error(
            TrashError::OriginExists {
                path: PathBuf::from("/a")
            },
            &at
        ),
        VfsError::AlreadyExists { .. }
    ));
    assert_eq!(
        ops_error(
            TrashError::OriginMissingParent {
                path: PathBuf::from("/gone")
            },
            &at
        ),
        OpsError::OriginMissingParent {
            location: loc(Path::new("/gone"))
        }
    );
}

#[test]
fn listings_sort_by_deletion_date() {
    let fx = fixture();
    for name in ["x", "y"] {
        fx.write(name, name);
        fx.trash(name);
        std::thread::sleep(Duration::from_millis(1100));
    }
    let listing = listing(&fx);
    listing.set_sort(SortSpec {
        key: SortKey::Deleted,
        descending: true,
        directories_first: false,
    });
    assert_eq!(names(&listing), ["y", "x"]);
}
