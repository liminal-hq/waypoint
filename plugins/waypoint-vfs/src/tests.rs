// Plugin-level tests: the commands run against a mock Tauri app with real temporary folders
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::fs;
use std::path::Path;
use std::sync::mpsc::{channel, Receiver};
use std::time::Duration;

use tauri::test::{mock_builder, mock_context, noop_assets, MockRuntime};
use tauri::{Listener, Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent};
use waypoint_path::FilePath;
use waypoint_protocol::{Location, VfsError};
use waypoint_vfs::{
    Filter, ListingEvent, ListingHandle, ListingPhase, ListingSnapshot, SortKey, SortSpec,
};

use crate::commands::{self, OpenOptions};
use crate::{init, on_window_event, Error, Vfs, LISTING_EVENT};

fn app() -> tauri::App<MockRuntime> {
    let app = mock_builder()
        .plugin(init())
        .build(mock_context(noop_assets()))
        .expect("the mock app builds");
    for label in ["main", "other"] {
        WebviewWindowBuilder::new(&app, label, WebviewUrl::App("index.html".into()))
            .build()
            .expect("the mock window opens");
    }
    app
}

fn window(app: &tauri::App<MockRuntime>, label: &str) -> tauri::Window<MockRuntime> {
    app.get_webview_window(label)
        .expect("the window exists")
        .as_ref()
        .window()
}

fn location(path: &Path) -> Location {
    FilePath::from_path(path).unwrap().to_location()
}

fn events(window: &tauri::Window<MockRuntime>) -> Receiver<ListingEvent> {
    let (sender, receiver) = channel();
    window.listen(LISTING_EVENT, move |event| {
        let _ = sender.send(serde_json::from_str(event.payload()).expect("a listing event"));
    });
    receiver
}

fn open(
    app: &tauri::App<MockRuntime>,
    label: &str,
    location: Location,
) -> Result<ListingSnapshot, Error> {
    tauri::async_runtime::block_on(commands::open_listing(
        window(app, label),
        app.state::<Vfs>(),
        location,
        None,
    ))
}

fn range(
    app: &tauri::App<MockRuntime>,
    label: &str,
    handle: ListingHandle,
    start: u32,
    count: u32,
) -> Result<Vec<waypoint_vfs::Entry>, Error> {
    tauri::async_runtime::block_on(commands::get_range(
        window(app, label),
        app.state::<Vfs>(),
        handle,
        start,
        count,
    ))
}

fn names(entries: &[waypoint_vfs::Entry]) -> Vec<&str> {
    entries.iter().map(|e| e.name.as_str()).collect()
}

fn folder_with(files: &[&str]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for name in files {
        fs::write(dir.path().join(name), b"x").unwrap();
    }
    dir
}

/// Waits for the event `wanted` accepts, ignoring others.
fn wait_for(
    events: &Receiver<ListingEvent>,
    wanted: impl Fn(&ListingEvent) -> bool,
) -> ListingEvent {
    loop {
        let event = events
            .recv_timeout(Duration::from_secs(10))
            .expect("the expected listing event arrived");
        if wanted(&event) {
            return event;
        }
    }
}

fn is_ready(event: &ListingEvent) -> bool {
    matches!(
        event,
        ListingEvent::Progress {
            phase: ListingPhase::Ready,
            ..
        }
    )
}

#[test]
fn opening_returns_a_scanning_snapshot_then_progress_ends_ready() {
    let dir = folder_with(&["b.txt", "a.txt", "c.txt"]);
    let app = app();
    let received = events(&window(&app, "main"));

    let snapshot = open(&app, "main", location(dir.path())).unwrap();
    assert_eq!(snapshot.phase, ListingPhase::Scanning);
    assert_eq!(snapshot.handle, ListingHandle(1));
    assert_eq!(snapshot.revision, 1);

    let ready = wait_for(&received, is_ready);
    let ListingEvent::Progress {
        handle,
        count,
        scanned,
        revision,
        ..
    } = ready
    else {
        unreachable!()
    };
    assert_eq!((handle, count, scanned), (snapshot.handle, 3, 3));
    assert!(revision > snapshot.revision);

    let entries = range(&app, "main", snapshot.handle, 0, 10).unwrap();
    assert_eq!(names(&entries), ["a.txt", "b.txt", "c.txt"]);
    assert_eq!(
        names(&range(&app, "main", snapshot.handle, 1, 1).unwrap()),
        ["b.txt"]
    );
}

#[test]
fn events_go_only_to_the_owning_window() {
    let dir = folder_with(&["a"]);
    let app = app();
    let mine = events(&window(&app, "main"));
    let theirs = events(&window(&app, "other"));
    open(&app, "main", location(dir.path())).unwrap();
    wait_for(&mine, is_ready);
    assert!(theirs.recv_timeout(Duration::from_millis(200)).is_err());
}

#[test]
fn sort_and_filter_return_new_revisions_and_reorder_the_view() {
    let dir = folder_with(&["a.txt", "b.txt", ".hidden"]);
    let app = app();
    let received = events(&window(&app, "main"));
    let snapshot = open(&app, "main", location(dir.path())).unwrap();
    wait_for(&received, is_ready);
    let state = app.state::<Vfs>();

    let sorted = tauri::async_runtime::block_on(commands::set_sort(
        window(&app, "main"),
        state.clone(),
        snapshot.handle,
        SortSpec {
            key: SortKey::Name,
            descending: true,
            directories_first: true,
            ..SortSpec::default()
        },
    ))
    .unwrap();
    assert!(sorted.revision > snapshot.revision);
    assert_eq!(
        names(&range(&app, "main", snapshot.handle, 0, 10).unwrap()),
        ["b.txt", "a.txt"]
    );

    let filtered = tauri::async_runtime::block_on(commands::set_filter(
        window(&app, "main"),
        state,
        snapshot.handle,
        Filter {
            show_hidden: true,
            only: None,
        },
    ))
    .unwrap();
    assert!(filtered.revision > sorted.revision);
    assert_eq!(filtered.count, 3);
}

