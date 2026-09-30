// Headless tests for the session reducer: scripted behaviour and randomised invariant checks.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::sync::{Arc, Mutex};

use waypoint_protocol::Location;
use waypoint_session::{apply, Command, Session, SessionEvent, SessionSnapshot, TabId};

fn loc(name: &str) -> Location {
    Location::new(format!("/{name}"), format!("file:///{name}"))
}

fn open(session: &mut Session, name: &str, activate: bool) -> TabId {
    let events = session
        .dispatch(Command::Open {
            location: loc(name),
            after: None,
            activate,
        })
        .unwrap();
    match &events[0] {
        SessionEvent::TabOpened { tab, .. } => tab.id,
        other => panic!("expected TabOpened, got {other:?}"),
    }
}

fn order(snapshot: &SessionSnapshot) -> Vec<u32> {
    snapshot.tabs.iter().map(|t| t.id.0).collect()
}

fn assert_invariants(snapshot: &SessionSnapshot) {
    match snapshot.active {
        None => assert!(snapshot.tabs.is_empty(), "tabs exist but none is active"),
        Some(active) => {
            assert_eq!(
                snapshot.tabs.iter().filter(|t| t.id == active).count(),
                1,
                "the active tab must be exactly one of the tabs"
            );
        }
    }
    let mut ids = order(snapshot);
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), snapshot.tabs.len(), "tab ids are unique");
}

#[test]
fn the_first_tab_is_active_even_when_not_asked() {
    let mut session = Session::new();
    let a = open(&mut session, "a", false);
    assert_eq!(session.snapshot().active, Some(a));
}

#[test]
fn open_after_inserts_next_to_a_tab_and_activate_switches() {
    let mut session = Session::new();
    let a = open(&mut session, "a", true);
    let b = open(&mut session, "b", false);
    let c = session
        .dispatch(Command::Open {
            location: loc("c"),
            after: Some(a),
            activate: true,
        })
        .unwrap();
    assert_eq!(c.len(), 2);
    let snapshot = session.snapshot();
    assert_eq!(order(&snapshot), vec![a.0, 3, b.0]);
    assert_eq!(snapshot.active, Some(TabId(3)));
}

#[test]
fn closing_the_active_tab_activates_the_one_that_takes_its_place() {
    let mut session = Session::new();
    let a = open(&mut session, "a", true);
    let b = open(&mut session, "b", false);
    let c = open(&mut session, "c", false);
    session.dispatch(Command::Activate { tab: b }).unwrap();
    session.dispatch(Command::Close { tab: b }).unwrap();
    assert_eq!(session.snapshot().active, Some(c));
    session.dispatch(Command::Close { tab: c }).unwrap();
    assert_eq!(session.snapshot().active, Some(a));
}

#[test]
fn closing_an_inactive_tab_keeps_the_active_one() {
    let mut session = Session::new();
    let a = open(&mut session, "a", true);
    let b = open(&mut session, "b", false);
    let events = session.dispatch(Command::Close { tab: b }).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(session.snapshot().active, Some(a));
}

#[test]
fn closing_the_last_tab_leaves_a_valid_empty_session() {
    let mut session = Session::new();
    let a = open(&mut session, "a", true);
    session.dispatch(Command::Close { tab: a }).unwrap();
    let snapshot = session.snapshot();
    assert!(snapshot.tabs.is_empty());
    assert_eq!(snapshot.active, None);
    // A tab opened afterwards is active again, with a fresh id.
    assert_eq!(open(&mut session, "b", false), TabId(2));
    assert_eq!(session.snapshot().active, Some(TabId(2)));
}

