// The pure reducer: a command applied to a store gives the new store and the events it made.
// One dispatcher, `reduce`, hands each command to the module for its concern.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::HashSet;

use thiserror::Error;
use waypoint_protocol::Location;

use crate::diff::store_events;
use crate::model::{
    Geometry, GroupId, PairId, PairLayout, SessionEvent, SessionSnapshot, TabColour, TabHints,
    TabId, ViewPrefs, WindowState,
};
use crate::store::{Outcome, Store, StorePolicy, CLOSED_LIMIT};

mod groups;
mod pairs;
mod tabs;
mod windows;

/// How a group's tabs are ordered by `SortGroup`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupSort {
    /// By the last part of the location's display path, ignoring case.
    Name,
    /// By the whole location URI.
    Location,
    /// Local (`file://`) locations first, then the rest, each by name.
    LocalFirst,
}

/// What `MoveTabs` moves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MoveWhat {
    /// These tabs. A pair or group whose members all travel keeps its identity; otherwise the
    /// tabs that travel leave it.
    Tabs(Vec<TabId>),
    /// A group and every tab in it.
    Group(GroupId),
    /// A pair's panes.
    Pair(PairId),
}

/// Where `MoveTabs` puts them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MoveTo {
    /// Another existing window, at `index` in its tab order (clamped).
    ExistingWindow { label: String, index: usize },
    /// A window made for the move. `label` `None` lets the store allocate `main-{n}`.
    NewWindow {
        label: Option<String>,
        geometry: Option<Geometry>,
    },
}

