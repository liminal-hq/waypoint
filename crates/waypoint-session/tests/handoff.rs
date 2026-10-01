// Headless tests for windows and the atomic hand-off of tabs between them.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use waypoint_session::{
    Command, Geometry, MoveTo, MoveWhat, PairLayout, SessionEvent, TabColour, TabHints, TabId,
    ViewMode, ViewPrefs,
};

mod common;
use common::{assert_ok, ids, loc, open, run, store_with, Mirror};

const W: &str = "main-1";

fn new_window() -> MoveTo {
    MoveTo::NewWindow {
        label: None,
        geometry: None,
    }
}

#[test]
fn open_window_allocates_labels_that_are_never_reused() {
    let mut s = waypoint_session::Store::new();
    let out = run(
        &mut s,
        "",
        Command::OpenWindow {
            location: Some(loc("a")),
            geometry: None,
        },
    );
    assert_eq!(out.windows_opened(), vec!["main-1".to_string()]);
    run(
        &mut s,
        "",
        Command::OpenWindow {
            location: None,
            geometry: None,
        },
    );
    run(&mut s, "main-2", Command::CloseWindow);
    let out = run(
        &mut s,
        "",
        Command::OpenWindow {
            location: None,
            geometry: None,
        },
    );
    assert_eq!(out.windows_opened(), vec!["main-3".to_string()]);
    assert_ok(&s);
}

#[test]
fn closing_the_last_tab_closes_the_window_and_the_last_window_reports_it() {
    let mut s = store_with(&["a"]);
    open(&mut s, W, "b");
    run(
        &mut s,
        "",
        Command::OpenWindow {
            location: Some(loc("c")),
            geometry: None,
        },
    );
    run(&mut s, W, Command::Close { tab: TabId(1) });
    let out = run(&mut s, W, Command::Close { tab: TabId(2) });
    assert_eq!(out.windows_closed(), vec![W.to_string()]);
    assert!(!out.last_window_closed);
    assert!(s.window(W).is_none());
    // The closed tabs remember their window.
    assert_eq!(s.closed().len(), 2);
    assert_eq!(s.closed()[0].window, W);
    let out = run(&mut s, "main-2", Command::Close { tab: TabId(3) });
    assert!(out.last_window_closed);
    assert!(s.windows().is_empty());
    // Reopening with no window left needs a window to land in.
    assert!(s.dispatch(W, Command::Reopen { tab: None }).is_err());
}

#[test]
fn reopen_goes_to_the_window_it_came_from_else_the_caller() {
    let mut s = store_with(&["a", "b"]);
    run(
        &mut s,
        "",
        Command::OpenWindow {
            location: Some(loc("c")),
            geometry: None,
        },
    );
    run(&mut s, W, Command::Close { tab: TabId(2) });
    run(&mut s, "main-2", Command::Reopen { tab: None });
    assert_eq!(ids(&s, W), vec![1, 2]);

    run(&mut s, W, Command::CloseWindow);
    assert_eq!(s.closed().len(), 2);
    run(
        &mut s,
        "main-2",
        Command::Reopen {
            tab: Some(TabId(1)),
        },
    );
    assert_ok(&s);
    assert_eq!(ids(&s, "main-2"), vec![1, 3]);
    assert_eq!(s.window("main-2").unwrap().active, Some(TabId(1)));
}

#[test]
fn geometry_and_view_are_state_with_events() {
    let mut s = store_with(&["a"]);
    let g = Geometry {
        x: None,
        y: None,
        width: 900,
        height: 600,
        maximised: false,
    };
    let out = run(&mut s, W, Command::SetGeometry { geometry: g });
    assert!(matches!(
        out.events[0].event,
        SessionEvent::GeometryChanged { .. }
    ));
    assert!(run(&mut s, W, Command::SetGeometry { geometry: g })
        .events
        .is_empty());
    let view = ViewPrefs {
        mode: ViewMode::Grid,
        show_hidden: true,
        icon_size: 96,
    };
    let out = run(&mut s, W, Command::SetView { view });
    assert!(matches!(
        out.events[0].event,
        SessionEvent::ViewChanged { .. }
    ));
    let snap = s.snapshot(W).unwrap();
    assert_eq!(snap.geometry, Some(g));
    assert_eq!(snap.view, view);
}

