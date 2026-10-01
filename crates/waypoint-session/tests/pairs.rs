// Headless tests for pairs: joining, panes, sizes, swapping and the split toggle.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use waypoint_session::{Command, PairLayout, PairOrigin, SessionEvent, TabId};

mod common;
use common::{assert_ok, ids, run, store_with};

const W: &str = "main-1";

fn join(s: &mut waypoint_session::Store, tabs: &[u32]) {
    run(
        s,
        W,
        Command::JoinPair {
            tabs: tabs.iter().map(|t| TabId(*t)).collect(),
            layout: PairLayout::SideBySide,
        },
    );
}

#[test]
fn joining_makes_the_panes_contiguous_next_to_the_first() {
    let mut s = store_with(&["a", "b", "c", "d"]);
    join(&mut s, &[1, 3]);
    assert_ok(&s);
    assert_eq!(ids(&s, W), vec![1, 3, 2, 4]);
    let p = &s.window(W).unwrap().pairs[0];
    assert_eq!(p.panes, vec![TabId(1), TabId(3)]);
    assert_eq!(p.sizes, vec![500, 500]);
    assert_eq!(p.origin, PairOrigin::Joined);
}

#[test]
fn joining_needs_two_free_tabs() {
    let mut s = store_with(&["a", "b", "c"]);
    join(&mut s, &[1, 2]);
    assert!(s
        .dispatch(
            W,
            Command::JoinPair {
                tabs: vec![TabId(1)],
                layout: PairLayout::Stacked
            }
        )
        .is_err());
    assert!(s
        .dispatch(
            W,
            Command::JoinPair {
                tabs: vec![TabId(2), TabId(3)],
                layout: PairLayout::Stacked
            }
        )
        .is_err());
    assert_ok(&s);
}

#[test]
fn a_pair_moves_as_one_and_never_straddles_a_group() {
    let mut s = store_with(&["a", "b", "c", "d"]);
    join(&mut s, &[1, 2]);
    run(
        &mut s,
        W,
        Command::Move {
            tab: TabId(2),
            index: 3,
        },
    );
    assert_ok(&s);
    assert_eq!(ids(&s, W), vec![3, 4, 1, 2]);
    // Grouping one pane takes its partner in too.
    run(
        &mut s,
        W,
        Command::CreateGroup {
            tabs: vec![TabId(2)],
            name: None,
        },
    );
    assert_ok(&s);
    let w = s.window(W).unwrap();
    assert_eq!(
        w.tab(TabId(1)).unwrap().group,
        w.tab(TabId(2)).unwrap().group
    );
    assert!(w.tab(TabId(1)).unwrap().group.is_some());
    // Joining tabs from other groups pulls them into the first pane's group.
    join(&mut s, &[3, 4]);
    run(
        &mut s,
        W,
        Command::CreateGroup {
            tabs: vec![TabId(3)],
            name: None,
        },
    );
    assert_ok(&s);
    let g = s.window(W).unwrap().tab(TabId(3)).unwrap().group;
    assert_eq!(s.window(W).unwrap().tab(TabId(4)).unwrap().group, g);
}

#[test]
fn separating_and_closing_one_half_dissolves_the_pair() {
    let mut s = store_with(&["a", "b", "c"]);
    join(&mut s, &[1, 2]);
    let id = s.window(W).unwrap().pairs[0].id;
    let out = run(&mut s, W, Command::SeparatePair { pair: id });
    assert!(out
        .events_for(W)
        .any(|e| matches!(e, SessionEvent::PairRemoved { .. })));
    assert_eq!(ids(&s, W), vec![1, 2, 3]);

    join(&mut s, &[1, 2]);
    run(&mut s, W, Command::Close { tab: TabId(2) });
    assert_ok(&s);
    assert!(s.window(W).unwrap().pairs.is_empty());
}

