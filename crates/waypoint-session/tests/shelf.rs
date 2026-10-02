// Headless tests for the Shelf: its commands, the events every window hears, persistence and
// randomised invariant checks.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use waypoint_protocol::Location;
use waypoint_session::{
    Command, Document, SessionError, SessionEvent, ShelfItemId, Store, DOCUMENT_VERSION,
    SHELF_LIMIT,
};

mod common;
use common::{assert_ok, loc, run, store_with, Lcg, Mirror};

const W: &str = "main-1";

fn add(store: &mut Store, window: &str, names: &[&str]) -> Result<usize, SessionError> {
    let outcome = store.dispatch(
        window,
        Command::AddToShelf {
            locations: names.iter().map(|n| loc(n)).collect(),
            added_ms: 1_000,
        },
    )?;
    Ok(outcome.events.len())
}

fn names(store: &Store) -> Vec<String> {
    store.shelf().iter().map(|i| i.name.clone()).collect()
}

fn ids(store: &Store) -> Vec<u64> {
    store.shelf().iter().map(|i| i.id.0).collect()
}

#[test]
fn adding_keeps_insertion_order_and_derives_name_and_origin() {
    let mut s = store_with(&["a"]);
    run(
        &mut s,
        W,
        Command::AddToShelf {
            locations: vec![
                Location::new("/home/a/b.txt", "file:///home/a/b.txt"),
                loc("z"),
            ],
            added_ms: 42,
        },
    );
    let items = s.shelf();
    assert_eq!(names(&s), vec!["b.txt", "z"]);
    assert_eq!(items[0].id, ShelfItemId(1));
    assert_eq!(items[0].added_ms, 42);
    assert_eq!(items[0].origin.display, "/home/a");
    assert_eq!(items[0].origin.uri, "file:///home/a");
    assert_eq!(items[1].origin.uri, "file:///");
    assert_ok(&s);
}

#[test]
fn a_location_already_on_the_shelf_is_not_added_again() {
    let mut s = store_with(&["a"]);
    assert_eq!(add(&mut s, W, &["x", "y", "x"]).unwrap(), 1);
    assert_eq!(names(&s), vec!["x", "y"]);
    let revision = s.revision();
    // Nothing new: no event and no revision.
    assert_eq!(add(&mut s, W, &["y", "x"]).unwrap(), 0);
    assert_eq!(s.revision(), revision);
    assert_eq!(ids(&s), vec![1, 2]);
}

#[test]
fn the_shelf_is_capped_and_a_refused_batch_adds_nothing() {
    let mut s = store_with(&["a"]);
    let many: Vec<String> = (0..SHELF_LIMIT).map(|i| format!("f{i}")).collect();
    let refs: Vec<&str> = many.iter().map(String::as_str).collect();
    add(&mut s, W, &refs).unwrap();
    assert_eq!(s.shelf().len(), SHELF_LIMIT);
    let before = s.clone();
    let err = add(&mut s, W, &["one-more", "f0"]).unwrap_err();
    assert_eq!(err, SessionError::ShelfFull(SHELF_LIMIT));
    assert_eq!(s, before, "an error changes nothing");
    // Only duplicates fit.
    assert_eq!(add(&mut s, W, &["f0"]).unwrap(), 0);
    run(
        &mut s,
        W,
        Command::RemoveFromShelf {
            ids: vec![ShelfItemId(1)],
        },
    );
    add(&mut s, W, &["one-more"]).unwrap();
    assert_eq!(s.shelf().len(), SHELF_LIMIT);
    assert_ok(&s);
}

#[test]
fn ids_are_never_reused() {
    let mut s = store_with(&["a"]);
    add(&mut s, W, &["x", "y"]).unwrap();
    run(
        &mut s,
        W,
        Command::RemoveFromShelf {
            ids: vec![ShelfItemId(2)],
        },
    );
    add(&mut s, W, &["y"]).unwrap();
    assert_eq!(ids(&s), vec![1, 3]);
    run(&mut s, W, Command::ClearShelf);
    add(&mut s, W, &["x"]).unwrap();
    assert_eq!(ids(&s), vec![4]);
}

