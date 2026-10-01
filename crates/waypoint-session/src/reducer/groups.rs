// Group commands: create, add, remove, rename, colour, collapse, sort, duplicate, move, ungroup
// and close.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::{HashMap, HashSet};

use crate::model::{Group, GroupId, Pair, PairId, TabId, TabSnapshot, WindowState};
use crate::reducer::tabs::close_tabs;
use crate::reducer::{
    expand_units, gather_after, move_block, unknown_tab, Command, GroupSort, SessionError,
};
use crate::store::Store;

fn group_exists(w: &WindowState, group: GroupId) -> Result<(), SessionError> {
    w.group(group)
        .map(|_| ())
        .ok_or(SessionError::UnknownGroup(group.0))
}

/// "Group N" for a group made without a name: N counts from the window's groups, and skips any
/// number a group in this window already carries. (The group's id is global to the session, so it
/// would number a window's first group "Group 7".)
fn default_group_name(w: &WindowState) -> String {
    let mut n = w.groups.len() + 1;
    while w.groups.iter().any(|g| g.name == format!("Group {n}")) {
        n += 1;
    }
    format!("Group {n}")
}

pub(crate) fn apply(store: &mut Store, window: &str, command: Command) -> Result<(), SessionError> {
    let wi = store.window_index(window)?;
    match command {
        Command::CreateGroup { tabs, name } => {
            if tabs.is_empty() {
                return Err(SessionError::Invalid("a group needs at least one tab"));
            }
            let id = GroupId(store.next_group);
            let w = &mut store.windows[wi];
            let members = expand_units(w, &tabs)?;
            let pinned = w.tab(members[0]).is_some_and(|t| t.pinned);
            for t in w.tabs.iter_mut().filter(|t| members.contains(&t.id)) {
                t.group = Some(id);
                t.pinned = pinned;
            }
            let name = name.unwrap_or_else(|| default_group_name(w));
            w.groups.push(Group {
                id,
                name,
                colour: None,
                collapsed: false,
            });
            store.next_group += 1;
        }
        Command::AddToGroup { tab, group } => {
            let w = &mut store.windows[wi];
            let lead = w.tab(tab).ok_or_else(|| unknown_tab(tab))?;
            group_exists(w, group)?;
            if lead.group == Some(group) {
                return Ok(());
            }
            let unit = w.unit(tab);
            let members = w.group_tabs(group);
            let pinned = members
                .first()
                .and_then(|m| w.tab(*m))
                .is_some_and(|t| t.pinned);
            for t in w.tabs.iter_mut().filter(|t| unit.contains(&t.id)) {
                t.group = Some(group);
                t.pinned = pinned;
            }
            if let Some(last) = members.last() {
                gather_after(w, *last, &unit);
            }
        }
        Command::RemoveFromGroup { tab } => {
            let w = &mut store.windows[wi];
            w.tab(tab).ok_or_else(|| unknown_tab(tab))?;
            let unit = w.unit(tab);
            for t in w.tabs.iter_mut().filter(|t| unit.contains(&t.id)) {
                t.group = None;
            }
        }
        Command::RenameGroup { group, name } => {
            group_mut(store, wi, group)?.name = name;
        }
        Command::SetGroupColour { group, colour } => {
            group_mut(store, wi, group)?.colour = colour;
        }
        Command::SetGroupCollapsed { group, collapsed } => {
            group_mut(store, wi, group)?.collapsed = collapsed;
        }
        Command::CollapseOthers { group } => {
            let w = &mut store.windows[wi];
            group_exists(w, group)?;
            for g in w.groups.iter_mut().filter(|g| g.id != group) {
                g.collapsed = true;
            }
        }
        Command::SortGroup { group, by } => {
            let w = &mut store.windows[wi];
            group_exists(w, group)?;
            sort_group(w, group, by);
        }
        Command::DuplicateGroup { group } => duplicate(store, wi, group)?,
        Command::MoveGroup { group, index } => {
            let w = &mut store.windows[wi];
            group_exists(w, group)?;
            let members = w.group_tabs(group);
            move_block(w, &members, index, None);
        }
        Command::Ungroup { group } => {
            let w = &mut store.windows[wi];
            group_exists(w, group)?;
            for t in w.tabs.iter_mut().filter(|t| t.group == Some(group)) {
                t.group = None;
            }
        }
        Command::CloseGroup { group } => {
            let w = &store.windows[wi];
            group_exists(w, group)?;
            let ids: HashSet<TabId> = w.group_tabs(group).into_iter().collect();
            close_tabs(store, wi, ids, true, None);
        }
        _ => unreachable!("routed by `reduce`"),
    }
    Ok(())
}

