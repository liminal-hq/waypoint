// Window commands: open, close, geometry, view and the atomic hand-off of tabs between windows.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::HashSet;

use waypoint_protocol::WindowKind;

use crate::model::{ClosedTab, Geometry, GroupId, PairId, TabId, TabSnapshot, WindowState};
use crate::reducer::{
    expand_units, remove_tabs, unknown_tab, Command, MoveTo, MoveWhat, SessionError,
};
use crate::store::{Store, CLOSED_LIMIT};

pub(crate) fn apply(store: &mut Store, window: &str, command: Command) -> Result<(), SessionError> {
    match command {
        Command::OpenWindow { location, geometry } => {
            let label = new_window(store, None, geometry)?;
            if let Some(location) = location {
                let wi = store.window_index(&label)?;
                let id = TabId(store.next_tab);
                store.next_tab += 1;
                let w = &mut store.windows[wi];
                w.tabs.push(TabSnapshot {
                    id,
                    location,
                    back: Vec::new(),
                    forward: Vec::new(),
                    pinned: false,
                    colour: None,
                    group: None,
                    hints: Default::default(),
                });
                w.active = Some(id);
            }
        }
        Command::RegisterWindow { label } => {
            if WindowKind::from_label(&label) != Some(WindowKind::Main) {
                return Err(SessionError::Invalid("not a main window label"));
            }
            new_window(store, Some(label), None)?;
        }
        Command::CloseWindow => {
            let wi = store.window_index(window)?;
            let w = store.windows.remove(wi);
            for (index, tab) in w.tabs.into_iter().enumerate() {
                store.closed.insert(
                    0,
                    ClosedTab {
                        tab,
                        window: w.label.clone(),
                        index: index as u32,
                    },
                );
            }
            store.closed.truncate(CLOSED_LIMIT);
        }
        Command::SetGeometry { geometry } => {
            let wi = store.window_index(window)?;
            store.windows[wi].geometry = Some(geometry);
        }
        Command::SetView { view } => {
            let wi = store.window_index(window)?;
            store.windows[wi].view = view;
        }
        Command::MoveTabs { what, to } => move_tabs(store, window, what, to)?,
        _ => unreachable!("routed by `reduce`"),
    }
    Ok(())
}

/// Adds an empty window under `label`, or the next `main-{n}`, and returns its label.
fn new_window(
    store: &mut Store,
    label: Option<String>,
    geometry: Option<Geometry>,
) -> Result<String, SessionError> {
    let label = match label {
        Some(label) => {
            if store.window(&label).is_some() {
                return Err(SessionError::Invalid("a window with that label exists"));
            }
            if let Some(n) = label
                .strip_prefix("main-")
                .and_then(|n| n.parse::<u32>().ok())
            {
                store.next_window = store.next_window.max(n + 1);
            }
            label
        }
        None => {
            let label = format!("main-{}", store.next_window);
            store.next_window += 1;
            label
        }
    };
    let mut w = WindowState::new(label.clone());
    w.geometry = geometry;
    store.windows.push(w);
    Ok(label)
}

fn move_tabs(
    store: &mut Store,
    window: &str,
    what: MoveWhat,
    to: MoveTo,
) -> Result<(), SessionError> {
    let wi = store.window_index(window)?;
    let (moving, active_moves): (Vec<TabId>, bool) = {
        let w = &store.windows[wi];
        let ids = match &what {
            MoveWhat::Tabs(tabs) => {
                if tabs.is_empty() {
                    return Err(SessionError::Invalid("nothing to move"));
                }
                for t in tabs {
                    w.tab(*t).ok_or_else(|| unknown_tab(*t))?;
                }
                let set: HashSet<TabId> = tabs.iter().copied().collect();
                w.tabs
                    .iter()
                    .map(|t| t.id)
                    .filter(|id| set.contains(id))
                    .collect()
            }
            MoveWhat::Group(g) => {
                w.group(*g).ok_or(SessionError::UnknownGroup(g.0))?;
                w.group_tabs(*g)
            }
            MoveWhat::Pair(p) => {
                let pair = w.pair(*p).ok_or(SessionError::UnknownPair(p.0))?;
                expand_units(w, &pair.panes)?
            }
        };
        let active = w.active.is_some_and(|a| ids.contains(&a));
        (ids, active)
    };
    let set: HashSet<TabId> = moving.iter().copied().collect();

    // Resolve the target before changing anything, so an error leaves the store as it was.
    let target_label = match &to {
        MoveTo::ExistingWindow { label, .. } => {
            if label == window {
                return Err(SessionError::Invalid("the tabs are already in that window"));
            }
            store.window_index(label)?;
            label.clone()
        }
        MoveTo::NewWindow { label, geometry } => new_window(store, label.clone(), *geometry)?,
    };

    // Groups and pairs whose members all travel keep their identity.
    let (groups, pairs, source_active) = {
        let w = &mut store.windows[wi];
        let source_active = w.active;
        let whole_groups: Vec<GroupId> = w
            .groups
            .iter()
            .map(|g| g.id)
            .filter(|g| {
                let members = w.group_tabs(*g);
                !members.is_empty() && members.iter().all(|m| set.contains(m))
            })
            .collect();
        let whole_pairs: Vec<PairId> = w
            .pairs
            .iter()
            .filter(|p| p.panes.iter().all(|m| set.contains(m)))
            .map(|p| p.id)
            .collect();
        let groups: Vec<_> = w
            .groups
            .iter()
            .filter(|g| whole_groups.contains(&g.id))
            .cloned()
            .collect();
        let pairs: Vec<_> = w
            .pairs
            .iter()
            .filter(|p| whole_pairs.contains(&p.id))
            .cloned()
            .collect();
        w.groups.retain(|g| !whole_groups.contains(&g.id));
        w.pairs.retain(|p| !whole_pairs.contains(&p.id));
        (groups, pairs, source_active)
    };
    let removed = remove_tabs(&mut store.windows[wi], &set, None);
    let mut tabs: Vec<TabSnapshot> = removed.into_iter().map(|r| r.tab).collect();
    for t in &mut tabs {
        if t.group.is_some_and(|g| !groups.iter().any(|x| x.id == g)) {
            t.group = None;
        }
    }
    if store.windows[wi].tabs.is_empty() && store.policy.close_window_on_last_tab {
        store.windows.remove(wi);
    }

    let ti = store.window_index(&target_label)?;
    let t = &mut store.windows[ti];
    let index = match to {
        MoveTo::ExistingWindow { index, .. } => index.min(t.tabs.len()),
        MoveTo::NewWindow { .. } => 0,
    };
    let first = tabs.first().map(|x| x.id);
    t.tabs.splice(index..index, tabs);
    t.groups.extend(groups);
    t.pairs.extend(pairs);
    if t.active.is_none() {
        t.active = if active_moves { source_active } else { first };
    } else if active_moves {
        t.active = source_active;
    }
    Ok(())
}