#[test]
fn open_options_set_the_initial_sort_and_filter() {
    let dir = folder_with(&["a", ".h"]);
    let app = app();
    let snapshot = tauri::async_runtime::block_on(commands::open_listing(
        window(&app, "main"),
        app.state::<Vfs>(),
        location(dir.path()),
        Some(OpenOptions {
            sort: None,
            filter: Some(Filter {
                show_hidden: true,
                only: None,
            }),
        }),
    ))
    .unwrap();
    assert!(snapshot.filter.show_hidden);
}

#[test]
fn a_closed_or_unknown_handle_is_stale_and_closing_it_again_is_fine() {
    let dir = folder_with(&["a"]);
    let app = app();
    let snapshot = open(&app, "main", location(dir.path())).unwrap();
    let close = |handle| {
        tauri::async_runtime::block_on(commands::close_listing(
            window(&app, "main"),
            app.state::<Vfs>(),
            handle,
        ))
    };
    close(snapshot.handle).unwrap();
    close(snapshot.handle).unwrap();
    close(ListingHandle(4242)).unwrap();
    let stale = range(&app, "main", snapshot.handle, 0, 1).unwrap_err();
    assert!(matches!(stale, Error::Vfs(VfsError::StaleHandle)));
    assert!(app.state::<Vfs>().registry.is_empty());
}

#[test]
fn another_windows_handle_is_stale_and_survives_its_close() {
    let dir = folder_with(&["a"]);
    let app = app();
    let snapshot = open(&app, "main", location(dir.path())).unwrap();
    assert!(matches!(
        range(&app, "other", snapshot.handle, 0, 1).unwrap_err(),
        Error::Vfs(VfsError::StaleHandle)
    ));
    tauri::async_runtime::block_on(commands::close_listing(
        window(&app, "other"),
        app.state::<Vfs>(),
        snapshot.handle,
    ))
    .unwrap();
    assert!(range(&app, "main", snapshot.handle, 0, 1).is_ok());
}

#[test]
fn destroying_a_window_closes_its_listings_only() {
    let dir = folder_with(&["a"]);
    let app = app();
    let mine = open(&app, "main", location(dir.path())).unwrap();
    let theirs = open(&app, "other", location(dir.path())).unwrap();
    assert_eq!(app.state::<Vfs>().registry.len(), 2);

    on_window_event(app.handle(), "main", &WindowEvent::Destroyed);

    assert_eq!(app.state::<Vfs>().registry.len(), 1);
    assert!(range(&app, "main", mine.handle, 0, 1).is_err());
    assert!(range(&app, "other", theirs.handle, 0, 1).is_ok());
}

#[test]
fn missing_and_non_folder_locations_are_rejected_with_typed_errors() {
    let dir = folder_with(&["file.txt"]);
    let app = app();
    let missing = open(&app, "main", location(&dir.path().join("nope"))).unwrap_err();
    assert!(matches!(missing, Error::Vfs(VfsError::NotFound { .. })));
    let file = open(&app, "main", location(&dir.path().join("file.txt"))).unwrap_err();
    assert!(matches!(file, Error::Vfs(VfsError::NotADirectory { .. })));
    let junk = open(&app, "main", Location::new("x", "not a uri")).unwrap_err();
    assert!(matches!(junk, Error::Vfs(VfsError::InvalidLocation { .. })));

    // On the wire the rejection is the tagged object, not a string.
    let wire = serde_json::to_value(&missing).unwrap();
    assert_eq!(wire["kind"], "notFound");
    assert!(wire["location"]["uri"].is_string());
    assert!(app.state::<Vfs>().registry.is_empty());
}

#[test]
fn a_file_created_under_a_ready_listing_arrives_as_a_changed_event() {
    let dir = folder_with(&["a.txt"]);
    let app = app();
    let received = events(&window(&app, "main"));
    let snapshot = open(&app, "main", location(dir.path())).unwrap();
    wait_for(&received, is_ready);

    fs::write(dir.path().join("b.txt"), b"x").unwrap();
    let changed = wait_for(&received, |e| matches!(e, ListingEvent::Changed { .. }));
    let ListingEvent::Changed { handle, count, .. } = changed else {
        unreachable!()
    };
    assert_eq!((handle, count), (snapshot.handle, 2));
}

#[test]
fn nothing_is_emitted_after_a_listing_closes() {
    let dir = folder_with(&["a.txt"]);
    let app = app();
    let received = events(&window(&app, "main"));
    let snapshot = open(&app, "main", location(dir.path())).unwrap();
    wait_for(&received, is_ready);
    tauri::async_runtime::block_on(commands::close_listing(
        window(&app, "main"),
        app.state::<Vfs>(),
        snapshot.handle,
    ))
    .unwrap();
    fs::write(dir.path().join("late.txt"), b"x").unwrap();
    assert!(received.recv_timeout(Duration::from_millis(600)).is_err());
}

#[test]
fn status_reports_the_features() {
    let app = app();
    let status = tauri::async_runtime::block_on(commands::get_status(app.state::<Vfs>())).unwrap();
    assert!(status.available);
    assert!(status.features.iter().any(|f| f == "listing"));
    assert!(status.features.iter().any(|f| f == "watch"));
    assert!(!status.features.iter().any(|f| f == "polling-fallback"));
}

fn ready_listing(
    app: &tauri::App<MockRuntime>,
    dir: &tempfile::TempDir,
) -> (ListingSnapshot, Vec<waypoint_vfs::Entry>) {
    let received = events(&window(app, "main"));
    let snapshot = open(app, "main", location(dir.path())).unwrap();
    wait_for(&received, is_ready);
    let entries = range(app, "main", snapshot.handle, 0, 100).unwrap();
    (snapshot, entries)
}

#[test]
fn typed_text_becomes_a_location_and_junk_is_rejected() {
    let app = app();
    let base = location(Path::new("/srv/data"));
    let parse = |input: &str| {
        tauri::async_runtime::block_on(commands::parse_location(
            app.state::<Vfs>(),
            input.to_owned(),
            base.clone(),
        ))
    };
    assert_eq!(parse("logs").unwrap().display, "/srv/data/logs");
    assert_eq!(parse("/etc").unwrap().uri, "file:///etc");
    assert!(parse("~").unwrap().display.starts_with('/'));
    assert!(matches!(
        parse("").unwrap_err(),
        Error::Vfs(VfsError::InvalidLocation { .. })
    ));
    assert!(matches!(
        parse("smb://host/share").unwrap_err(),
        Error::Vfs(VfsError::Unsupported { .. })
    ));
    drop(app);
}

