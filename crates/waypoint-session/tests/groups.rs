// Headless tests for tab groups, pinning and the closed-tab history.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use waypoint_session::{Command, GroupId, GroupSort, SessionEvent, TabColour, TabId};

mod common;
use common::{assert_ok, ids, loc, open, run, store_with};

const W: &str = "main-1";

fn group_of(store: &waypoint_session::Store, tab: u32) -> Option<GroupId> {
    store.window(W).unwrap().tab(TabId(tab)).unwrap().group
}

#[test]
fn creating_a_group_gathers_its_tabs_at_the_first_one() {
    let mut s = store_with(&["a", "b", "c", "d"]);
    let out = run(
        &mut s,
        W,
        Command::CreateGroup {
            tabs: vec![TabId(4), TabId(2)],
            name: Some("Work".into()),
        },
    );
    assert_ok(&s);
    assert_eq!(ids(&s, W), vec![1, 2, 4, 3]);
    let w = s.window(W).unwrap();
    assert_eq!(w.groups.len(), 1);
    assert_eq!(w.groups[0].name, "Work");
    assert!(out
        .events_for(W)
        .any(|e| matches!(e, SessionEvent::GroupCreated { .. })));
    assert_eq!(group_of(&s, 2), group_of(&s, 4));
    assert_eq!(group_of(&s, 1), None);
}

#[test]
fn a_default_group_name_is_numbered() {
    let mut s = store_with(&["a"]);
    run(
        &mut s,
        W,
        Command::CreateGroup {
            tabs: vec![TabId(1)],
            name: None,
        },
    );
    assert_eq!(s.window(W).unwrap().groups[0].name, "Group 1");
}

#[test]
fn default_group_names_count_the_windows_groups_and_skip_taken_numbers() {
    let mut s = store_with(&["a", "b", "c", "d"]);
    let make = |s: &mut waypoint_session::Store, tab: u32| {
        run(
            s,
            W,
            Command::CreateGroup {
                tabs: vec![TabId(tab)],
                name: None,
            },
        );
    };
    make(&mut s, 1);
    make(&mut s, 2);
    // A second window starts at "Group 1" although the session's group ids are already at 3.
    s.dispatch(
        "main-2",
        Command::OpenWindow {
            location: Some(loc("x")),
            geometry: None,
        },
    )
    .unwrap();
    run(
        &mut s,
        "main-2",
        Command::CreateGroup {
            tabs: vec![TabId(5)],
            name: None,
        },
    );
    assert_eq!(s.window("main-2").unwrap().groups[0].name, "Group 1");
    // Ungrouping "Group 1" leaves "Group 2"; the next group is "Group 3", not a repeat of "Group 2".
    let first = s.window(W).unwrap().groups[0].id;
    run(&mut s, W, Command::Ungroup { group: first });
    make(&mut s, 3);
    let names: Vec<_> = s
        .window(W)
        .unwrap()
        .groups
        .iter()
        .map(|g| g.name.clone())
        .collect();
    assert_eq!(names, vec!["Group 2", "Group 3"]);
}

#[test]
fn add_to_group_moves_the_tab_to_the_end_of_the_group() {
    let mut s = store_with(&["a", "b", "c", "d"]);
    run(
        &mut s,
        W,
        Command::CreateGroup {
            tabs: vec![TabId(2), TabId(3)],
            name: None,
        },
    );
    let g = group_of(&s, 2).unwrap();
    run(
        &mut s,
        W,
        Command::AddToGroup {
            tab: TabId(1),
            group: g,
        },
    );
    assert_ok(&s);
    assert_eq!(ids(&s, W), vec![2, 3, 1, 4]);
    assert_eq!(group_of(&s, 1), Some(g));
    // Adding a member again changes nothing.
    assert!(run(
        &mut s,
        W,
        Command::AddToGroup {
            tab: TabId(1),
            group: g
        }
    )
    .events
    .is_empty());
}

#[test]
fn removing_from_the_middle_of_a_group_lands_after_it() {
    let mut s = store_with(&["a", "b", "c", "d"]);
    run(
        &mut s,
        W,
        Command::CreateGroup {
            tabs: vec![TabId(1), TabId(2), TabId(3)],
            name: None,
        },
    );
    run(&mut s, W, Command::RemoveFromGroup { tab: TabId(2) });
    assert_ok(&s);
    assert_eq!(ids(&s, W), vec![1, 3, 2, 4]);
    assert_eq!(group_of(&s, 2), None);
}