#[test]
fn removing_ignores_ids_that_are_gone_and_clearing_an_empty_shelf_is_quiet() {
    let mut s = store_with(&["a"]);
    add(&mut s, W, &["x", "y", "z"]).unwrap();
    let out = run(
        &mut s,
        W,
        Command::RemoveFromShelf {
            ids: vec![ShelfItemId(2), ShelfItemId(99)],
        },
    );
    assert_eq!(out.events.len(), 1);
    assert_eq!(names(&s), vec!["x", "z"]);
    let out = run(
        &mut s,
        W,
        Command::RemoveFromShelf {
            ids: vec![ShelfItemId(2)],
        },
    );
    assert!(out.events.is_empty());
    run(&mut s, W, Command::ClearShelf);
    assert!(run(&mut s, W, Command::ClearShelf).events.is_empty());
}

#[test]
fn moving_an_item_reorders_and_clamps() {
    let mut s = store_with(&["a"]);
    add(&mut s, W, &["x", "y", "z"]).unwrap();
    let mv = |s: &mut Store, id, to_index| {
        run(
            s,
            W,
            Command::MoveShelfItem {
                id: ShelfItemId(id),
                to_index,
            },
        )
    };
    mv(&mut s, 3, 0);
    assert_eq!(names(&s), vec!["z", "x", "y"]);
    mv(&mut s, 3, 99);
    assert_eq!(names(&s), vec!["x", "y", "z"]);
    assert!(mv(&mut s, 2, 1).events.is_empty(), "already there");
    let err = s
        .dispatch(
            W,
            Command::MoveShelfItem {
                id: ShelfItemId(9),
                to_index: 0,
            },
        )
        .unwrap_err();
    assert_eq!(err, SessionError::UnknownShelfItem(9));
}

#[test]
fn every_window_hears_the_shelf_and_a_new_window_starts_with_it() {
    let mut s = store_with(&["a"]);
    run(
        &mut s,
        W,
        Command::OpenWindow {
            location: Some(loc("b")),
            geometry: None,
        },
    );
    let out = run(
        &mut s,
        W,
        Command::AddToShelf {
            locations: vec![loc("x")],
            added_ms: 1,
        },
    );
    for window in ["main-1", "main-2"] {
        let heard: Vec<_> = out
            .events_for(window)
            .filter(|e| matches!(e, SessionEvent::ShelfChanged { shelf, .. } if shelf.len() == 1))
            .collect();
        assert_eq!(heard.len(), 1, "{window} hears ShelfChanged once");
    }
    let opened = run(
        &mut s,
        W,
        Command::OpenWindow {
            location: Some(loc("c")),
            geometry: None,
        },
    );
    assert!(opened
        .events_for("main-3")
        .any(|e| matches!(e, SessionEvent::ShelfChanged { shelf, .. } if shelf.len() == 1)));
}

#[test]
fn a_command_needs_a_live_window() {
    let mut s = store_with(&["a"]);
    let err = add(&mut s, "main-9", &["x"]).unwrap_err();
    assert_eq!(err, SessionError::UnknownWindow("main-9".into()));
}

#[test]
fn the_shelf_survives_a_round_trip_and_ids_stay_unused() {
    let mut s = store_with(&["a"]);
    add(&mut s, W, &["x", "y", "z"]).unwrap();
    run(
        &mut s,
        W,
        Command::RemoveFromShelf {
            ids: vec![ShelfItemId(3)],
        },
    );
    let json = serde_json::to_string(&s.to_document()).unwrap();
    let doc: Document = serde_json::from_str(&json).unwrap();
    let (mut restored, notes) = Store::from_document(doc).unwrap();
    assert!(notes.is_empty(), "{notes:?}");
    assert_eq!(restored, s);
    add(&mut restored, W, &["w"]).unwrap();
    assert_eq!(ids(&restored), vec![1, 2, 4], "3 is never handed out again");
}