#[test]
fn a_location_is_described_with_its_breadcrumbs() {
    let app = app();
    let info = tauri::async_runtime::block_on(commands::describe_location(
        app.state::<Vfs>(),
        location(Path::new("/home/a/Music")),
    ))
    .unwrap();
    assert_eq!(info.segments.len(), 4);
    assert_eq!(info.parent.unwrap().display, "/home/a");
}

#[test]
fn an_entry_resolves_to_its_location_and_selections_add_up() {
    let dir = folder_with(&["a.txt", "bb.txt"]);
    let app = app();
    let (snapshot, entries) = ready_listing(&app, &dir);
    let b = entries.iter().find(|e| e.name == "bb.txt").unwrap();

    let there = tauri::async_runtime::block_on(commands::entry_location(
        window(&app, "main"),
        app.state::<Vfs>(),
        snapshot.handle,
        b.id,
    ))
    .unwrap();
    assert!(there.display.ends_with("/bb.txt"));
    assert!(there.uri.starts_with("file://"));

    let summarise = |selection| {
        tauri::async_runtime::block_on(commands::summarise_selection(
            window(&app, "main"),
            app.state::<Vfs>(),
            snapshot.handle,
            selection,
        ))
        .unwrap()
    };
    let chosen = summarise(waypoint_vfs::SelectionSpec::Chosen { ids: vec![b.id] });
    assert_eq!((chosen.count, chosen.total_size), (1, 1));
    let rest = summarise(waypoint_vfs::SelectionSpec::AllExcept { ids: vec![b.id] });
    assert_eq!((rest.count, rest.total_size), (1, 1));
}

#[test]
fn a_selection_resolves_to_locations_in_view_order_for_the_owning_window_only() {
    let dir = folder_with(&["a.txt", "b.txt", "c.txt", ".hidden"]);
    let app = app();
    let (snapshot, entries) = ready_listing(&app, &dir);
    let id = |name: &str| entries.iter().find(|e| e.name == name).unwrap().id;
    let vfs = app.state::<Vfs>();
    let names = |locations: Vec<Location>| -> Vec<String> {
        locations
            .into_iter()
            .map(|l| l.display.rsplit('/').next().unwrap().to_owned())
            .collect()
    };

    // Chosen ids come back in the order the view shows them, once each, whatever order they came in.
    let chosen = waypoint_vfs::SelectionSpec::Chosen {
        ids: vec![id("c.txt"), id("a.txt"), id("c.txt")],
    };
    let resolved = vfs
        .resolve_selection("main", snapshot.handle, &chosen)
        .unwrap();
    assert_eq!(names(resolved.clone()), ["a.txt", "c.txt"]);
    assert!(resolved[0].uri.starts_with("file://") && resolved[0].uri.ends_with("/a.txt"));

    // "All except" covers the rest of the view, and not what the filter hides.
    let rest = waypoint_vfs::SelectionSpec::AllExcept {
        ids: vec![id("b.txt")],
    };
    let resolved = vfs
        .resolve_selection("main", snapshot.handle, &rest)
        .unwrap();
    assert_eq!(names(resolved), ["a.txt", "c.txt"]);

    // An id the view does not hold is dropped, as `summarise_selection` drops it.
    let gone = waypoint_vfs::SelectionSpec::Chosen {
        ids: vec![waypoint_protocol::EntryId(9999)],
    };
    assert!(vfs
        .resolve_selection("main", snapshot.handle, &gone)
        .unwrap()
        .is_empty());

    // Another window cannot resolve this window's listing.
    let foreign = vfs.resolve_selection("other", snapshot.handle, &chosen);
    assert_eq!(foreign, Err(VfsError::StaleHandle));
}

#[test]
fn entries_are_located_one_by_one_in_the_order_asked_for_the_owning_window_only() {
    let dir = folder_with(&["a.txt", "b.txt", "c.txt"]);
    let app = app();
    let (snapshot, entries) = ready_listing(&app, &dir);
    let id = |name: &str| entries.iter().find(|e| e.name == name).unwrap().id;
    let vfs = app.state::<Vfs>();

    // The order asked for is kept (not the view's), and a repeated id is answered twice.
    let ids = [
        id("c.txt"),
        id("a.txt"),
        id("c.txt"),
        waypoint_protocol::EntryId(9999),
    ];
    let located = vfs.locate_entries("main", snapshot.handle, &ids).unwrap();
    let tail = |l: &Option<Location>| {
        l.as_ref()
            .map(|l| l.display.rsplit('/').next().unwrap().to_owned())
    };
    let names: Vec<_> = located.iter().map(tail).collect();
    assert_eq!(
        names,
        [
            Some("c.txt".to_owned()),
            Some("a.txt".to_owned()),
            Some("c.txt".to_owned()),
            None
        ]
    );
    assert!(located[0].as_ref().unwrap().uri.starts_with("file://"));

    // Another window cannot locate this window's entries.
    assert_eq!(
        vfs.locate_entries("other", snapshot.handle, &ids),
        Err(VfsError::StaleHandle)
    );
}

#[test]
fn entry_commands_are_scoped_to_the_owning_window() {
    let dir = folder_with(&["a.txt"]);
    let app = app();
    let (snapshot, entries) = ready_listing(&app, &dir);
    let id = entries[0].id;
    let foreign = tauri::async_runtime::block_on(commands::open_entry(
        window(&app, "other"),
        app.state::<Vfs>(),
        snapshot.handle,
        id,
    ))
    .unwrap_err();
    assert!(matches!(foreign, Error::Vfs(VfsError::StaleHandle)));
    let unknown = tauri::async_runtime::block_on(commands::open_entry(
        window(&app, "main"),
        app.state::<Vfs>(),
        snapshot.handle,
        waypoint_protocol::EntryId(999),
    ))
    .unwrap_err();
    assert!(matches!(unknown, Error::Vfs(VfsError::NotFound { .. })));
}