#[test]
fn a_group_loses_its_identity_when_its_last_tab_leaves() {
    let mut s = store_with(&["a", "b"]);
    run(
        &mut s,
        W,
        Command::CreateGroup {
            tabs: vec![TabId(1)],
            name: None,
        },
    );
    let g = group_of(&s, 1).unwrap();
    let out = run(&mut s, W, Command::RemoveFromGroup { tab: TabId(1) });
    assert!(s.window(W).unwrap().groups.is_empty());
    assert!(out
        .events_for(W)
        .any(|e| matches!(e, SessionEvent::GroupRemoved { group, .. } if *group == g)));
}

#[test]
fn a_tab_in_a_group_cannot_be_dragged_out_and_others_cannot_be_dropped_inside() {
    let mut s = store_with(&["a", "b", "c", "d"]);
    run(
        &mut s,
        W,
        Command::CreateGroup {
            tabs: vec![TabId(2), TabId(3)],
            name: None,
        },
    );
    // Moving a member far to the right keeps it inside the group's run.
    run(
        &mut s,
        W,
        Command::Move {
            tab: TabId(2),
            index: 3,
        },
    );
    assert_ok(&s);
    assert_eq!(ids(&s, W), vec![1, 3, 2, 4]);
    // Dropping an outsider into the middle of the run lands it after the group.
    run(
        &mut s,
        W,
        Command::Move {
            tab: TabId(1),
            index: 1,
        },
    );
    assert_ok(&s);
    assert_eq!(ids(&s, W), vec![3, 2, 1, 4]);
}

#[test]
fn rename_colour_collapse_and_collapse_others() {
    let mut s = store_with(&["a", "b", "c"]);
    for t in [1u32, 2] {
        run(
            &mut s,
            W,
            Command::CreateGroup {
                tabs: vec![TabId(t)],
                name: None,
            },
        );
    }
    let (g1, g2) = (group_of(&s, 1).unwrap(), group_of(&s, 2).unwrap());
    run(
        &mut s,
        W,
        Command::RenameGroup {
            group: g1,
            name: "One".into(),
        },
    );
    run(
        &mut s,
        W,
        Command::SetGroupColour {
            group: g1,
            colour: Some(TabColour::Teal),
        },
    );
    run(&mut s, W, Command::CollapseOthers { group: g1 });
    let w = s.window(W).unwrap();
    assert_eq!(w.group(g1).unwrap().name, "One");
    assert_eq!(w.group(g1).unwrap().colour, Some(TabColour::Teal));
    assert!(!w.group(g1).unwrap().collapsed);
    assert!(w.group(g2).unwrap().collapsed);
    // Collapsing again is a no-op.
    assert!(run(&mut s, W, Command::CollapseOthers { group: g1 })
        .events
        .is_empty());
    run(
        &mut s,
        W,
        Command::SetGroupCollapsed {
            group: g2,
            collapsed: false,
        },
    );
    assert!(!s.window(W).unwrap().group(g2).unwrap().collapsed);
}

#[test]
fn sorting_by_name_location_and_local_first() {
    let mut s = store_with(&["a"]);
    let b = open(&mut s, W, "Beta");
    let c = open(&mut s, W, "alpha");
    s.dispatch(
        W,
        Command::Navigate {
            tab: c,
            location: waypoint_protocol::Location::new("ssh://host/zeta", "sftp://host/zeta"),
        },
    )
    .unwrap();
    run(
        &mut s,
        W,
        Command::CreateGroup {
            tabs: vec![TabId(1), b, c],
            name: None,
        },
    );
    let g = group_of(&s, 1).unwrap();
    run(
        &mut s,
        W,
        Command::SortGroup {
            group: g,
            by: GroupSort::Name,
        },
    );
    assert_ok(&s);
    // a, Beta, zeta (case-insensitive by last path part).
    assert_eq!(ids(&s, W), vec![1, 2, 3]);
    run(
        &mut s,
        W,
        Command::SortGroup {
            group: g,
            by: GroupSort::LocalFirst,
        },
    );
    assert_eq!(ids(&s, W), vec![1, 2, 3]);
    run(
        &mut s,
        W,
        Command::SortGroup {
            group: g,
            by: GroupSort::Location,
        },
    );
    // file:///Beta < file:///a < sftp://host/zeta by URI.
    assert_eq!(ids(&s, W), vec![2, 1, 3]);
}