#[test]
fn a_third_pane_survives_losing_one() {
    let mut s = store_with(&["a", "b", "c", "d"]);
    join(&mut s, &[1, 2, 3]);
    assert_eq!(
        s.window(W).unwrap().pairs[0].sizes.iter().sum::<u32>(),
        1000
    );
    let out = run(&mut s, W, Command::Close { tab: TabId(2) });
    assert_ok(&s);
    let p = &s.window(W).unwrap().pairs[0];
    assert_eq!(p.panes, vec![TabId(1), TabId(3)]);
    assert_eq!(p.sizes, vec![500, 500]);
    assert!(out
        .events_for(W)
        .any(|e| matches!(e, SessionEvent::PairChanged { .. })));
}

#[test]
fn layout_sizes_and_swap() {
    let mut s = store_with(&["a", "b", "c"]);
    join(&mut s, &[1, 2]);
    let id = s.window(W).unwrap().pairs[0].id;
    run(
        &mut s,
        W,
        Command::SetPairLayout {
            pair: id,
            layout: PairLayout::Stacked,
        },
    );
    run(
        &mut s,
        W,
        Command::SetPairSizes {
            pair: id,
            sizes: vec![300, 700],
        },
    );
    run(&mut s, W, Command::SwapPanes { pair: id });
    assert_ok(&s);
    let p = &s.window(W).unwrap().pairs[0];
    assert_eq!(p.layout, PairLayout::Stacked);
    assert_eq!(p.panes, vec![TabId(2), TabId(1)]);
    assert_eq!(p.sizes, vec![700, 300]);
    assert_eq!(ids(&s, W), vec![2, 1, 3]);
    for bad in [vec![500], vec![0, 1000], vec![400, 400]] {
        assert!(s
            .dispatch(
                W,
                Command::SetPairSizes {
                    pair: id,
                    sizes: bad
                }
            )
            .is_err());
    }
}

#[test]
fn toggling_a_split_creates_a_second_pane_and_toggling_again_closes_it() {
    let mut s = store_with(&["a", "b"]);
    run(&mut s, W, Command::ToggleSplit { tab: TabId(1) });
    assert_ok(&s);
    assert_eq!(ids(&s, W), vec![1, 3, 2]);
    let w = s.window(W).unwrap();
    let p = &w.pairs[0];
    assert_eq!(p.origin, PairOrigin::Toggle { created: TabId(3) });
    assert_eq!(
        w.tab(TabId(3)).unwrap().location,
        w.tab(TabId(1)).unwrap().location
    );
    // Toggling from the created pane closes it and leaves the original active.
    run(&mut s, W, Command::Activate { tab: TabId(3) });
    run(&mut s, W, Command::ToggleSplit { tab: TabId(3) });
    assert_ok(&s);
    assert_eq!(ids(&s, W), vec![1, 2]);
    assert_eq!(s.window(W).unwrap().active, Some(TabId(1)));
    assert!(s.window(W).unwrap().pairs.is_empty());
    // The pane goes to Recently Closed with its history, so Reopen Closed Tab brings it back.
    assert_eq!(s.closed().len(), 1);
    assert_eq!(s.closed()[0].tab.id, TabId(3));
    run(&mut s, W, Command::Reopen { tab: None });
    assert_ok(&s);
    assert_eq!(ids(&s, W), vec![1, 3, 2]);
    assert!(s.window(W).unwrap().pairs.is_empty());
    assert_eq!(s.window(W).unwrap().active, Some(TabId(3)));
    assert!(s.closed().is_empty());
}

#[test]
fn toggling_a_joined_pair_only_separates_it() {
    let mut s = store_with(&["a", "b"]);
    join(&mut s, &[1, 2]);
    run(&mut s, W, Command::ToggleSplit { tab: TabId(2) });
    assert_eq!(ids(&s, W), vec![1, 2]);
    assert!(s.window(W).unwrap().pairs.is_empty());
}

#[test]
fn pinning_a_pair_pins_both_panes() {
    let mut s = store_with(&["a", "b", "c"]);
    join(&mut s, &[2, 3]);
    run(
        &mut s,
        W,
        Command::Pin {
            tab: TabId(3),
            pinned: true,
        },
    );
    assert_ok(&s);
    assert_eq!(ids(&s, W), vec![2, 3, 1]);
}