#[test]
fn opening_a_folder_as_a_file_is_refused() {
    let dir = folder_with(&[]);
    fs::create_dir(dir.path().join("sub")).unwrap();
    let app = app();
    let (snapshot, entries) = ready_listing(&app, &dir);
    let error = tauri::async_runtime::block_on(commands::open_entry(
        window(&app, "main"),
        app.state::<Vfs>(),
        snapshot.handle,
        entries[0].id,
    ))
    .unwrap_err();
    assert!(matches!(error, Error::Vfs(VfsError::Unsupported { .. })));
}

#[test]
fn free_space_is_reported_for_a_real_folder_and_null_for_a_missing_one() {
    let dir = folder_with(&[]);
    let space = tauri::async_runtime::block_on(commands::get_free_space(location(dir.path())))
        .unwrap()
        .expect("space");
    assert!(space.total_bytes >= space.free_bytes);
    let missing = tauri::async_runtime::block_on(commands::get_free_space(location(
        &dir.path().join("gone"),
    )))
    .unwrap();
    assert!(missing.is_none());
}

#[test]
fn check_folder_tells_a_writable_folder_from_a_file_a_read_only_folder_and_a_missing_one() {
    use std::os::unix::fs::PermissionsExt;
    let app = app();
    let dir = folder_with(&["a.txt"]);
    let check = |path: &Path| {
        tauri::async_runtime::block_on(commands::check_folder(app.state::<Vfs>(), location(path)))
    };
    let folder = check(dir.path()).unwrap();
    assert!(folder.is_folder && folder.writable);
    let file = check(&dir.path().join("a.txt")).unwrap();
    assert!(!file.is_folder && !file.writable);
    let locked = dir.path().join("locked");
    fs::create_dir(&locked).unwrap();
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o555)).unwrap();
    let locked_check = check(&locked).unwrap();
    assert!(locked_check.is_folder && !locked_check.writable);
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
    let missing = check(&dir.path().join("gone")).unwrap_err();
    assert!(matches!(missing, Error::Vfs(VfsError::NotFound { .. })));
}

// ---- the Trash ----

fn trash_location() -> Location {
    Location::new("Trash", "trash:/")
}

fn trashed(id: &str, name: &str) -> waypoint_vfs::TrashedItem {
    waypoint_vfs::TrashedItem {
        id: id.to_owned(),
        name: name.to_owned(),
        original_path: "/home/a".to_owned(),
        deleted_ms: 1_700_000_000_000,
        size: 5,
        is_dir: false,
    }
}

#[test]
fn the_trash_cannot_be_opened_until_the_app_gives_the_plugin_one() {
    let app = app();
    let error = open(&app, "main", trash_location()).unwrap_err();
    assert!(matches!(error, Error::Vfs(VfsError::Unsupported { .. })));
    let info =
        tauri::async_runtime::block_on(commands::get_trash_info(app.state::<Vfs>(), None)).unwrap();
    assert!(!info.available);
    assert_eq!(info.count, 0);
    assert_eq!(info.total_bytes, None);
    assert!(info.reason.is_some());
}

#[test]
fn a_trash_listing_serves_items_by_their_original_names_and_reads_only() {
    let source = std::sync::Arc::new(waypoint_vfs::MemoryTrashSource::new());
    source.add(trashed("t|1", "zebra.txt"));
    source.add(trashed("t|2", "apple.txt"));
    let app = app();
    app.state::<Vfs>().set_trash_source(source.clone());
    let received = events(&window(&app, "main"));

    let snapshot = open(&app, "main", trash_location()).unwrap();
    assert!(snapshot.read_only);
    assert_eq!(snapshot.layout, waypoint_vfs::ListingLayout::Trash);
    wait_for(&received, is_ready);
    let entries = range(&app, "main", snapshot.handle, 0, 10).unwrap();
    assert_eq!(names(&entries), ["apple.txt", "zebra.txt"]);
    assert_eq!(entries[0].original_path.as_deref(), Some("/home/a"));
    assert_eq!(entries[0].deleted_ms, Some(1_700_000_000_000));

    // The sizes are added up only when asked for.
    let info =
        tauri::async_runtime::block_on(commands::get_trash_info(app.state::<Vfs>(), None)).unwrap();
    assert_eq!((info.count, info.total_bytes), (2, None));
    let info =
        tauri::async_runtime::block_on(commands::get_trash_info(app.state::<Vfs>(), Some(true)))
            .unwrap();
    assert_eq!(info.count, 2);
    assert_eq!(info.total_bytes, Some(10));

    // An entry resolves to its `trash:` location, losslessly.
    let at = tauri::async_runtime::block_on(commands::entry_location(
        window(&app, "main"),
        app.state::<Vfs>(),
        snapshot.handle,
        entries[0].id,
    ))
    .unwrap();
    assert_eq!(at.uri, "trash:/t%7C2");

    // Items are not opened in an application.
    let refused = tauri::async_runtime::block_on(commands::open_entry(
        window(&app, "main"),
        app.state::<Vfs>(),
        snapshot.handle,
        entries[0].id,
    ))
    .unwrap_err();
    assert!(matches!(refused, Error::Vfs(VfsError::Unsupported { .. })));

    // What the sidebar reads.
    let info =
        tauri::async_runtime::block_on(commands::get_trash_info(app.state::<Vfs>(), None)).unwrap();
    assert_eq!((info.available, info.count), (true, 2));
    let status = tauri::async_runtime::block_on(commands::get_status(app.state::<Vfs>())).unwrap();
    assert!(status.features.contains(&"trash-view".to_owned()));

    // An unavailable Trash says why where the list would be.
    source.set_unavailable(Some("the Trash portal can only move files to the trash"));
    let again = open(&app, "main", trash_location()).unwrap_err();
    assert!(matches!(
        again,
        Error::Vfs(VfsError::Unsupported { what }) if what.contains("portal")
    ));
    let info =
        tauri::async_runtime::block_on(commands::get_trash_info(app.state::<Vfs>(), None)).unwrap();
    assert!(!info.available);
    let status = tauri::async_runtime::block_on(commands::get_status(app.state::<Vfs>())).unwrap();
    assert!(!status.features.contains(&"trash-view".to_owned()));
}