#[test]
fn move_reorders_and_clamps() {
    let mut session = Session::new();
    let a = open(&mut session, "a", true);
    let b = open(&mut session, "b", false);
    let c = open(&mut session, "c", false);
    let events = session
        .dispatch(Command::Move { tab: a, index: 99 })
        .unwrap();
    assert!(matches!(events[0], SessionEvent::TabMoved { index: 2, .. }));
    assert_eq!(order(&session.snapshot()), vec![b.0, c.0, a.0]);
    assert!(session
        .dispatch(Command::Move { tab: a, index: 2 })
        .unwrap()
        .is_empty());
}

#[test]
fn navigate_back_and_forward_keep_the_stacks_in_step() {
    let mut session = Session::new();
    let a = open(&mut session, "home", true);
    for name in ["one", "two", "three"] {
        session
            .dispatch(Command::Navigate {
                tab: a,
                location: loc(name),
            })
            .unwrap();
    }
    session.dispatch(Command::Back { tab: a }).unwrap();
    session.dispatch(Command::Back { tab: a }).unwrap();
    let tab = session.snapshot().tabs[0].clone();
    assert_eq!(tab.location, loc("one"));
    assert_eq!(tab.back, vec![loc("home")]);
    assert_eq!(tab.forward, vec![loc("three"), loc("two")]);

    session.dispatch(Command::Forward { tab: a }).unwrap();
    assert_eq!(session.snapshot().tabs[0].location, loc("two"));

    // Navigating somewhere new discards the forward stack.
    session
        .dispatch(Command::Navigate {
            tab: a,
            location: loc("elsewhere"),
        })
        .unwrap();
    let tab = session.snapshot().tabs[0].clone();
    assert!(tab.forward.is_empty());
    assert_eq!(tab.back, vec![loc("home"), loc("one"), loc("two")]);
}

#[test]
fn commands_that_change_nothing_make_no_events_and_keep_the_revision() {
    let mut session = Session::new();
    let a = open(&mut session, "a", true);
    let before = session.snapshot();
    for command in [
        Command::Activate { tab: a },
        Command::Back { tab: a },
        Command::Forward { tab: a },
        Command::Navigate {
            tab: a,
            location: loc("a"),
        },
    ] {
        assert!(session.dispatch(command).unwrap().is_empty());
    }
    assert_eq!(session.snapshot(), before);
}

#[test]
fn an_unknown_tab_is_an_error_and_changes_nothing() {
    let mut session = Session::new();
    open(&mut session, "a", true);
    let before = session.snapshot();
    let ghost = TabId(99);
    for command in [
        Command::Close { tab: ghost },
        Command::Activate { tab: ghost },
        Command::Move {
            tab: ghost,
            index: 0,
        },
        Command::Back { tab: ghost },
        Command::Open {
            location: loc("x"),
            after: Some(ghost),
            activate: true,
        },
    ] {
        assert!(session.dispatch(command).is_err());
    }
    assert_eq!(session.snapshot(), before);
}

#[test]
fn the_pure_apply_does_not_touch_its_input() {
    let (first, _) = apply(
        &SessionSnapshot {
            revision: 0,
            tabs: vec![],
            active: None,
        },
        Command::Open {
            location: loc("a"),
            after: None,
            activate: true,
        },
    )
    .unwrap();
    let (second, events) = apply(&first, Command::Close { tab: TabId(1) }).unwrap();
    assert_eq!(first.tabs.len(), 1);
    assert!(second.tabs.is_empty());
    assert_eq!(events.len(), 1);
}

#[test]
fn tab_closed_hooks_fire_for_closes_and_for_shutdown() {
    let closed = Arc::new(Mutex::new(Vec::new()));
    let mut session = Session::new();
    let sink = Arc::clone(&closed);
    session.on_tab_closed(move |tab| sink.lock().unwrap().push(tab));
    let a = open(&mut session, "a", true);
    let b = open(&mut session, "b", false);
    open(&mut session, "c", false);
    session.dispatch(Command::Close { tab: a }).unwrap();
    assert_eq!(*closed.lock().unwrap(), vec![a]);
    session.shutdown();
    let closed = closed.lock().unwrap();
    assert_eq!(closed.len(), 3);
    assert!(closed.contains(&b));
}