/// What a caller can ask of the store. Commands name tabs, groups and pairs by id; the window a
/// command is for is the `window` given to `dispatch`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    // Tabs.
    /// Opens a tab at `location`, after tab `after` (and after its pair, joining its group) or at
    /// the end. The first tab of an empty window is always activated, whatever `activate` says.
    Open {
        location: Location,
        after: Option<TabId>,
        activate: bool,
    },
    /// Closes a tab and records it in the closed list. Closing the active tab activates the first
    /// live entry of the MRU list, or else the tab that takes its place (the one before it when it
    /// was last). Closing a window's last tab closes the window.
    Close {
        tab: TabId,
    },
    /// Activates a tab and moves it to the front of the MRU list. Activating the active tab does
    /// nothing.
    Activate {
        tab: TabId,
    },
    /// Moves a tab (with its pair) to `index` in the display order. A tab in a group stays in it;
    /// a tab outside any group lands next to a group rather than inside it; pinned tabs stay in
    /// front of unpinned ones.
    Move {
        tab: TabId,
        index: usize,
    },
    /// Goes somewhere new: the current location joins the back stack and the forward stack clears.
    Navigate {
        tab: TabId,
        location: Location,
    },
    Back {
        tab: TabId,
    },
    Forward {
        tab: TabId,
    },
    /// Pins or unpins a tab. A tab in a pair or group carries the whole pair or group with it.
    Pin {
        tab: TabId,
        pinned: bool,
    },
    SetColour {
        tab: TabId,
        colour: Option<TabColour>,
    },
    SetHints {
        tab: TabId,
        hints: TabHints,
    },
    /// Brings a closed tab back (the newest one when `tab` is `None`) at its old index in the
    /// window it was closed in, or in this window when that window is gone, and activates it.
    Reopen {
        tab: Option<TabId>,
    },

    // Groups.
    /// Groups the given tabs (and the rest of their pairs) at the position of the first of them.
    CreateGroup {
        tabs: Vec<TabId>,
        name: Option<String>,
    },
    /// Moves a tab (with its pair) to the end of a group and into it.
    AddToGroup {
        tab: TabId,
        group: GroupId,
    },
    /// Takes a tab (with its pair) out of its group; it stays at the group's edge or lands after it.
    RemoveFromGroup {
        tab: TabId,
    },
    RenameGroup {
        group: GroupId,
        name: String,
    },
    SetGroupColour {
        group: GroupId,
        colour: Option<TabColour>,
    },
    SetGroupCollapsed {
        group: GroupId,
        collapsed: bool,
    },
    /// Collapses every other group in the window.
    CollapseOthers {
        group: GroupId,
    },
    SortGroup {
        group: GroupId,
        by: GroupSort,
    },
    /// Copies a group's tabs (new ids, same locations and history) into a new group after it.
    DuplicateGroup {
        group: GroupId,
    },
    /// Moves a group to `index` (the position of its first tab) in the display order.
    MoveGroup {
        group: GroupId,
        index: usize,
    },
    /// Dissolves a group; its tabs stay where they are.
    Ungroup {
        group: GroupId,
    },
    /// Closes every tab in a group.
    CloseGroup {
        group: GroupId,
    },

    // Pairs.
    /// Pairs two or more tabs, none already in a pair, next to the first; they join its group.
    JoinPair {
        tabs: Vec<TabId>,
        layout: PairLayout,
    },
    SeparatePair {
        pair: PairId,
    },
    SetPairLayout {
        pair: PairId,
        layout: PairLayout,
    },
    /// Sets the share of each pane, in thousandths adding up to 1000.
    SetPairSizes {
        pair: PairId,
        sizes: Vec<u32>,
    },
    /// Reverses the order of the panes (with their sizes).
    SwapPanes {
        pair: PairId,
    },
    /// Splits a tab into a pair with a new tab at the same location, or undoes such a split by
    /// closing the tab it made; a pair made by joining is only separated.
    ToggleSplit {
        tab: TabId,
    },

    // Windows.
    /// Makes a window `main-{n}`, with a first tab at `location` when given.
    OpenWindow {
        location: Option<Location>,
        geometry: Option<Geometry>,
    },
    /// Closes the window; its tabs go to the closed list.
    CloseWindow,
    SetGeometry {
        geometry: Geometry,
    },
    SetView {
        view: ViewPrefs,
    },
    /// Moves tabs to another window in one step: ids, history, pin, colour, group, pair and hints
    /// all travel. The source sees `TabClosed` and the target `TabOpened`.
    MoveTabs {
        what: MoveWhat,
        to: MoveTo,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SessionError {
    #[error("no such tab: {0}")]
    UnknownTab(u32),
    #[error("no such group: {0}")]
    UnknownGroup(u32),
    #[error("no such pair: {0}")]
    UnknownPair(u32),
    #[error("no such window: {0}")]
    UnknownWindow(String),
    #[error("tab {0} is already in a pair")]
    AlreadyPaired(u32),
    #[error("invalid command: {0}")]
    Invalid(&'static str),
}

/// Facts a command passes to event generation that the state alone does not show.
#[derive(Default)]
pub(crate) struct Facts {
    pub(crate) reopened: Option<TabId>,
}

/// Applies a command to a copy of the store and returns it with the events the change made.
/// Changing nothing makes no events and keeps the revision; an error leaves the input untouched.
pub(crate) fn reduce(
    store: &Store,
    window: &str,
    command: Command,
) -> Result<(Store, Outcome), SessionError> {
    let mut next = store.clone();
    let mut facts = Facts::default();
    match command {
        Command::Open { .. }
        | Command::Close { .. }
        | Command::Activate { .. }
        | Command::Move { .. }
        | Command::Navigate { .. }
        | Command::Back { .. }
        | Command::Forward { .. }
        | Command::Pin { .. }
        | Command::SetColour { .. }
        | Command::SetHints { .. }
        | Command::Reopen { .. } => tabs::apply(&mut next, window, command, &mut facts)?,
        Command::CreateGroup { .. }
        | Command::AddToGroup { .. }
        | Command::RemoveFromGroup { .. }
        | Command::RenameGroup { .. }
        | Command::SetGroupColour { .. }
        | Command::SetGroupCollapsed { .. }
        | Command::CollapseOthers { .. }
        | Command::SortGroup { .. }
        | Command::DuplicateGroup { .. }
        | Command::MoveGroup { .. }
        | Command::Ungroup { .. }
        | Command::CloseGroup { .. } => groups::apply(&mut next, window, command)?,
        Command::JoinPair { .. }
        | Command::SeparatePair { .. }
        | Command::SetPairLayout { .. }
        | Command::SetPairSizes { .. }
        | Command::SwapPanes { .. }
        | Command::ToggleSplit { .. } => pairs::apply(&mut next, window, command)?,
        Command::OpenWindow { .. }
        | Command::CloseWindow
        | Command::SetGeometry { .. }
        | Command::SetView { .. }
        | Command::MoveTabs { .. } => windows::apply(&mut next, window, command)?,
    }
    next.settle();

    let mut events = store_events(store, &next, facts.reopened);
    for event in &mut events {
        next.revision += 1;
        event.event.set_revision(next.revision);
    }
    let last_window_closed = !store.windows.is_empty() && next.windows.is_empty();
    Ok((
        next,
        Outcome {
            events,
            last_window_closed,
        },
    ))
}

/// The pure form of the reducer over one window's snapshot, as milestone 2 had it: the new
/// snapshot and the events for that window. The snapshot becomes a one-window store that keeps an
/// empty window when its last tab closes. Ids are allocated above every id in the snapshot, so
/// replaying from a snapshot is deterministic.
pub fn apply(
    snapshot: &SessionSnapshot,
    command: Command,
) -> Result<(SessionSnapshot, Vec<SessionEvent>), SessionError> {
    reject_multi_window(&command)?;
    let mut store = store_from_snapshot(snapshot);
    let outcome = store.dispatch(COMPAT_WINDOW, command)?;
    let events = outcome
        .events_for(COMPAT_WINDOW)
        .cloned()
        .collect::<Vec<_>>();
    let next = store
        .snapshot(COMPAT_WINDOW)
        .ok_or_else(|| SessionError::UnknownWindow(COMPAT_WINDOW.to_string()))?;
    Ok((next, events))
}

/// The one-window view cannot express the window commands: closing its window would leave no
/// snapshot to read, and moving tabs away (or opening another window) changes state the view never
/// shows, with no close events for the tabs that left. They belong to `Store`, so the view rejects
/// them before anything changes.
pub(crate) fn reject_multi_window(command: &Command) -> Result<(), SessionError> {
    match command {
        Command::CloseWindow | Command::OpenWindow { .. } | Command::MoveTabs { .. } => Err(
            SessionError::Invalid("a single-window session cannot change its windows"),
        ),
        _ => Ok(()),
    }
}

/// The label of the single window behind `apply` and `Session`.
pub(crate) const COMPAT_WINDOW: &str = "main-1";

pub(crate) fn store_from_snapshot(snapshot: &SessionSnapshot) -> Store {
    let mut store = Store::with_policy(StorePolicy {
        close_window_on_last_tab: false,
    });
    store.revision = snapshot.revision;
    store.next_window = 2;
    store.closed = snapshot.closed.clone();
    store.closed.truncate(CLOSED_LIMIT);
    store.windows.push(WindowState {
        label: COMPAT_WINDOW.to_string(),
        tabs: snapshot.tabs.clone(),
        active: snapshot.active,
        mru: snapshot.mru.clone(),
        groups: snapshot.groups.clone(),
        pairs: snapshot.pairs.clone(),
        geometry: snapshot.geometry,
        view: snapshot.view,
    });
    let max = |it: &mut dyn Iterator<Item = u32>| it.max().unwrap_or(0) + 1;
    store.next_tab = max(&mut snapshot
        .tabs
        .iter()
        .map(|t| t.id.0)
        .chain(snapshot.closed.iter().map(|c| c.tab.id.0)));
    store.next_group = max(&mut snapshot.groups.iter().map(|g| g.id.0));
    store.next_pair = max(&mut snapshot.pairs.iter().map(|p| p.id.0));
    store
}

// Helpers the command modules share.

pub(crate) fn unknown_tab(tab: TabId) -> SessionError {
    SessionError::UnknownTab(tab.0)
}

/// The tabs of `ids` and the rest of their pairs, in display order, each once.
pub(crate) fn expand_units(w: &WindowState, ids: &[TabId]) -> Result<Vec<TabId>, SessionError> {
    let mut wanted: HashSet<TabId> = HashSet::new();
    for id in ids {
        if w.index_of(*id).is_none() {
            return Err(unknown_tab(*id));
        }
        wanted.extend(w.unit(*id));
    }
    Ok(w.tabs
        .iter()
        .map(|t| t.id)
        .filter(|id| wanted.contains(id))
        .collect())
}

/// What leaving the window did to the active tab and the list of closed tabs.
pub(crate) struct Removed {
    pub(crate) tab: crate::model::TabSnapshot,
    pub(crate) index: usize,
}

/// Takes tabs out of a window and keeps its active tab and MRU list valid: when the active tab
/// goes, `prefer` (when it stays) or the first live MRU entry takes over, or else the tab that
/// takes its place, or the one before it. Returns what was removed, in display order.
pub(crate) fn remove_tabs(
    w: &mut WindowState,
    ids: &HashSet<TabId>,
    prefer: Option<TabId>,
) -> Vec<Removed> {
    let active_index = w.active.and_then(|a| w.index_of(a));
    let active_goes = w.active.is_some_and(|a| ids.contains(&a));
    let mut removed = Vec::new();
    let mut kept = Vec::with_capacity(w.tabs.len());
    let mut before_active = 0;
    for (index, tab) in std::mem::take(&mut w.tabs).into_iter().enumerate() {
        if ids.contains(&tab.id) {
            removed.push(Removed { tab, index });
        } else {
            if active_index.is_some_and(|a| index < a) {
                before_active += 1;
            }
            kept.push(tab);
        }
    }
    w.tabs = kept;
    w.mru.retain(|id| !ids.contains(id));
    if active_goes {
        let live = |id: &TabId| w.index_of(*id).is_some();
        w.active = prefer
            .filter(live)
            .or_else(|| w.mru.first().copied())
            .or_else(|| {
                w.tabs
                    .get(before_active)
                    .or_else(|| before_active.checked_sub(1).and_then(|i| w.tabs.get(i)))
                    .map(|t| t.id)
            });
    }
    removed
}

/// Moves `block` (tabs in the order they should land) so its first tab sits at `index` among the
/// remaining tabs (clamped). When `within` names a group the block stays inside that group's run.
pub(crate) fn move_block(
    w: &mut WindowState,
    block: &[TabId],
    index: usize,
    within: Option<GroupId>,
) {
    let wanted: HashSet<TabId> = block.iter().copied().collect();
    let mut moving = Vec::new();
    let mut rest = Vec::new();
    for tab in std::mem::take(&mut w.tabs) {
        if wanted.contains(&tab.id) {
            moving.push(tab);
        } else {
            rest.push(tab);
        }
    }
    moving.sort_by_key(|t| block.iter().position(|b| *b == t.id));
    let mut at = index.min(rest.len());
    if let Some(group) = within {
        let positions: Vec<usize> = rest
            .iter()
            .enumerate()
            .filter(|(_, t)| t.group == Some(group))
            .map(|(i, _)| i)
            .collect();
        if let (Some(first), Some(last)) = (positions.first(), positions.last()) {
            at = at.clamp(*first, last + 1);
        }
    }
    rest.splice(at..at, moving);
    w.tabs = rest;
}

/// Puts `ids` (in that order) right after `anchor`, which stays where it is.
pub(crate) fn gather_after(w: &mut WindowState, anchor: TabId, ids: &[TabId]) {
    let wanted: HashSet<TabId> = ids.iter().copied().collect();
    let mut moving = Vec::new();
    let mut rest = Vec::new();
    for tab in std::mem::take(&mut w.tabs) {
        if wanted.contains(&tab.id) && tab.id != anchor {
            moving.push(tab);
        } else {
            rest.push(tab);
        }
    }
    moving.sort_by_key(|t| ids.iter().position(|b| *b == t.id));
    let at = rest
        .iter()
        .position(|t| t.id == anchor)
        .map_or(rest.len(), |i| i + 1);
    rest.splice(at..at, moving);
    w.tabs = rest;
}