fn group_mut(store: &mut Store, wi: usize, group: GroupId) -> Result<&mut Group, SessionError> {
    store.windows[wi]
        .groups
        .iter_mut()
        .find(|g| g.id == group)
        .ok_or(SessionError::UnknownGroup(group.0))
}

fn sort_key(tab: &TabSnapshot, by: GroupSort) -> (bool, String) {
    let name = || {
        let display = tab.location.display.trim_end_matches(['/', '\\']);
        display
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or(display)
            .to_lowercase()
    };
    match by {
        GroupSort::Name => (false, name()),
        GroupSort::Location => (false, tab.location.uri.clone()),
        GroupSort::LocalFirst => (!tab.location.uri.starts_with("file://"), name()),
    }
}

/// Reorders a group's tabs in place, keeping each pair's panes together (a pair sorts by its
/// first pane).
fn sort_group(w: &mut WindowState, group: GroupId, by: GroupSort) {
    let members = w.group_tabs(group);
    let mut units: Vec<Vec<TabId>> = Vec::new();
    let mut seen: HashSet<TabId> = HashSet::new();
    for id in &members {
        if seen.contains(id) {
            continue;
        }
        let unit: Vec<TabId> = w.unit(*id);
        seen.extend(unit.iter().copied());
        units.push(unit);
    }
    units.sort_by_cached_key(|u| w.tab(u[0]).map(|t| sort_key(t, by)).unwrap_or_default());
    let order: Vec<TabId> = units.into_iter().flatten().collect();
    let first = w.index_of(members[0]).unwrap_or(0);
    move_block(w, &order, first, None);
}

fn duplicate(store: &mut Store, wi: usize, group: GroupId) -> Result<(), SessionError> {
    group_exists(&store.windows[wi], group)?;
    let new_group = GroupId(store.next_group);
    store.next_group += 1;
    let members = store.windows[wi].group_tabs(group);
    let mut ids: HashMap<TabId, TabId> = HashMap::new();
    for m in &members {
        ids.insert(*m, TabId(store.next_tab));
        store.next_tab += 1;
    }
    let first_pair = store.next_pair;
    let w = &mut store.windows[wi];
    let copies: Vec<TabSnapshot> = members
        .iter()
        .filter_map(|m| w.tab(*m))
        .map(|t| TabSnapshot {
            id: ids[&t.id],
            group: Some(new_group),
            ..t.clone()
        })
        .collect();
    let mut pairs: Vec<Pair> = Vec::new();
    let mut seen: HashSet<PairId> = HashSet::new();
    for m in &members {
        if let Some(p) = w.pair_of(*m) {
            if seen.insert(p.id) {
                let id = PairId(first_pair + pairs.len() as u32);
                pairs.push(Pair {
                    id,
                    panes: p.panes.iter().filter_map(|x| ids.get(x).copied()).collect(),
                    origin: crate::model::PairOrigin::Joined,
                    ..p.clone()
                });
            }
        }
    }
    store.next_pair += pairs.len() as u32;
    let source = w.group(group).cloned();
    if let Some(source) = source {
        w.groups.push(Group {
            id: new_group,
            collapsed: false,
            ..source
        });
    }
    w.pairs.extend(pairs);
    let end = members
        .last()
        .and_then(|m| w.index_of(*m))
        .map_or(w.tabs.len(), |i| i + 1);
    w.tabs.splice(end..end, copies);
    Ok(())
}