#[test]
fn moving_a_tab_to_a_new_window_keeps_its_id_and_everything_on_it() {
    let mut s = store_with(&["a", "b", "c"]);
    s.dispatch(
        W,
        Command::Navigate {
            tab: TabId(2),
            location: loc("x"),
        },
    )
    .unwrap();
    s.dispatch(
        W,
        Command::Navigate {
            tab: TabId(2),
            location: loc("y"),
        },
    )
    .unwrap();
    s.dispatch(W, Command::Back { tab: TabId(2) }).unwrap();
    run(
        &mut s,
        W,
        Command::SetColour {
            tab: TabId(2),
            colour: Some(TabColour::Green),
        },
    );
    run(
        &mut s,
        W,
        Command::Pin {
            tab: TabId(2),
            pinned: true,
        },
    );
    let hints = TabHints {
        scroll_top: 420,
        focused: Some("notes.txt".into()),
    };
    run(
        &mut s,
        W,
        Command::SetHints {
            tab: TabId(2),
            hints: hints.clone(),
        },
    );
    let before = s.window(W).unwrap().tab(TabId(2)).unwrap().clone();
    run(&mut s, W, Command::Activate { tab: TabId(2) });

    let out = run(
        &mut s,
        W,
        Command::MoveTabs {
            what: MoveWhat::Tabs(vec![TabId(2)]),
            to: new_window(),
        },
    );
    assert_ok(&s);
    assert_eq!(out.windows_opened(), vec!["main-2".to_string()]);
    assert_eq!(ids(&s, W), vec![1, 3]);
    assert_eq!(ids(&s, "main-2"), vec![2]);
    let moved = s.window("main-2").unwrap().tab(TabId(2)).unwrap();
    assert_eq!(*moved, before);
    assert_eq!(moved.hints, hints);
    assert!(!s
        .window("main-2")
        .unwrap()
        .tab(TabId(2))
        .unwrap()
        .back
        .is_empty());
    assert_eq!(s.window("main-2").unwrap().active, Some(TabId(2)));
    assert!(s.window(W).unwrap().active.is_some());
    // The source sees the tab close, the target sees it open, all in one step.
    assert!(out
        .events_for(W)
        .any(|e| matches!(e, SessionEvent::TabClosed { tab, .. } if *tab == TabId(2))));
    assert!(out
        .events_for("main-2")
        .any(|e| matches!(e, SessionEvent::TabOpened { tab, .. } if tab.id == TabId(2))));
    // A hand-off is not a close: nothing enters the closed list.
    assert!(s.closed().is_empty());
}

#[test]
fn moving_the_only_tab_closes_the_source_window() {
    let mut s = store_with(&["a"]);
    run(
        &mut s,
        "",
        Command::OpenWindow {
            location: Some(loc("b")),
            geometry: None,
        },
    );
    let out = run(
        &mut s,
        W,
        Command::MoveTabs {
            what: MoveWhat::Tabs(vec![TabId(1)]),
            to: MoveTo::ExistingWindow {
                label: "main-2".into(),
                index: 0,
            },
        },
    );
    assert_ok(&s);
    assert_eq!(out.windows_closed(), vec![W.to_string()]);
    assert_eq!(ids(&s, "main-2"), vec![1, 2]);
    assert!(s.closed().is_empty());
    assert!(!out.last_window_closed);
}

