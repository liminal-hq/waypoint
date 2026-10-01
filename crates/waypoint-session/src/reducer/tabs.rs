// Tab commands: open, close, activate, move, navigate, pin, colour, hints and reopen.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::HashSet;

use crate::model::{ClosedTab, TabId, TabSnapshot};
use crate::reducer::{move_block, remove_tabs, unknown_tab, Command, Facts, SessionError};
use crate::store::{Store, CLOSED_LIMIT};

pub(crate) fn apply(
    store: &mut Store,
    window: &str,
    command: Command,
    facts: &mut Facts,
) -> Result<(), SessionError> {
    let wi = store.window_index(window)?;
    match command {
        Command::Open {
            location,
            after,
            activate,
        } => {
            let id = TabId(store.next_tab);
            let w = &mut store.windows[wi];
            let (index, group, pinned) = match after {
                Some(after) => {
                    let anchor = w.tab(after).ok_or_else(|| unknown_tab(after))?;
                    let end = w
                        .unit(after)
                        .iter()
                        .filter_map(|u| w.index_of(*u))
                        .max()
                        .unwrap_or(0);
                    // A pinned tab outside any group does not pass its pin to a new tab.
                    let group = anchor.group;
                    (end + 1, group, group.is_some() && anchor.pinned)
                }
                None => (w.tabs.len(), None, false),
            };
            w.tabs.insert(
                index,
                TabSnapshot {
                    id,
                    location,
                    back: Vec::new(),
                    forward: Vec::new(),
                    pinned,
                    colour: None,
                    group,
                    hints: Default::default(),
                },
            );
            if activate || w.active.is_none() {
                w.active = Some(id);
            }
            store.next_tab += 1;
        }
        Command::Close { tab } => {
            if store.windows[wi].index_of(tab).is_none() {
                return Err(unknown_tab(tab));
            }
            close_tabs(store, wi, [tab].into_iter().collect(), true, None);
        }
        Command::Activate { tab } => {
            let w = &mut store.windows[wi];
            if w.index_of(tab).is_none() {
                return Err(unknown_tab(tab));
            }
            if w.active != Some(tab) {
                w.active = Some(tab);
                w.mru.retain(|t| *t != tab);
                w.mru.insert(0, tab);
            }
        }
        Command::Move { tab, index } => {
            let w = &mut store.windows[wi];
            let group = w.tab(tab).ok_or_else(|| unknown_tab(tab))?.group;
            let unit = w.unit(tab);
            move_block(w, &unit, index, group);
        }
        Command::Navigate { tab, location } => {
            let t = tab_mut(store, wi, tab)?;
            if t.location != location {
                let previous = std::mem::replace(&mut t.location, location);
                t.back.push(previous);
                t.forward.clear();
            }
        }
        Command::Back { tab } => {
            let t = tab_mut(store, wi, tab)?;
            if let Some(previous) = t.back.pop() {
                let current = std::mem::replace(&mut t.location, previous);
                t.forward.push(current);
            }
        }
        Command::Forward { tab } => {
            let t = tab_mut(store, wi, tab)?;
            if let Some(following) = t.forward.pop() {
                let current = std::mem::replace(&mut t.location, following);
                t.back.push(current);
            }
        }
        Command::Pin { tab, pinned } => {
            let w = &mut store.windows[wi];
            let group = w.tab(tab).ok_or_else(|| unknown_tab(tab))?.group;
            let set: HashSet<TabId> = match group {
                Some(g) => w.group_tabs(g).into_iter().collect(),
                None => w.unit(tab).into_iter().collect(),
            };
            for t in w.tabs.iter_mut().filter(|t| set.contains(&t.id)) {
                t.pinned = pinned;
            }
        }
        Command::SetColour { tab, colour } => {
            tab_mut(store, wi, tab)?.colour = colour;
        }
        Command::SetHints { tab, hints } => {
            tab_mut(store, wi, tab)?.hints = hints;
        }
        Command::Reopen { tab } => reopen(store, wi, tab, facts)?,
        _ => unreachable!("routed by `reduce`"),
    }
    Ok(())
}

fn tab_mut(store: &mut Store, wi: usize, tab: TabId) -> Result<&mut TabSnapshot, SessionError> {
    store.windows[wi]
        .tabs
        .iter_mut()
        .find(|t| t.id == tab)
        .ok_or_else(|| unknown_tab(tab))
}

/// Removes tabs from a window, recording them as closed when `record` is set, and closes the
/// window when it is left empty and the policy says so.
pub(crate) fn close_tabs(
    store: &mut Store,
    wi: usize,
    ids: HashSet<TabId>,
    record: bool,
    prefer: Option<TabId>,
) {
    let label = store.windows[wi].label.clone();
    let removed = remove_tabs(&mut store.windows[wi], &ids, prefer);
    if record {
        for r in removed {
            store.closed.insert(
                0,
                ClosedTab {
                    tab: r.tab,
                    window: label.clone(),
                    index: r.index as u32,
                },
            );
        }
        store.closed.truncate(CLOSED_LIMIT);
    }
    if store.windows[wi].tabs.is_empty() && store.policy.close_window_on_last_tab {
        store.windows.remove(wi);
    }
}

fn reopen(
    store: &mut Store,
    wi: usize,
    tab: Option<TabId>,
    facts: &mut Facts,
) -> Result<(), SessionError> {
    let position = match tab {
        None if store.closed.is_empty() => return Ok(()),
        None => 0,
        Some(id) => store
            .closed
            .iter()
            .position(|c| c.tab.id == id)
            .ok_or_else(|| unknown_tab(id))?,
    };
    let entry = store.closed.remove(position);
    let wi = store.window_index(&entry.window).unwrap_or(wi);
    let w = &mut store.windows[wi];
    let mut tab = entry.tab;
    let index = (entry.index as usize).min(w.tabs.len());
    // The old group is kept only when the tab goes back inside or next to its run.
    tab.group = tab.group.filter(|g| {
        let members: Vec<usize> = w
            .tabs
            .iter()
            .enumerate()
            .filter(|(_, t)| t.group == Some(*g))
            .map(|(i, _)| i)
            .collect();
        match (members.first(), members.last()) {
            (Some(first), Some(last)) => {
                *first <= index && index <= last + 1 && w.tabs[*first].pinned == tab.pinned
            }
            _ => false,
        }
    });
    let id = tab.id;
    w.tabs.insert(index, tab);
    w.active = Some(id);
    w.mru.retain(|t| *t != id);
    w.mru.insert(0, id);
    facts.reopened = Some(id);
    Ok(())
}