#[test]
fn duplicating_a_group_copies_tabs_with_new_ids_after_the_original() {
    let mut s = store_with(&["a", "b", "c"]);
    run(
        &mut s,
        W,
        Command::CreateGroup {
            tabs: vec![TabId(1), TabId(2)],
            name: Some("G".into()),
        },
    );
    let g = group_of(&s, 1).unwrap();
    s.dispatch(
        W,
        Command::Navigate {
            tab: TabId(1),
            location: loc("x"),
        },
    )
    .unwrap();
    run(&mut s, W, Command::DuplicateGroup { group: g });
    assert_ok(&s);
    assert_eq!(ids(&s, W), vec![1, 2, 4, 5, 3]);
    let w = s.window(W).unwrap();
    assert_eq!(w.groups.len(), 2);
    let copy = w.tab(TabId(4)).unwrap();
    assert_eq!(copy.location, loc("x"));
    assert_eq!(copy.back, vec![loc("a")]);
    assert_ne!(copy.group, Some(g));
}

#[test]
fn move_ungroup_and_close_a_group() {
    let mut s = store_with(&["a", "b", "c", "d"]);
    run(
        &mut s,
        W,
        Command::CreateGroup {
            tabs: vec![TabId(1), TabId(2)],
            name: None,
        },
    );
    let g = group_of(&s, 1).unwrap();
    run(&mut s, W, Command::MoveGroup { group: g, index: 2 });
    assert_ok(&s);
    assert_eq!(ids(&s, W), vec![3, 4, 1, 2]);
    run(&mut s, W, Command::Ungroup { group: g });
    assert!(s.window(W).unwrap().groups.is_empty());
    assert_eq!(ids(&s, W), vec![3, 4, 1, 2]);

    run(
        &mut s,
        W,
        Command::CreateGroup {
            tabs: vec![TabId(1), TabId(2)],
            name: None,
        },
    );
    let g = group_of(&s, 1).unwrap();
    run(&mut s, W, Command::CloseGroup { group: g });
    assert_ok(&s);
    assert_eq!(ids(&s, W), vec![3, 4]);
    assert_eq!(s.closed().len(), 2);
}

#[test]
fn pinning_a_tab_moves_it_before_unpinned_ones_and_carries_its_group() {
    let mut s = store_with(&["a", "b", "c", "d"]);
    run(
        &mut s,
        W,
        Command::Pin {
            tab: TabId(3),
            pinned: true,
        },
    );
    assert_ok(&s);
    assert_eq!(ids(&s, W), vec![3, 1, 2, 4]);
    run(
        &mut s,
        W,
        Command::Pin {
            tab: TabId(2),
            pinned: true,
        },
    );
    assert_eq!(ids(&s, W), vec![3, 2, 1, 4]);
    // A pinned tab cannot be dragged past the unpinned ones, nor an unpinned one in front.
    run(
        &mut s,
        W,
        Command::Move {
            tab: TabId(3),
            index: 3,
        },
    );
    assert_eq!(ids(&s, W), vec![2, 3, 1, 4]);
    run(
        &mut s,
        W,
        Command::Move {
            tab: TabId(4),
            index: 0,
        },
    );
    assert_eq!(ids(&s, W), vec![2, 3, 4, 1]);
    // Unpinning leaves it first among the unpinned.
    run(
        &mut s,
        W,
        Command::Pin {
            tab: TabId(3),
            pinned: false,
        },
    );
    assert_eq!(ids(&s, W), vec![2, 3, 4, 1]);

    // Pinning one member pins the whole group and keeps its label.
    run(
        &mut s,
        W,
        Command::CreateGroup {
            tabs: vec![TabId(4), TabId(1)],
            name: Some("Keep".into()),
        },
    );
    run(
        &mut s,
        W,
        Command::Pin {
            tab: TabId(1),
            pinned: true,
        },
    );
    assert_ok(&s);
    assert_eq!(ids(&s, W), vec![2, 4, 1, 3]);
    let w = s.window(W).unwrap();
    assert!(w.tab(TabId(4)).unwrap().pinned && w.tab(TabId(1)).unwrap().pinned);
    assert_eq!(w.groups[0].name, "Keep");
}

