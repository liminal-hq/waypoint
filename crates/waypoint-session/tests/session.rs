// Headless tests for the session reducer: scripted behaviour and randomised invariant checks.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::sync::{Arc, Mutex};

use waypoint_session::{apply, Command, Session, SessionEvent, SessionSnapshot, TabId};

mod common;
use common::{loc, replay, Lcg};

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
        &SessionSnapshot::empty(),
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
            let command = match rng.next(9) {
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
                7 => Command::Reopen { tab: None },
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
            replayed.closed = after.closed.clone();
            assert_eq!(replayed, after, "seed {seed} step {step}");
        }
    }
}

/// Long random sequences over a many-window store, every command in the language. After every
/// step all invariants hold, the revision rises by exactly the number of events, and a mirror that
/// only sees events equals the store.
#[test]
fn random_store_sequences_hold_every_invariant() {
    use common::Mirror;
    use waypoint_session::{
        GroupId, GroupSort, MoveTo, MoveWhat, PairId, PairLayout, Store, TabColour, TabHints,
    };

    let mut stats = std::collections::BTreeMap::<&'static str, u32>::new();
    let colours = [None, Some(TabColour::Red), Some(TabColour::Blue)];
    for seed in 0..120u64 {
        let mut rng = Lcg(seed + 7);
        let mut store = Store::new();
        let mut mirror = Mirror::default();
        let mut moved_groups = 0;
        for step in 0..250 {
            let labels: Vec<String> = store.windows().iter().map(|w| w.label.clone()).collect();
            let window = if labels.is_empty() || rng.next(40) == 0 {
                "main-1".to_string()
            } else {
                labels[rng.next(labels.len())].clone()
            };
            let w = store.window(&window).cloned();
            let tab = |rng: &mut Lcg| match &w {
                Some(w) if !w.tabs.is_empty() && rng.next(12) != 0 => {
                    w.tabs[rng.next(w.tabs.len())].id
                }
                _ => TabId(5000),
            };
            let group = |rng: &mut Lcg| match &w {
                Some(w) if !w.groups.is_empty() && rng.next(12) != 0 => {
                    w.groups[rng.next(w.groups.len())].id
                }
                _ => GroupId(5000),
            };
            let pair = |rng: &mut Lcg| match &w {
                Some(w) if !w.pairs.is_empty() && rng.next(12) != 0 => {
                    w.pairs[rng.next(w.pairs.len())].id
                }
                _ => PairId(5000),
            };
            let tabs =
                |rng: &mut Lcg| -> Vec<TabId> { (0..1 + rng.next(3)).map(|_| tab(rng)).collect() };
            let command = match rng.next(40) {
                0..=3 => Command::Open {
                    location: loc(&format!("p{}", rng.next(6))),
                    after: if rng.next(2) == 0 {
                        None
                    } else {
                        Some(tab(&mut rng))
                    },
                    activate: rng.next(2) == 0,
                },
                4..=5 => Command::Close { tab: tab(&mut rng) },
                6..=7 => Command::Activate { tab: tab(&mut rng) },
                8..=9 => Command::Move {
                    tab: tab(&mut rng),
                    index: rng.next(9),
                },
                10 => Command::Navigate {
                    tab: tab(&mut rng),
                    location: loc(&format!("n{}", rng.next(4))),
                },
                11 => Command::Back { tab: tab(&mut rng) },
                12 => Command::Forward { tab: tab(&mut rng) },
                13..=14 => Command::Pin {
                    tab: tab(&mut rng),
                    pinned: rng.next(3) != 0,
                },
                15 => Command::SetColour {
                    tab: tab(&mut rng),
                    colour: colours[rng.next(3)],
                },
                16 => Command::SetHints {
                    tab: tab(&mut rng),
                    hints: TabHints {
                        scroll_top: rng.next(500) as u32,
                        focused: None,
                    },
                },
                17 => Command::Reopen { tab: None },
                18..=19 => Command::CreateGroup {
                    tabs: tabs(&mut rng),
                    name: None,
                },
                20 => Command::AddToGroup {
                    tab: tab(&mut rng),
                    group: group(&mut rng),
                },
                21 => Command::RemoveFromGroup { tab: tab(&mut rng) },
                22 => Command::RenameGroup {
                    group: group(&mut rng),
                    name: format!("g{}", rng.next(9)),
                },
                23 => Command::SetGroupCollapsed {
                    group: group(&mut rng),
                    collapsed: rng.next(2) == 0,
                },
                24 => Command::CollapseOthers {
                    group: group(&mut rng),
                },
                25 => Command::SortGroup {
                    group: group(&mut rng),
                    by: [GroupSort::Name, GroupSort::Location, GroupSort::LocalFirst][rng.next(3)],
                },
                26 => Command::DuplicateGroup {
                    group: group(&mut rng),
                },
                27 => Command::MoveGroup {
                    group: group(&mut rng),
                    index: rng.next(9),
                },
                28 => match rng.next(2) {
                    0 => Command::Ungroup {
                        group: group(&mut rng),
                    },
                    _ => Command::CloseGroup {
                        group: group(&mut rng),
                    },
                },
                29..=30 => Command::JoinPair {
                    tabs: tabs(&mut rng),
                    layout: PairLayout::SideBySide,
                },
                31 => match rng.next(3) {
                    0 => Command::SeparatePair {
                        pair: pair(&mut rng),
                    },
                    1 => Command::SwapPanes {
                        pair: pair(&mut rng),
                    },
                    _ => Command::SetPairLayout {
                        pair: pair(&mut rng),
                        layout: PairLayout::Stacked,
                    },
                },
                32..=33 => Command::ToggleSplit { tab: tab(&mut rng) },
                34 => Command::OpenWindow {
                    location: Some(loc("w")),
                    geometry: None,
                },
                35 => Command::CloseWindow,
                36..=39 => {
                    let what = match rng.next(3) {
                        0 => MoveWhat::Tabs(tabs(&mut rng)),
                        1 => MoveWhat::Group(group(&mut rng)),
                        _ => MoveWhat::Pair(pair(&mut rng)),
                    };
                    let to = if labels.len() > 1 && rng.next(2) == 0 {
                        MoveTo::ExistingWindow {
                            label: labels[rng.next(labels.len())].clone(),
                            index: rng.next(6),
                        }
                    } else {
                        MoveTo::NewWindow {
                            label: None,
                            geometry: None,
                        }
                    };
                    Command::MoveTabs { what, to }
                }
                _ => unreachable!(),
            };
            if matches!(
                command,
                Command::MoveTabs {
                    what: MoveWhat::Group(_),
                    ..
                }
            ) {
                moved_groups += 1;
            }
            let before = store.clone();
            let Ok(outcome) = store.dispatch(&window, command) else {
                assert_eq!(
                    store, before,
                    "seed {seed} step {step}: error changed the store"
                );
                continue;
            };
            let problems = store.violations();
            assert!(problems.is_empty(), "seed {seed} step {step}: {problems:?}");
            assert_eq!(
                store.revision(),
                before.revision() + outcome.events.len() as u64,
                "seed {seed} step {step}"
            );
            if outcome.events.is_empty() {
                assert_eq!(
                    store, before,
                    "seed {seed} step {step}: no events but a change"
                );
            }
            for (i, e) in outcome.events.iter().enumerate() {
                assert_eq!(e.event.revision(), before.revision() + 1 + i as u64);
            }
            for e in &outcome.events {
                let name = match &e.event {
                    SessionEvent::GroupCreated { .. } => "groupCreated",
                    SessionEvent::PairCreated { .. } => "pairCreated",
                    SessionEvent::TabReopened { .. } => "tabReopened",
                    SessionEvent::WindowOpened { .. } => "windowOpened",
                    SessionEvent::WindowClosed { .. } => "windowClosed",
                    SessionEvent::TabMoved { .. } => "tabMoved",
                    SessionEvent::GroupRemoved { .. } => "groupRemoved",
                    SessionEvent::PairRemoved { .. } => "pairRemoved",
                    _ => continue,
                };
                *stats.entry(name).or_default() += 1;
            }
            mirror.apply(&outcome.events);
            if let Err(why) = mirror.matches(&store) {
                panic!("seed {seed} step {step}: {why}");
            }
            // Tab ids never repeat across live and closed tabs.
            let mut seen = std::collections::HashSet::new();
            for t in store
                .windows()
                .iter()
                .flat_map(|w| w.tabs.iter().map(|t| t.id))
            {
                assert!(seen.insert(t));
            }
            for c in store.closed() {
                assert!(seen.insert(c.tab.id));
            }
        }
        let _ = moved_groups;
    }
    eprintln!("{stats:?}");
    for key in [
        "groupCreated",
        "pairCreated",
        "tabReopened",
        "windowOpened",
        "windowClosed",
        "tabMoved",
        "groupRemoved",
        "pairRemoved",
    ] {
        assert!(
            stats.get(key).copied().unwrap_or(0) > 50,
            "the generator never exercised {key}"
        );
    }
}
