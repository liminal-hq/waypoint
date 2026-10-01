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
        tauri::async_runtime::block_on(commands::parse_location(input.to_owned(), base.clone()))
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
    let info = tauri::async_runtime::block_on(commands::describe_location(location(Path::new(
        "/home/a/Music",
    ))))
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
