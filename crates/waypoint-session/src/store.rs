// The store: every window's state under one revision, one writer and one set of id counters.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::model::{
    ClosedTab, SessionEvent, SessionSnapshot, StoreSnapshot, TabId, ViewPrefs, WindowEvent,
    WindowState, WindowSummary, Workspace,
};
use crate::reducer::{reduce, Command, SessionError};

/// How many closed tabs the store remembers.
pub const CLOSED_LIMIT: usize = 10;

/// Knobs that differ between the app and the milestone 2 compatibility view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StorePolicy {
    /// Closing a window's last tab closes the window (the app's rule). When off, the window stays
    /// and is empty, which is what the milestone 2 per-window `Session` did.
    pub close_window_on_last_tab: bool,
}

impl Default for StorePolicy {
    fn default() -> Self {
        Self {
            close_window_on_last_tab: true,
        }
    }
}

/// What one command did: the events in revision order and what happened to the set of windows.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Outcome {
    pub events: Vec<WindowEvent>,
    /// The store had windows before the command and has none now: the app quits.
    pub last_window_closed: bool,
}

impl Outcome {
    pub fn windows_opened(&self) -> Vec<String> {
        self.events
            .iter()
            .filter_map(|e| match &e.event {
                SessionEvent::WindowOpened { window, .. } => Some(window.clone()),
                _ => None,
            })
            .collect()
    }

    pub fn windows_closed(&self) -> Vec<String> {
        self.events
            .iter()
            .filter_map(|e| match &e.event {
                SessionEvent::WindowClosed { window, .. } => Some(window.clone()),
                _ => None,
            })
            .collect()
    }

    /// The events that belong to one window, in order.
    pub fn events_for<'a>(&'a self, window: &'a str) -> impl Iterator<Item = &'a SessionEvent> {
        self.events
            .iter()
            .filter(move |e| e.window == window)
            .map(|e| &e.event)
    }
}

/// Every window of the app. Plain data with one writer: `dispatch` applies a command to a copy and
/// replaces the state only when it succeeds, so an error changes nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Store {
    pub(crate) revision: u64,
    pub(crate) windows: Vec<WindowState>,
    /// Newest first, at most `CLOSED_LIMIT`.
    pub(crate) closed: Vec<ClosedTab>,
    pub(crate) next_tab: u32,
    pub(crate) next_group: u32,
    pub(crate) next_pair: u32,
    pub(crate) next_window: u32,
    /// Global, in creation order.
    pub(crate) workspaces: Vec<Workspace>,
    pub(crate) next_workspace: u32,
    pub(crate) policy: StorePolicy,
    /// The view a window made from now on starts with (the Settings window's choice). Not part of
    /// the document: windows already in the store keep their own.
    pub(crate) new_window_view: ViewPrefs,
}

impl Default for Store {
    fn default() -> Self {
        Self::new()
    }
}

impl Store {
    /// An empty store: no windows, so the first `OpenWindow` makes `main-1`.
    pub fn new() -> Self {
        Self::with_policy(StorePolicy::default())
    }

    pub fn with_policy(policy: StorePolicy) -> Self {
        Self {
            revision: 0,
            windows: Vec::new(),
            closed: Vec::new(),
            next_tab: 1,
            next_group: 1,
            next_pair: 1,
            next_window: 1,
            workspaces: Vec::new(),
            next_workspace: 1,
            policy,
            new_window_view: ViewPrefs::default(),
        }
    }

    pub fn policy(&self) -> StorePolicy {
        self.policy
    }

    /// Changes the policy, for a store restored from a document (which carries none).
    pub fn set_policy(&mut self, policy: StorePolicy) {
        self.policy = policy;
    }

    /// Sets the view that windows made from now on start with. It changes no window that exists,
    /// is not a mutation (no revision, no event) and is not saved: the app sets it from its
    /// settings at start-up and whenever they change.
    pub fn set_new_window_view(&mut self, view: ViewPrefs) {
        self.new_window_view = view;
    }