#[test]
fn the_trash_is_described_and_parsed_like_any_location() {
    let app = app();
    let info = tauri::async_runtime::block_on(commands::describe_location(
        app.state::<Vfs>(),
        trash_location(),
    ))
    .unwrap();
    assert_eq!(info.parent, None);
    assert_eq!(info.segments[0].label, "Trash");
    let parsed = tauri::async_runtime::block_on(commands::parse_location(
        app.state::<Vfs>(),
        "trash:".to_owned(),
        trash_location(),
    ))
    .unwrap();
    assert_eq!(parsed.uri, "trash:/");
}

#[test]
fn status_reports_the_details_capabilities() {
    let app = app();
    let status = tauri::async_runtime::block_on(commands::get_status(app.state::<Vfs>())).unwrap();
    for feature in [
        "entry-details",
        "folder-size",
        "text-head",
        "preview-protocol",
    ] {
        assert!(status.features.iter().any(|f| f == feature), "{feature}");
    }
}

#[test]
fn details_and_text_heads_are_scoped_to_the_owning_window() {
    let dir = folder_with(&["a.txt"]);
    fs::write(dir.path().join("blob.bin"), b"a\0b").unwrap();
    let app = app();
    let (snapshot, entries) = ready_listing(&app, &dir);
    let text = entries.iter().find(|e| e.name == "a.txt").unwrap().id;
    let blob = entries.iter().find(|e| e.name == "blob.bin").unwrap().id;
    let details = |label: &str, id| {
        tauri::async_runtime::block_on(commands::entry_details(
            window(&app, label),
            app.state::<Vfs>(),
            snapshot.handle,
            id,
        ))
    };
    let head = |label: &str, id, max| {
        tauri::async_runtime::block_on(commands::read_text_head(
            window(&app, label),
            app.state::<Vfs>(),
            snapshot.handle,
            id,
            max,
        ))
    };
    assert_eq!(details("main", text).unwrap().size, Some(1));
    assert!(matches!(
        details("other", text),
        Err(Error::Vfs(VfsError::StaleHandle))
    ));
    assert!(matches!(
        details("main", waypoint_protocol::EntryId(999)),
        Err(Error::Vfs(VfsError::NotFound { .. }))
    ));
    assert_eq!(head("main", text, None).unwrap().text, "x");
    assert!(matches!(
        head("other", text, None),
        Err(Error::Vfs(VfsError::StaleHandle))
    ));
    assert!(matches!(
        head("main", blob, None),
        Err(Error::Vfs(VfsError::NotText { .. }))
    ));
    // A limit the caller gives is honoured.
    fs::write(dir.path().join("a.txt"), "abcdef").unwrap();
    assert_eq!(head("main", text, Some(3)).unwrap().text, "abc");
}

mod folder_size {
    use std::sync::mpsc::{channel, Receiver};

    use waypoint_vfs::FolderSizeEvent;

    use super::*;

    fn start(
        app: &tauri::App<MockRuntime>,
        label: &str,
        handle: ListingHandle,
        id: waypoint_protocol::EntryId,
    ) -> (Result<u64, VfsError>, Receiver<FolderSizeEvent>) {
        let (sender, receiver) = channel();
        let started = app
            .state::<Vfs>()
            .start_folder_size(label, handle, id, move |event| sender.send(event).is_ok());
        (started, receiver)
    }

    fn last(receiver: &Receiver<FolderSizeEvent>) -> FolderSizeEvent {
        loop {
            let event = receiver
                .recv_timeout(Duration::from_secs(10))
                .expect("the run ended");
            if !matches!(event, FolderSizeEvent::Progress { .. }) {
                return event;
            }
        }
    }

    fn tree() -> tempfile::TempDir {
        let dir = folder_with(&[]);
        fs::create_dir_all(dir.path().join("sub/inner")).unwrap();
        fs::write(dir.path().join("sub/a"), vec![0u8; 100]).unwrap();
        fs::write(dir.path().join("sub/inner/b"), vec![0u8; 23]).unwrap();
        fs::write(dir.path().join("file"), b"x").unwrap();
        dir
    }

    #[test]
    fn a_folder_of_a_listing_is_totalled_and_the_run_ends_with_done() {
        let dir = tree();
        let app = app();
        let (snapshot, entries) = ready_listing(&app, &dir);
        let sub = entries.iter().find(|e| e.name == "sub").unwrap().id;
        let (job, events) = start(&app, "main", snapshot.handle, sub);
        assert_eq!(job.unwrap(), 1);
        let FolderSizeEvent::Done { totals } = last(&events) else {
            panic!("expected done");
        };
        assert_eq!((totals.bytes, totals.files, totals.folders), (123, 2, 1));
        // The run is forgotten once it ends.
        for _ in 0..100 {
            if app.state::<Vfs>().size_jobs.len() == 0 {
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        panic!("the finished run was not forgotten");
    }

    #[test]
    fn a_file_and_another_windows_handle_start_nothing() {
        let dir = tree();
        let app = app();
        let (snapshot, entries) = ready_listing(&app, &dir);
        let file = entries.iter().find(|e| e.name == "file").unwrap().id;
        let (started, _) = start(&app, "main", snapshot.handle, file);
        assert!(matches!(started, Err(VfsError::NotADirectory { .. })));
        let sub = entries.iter().find(|e| e.name == "sub").unwrap().id;
        let (started, _) = start(&app, "other", snapshot.handle, sub);
        assert_eq!(started, Err(VfsError::StaleHandle));
        assert_eq!(app.state::<Vfs>().size_jobs.len(), 0);
    }

    #[test]
    fn a_page_that_goes_away_still_ends_the_run_with_one_terminal_event() {
        let dir = folder_with(&[]);
        for d in 0..20 {
            let sub = dir.path().join(format!("d{d}"));
            fs::create_dir(&sub).unwrap();
            for f in 0..200 {
                fs::write(sub.join(format!("f{f}")), b"x").unwrap();
            }
        }
        let app = app();
        let (snapshot, entries) = ready_listing(&app, &dir);
        let first = entries[0].id;
        // A sink that refuses the first event stands for a page that went away: the run cancels.
        let (sender, receiver) = channel();
        let calls = std::sync::atomic::AtomicUsize::new(0);
        app.state::<Vfs>()
            .start_folder_size("main", snapshot.handle, first, move |event| {
                let keep = calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst) > 0;
                let _ = sender.send(event);
                keep
            })
            .unwrap();
        // Either the walk was too quick to report (done) or it was cancelled by the refusal; both
        // end the stream with exactly one terminal event.
        assert!(matches!(
            last(&receiver),
            FolderSizeEvent::Done { .. } | FolderSizeEvent::Cancelled { .. }
        ));
    }

    #[test]
    fn a_window_cancels_only_its_own_run_and_closing_the_window_cancels_all() {
        let app = app();
        let vfs = app.state::<Vfs>();
        let (id, token) = vfs.size_jobs.start("main");
        assert!(!vfs.size_jobs.cancel("other", id));
        assert!(!token.is_cancelled());
        tauri::async_runtime::block_on(commands::cancel_folder_size(
            window(&app, "main"),
            app.state::<Vfs>(),
            id,
        ))
        .unwrap();
        assert!(token.is_cancelled());
        let (_, second) = vfs.size_jobs.start("other");
        on_window_event(app.handle(), "other", &WindowEvent::Destroyed);
        assert!(second.is_cancelled());
    }
}

mod dir_scan {
    use std::sync::mpsc::{channel, Receiver};