#[test]
fn a_version_one_document_without_a_shelf_still_loads() {
    let s = store_with(&["a", "b"]);
    let mut json: serde_json::Value = serde_json::to_value(s.to_document()).unwrap();
    let body = json["body"].as_object_mut().unwrap();
    body.remove("shelf");
    body.remove("nextShelf");
    assert_eq!(json["version"], DOCUMENT_VERSION);
    let doc: Document = serde_json::from_value(json).unwrap();
    let (mut restored, notes) = Store::from_document(doc).unwrap();
    assert!(notes.is_empty(), "{notes:?}");
    assert_eq!(restored, s);
    assert!(restored.shelf().is_empty());
    add(&mut restored, W, &["x"]).unwrap();
    assert_eq!(ids(&restored), vec![1]);
}

#[test]
fn a_damaged_shelf_is_repaired_on_load() {
    let mut s = store_with(&["a"]);
    add(&mut s, W, &["x", "y"]).unwrap();
    let mut doc = s.to_document();
    let first = doc.body.shelf[0].clone();
    doc.body.shelf.push(first); // a repeated id and location
    let mut other = doc.body.shelf[1].clone();
    other.id = ShelfItemId(50); // a repeated location under a fresh id
    doc.body.shelf.push(other);
    doc.body.next_shelf = 1; // a counter below the ids in use
    let (restored, notes) = Store::from_document(doc).unwrap();
    assert_eq!(names(&restored), vec!["x", "y"]);
    assert_eq!(notes.len(), 2, "{notes:?}");
    assert_ok(&restored);
    let mut restored = restored;
    add(&mut restored, W, &["n"]).unwrap();
    assert_eq!(*ids(&restored).last().unwrap(), 3);
}

#[test]
fn forgetting_the_shelf_is_silent_and_keeps_ids_unused() {
    let mut s = store_with(&["a"]);
    add(&mut s, W, &["x", "y"]).unwrap();
    let revision = s.revision();
    s.forget_shelf();
    assert!(s.shelf().is_empty());
    assert_eq!(s.revision(), revision);
    assert_ok(&s);
    add(&mut s, W, &["x"]).unwrap();
    assert_eq!(ids(&s), vec![3]);
}

#[test]
fn random_commands_keep_the_invariants_and_every_mirror_current() {
    let mut fresh = Store::new();
    let mut mirror = Mirror::default();
    for name in ["a", "b"] {
        let o = run(
            &mut fresh,
            W,
            Command::OpenWindow {
                location: Some(loc(name)),
                geometry: None,
            },
        );
        mirror.apply(&o.events);
    }
    let mut seen: std::collections::HashSet<u64> = std::collections::HashSet::new();
    let mut rng = Lcg(7);
    for _ in 0..400 {
        let window = if rng.next(2) == 0 { "main-1" } else { "main-2" };
        let command = match rng.next(5) {
            0 | 1 => Command::AddToShelf {
                locations: (0..=rng.next(3))
                    .map(|_| loc(&format!("d/f{}", rng.next(40))))
                    .collect(),
                added_ms: 5,
            },
            2 => Command::RemoveFromShelf {
                ids: (0..=rng.next(3))
                    .map(|_| ShelfItemId(1 + rng.next(60) as u64))
                    .collect(),
            },
            3 => Command::MoveShelfItem {
                id: ShelfItemId(1 + rng.next(60) as u64),
                to_index: rng.next(50),
            },
            _ => {
                if rng.next(8) == 0 {
                    Command::ClearShelf
                } else {
                    continue;
                }
            }
        };
        let before = fresh.revision();
        if let Ok(o) = fresh.dispatch(window, command) {
            mirror.apply(&o.events);
            assert_eq!(fresh.revision(), before + o.events.len() as u64);
        }
        assert_ok(&fresh);
        for item in fresh.shelf() {
            // Each id names one location for the life of the store.
            seen.insert(item.id.0);
        }
        let mut uris: Vec<_> = fresh
            .shelf()
            .iter()
            .map(|i| i.location.uri.clone())
            .collect();
        uris.sort();
        uris.dedup();
        assert_eq!(uris.len(), fresh.shelf().len());
        assert!(fresh.shelf().len() <= SHELF_LIMIT);
        mirror.matches(&fresh).unwrap();
    }
    assert!(!seen.is_empty());
}
