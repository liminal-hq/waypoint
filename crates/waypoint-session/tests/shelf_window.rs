// Headless tests for the Shelf window: undocking and docking, who hears it, the Shelf window as a
// caller and a recipient, and what is remembered and saved.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use waypoint_protocol::SHELF_LABEL;
use waypoint_session::{
    Command, Document, Geometry, SessionError, SessionEvent, ShelfWindow, Store,
};

mod common;
use common::{assert_ok, loc, run, store_with, Mirror};

const W: &str = "main-1";

fn undock(store: &mut Store, window: &str) {
    run(store, window, Command::SetShelfUndocked { undocked: true });
}

fn geometry(x: i32) -> Geometry {
    Geometry {
        x: Some(x),
        y: Some(40),
        width: 640,
        height: 220,
        maximised: false,
    }
}

fn changes(outcome: &waypoint_session::Outcome, window: &str) -> Vec<ShelfWindow> {
    outcome
        .events_for(window)
        .filter_map(|e| match e {
            SessionEvent::ShelfWindowChanged { shelf_window, .. } => Some(*shelf_window),
            _ => None,
        })
        .collect()
}

#[test]
fn the_shelf_starts_docked() {
    let s = store_with(&["a"]);
    assert_eq!(*s.shelf_window(), ShelfWindow::default());
    assert!(
        s.snapshot(SHELF_LABEL).is_none(),
        "no Shelf window while docked"
    );
}

#[test]
fn undocking_tells_every_window_and_the_shelf_window_reads_the_shared_shelf() {
    let mut s = store_with(&["a"]);
    run(
        &mut s,
        W,
        Command::OpenWindow {
            location: Some(loc("b")),
            geometry: None,
        },
    );
    run(
        &mut s,
        W,
        Command::AddToShelf {
            locations: vec![loc("x")],
            added_ms: 1,
        },
    );
    let outcome = s
        .dispatch(W, Command::SetShelfUndocked { undocked: true })
        .unwrap();
    for window in [W, "main-2", SHELF_LABEL] {
        let told = changes(&outcome, window);
        assert_eq!(told.len(), 1, "{window} hears it once");
        assert!(told[0].undocked);
    }
    let snapshot = s
        .snapshot(SHELF_LABEL)
        .expect("a snapshot for the Shelf window");
    assert!(snapshot.tabs.is_empty());
    assert_eq!(snapshot.shelf.len(), 1);
    assert!(snapshot.shelf_window.undocked);
    assert_ok(&s);
}

#[test]
fn docking_again_tells_the_windows_and_ends_the_shelf_window_snapshot() {
    let mut s = store_with(&["a"]);
    undock(&mut s, W);
    let outcome = s
        .dispatch(SHELF_LABEL, Command::SetShelfUndocked { undocked: false })
        .unwrap();
    assert_eq!(changes(&outcome, W).len(), 1);
    assert!(!changes(&outcome, W)[0].undocked);
    assert!(s.snapshot(SHELF_LABEL).is_none());
    // Docking when already docked changes nothing.
    let quiet = s
        .dispatch(W, Command::SetShelfUndocked { undocked: false })
        .unwrap();
    assert!(quiet.events.is_empty());
}

#[test]
fn the_shelf_window_may_change_the_shelf_only_while_it_exists() {
    let mut s = store_with(&["a"]);
    let err = s
        .dispatch(
            SHELF_LABEL,
            Command::AddToShelf {
                locations: vec![loc("x")],
                added_ms: 1,
            },
        )
        .unwrap_err();
    assert_eq!(err, SessionError::UnknownWindow(SHELF_LABEL.into()));
    undock(&mut s, W);
    let outcome = s
        .dispatch(
            SHELF_LABEL,
            Command::AddToShelf {
                locations: vec![loc("x")],
                added_ms: 1,
            },
        )
        .unwrap();
    // Both the main window and the Shelf window hear the change.
    for window in [W, SHELF_LABEL] {
        assert!(outcome
            .events_for(window)
            .any(|e| matches!(e, SessionEvent::ShelfChanged { shelf, .. } if shelf.len() == 1)));
    }
    // A change from a main window reaches the Shelf window too.
    let outcome = s.dispatch(W, Command::ClearShelf).unwrap();
    assert!(outcome
        .events_for(SHELF_LABEL)
        .any(|e| matches!(e, SessionEvent::ShelfChanged { shelf, .. } if shelf.is_empty())));
}