    use waypoint_vfs::{DirScanCache, DirScanEvent, DirScanOptions};

    use super::*;

    fn start(
        app: &tauri::App<MockRuntime>,
        label: &str,
        root: &Path,
        cache: Option<DirScanCache>,
    ) -> (Result<u64, VfsError>, Receiver<DirScanEvent>) {
        let (sender, receiver) = channel();
        let started = app.state::<Vfs>().start_dir_scan(
            label,
            location(root),
            DirScanOptions::default(),
            cache,
            move |event| sender.send(event).is_ok(),
        );
        (started, receiver)
    }

    fn terminal(receiver: &Receiver<DirScanEvent>) -> DirScanEvent {
        loop {
            let event = receiver
                .recv_timeout(Duration::from_secs(10))
                .expect("the scan ended");
            if !matches!(
                event,
                DirScanEvent::Progress { .. } | DirScanEvent::Partial { .. }
            ) {
                return event;
            }
        }
    }

    fn tree() -> tempfile::TempDir {
        let dir = folder_with(&["loose"]);
        fs::create_dir_all(dir.path().join("A/inner")).unwrap();
        fs::write(dir.path().join("A/inner/x"), vec![0u8; 40]).unwrap();
        dir
    }

    #[test]
    fn the_status_reports_the_scan() {
        let app = app();
        let status =
            tauri::async_runtime::block_on(commands::get_status(app.state::<Vfs>())).unwrap();
        assert!(status.features.iter().any(|f| f == "dir-size-scan"));
    }

    #[test]
    fn a_scan_ends_with_done_is_cached_and_is_forgotten() {
        let dir = tree();
        let data = tempfile::tempdir().unwrap();
        let app = app();
        let cache = DirScanCache::in_dir(data.path());
        let (job, events) = start(&app, "main", dir.path(), Some(cache.clone()));
        assert_eq!(job.unwrap(), 1);
        let DirScanEvent::Done { result } = terminal(&events) else {
            panic!("expected done");
        };
        assert_eq!(result.rows[0].name, "A");
        assert_eq!(result.rows[0].bytes, 40);
        assert_eq!(cache.load(&location(dir.path())), Some(result));
        for _ in 0..100 {
            if app.state::<Vfs>().size_jobs.len() == 0 {
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        panic!("the finished scan was not forgotten");
    }

    #[test]
    fn a_missing_root_fails_and_a_cancelled_scan_is_not_cached() {
        let data = tempfile::tempdir().unwrap();
        let cache = DirScanCache::in_dir(data.path());
        let app = app();
        let (_, events) = start(
            &app,
            "main",
            Path::new("/definitely/not/here"),
            Some(cache.clone()),
        );
        assert!(matches!(terminal(&events), DirScanEvent::Failed { .. }));

        let dir = tree();
        let (job, events) = {
            let (sender, receiver) = channel();
            let calls = std::sync::atomic::AtomicUsize::new(0);
            let job = app
                .state::<Vfs>()
                .start_dir_scan(
                    "main",
                    location(dir.path()),
                    DirScanOptions::default(),
                    Some(cache.clone()),
                    move |event| {
                        // Refuse the first event: a page that went away cancels the scan.
                        let keep = calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst) > 0;
                        let _ = sender.send(event);
                        keep
                    },
                )
                .unwrap();
            (job, receiver)
        };
        assert!(job > 0);
        // A scan this small may finish before the refusal lands; either way one terminal event.
        let end = terminal(&events);
        assert!(matches!(
            end,
            DirScanEvent::Done { .. } | DirScanEvent::Cancelled { .. }
        ));
        if matches!(end, DirScanEvent::Cancelled { .. }) {
            assert_eq!(cache.load(&location(dir.path())), None);
        }
    }

    #[test]
    fn a_window_cancels_only_its_own_scan() {
        let app = app();
        let vfs = app.state::<Vfs>();
        let (id, token) = vfs.size_jobs.start("main");
        tauri::async_runtime::block_on(commands::cancel_dir_scan(
            window(&app, "other"),
            app.state::<Vfs>(),
            id,
        ))
        .unwrap();
        assert!(!token.is_cancelled());
        tauri::async_runtime::block_on(commands::cancel_dir_scan(
            window(&app, "main"),
            app.state::<Vfs>(),
            id,
        ))
        .unwrap();
        assert!(token.is_cancelled());
    }
}

mod connections {
    //! The connection commands over a fake server: lazy connecting through a listing, the person's
    //! answers, the saved connections and the events every window hears.

    use std::sync::Arc;

    use waypoint_connections::{
        AnswerInput, ConnectionDraft, ConnectionStatus, ConnectionsChanged, Credentials,
        MemorySecrets, Remembered,
    };
    use waypoint_path::{CaseRule, RemoteScheme};
    use waypoint_protocol::ConnectionState;
    use waypoint_vfs::FakeRemoteProvider;