#[test]
fn moving_into_an_existing_window_inserts_at_the_index_and_keeps_its_active_tab() {
    let mut s = store_with(&["a", "b"]);
    run(
        &mut s,
        "",
        Command::OpenWindow {
            location: Some(loc("c")),
            geometry: None,
        },
    );
    open(&mut s, "main-2", "d");
    run(
        &mut s,
        W,
        Command::MoveTabs {
            what: MoveWhat::Tabs(vec![TabId(2)]),
            to: MoveTo::ExistingWindow {
                label: "main-2".into(),
                index: 1,
            },
        },
    );
    assert_ok(&s);
    assert_eq!(ids(&s, "main-2"), vec![3, 2, 4]);
    assert_eq!(s.window("main-2").unwrap().active, Some(TabId(3)));
    // The same window, or one that does not exist, is an error and changes nothing.
    let before = s.clone();
    assert!(s
        .dispatch(
            W,
            Command::MoveTabs {
                what: MoveWhat::Tabs(vec![TabId(1)]),
                to: MoveTo::ExistingWindow {
                    label: W.into(),
                    index: 0
                },
            }
        )
        .is_err());
    assert!(s
        .dispatch(
            W,
            Command::MoveTabs {
                what: MoveWhat::Tabs(vec![TabId(1)]),
                to: MoveTo::ExistingWindow {
                    label: "main-9".into(),
                    index: 0
                },
            }
        )
        .is_err());
    assert_eq!(s, before);
}

#[test]
fn a_group_and_a_pair_travel_with_their_identity() {
    let mut s = store_with(&["a", "b", "c", "d", "e"]);
    run(
        &mut s,
        W,
        Command::CreateGroup {
            tabs: vec![TabId(2), TabId(3)],
            name: Some("G".into()),
        },
    );
    run(
        &mut s,
        W,
        Command::JoinPair {
            tabs: vec![TabId(4), TabId(5)],
            layout: PairLayout::Stacked,
        },
    );
    let g = s.window(W).unwrap().groups[0].clone();
    let p = s.window(W).unwrap().pairs[0].clone();
    run(
        &mut s,
        W,
        Command::MoveTabs {
            what: MoveWhat::Group(g.id),
            to: new_window(),
        },
    );
    assert_ok(&s);
    assert_eq!(s.window("main-2").unwrap().groups, vec![g]);
    assert!(s.window(W).unwrap().groups.is_empty());
    run(
        &mut s,
        W,
        Command::MoveTabs {
            what: MoveWhat::Pair(p.id),
            to: new_window(),
        },
    );
    assert_ok(&s);
    assert_eq!(s.window("main-3").unwrap().pairs, vec![p]);
    assert_eq!(ids(&s, W), vec![1]);
}

#[test]
fn moving_part_of_a_group_or_pair_leaves_the_rest_whole() {
    let mut s = store_with(&["a", "b", "c", "d"]);
    run(
        &mut s,
        W,
        Command::CreateGroup {
            tabs: vec![TabId(1), TabId(2), TabId(3)],
            name: None,
        },
    );
    run(
        &mut s,
        W,
        Command::JoinPair {
            tabs: vec![TabId(1), TabId(2)],
            layout: PairLayout::SideBySide,
        },
    );
    run(
        &mut s,
        W,
        Command::MoveTabs {
            what: MoveWhat::Tabs(vec![TabId(2)]),
            to: new_window(),
        },
    );
    assert_ok(&s);
    // The pair lost a pane so it dissolved, the group kept two tabs, and the mover left both.
    assert!(s.window(W).unwrap().pairs.is_empty());
    assert_eq!(s.window(W).unwrap().groups.len(), 1);
    assert_eq!(
        s.window("main-2").unwrap().tab(TabId(2)).unwrap().group,
        None
    );
    assert!(s.window("main-2").unwrap().groups.is_empty());
}

#[test]
fn moving_the_active_tab_hands_the_source_to_its_mru_or_neighbour() {
    let mut s = store_with(&["a", "b", "c"]);
    for t in [3u32, 1, 2] {
        run(&mut s, W, Command::Activate { tab: TabId(t) });
    }
    run(
        &mut s,
        W,
        Command::MoveTabs {
            what: MoveWhat::Tabs(vec![TabId(2)]),
            to: new_window(),
        },
    );
    assert_eq!(s.window(W).unwrap().active, Some(TabId(1)));
    assert_eq!(s.window(W).unwrap().mru, vec![TabId(1), TabId(3)]);
}