#[test]
fn colour_and_hints_emit_tab_changed() {
    let mut s = store_with(&["a"]);
    let out = run(
        &mut s,
        W,
        Command::SetColour {
            tab: TabId(1),
            colour: Some(TabColour::Red),
        },
    );
    assert!(matches!(
        out.events[0].event,
        SessionEvent::TabChanged { .. }
    ));
    assert!(run(
        &mut s,
        W,
        Command::SetColour {
            tab: TabId(1),
            colour: Some(TabColour::Red)
        }
    )
    .events
    .is_empty());
}

#[test]
fn closing_records_the_tab_and_reopen_restores_it_in_place() {
    let mut s = store_with(&["a", "b", "c"]);
    s.dispatch(
        W,
        Command::Navigate {
            tab: TabId(2),
            location: loc("x"),
        },
    )
    .unwrap();
    run(
        &mut s,
        W,
        Command::SetColour {
            tab: TabId(2),
            colour: Some(TabColour::Pink),
        },
    );
    run(&mut s, W, Command::Close { tab: TabId(2) });
    assert_eq!(s.closed().len(), 1);
    assert_eq!(s.closed()[0].index, 1);
    let out = run(&mut s, W, Command::Reopen { tab: None });
    assert_ok(&s);
    assert_eq!(ids(&s, W), vec![1, 2, 3]);
    let w = s.window(W).unwrap();
    let tab = w.tab(TabId(2)).unwrap();
    assert_eq!(tab.location, loc("x"));
    assert_eq!(tab.colour, Some(TabColour::Pink));
    assert_eq!(w.active, Some(TabId(2)));
    assert!(s.closed().is_empty());
    assert!(matches!(
        out.events[0].event,
        SessionEvent::TabReopened { index: 1, .. }
    ));
    // Nothing left to reopen is a no-op, and an unknown id an error.
    assert!(run(&mut s, W, Command::Reopen { tab: None })
        .events
        .is_empty());
    assert!(s
        .dispatch(
            W,
            Command::Reopen {
                tab: Some(TabId(77))
            }
        )
        .is_err());
}

#[test]
fn the_closed_list_keeps_ten_newest_first() {
    let mut s = store_with(&["a"]);
    for n in 0..12 {
        open(&mut s, W, &format!("t{n}"));
    }
    for id in 2..=13 {
        run(&mut s, W, Command::Close { tab: TabId(id) });
    }
    assert_eq!(s.closed().len(), 10);
    assert_eq!(s.closed()[0].tab.id, TabId(13));
    assert_eq!(s.closed()[9].tab.id, TabId(4));
}

#[test]
fn a_reopened_tab_rejoins_its_group_only_where_the_group_still_is() {
    let mut s = store_with(&["a", "b", "c"]);
    run(
        &mut s,
        W,
        Command::CreateGroup {
            tabs: vec![TabId(1), TabId(2)],
            name: None,
        },
    );
    let g = group_of(&s, 1).unwrap();
    run(&mut s, W, Command::Close { tab: TabId(2) });
    run(&mut s, W, Command::Reopen { tab: None });
    assert_ok(&s);
    assert_eq!(group_of(&s, 2), Some(g));
    // Closing the whole group removes it, so a reopened member comes back ungrouped.
    run(&mut s, W, Command::CloseGroup { group: g });
    run(&mut s, W, Command::Reopen { tab: None });
    assert_ok(&s);
    assert_eq!(group_of(&s, 2), None);
}

#[test]
fn mru_decides_the_next_active_tab_and_the_neighbour_rule_covers_the_rest() {
    let mut s = store_with(&["a", "b", "c", "d"]);
    for t in [4u32, 1, 3] {
        run(&mut s, W, Command::Activate { tab: TabId(t) });
    }
    assert_eq!(s.window(W).unwrap().mru, vec![TabId(3), TabId(1), TabId(4)]);
    run(&mut s, W, Command::Close { tab: TabId(3) });
    assert_eq!(s.window(W).unwrap().active, Some(TabId(1)));
    // Closing an inactive tab drops it from the list without touching the active tab.
    run(&mut s, W, Command::Close { tab: TabId(4) });
    assert_eq!(s.window(W).unwrap().mru, vec![TabId(1)]);
    assert_eq!(s.window(W).unwrap().active, Some(TabId(1)));
    // With no MRU entry left, the neighbour takes over.
    run(&mut s, W, Command::Close { tab: TabId(1) });
    assert_eq!(s.window(W).unwrap().active, Some(TabId(2)));
}
