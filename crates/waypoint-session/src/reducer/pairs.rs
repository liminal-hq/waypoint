// Pair commands: join, separate, layout, sizes, swap and the split toggle.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::model::{Pair, PairId, PairLayout, PairOrigin, TabId, TabSnapshot};
use crate::reducer::tabs::close_tabs;
use crate::reducer::{gather_after, unknown_tab, Command, SessionError};
use crate::store::{equal_sizes, Store};

fn pair_mut(store: &mut Store, wi: usize, pair: PairId) -> Result<&mut Pair, SessionError> {
    store.windows[wi]
        .pairs
        .iter_mut()
        .find(|p| p.id == pair)
        .ok_or(SessionError::UnknownPair(pair.0))
}

pub(crate) fn apply(store: &mut Store, window: &str, command: Command) -> Result<(), SessionError> {
    let wi = store.window_index(window)?;
    match command {
        Command::JoinPair { tabs, layout } => {
            let mut unique: Vec<TabId> = Vec::new();
            for t in &tabs {
                if !unique.contains(t) {
                    unique.push(*t);
                }
            }
            if unique.len() < 2 {
                return Err(SessionError::Invalid("a pair needs at least two tabs"));
            }
            let id = PairId(store.next_pair);
            let w = &mut store.windows[wi];
            for t in &unique {
                w.tab(*t).ok_or_else(|| unknown_tab(*t))?;
                if w.pair_of(*t).is_some() {
                    return Err(SessionError::AlreadyPaired(t.0));
                }
            }
            let (group, pinned) = {
                let lead = w.tab(unique[0]).ok_or_else(|| unknown_tab(unique[0]))?;
                (lead.group, lead.pinned)
            };
            for t in w.tabs.iter_mut().filter(|t| unique.contains(&t.id)) {
                t.group = group;
                t.pinned = pinned;
            }
            gather_after(w, unique[0], &unique[1..]);
            w.pairs.push(Pair {
                id,
                sizes: equal_sizes(unique.len()),
                panes: unique,
                layout,
                origin: PairOrigin::Joined,
            });
            store.next_pair += 1;
        }
        Command::SeparatePair { pair } => {
            pair_mut(store, wi, pair)?;
            store.windows[wi].pairs.retain(|p| p.id != pair);
        }
        Command::SetPairLayout { pair, layout } => {
            pair_mut(store, wi, pair)?.layout = layout;
        }
        Command::SetPairSizes { pair, sizes } => {
            let p = pair_mut(store, wi, pair)?;
            if sizes.len() != p.panes.len()
                || sizes.contains(&0)
                || sizes.iter().sum::<u32>() != 1000
            {
                return Err(SessionError::Invalid(
                    "pair sizes need one positive share per pane, adding up to 1000",
                ));
            }
            p.sizes = sizes;
        }
        Command::SwapPanes { pair } => {
            let p = pair_mut(store, wi, pair)?;
            p.panes.reverse();
            p.sizes.reverse();
            // The tab order follows the pane order when the state settles.
        }
        Command::ToggleSplit { tab } => toggle_split(store, wi, tab)?,
        _ => unreachable!("routed by `reduce`"),
    }
    Ok(())
}

fn toggle_split(store: &mut Store, wi: usize, tab: TabId) -> Result<(), SessionError> {
    let w = &store.windows[wi];
    let lead = w.tab(tab).ok_or_else(|| unknown_tab(tab))?;
    if let Some(pair) = w.pair_of(tab).cloned() {
        match pair.origin {
            PairOrigin::Toggle { created } if pair.panes.contains(&created) => {
                // Closing the pane toggling made leaves the other one, active if this was.
                let keep = pair.panes.iter().copied().find(|p| *p != created);
                close_tabs(store, wi, [created].into_iter().collect(), true, keep);
            }
            _ => store.windows[wi].pairs.retain(|p| p.id != pair.id),
        }
        return Ok(());
    }
    let id = TabId(store.next_tab);
    let pair_id = PairId(store.next_pair);
    let copy = TabSnapshot {
        id,
        location: lead.location.clone(),
        back: Vec::new(),
        forward: Vec::new(),
        pinned: lead.pinned,
        colour: lead.colour,
        group: lead.group,
        hints: Default::default(),
    };
    let w = &mut store.windows[wi];
    let at = w.index_of(tab).map_or(w.tabs.len(), |i| i + 1);
    w.tabs.insert(at, copy);
    w.pairs.push(Pair {
        id: pair_id,
        panes: vec![tab, id],
        layout: PairLayout::SideBySide,
        sizes: equal_sizes(2),
        origin: PairOrigin::Toggle { created: id },
    });
    store.next_tab += 1;
    store.next_pair += 1;
    Ok(())
}