#[test]
fn a_mirror_following_events_stays_equal_across_a_hand_off() {
    let mut mirror = Mirror::default();
    let mut store = waypoint_session::Store::new();
    for cmd in [
        Command::OpenWindow {
            location: Some(loc("a")),
            geometry: None,
        },
        Command::Open {
            location: loc("b"),
            after: None,
            activate: false,
        },
        Command::Open {
            location: loc("c"),
            after: None,
            activate: false,
        },
        Command::CreateGroup {
            tabs: vec![TabId(2), TabId(3)],
            name: None,
        },
        Command::MoveTabs {
            what: MoveWhat::Tabs(vec![TabId(3)]),
            to: new_window(),
        },
        Command::MoveTabs {
            what: MoveWhat::Tabs(vec![TabId(1)]),
            to: MoveTo::ExistingWindow {
                label: "main-2".into(),
                index: 0,
            },
        },
    ] {
        let out = store.dispatch(W, cmd).unwrap();
        mirror.apply(&out.events);
        mirror.matches(&store).unwrap();
    }
    assert_eq!(store.windows().len(), 2);
}

#[test]
fn closing_a_window_sends_its_tabs_to_the_closed_list_and_tells_listeners() {
    let mut s = store_with(&["a", "b"]);
    run(
        &mut s,
        "",
        Command::OpenWindow {
            location: Some(loc("c")),
            geometry: None,
        },
    );
    let out = run(&mut s, W, Command::CloseWindow);
    let closed: Vec<TabId> = out
        .events_for(W)
        .filter_map(|e| match e {
            SessionEvent::TabClosed { tab, .. } => Some(*tab),
            _ => None,
        })
        .collect();
    assert_eq!(closed, vec![TabId(1), TabId(2)]);
    assert_eq!(out.windows_closed(), vec![W.to_string()]);
    assert_eq!(s.closed().len(), 2);
    assert!(s.dispatch(W, Command::CloseWindow).is_err());
}

#[test]
fn registering_a_window_makes_it_under_its_label_and_keeps_later_labels_above_it() {
    let mut store = waypoint_session::Store::new();
    let outcome = store
        .dispatch(
            "main-3",
            Command::RegisterWindow {
                label: "main-3".into(),
            },
        )
        .unwrap();
    assert_eq!(outcome.windows_opened(), vec!["main-3".to_string()]);
    assert!(store.window("main-3").is_some_and(|w| w.tabs.is_empty()));
    let next = store
        .dispatch(
            "main-3",
            Command::OpenWindow {
                location: None,
                geometry: None,
            },
        )
        .unwrap();
    assert_eq!(next.windows_opened(), vec!["main-4".to_string()]);
    assert_ok(&store);
}

#[test]
fn registering_a_taken_or_foreign_label_is_an_error_and_changes_nothing() {
    let mut store = store_with(&["a"]);
    let before = store.clone();
    for label in ["main-1", "settings", "nonsense"] {
        let result = store.dispatch(
            "main-1",
            Command::RegisterWindow {
                label: label.into(),
            },
        );
        assert!(result.is_err(), "{label}");
        assert_eq!(store, before);
    }
}

#[test]
fn window_summaries_title_a_window_by_its_active_folder_and_mark_the_caller() {
    let mut s = store_with(&["Documents", "Music"]);
    run(
        &mut s,
        "",
        Command::OpenWindow {
            location: Some(waypoint_protocol::Location::new(
                "C:\\Users\\me\\",
                "file:///C:/Users/me",
            )),
            geometry: None,
        },
    );
    run(
        &mut s,
        "",
        Command::OpenWindow {
            location: Some(waypoint_protocol::Location::new("/", "file:///")),
            geometry: None,
        },
    );
    run(&mut s, W, Command::Activate { tab: TabId(2) });
    let summaries = s.window_summaries("main-2");
    assert_eq!(summaries.len(), 3);
    assert_eq!(
        (
            summaries[0].title.as_str(),
            summaries[0].tab_count,
            summaries[0].active
        ),
        ("Music", 2, false)
    );
    assert_eq!(
        (
            summaries[1].title.as_str(),
            summaries[1].tab_count,
            summaries[1].active
        ),
        ("me", 1, true)
    );
    assert_eq!(summaries[2].title, "/");
}