    pub fn new_window_view(&self) -> ViewPrefs {
        self.new_window_view
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn windows(&self) -> &[WindowState] {
        &self.windows
    }

    pub fn window(&self, label: &str) -> Option<&WindowState> {
        self.windows.iter().find(|w| w.label == label)
    }

    pub(crate) fn window_index(&self, label: &str) -> Result<usize, SessionError> {
        self.windows
            .iter()
            .position(|w| w.label == label)
            .ok_or_else(|| SessionError::UnknownWindow(label.to_string()))
    }

    /// Every window as a menu lists it, in the order they were opened; `caller` is marked.
    pub fn window_summaries(&self, caller: &str) -> Vec<WindowSummary> {
        self.windows
            .iter()
            .map(|w| w.summary(w.label == caller))
            .collect()
    }

    /// Recently closed tabs, newest first.
    pub fn closed(&self) -> &[ClosedTab] {
        &self.closed
    }

    /// Every workspace, in creation order.
    pub fn workspaces(&self) -> &[Workspace] {
        &self.workspaces
    }

    /// The window `tab` is in.
    pub fn window_of(&self, tab: TabId) -> Option<&WindowState> {
        self.windows.iter().find(|w| w.index_of(tab).is_some())
    }

    /// One window's session at the current revision, or `None` for an unknown label.
    pub fn snapshot(&self, label: &str) -> Option<SessionSnapshot> {
        let w = self.window(label)?;
        Some(SessionSnapshot {
            revision: self.revision,
            tabs: w.tabs.clone(),
            active: w.active,
            mru: w.mru.clone(),
            groups: w.groups.clone(),
            pairs: w.pairs.clone(),
            geometry: w.geometry,
            view: w.view,
            closed: self.closed.clone(),
            workspaces: self.workspaces.clone(),
            workspace: w.workspace,
        })
    }

    /// Applies a command, with `window` as the window it is for (the caller's window; a command
    /// that makes a window ignores it). On an error the store is unchanged. A command that
    /// changes nothing returns no events and keeps the revision.
    pub fn dispatch(&mut self, window: &str, command: Command) -> Result<Outcome, SessionError> {
        let (next, outcome) = reduce(self, window, command)?;
        *self = next;
        Ok(outcome)
    }

    /// The whole store as plain data, histories untouched.
    pub fn to_snapshot(&self) -> StoreSnapshot {
        StoreSnapshot {
            revision: self.revision,
            windows: self.windows.clone(),
            closed: self.closed.clone(),
            next_tab: self.next_tab,
            next_group: self.next_group,
            next_pair: self.next_pair,
            next_window: self.next_window,
            workspaces: self.workspaces.clone(),
            next_workspace: self.next_workspace,
        }
    }

    /// Everything wrong with the store, as readable lines; empty when every invariant holds.
    pub fn violations(&self) -> Vec<String> {
        let mut out = Vec::new();
        let mut labels = std::collections::HashSet::new();
        let mut tabs = std::collections::HashSet::new();
        let mut groups = std::collections::HashSet::new();
        let mut pairs = std::collections::HashSet::new();
        for w in &self.windows {
            if !labels.insert(w.label.as_str()) {
                out.push(format!("{}: window label is not unique", w.label));
            }
            out.extend(w.violations());
            for t in &w.tabs {
                if !tabs.insert(t.id) {
                    out.push(format!("tab {} is in two windows", t.id.0));
                }
                if t.id.0 >= self.next_tab {
                    out.push(format!("tab {} is at or above next_tab", t.id.0));
                }
            }
            for g in &w.groups {
                if !groups.insert(g.id) {
                    out.push(format!("group {} is in two windows", g.id.0));
                }
                if g.id.0 >= self.next_group {
                    out.push(format!("group {} is at or above next_group", g.id.0));
                }
            }
            for p in &w.pairs {
                if !pairs.insert(p.id) {
                    out.push(format!("pair {} is in two windows", p.id.0));
                }
                if p.id.0 >= self.next_pair {
                    out.push(format!("pair {} is at or above next_pair", p.id.0));
                }
            }
        }
        let mut workspace_ids = std::collections::HashSet::new();
        let mut workspace_names = std::collections::HashSet::new();
        for ws in &self.workspaces {
            if !workspace_ids.insert(ws.id) {
                out.push(format!("workspace {} is not unique", ws.id.0));
            }
            if ws.id.0 >= self.next_workspace {
                out.push(format!(
                    "workspace {} is at or above next_workspace",
                    ws.id.0
                ));
            }
            if !workspace_names.insert(workspace_key(&ws.name)) || ws.name.trim().is_empty() {
                out.push(format!(
                    "workspace {} has an empty or repeated name",
                    ws.id.0
                ));
            }
            let mut uris = std::collections::HashSet::new();
            if !ws.locations.iter().all(|l| uris.insert(l.uri.as_str())) {
                out.push(format!("workspace {} repeats a folder", ws.id.0));
            }
        }
        for w in &self.windows {
            if w.workspace.is_some_and(|id| !workspace_ids.contains(&id)) {
                out.push(format!("{}: the active workspace does not exist", w.label));
            }
        }
        if self.closed.len() > CLOSED_LIMIT {
            out.push("too many closed tabs".to_string());
        }
        for c in &self.closed {
            if tabs.contains(&c.tab.id) {
                out.push(format!("closed tab {} is also open", c.tab.id.0));
            }
            if c.tab.id.0 >= self.next_tab {
                out.push(format!("closed tab {} is at or above next_tab", c.tab.id.0));
            }
        }
        out
    }

    /// Brings every window into a legal shape after a command: prunes what dangles, then orders.
    pub(crate) fn settle(&mut self) {
        let workspace_ids: std::collections::HashSet<_> =
            self.workspaces.iter().map(|w| w.id).collect();
        for w in &mut self.windows {
            if w.workspace.is_some_and(|id| !workspace_ids.contains(&id)) {
                w.workspace = None;
            }
        }
        for w in &mut self.windows {
            let live: std::collections::HashSet<TabId> = w.tabs.iter().map(|t| t.id).collect();
            // Panes that no longer exist leave their pair; a pair of fewer than two is gone.
            for p in &mut w.pairs {
                let before = p.panes.len();
                let keep: Vec<bool> = p.panes.iter().map(|id| live.contains(id)).collect();
                if keep.iter().any(|k| !k) {
                    let mut it = keep.iter();
                    p.panes.retain(|_| *it.next().unwrap_or(&true));
                    p.sizes = equal_sizes(p.panes.len());
                }
                debug_assert!(p.panes.len() <= before);
            }
            w.pairs.retain(|p| p.panes.len() >= 2);
            let group_ids: std::collections::HashSet<_> = w.groups.iter().map(|g| g.id).collect();
            for t in &mut w.tabs {
                if t.group.is_some_and(|g| !group_ids.contains(&g)) {
                    t.group = None;
                }
            }
            let used: std::collections::HashSet<_> =
                w.tabs.iter().filter_map(|t| t.group).collect();
            w.groups.retain(|g| used.contains(&g.id));
            w.mru.retain(|id| live.contains(id));
            if w.active.is_none_or(|a| !live.contains(&a)) {
                w.active = w.tabs.first().map(|t| t.id);
            }
            let order = w.canonical_order();
            if order.iter().ne(w.tabs.iter().map(|t| &t.id)) {
                let mut taken: Vec<Option<_>> =
                    std::mem::take(&mut w.tabs).into_iter().map(Some).collect();
                let old: Vec<TabId> = taken.iter().flatten().map(|t| t.id).collect();
                w.tabs = order
                    .iter()
                    .filter_map(|id| {
                        let at = old.iter().position(|o| o == id)?;
                        taken[at].take()
                    })
                    .collect();
            }
        }
    }
}

/// `n` equal shares of 1000, the remainder going to the first panes.
pub(crate) fn equal_sizes(n: usize) -> Vec<u32> {
    if n == 0 {
        return Vec::new();
    }
    let base = 1000 / n as u32;
    let extra = (1000 % n as u32) as usize;
    (0..n).map(|i| base + u32::from(i < extra)).collect()
}

/// What makes two workspace names the same: they match ignoring case and surrounding space.
pub(crate) fn workspace_key(name: &str) -> String {
    name.trim().to_lowercase()
}