#[test]
fn events_serialise_with_a_kind_tag() {
    let mut session = Session::new();
    let events = session
        .dispatch(Command::Open {
            location: loc("a"),
            after: None,
            activate: true,
        })
        .unwrap();
    let json = serde_json::to_value(&events[1]).unwrap();
    assert_eq!(
        json,
        serde_json::json!({ "kind": "tabActivated", "tab": 1, "revision": 2 })
    );
}

/// A tiny deterministic generator, so the sequences are reproducible without a dependency.
struct Lcg(u64);
impl Lcg {
    fn next(&mut self, bound: usize) -> usize {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.0 >> 33) as usize) % bound.max(1)
    }
}

#[test]
fn random_command_sequences_hold_the_invariants() {
    for seed in 0..200u64 {
        let mut rng = Lcg(seed);
        let mut session = Session::new();
        let mut last_revision = 0;
        let mut replayed = session.snapshot();
        for step in 0..150 {
            let snapshot = session.snapshot();
            let pick = |rng: &mut Lcg| {
                // Mostly real tabs, sometimes one that does not exist.
                match snapshot.tabs.len() {
                    0 => TabId(999),
                    n if rng.next(10) == 0 => TabId(1000 + n as u32),
                    n => snapshot.tabs[rng.next(n)].id,
                }
            };
            let location = loc(&format!("p{}", rng.next(5)));
            let command = match rng.next(8) {
                0 | 1 => Command::Open {
                    location,
                    after: if rng.next(2) == 0 {
                        None
                    } else {
                        Some(pick(&mut rng))
                    },
                    activate: rng.next(2) == 0,
                },
                2 => Command::Close {
                    tab: pick(&mut rng),
                },
                3 => Command::Activate {
                    tab: pick(&mut rng),
                },
                4 => Command::Move {
                    tab: pick(&mut rng),
                    index: rng.next(8),
                },
                5 => Command::Navigate {
                    tab: pick(&mut rng),
                    location,
                },
                6 => Command::Back {
                    tab: pick(&mut rng),
                },
                _ => Command::Forward {
                    tab: pick(&mut rng),
                },
            };
            let Ok(events) = session.dispatch(command) else {
                assert_eq!(session.snapshot(), snapshot, "seed {seed} step {step}");
                continue;
            };
            let after = session.snapshot();
            assert_invariants(&after);
            for event in &events {
                assert_eq!(event.revision(), last_revision + 1, "strictly increasing");
                last_revision = event.revision();
                replay(&mut replayed, event);
            }
            assert_eq!(after.revision, last_revision);
            // History never goes out of step: the events alone rebuild the same session.
            assert_eq!(replayed, after, "seed {seed} step {step}");
        }
    }
}

/// Applies one event to a snapshot the way a frontend mirror does.
fn replay(snapshot: &mut SessionSnapshot, event: &SessionEvent) {
    snapshot.revision = event.revision();
    match event {
        SessionEvent::TabOpened { tab, index, .. } => {
            snapshot.tabs.insert(*index as usize, tab.clone())
        }
        SessionEvent::TabClosed { tab, .. } => {
            snapshot.tabs.retain(|t| t.id != *tab);
            if snapshot.active == Some(*tab) {
                snapshot.active = None;
            }
        }
        SessionEvent::TabActivated { tab, .. } => snapshot.active = Some(*tab),
        SessionEvent::TabMoved { tab, index, .. } => {
            let from = snapshot.tabs.iter().position(|t| t.id == *tab).unwrap();
            let moved = snapshot.tabs.remove(from);
            snapshot.tabs.insert(*index as usize, moved);
        }
        SessionEvent::TabNavigated { tab, .. } => {
            let slot = snapshot.tabs.iter_mut().find(|t| t.id == tab.id).unwrap();
            *slot = tab.clone();
        }
    }
}