#[test]
fn on_top_is_told_and_geometry_is_saved_quietly() {
    let mut s = store_with(&["a"]);
    undock(&mut s, W);
    let outcome = s
        .dispatch(SHELF_LABEL, Command::SetShelfOnTop { on_top: true })
        .unwrap();
    assert!(changes(&outcome, W)[0].on_top);
    let before = s.revision();
    let outcome = s
        .dispatch(
            SHELF_LABEL,
            Command::SetShelfGeometry {
                geometry: geometry(10),
            },
        )
        .unwrap();
    assert!(outcome.events.is_empty(), "a move or resize makes no event");
    assert_eq!(s.revision(), before);
    assert_eq!(s.shelf_window().geometry, Some(geometry(10)));
}

#[test]
fn the_window_remembers_where_it_was_across_docking() {
    let mut s = store_with(&["a"]);
    undock(&mut s, W);
    run(&mut s, SHELF_LABEL, Command::SetShelfOnTop { on_top: true });
    run(
        &mut s,
        SHELF_LABEL,
        Command::SetShelfGeometry {
            geometry: geometry(10),
        },
    );
    run(
        &mut s,
        SHELF_LABEL,
        Command::SetShelfUndocked { undocked: false },
    );
    undock(&mut s, W);
    let state = *s.shelf_window();
    assert!(state.undocked && state.on_top);
    assert_eq!(state.geometry, Some(geometry(10)));
}

#[test]
fn a_window_opened_while_undocked_starts_knowing_it() {
    let mut s = store_with(&["a"]);
    undock(&mut s, W);
    let outcome = s
        .dispatch(
            W,
            Command::OpenWindow {
                location: None,
                geometry: None,
            },
        )
        .unwrap();
    assert!(changes(&outcome, "main-2")[0].undocked);
    assert!(s.snapshot("main-2").unwrap().shelf_window.undocked);
}

#[test]
fn mirrors_follow_the_shelf_window_by_events_alone() {
    let mut s = store_with(&["a"]);
    let mut mirror = Mirror::default();
    mirror.windows.insert(W.to_string(), s.snapshot(W).unwrap());
    for command in [
        Command::SetShelfUndocked { undocked: true },
        Command::AddToShelf {
            locations: vec![loc("x"), loc("y")],
            added_ms: 2,
        },
        Command::SetShelfOnTop { on_top: true },
        Command::SetShelfUndocked { undocked: false },
    ] {
        let outcome = s.dispatch(W, command).unwrap();
        // The Shelf window is not a window of the store; its events are checked on their own above.
        let main_only: Vec<_> = outcome
            .events
            .iter()
            .filter(|e| e.window != SHELF_LABEL)
            .cloned()
            .collect();
        mirror.apply(&main_only);
        mirror.matches(&s).unwrap();
    }
}

#[test]
fn the_state_survives_a_round_trip_and_an_older_document_loads_docked() {
    let mut s = store_with(&["a"]);
    undock(&mut s, W);
    run(&mut s, SHELF_LABEL, Command::SetShelfOnTop { on_top: true });
    run(
        &mut s,
        SHELF_LABEL,
        Command::SetShelfGeometry {
            geometry: geometry(10),
        },
    );
    let json = serde_json::to_string(&s.to_document()).unwrap();
    assert!(json.contains("\"shelfWindow\""));
    let doc: Document = serde_json::from_str(&json).unwrap();
    let (restored, notes) = Store::from_document(doc).unwrap();
    assert!(notes.is_empty(), "{notes:?}");
    assert_eq!(restored, s);

    let mut json: serde_json::Value = serde_json::to_value(s.to_document()).unwrap();
    json["body"].as_object_mut().unwrap().remove("shelfWindow");
    let doc: Document = serde_json::from_value(json).unwrap();
    let (older, _) = Store::from_document(doc).unwrap();
    assert_eq!(*older.shelf_window(), ShelfWindow::default());
}