    use super::*;
    use crate::connections::{self as cmd, CONNECTIONS_EVENT, CONNECTION_STATE_EVENT};
    use crate::{init_with, Options};

    fn app_with(server: &FakeRemoteProvider) -> tauri::App<MockRuntime> {
        let credentials = Arc::new(Credentials::new(Arc::new(MemorySecrets::new())));
        let server = server.clone().with_credentials(credentials.clone());
        let app = mock_builder()
            .plugin(init_with(Options {
                providers: vec![Arc::new(server)],
                credentials: Some(credentials),
                storage: None,
                suggestions: Some(Arc::new(|| vec!["nas".to_owned()])),
            }))
            .build(mock_context(noop_assets()))
            .expect("the mock app builds");
        WebviewWindowBuilder::new(&app, "main", WebviewUrl::App("index.html".into()))
            .build()
            .expect("the mock window opens");
        app
    }

    fn states(app: &tauri::App<MockRuntime>) -> Receiver<ConnectionStatus> {
        let (sender, receiver) = channel();
        app.listen_any(CONNECTION_STATE_EVENT, move |event| {
            let _ = sender.send(serde_json::from_str(event.payload()).expect("a state"));
        });
        receiver
    }

    #[test]
    fn a_listing_connects_lazily_and_holds_the_login_until_it_closes() {
        let server = FakeRemoteProvider::new(RemoteScheme::Sftp, CaseRule::Sensitive);
        let root = server.root("me@nas.lan");
        server.put_file(&root.join("a.txt").unwrap(), b"x");
        let app = app_with(&server);
        let heard = states(&app);
        let snapshot = open(&app, "main", root.to_location()).unwrap();
        let first = heard
            .recv_timeout(Duration::from_secs(10))
            .expect("a state event");
        assert_eq!(first.key, "sftp://me@nas.lan");
        assert_eq!(first.state, ConnectionState::Connected);
        tauri::async_runtime::block_on(commands::close_listing(
            window(&app, "main"),
            app.state::<Vfs>(),
            snapshot.handle,
        ))
        .unwrap();
        let state = tauri::async_runtime::block_on(cmd::connection_state(
            app.state::<Vfs>(),
            root.to_location(),
        ))
        .unwrap()
        .unwrap();
        assert_eq!(state.state, ConnectionState::Connected);
        // A local folder has no login.
        let home = location(Path::new("/"));
        assert!(
            tauri::async_runtime::block_on(cmd::connection_state(app.state::<Vfs>(), home))
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn a_login_is_answered_through_connect_and_remembered() {
        let server = FakeRemoteProvider::new(RemoteScheme::Sftp, CaseRule::Sensitive);
        server.require_password(Some("me"), "hunter2");
        let app = app_with(&server);
        let at = server.root("me@nas.lan").to_location();
        let refused = tauri::async_runtime::block_on(cmd::connect(
            app.state::<Vfs>(),
            at.clone(),
            None,
            None,
        ))
        .unwrap_err();
        let json = serde_json::to_value(&refused).unwrap();
        assert_eq!(json["kind"], "authRequired");
        let answer: AnswerInput =
            serde_json::from_str(r#"{"kind":"password","user":"me","password":"hunter2"}"#)
                .unwrap();
        let remembered = tauri::async_runtime::block_on(cmd::connect(
            app.state::<Vfs>(),
            at,
            Some(answer),
            Some(true),
        ))
        .unwrap();
        assert_eq!(remembered, Remembered::Kept);
    }

    #[test]
    fn server_text_parses_once_a_provider_serves_it_and_a_saved_name_labels_the_root() {
        let server = FakeRemoteProvider::new(RemoteScheme::Sftp, CaseRule::Sensitive);
        let app = app_with(&server);
        let base = location(Path::new("/srv"));
        let typed = tauri::async_runtime::block_on(commands::parse_location_text(
            app.state::<Vfs>(),
            "sftp://me:pw@NAS.lan/srv".into(),
            base.clone(),
        ))
        .unwrap();
        assert!(typed.password_dropped);
        assert_eq!(typed.location.uri, "sftp://me@nas.lan/srv");
        let described = |at: &waypoint_protocol::Location| {
            tauri::async_runtime::block_on(commands::describe_location(
                app.state::<Vfs>(),
                at.clone(),
            ))
            .unwrap()
        };
        let before = described(&typed.location);
        assert_eq!(before.segments[0].label, "me@nas.lan");
        assert_eq!(before.connection.as_deref(), Some("sftp://me@nas.lan"));
        tauri::async_runtime::block_on(cmd::add_connection(
            app.state::<Vfs>(),
            ConnectionDraft {
                name: "NAS".into(),
                scheme: "sftp".into(),
                host: "nas.lan".into(),
                user: Some("me".into()),
                ..ConnectionDraft::default()
            },
        ))
        .unwrap();
        assert_eq!(described(&typed.location).segments[0].label, "NAS");
    }

    #[test]
    fn saved_connections_are_edited_and_every_window_hears_it() {
        let server = FakeRemoteProvider::new(RemoteScheme::Sftp, CaseRule::Sensitive);
        let app = app_with(&server);
        let (sender, heard) = channel();
        app.listen_any(CONNECTIONS_EVENT, move |event| {
            let change: ConnectionsChanged =
                serde_json::from_str(event.payload()).expect("a change");
            let _ = sender.send(change);
        });
        let entry = tauri::async_runtime::block_on(cmd::add_connection(
            app.state::<Vfs>(),
            ConnectionDraft {
                name: "NAS".into(),
                scheme: "sftp".into(),
                host: "nas.lan".into(),
                user: Some("me".into()),
                ..ConnectionDraft::default()
            },
        ))
        .unwrap();
        assert_eq!(entry.location.uri, "sftp://me@nas.lan/");
        let change = heard.recv_timeout(Duration::from_secs(10)).unwrap();
        assert_eq!(change.revision, 1);
        let refused = tauri::async_runtime::block_on(cmd::add_connection(
            app.state::<Vfs>(),
            ConnectionDraft {
                scheme: "sftp".into(),
                ..ConnectionDraft::default()
            },
        ))
        .unwrap_err();
        assert_eq!(
            serde_json::to_value(&refused).unwrap()["kind"],
            "connections"
        );
        let overview =
            tauri::async_runtime::block_on(cmd::list_connections(app.state::<Vfs>())).unwrap();
        assert_eq!(overview.connections.connections.len(), 1);
        let support =
            tauri::async_runtime::block_on(cmd::connection_support(app.state::<Vfs>())).unwrap();
        assert_eq!(support.schemes, ["sftp"]);
        assert_eq!(support.keyring, None);
        let suggested =
            tauri::async_runtime::block_on(cmd::suggested_servers(app.state::<Vfs>())).unwrap();
        assert_eq!(suggested[0].location.uri, "sftp://nas/");
        let parsed = tauri::async_runtime::block_on(cmd::parse_address_text(
            app.state::<Vfs>(),
            "smb://files/share".into(),
        ));
        assert!(parsed.is_err(), "no provider serves smb here");
    }

    #[test]
    fn only_server_protocols_are_connectable() {
        let all = vec!["archive", "dav", "davs", "git+file", "s3", "sftp", "smb"];
        assert_eq!(
            crate::connections::connectable(all),
            ["dav", "davs", "s3", "sftp", "smb"]
        );
    }

    #[test]
    fn a_protocol_turned_off_while_the_app_runs_says_so_and_turning_it_on_again_needs_no_restart() {
        let server = FakeRemoteProvider::new(RemoteScheme::Sftp, CaseRule::Sensitive);
        let root = server.root("me@nas.lan");
        let app = app_with(&server);
        let (sender, heard) = channel();
        app.listen_any(crate::PROTOCOLS_EVENT, move |event| {
            let _ = sender.send(
                serde_json::from_str::<waypoint_connections::ProtocolsChanged>(event.payload())
                    .expect("the protocols"),
            );
        });
        let state = app.state::<Vfs>();
        let support =
            || tauri::async_runtime::block_on(cmd::connection_support(app.state::<Vfs>())).unwrap();
        assert_eq!(
            (support().schemes, support().off),
            (vec!["sftp".to_owned()], vec![])
        );

        // The app turns the provider off: nothing serves the scheme, and the reason is typed.
        let provider = state.remote().turn_off("sftp").expect("it was serving");
        crate::announce_protocols(app.handle());
        let change = heard
            .recv_timeout(Duration::from_secs(10))
            .expect("an event");
        assert_eq!(
            (change.schemes, change.off),
            (vec![], vec!["sftp".to_owned()])
        );
        assert_eq!(support().off, ["sftp"]);
        let refused = open(&app, "main", root.to_location()).unwrap_err();
        assert!(
            matches!(refused, Error::Vfs(VfsError::ProtocolOff { ref scheme }) if scheme == "sftp"),
            "{refused:?}"
        );
        let parsed = tauri::async_runtime::block_on(cmd::parse_address_text(
            app.state::<Vfs>(),
            "sftp://me@nas.lan/".into(),
        ))
        .unwrap_err();
        assert!(
            matches!(parsed, Error::Vfs(VfsError::ProtocolOff { .. })),
            "{parsed:?}"
        );
        let connect = tauri::async_runtime::block_on(cmd::connect(
            app.state::<Vfs>(),
            root.to_location(),
            None,
            Some(false),
        ))
        .unwrap_err();
        assert!(
            matches!(connect, Error::Vfs(VfsError::ProtocolOff { .. })),
            "{connect:?}"
        );

        // Turned on again, the same address opens.
        state.remote().register(provider);
        crate::announce_protocols(app.handle());
        let change = heard
            .recv_timeout(Duration::from_secs(10))
            .expect("an event");
        assert_eq!(
            (change.schemes, change.off),
            (vec!["sftp".to_owned()], vec![])
        );
        open(&app, "main", root.to_location()).expect("opens once it is on");
    }
}

/// An overlay that marks `b.txt` modified as soon as a listing is attached, and again later.
struct MarkB {
    sink: std::sync::Mutex<Option<waypoint_vfs::MarkSink>>,
}

struct NoGuard;

impl waypoint_vfs::OverlayGuard for NoGuard {}

impl waypoint_vfs::FolderOverlay for MarkB {
    fn attach(
        &self,
        _folder: &waypoint_path::VfsPath,
        sink: waypoint_vfs::MarkSink,
    ) -> Option<Box<dyn waypoint_vfs::OverlayGuard>> {
        let mark = waypoint_vfs::GitMark {
            unstaged: Some(waypoint_vfs::GitChange::Modified),
            ..Default::default()
        };
        sink(waypoint_vfs::FolderMarks {
            default: None,
            names: [(std::ffi::OsString::from("b.txt"), mark)].into(),
        });
        *self.sink.lock().unwrap() = Some(sink);
        Some(Box::new(NoGuard))
    }
}

#[test]
fn an_overlay_the_app_gave_decorates_the_listings_it_opens() {
    let dir = folder_with(&["a.txt", "b.txt"]);
    let app = app();
    let overlay = std::sync::Arc::new(MarkB {
        sink: Default::default(),
    });
    app.state::<Vfs>().set_overlay(overlay.clone());
    let received = events(&window(&app, "main"));
    let snapshot = open(&app, "main", location(dir.path())).unwrap();
    wait_for(&received, is_ready);
    let entries = range(&app, "main", snapshot.handle, 0, 10).unwrap();
    assert!(entries[0].git.is_none());
    assert_eq!(
        entries[1].git.unwrap().unstaged,
        Some(waypoint_vfs::GitChange::Modified)
    );
    // A later change of marks reaches the window as a patch.
    let send = overlay.sink.lock().unwrap().clone().unwrap();
    send(waypoint_vfs::FolderMarks::default());
    let changed = wait_for(&received, |event| {
        matches!(event, ListingEvent::Changed { .. })
    });
    assert!(matches!(changed, ListingEvent::Changed { .. }));
    assert!(range(&app, "main", snapshot.handle, 0, 10).unwrap()[1]
        .git
        .is_none());
}
