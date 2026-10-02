// The persisted form of the store: a versioned document, the checks and repairs that run when it
// loads, and the storage trait the app implements.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use ts_rs::TS;

use crate::model::{ShelfItem, StoreSnapshot, TabId, TabSnapshot, WindowState, Workspace};
use crate::store::{equal_sizes, workspace_key, Store, StorePolicy, CLOSED_LIMIT, SHELF_LIMIT};
use waypoint_protocol::WindowKind;

/// The document format this build writes and reads.
pub const DOCUMENT_VERSION: u32 = 1;

/// Back and forward history kept per tab when saving.
const HISTORY_LIMIT: usize = 100;

/// The store as a file: a version so a later build can migrate, and the body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct Document {
    pub version: u32,
    pub body: StoreSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DocumentError {
    #[error("the document is version {found}, this build reads version {supported}")]
    UnsupportedVersion { found: u32, supported: u32 },
    #[error("the document cannot be repaired: {0}")]
    Corrupt(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum StorageError {
    #[error("storage failed: {0}")]
    Io(String),
    /// The stored bytes are not a document; the app keeps them aside and tries another copy.
    #[error("the stored session is unreadable: {0}")]
    Unreadable(String),
}

/// Where the document lives. The app implements it over its key-value store; this crate only
/// defines the seam.
pub trait SessionStorage: Send + Sync {
    /// The saved document, or `None` when nothing was saved yet.
    fn load(&self) -> Result<Option<Document>, StorageError>;
    fn save(&self, document: &Document) -> Result<(), StorageError>;
}

fn cap<T: Clone>(items: &[T]) -> Vec<T> {
    items[items.len().saturating_sub(HISTORY_LIMIT)..].to_vec()
}

fn cap_tab(tab: &mut TabSnapshot) {
    tab.back = cap(&tab.back);
    tab.forward = cap(&tab.forward);
}

impl Store {
    /// The store as a document. Each tab keeps its latest 100 back and 100 forward entries.
    pub fn to_document(&self) -> Document {
        let mut body = self.to_snapshot();
        for w in &mut body.windows {
            w.tabs.iter_mut().for_each(cap_tab);
        }
        body.closed.iter_mut().for_each(|c| cap_tab(&mut c.tab));
        Document {
            version: DOCUMENT_VERSION,
            body,
        }
    }

    /// Restores a store from a document, repairing what can be repaired and reporting each
    /// repair. A document of another version, or one that still breaks an invariant after
    /// repair, is an error.
    pub fn from_document(document: Document) -> Result<(Store, Vec<String>), DocumentError> {
        if document.version != DOCUMENT_VERSION {
            return Err(DocumentError::UnsupportedVersion {
                found: document.version,
                supported: DOCUMENT_VERSION,
            });
        }
        let body = document.body;
        let mut notes = Vec::new();
        let mut store = Store::with_policy(StorePolicy::default());
        store.revision = body.revision;
        store.next_tab = body.next_tab.max(1);
        store.next_group = body.next_group.max(1);
        store.next_pair = body.next_pair.max(1);
        store.next_window = body.next_window.max(1);
        store.next_workspace = body.next_workspace.max(1);
        repair_workspaces(&mut store, body.workspaces, &mut notes);
        store.next_shelf = body.next_shelf.max(1);
        repair_shelf(&mut store, body.shelf, &mut notes);

        let mut labels: HashSet<String> = HashSet::new();
        let mut tabs: HashSet<TabId> = HashSet::new();
        let mut groups = HashSet::new();
        let mut pairs = HashSet::new();
        for mut w in body.windows {
            if WindowKind::from_label(&w.label) != Some(WindowKind::Main) {
                return Err(DocumentError::Corrupt(format!(
                    "{} is not a main window label",
                    w.label
                )));
            }
            if !labels.insert(w.label.clone()) {
                notes.push(format!("dropped a second window labelled {}", w.label));
                continue;
            }
            repair_window(&mut w, &mut tabs, &mut groups, &mut pairs, &mut notes);
            if w.workspace
                .is_some_and(|id| store.workspaces.iter().all(|x| x.id != id))
            {
                w.workspace = None;
                notes.push(format!("{}: its active workspace is gone", w.label));
            }
            if w.tabs.is_empty() {
                notes.push(format!("dropped the empty window {}", w.label));
                continue;
            }
            store.windows.push(w);
        }
        for mut c in body.closed {
            if tabs.insert(c.tab.id) {
                cap_tab(&mut c.tab);
                c.tab.group = None;
                store.closed.push(c);
            } else {
                notes.push(format!(
                    "dropped closed tab {} that is also open",
                    c.tab.id.0
                ));
            }
        }
        store.closed.truncate(CLOSED_LIMIT);
        raise_counters(&mut store);
        let problems = store.violations();
        if let Some(first) = problems.first() {
            return Err(DocumentError::Corrupt(first.clone()));
        }
        Ok((store, notes))
    }
}

fn repair_window(
    w: &mut WindowState,
    tabs: &mut HashSet<TabId>,
    groups: &mut HashSet<crate::model::GroupId>,
    pairs: &mut HashSet<crate::model::PairId>,
    notes: &mut Vec<String>,
) {
    let before = w.tabs.len();
    w.tabs.retain(|t| tabs.insert(t.id));
    if w.tabs.len() != before {
        notes.push(format!("{}: dropped tabs with repeated ids", w.label));
    }
    w.tabs.iter_mut().for_each(cap_tab);
    w.groups.retain(|g| groups.insert(g.id));
    w.pairs.retain(|p| pairs.insert(p.id));

    // Pairs: only panes that exist and are in no earlier pair; a pair keeps its first pane's
    // group and pin state.
    let mut paired: HashSet<TabId> = HashSet::new();
    for p in &mut w.pairs {
        let panes = std::mem::take(&mut p.panes);
        let live: Vec<TabId> = panes
            .iter()
            .copied()
            .filter(|id| w.tabs.iter().any(|t| t.id == *id) && paired.insert(*id))
            .collect();
        if live.len() != panes.len() {
            notes.push(format!("{}: pair {} lost panes", w.label, p.id.0));
        }
        p.panes = live;
        if p.sizes.len() != p.panes.len() || p.sizes.iter().sum::<u32>() != 1000 {
            p.sizes = equal_sizes(p.panes.len());
        }
    }
    w.pairs.retain(|p| p.panes.len() >= 2);
    let lead_state: Vec<(Vec<TabId>, Option<crate::model::GroupId>, bool)> = w
        .pairs
        .iter()
        .map(|p| {
            let lead = w.tabs.iter().find(|t| t.id == p.panes[0]);
            (
                p.panes.clone(),
                lead.and_then(|t| t.group),
                lead.is_some_and(|t| t.pinned),
            )
        })
        .collect();
    for (panes, group, pinned) in lead_state {
        for t in w.tabs.iter_mut().filter(|t| panes.contains(&t.id)) {
            t.group = group;
            t.pinned = pinned;
        }
    }

    // Groups: dangling references go, each group takes its first member's pin state, and a group
    // without members is dropped.
    let known: HashSet<_> = w.groups.iter().map(|g| g.id).collect();
    for t in &mut w.tabs {
        if t.group.is_some_and(|g| !known.contains(&g)) {
            t.group = None;
            notes.push(format!("{}: tab {} left a missing group", w.label, t.id.0));
        }
    }
    let group_ids: Vec<_> = w.groups.iter().map(|g| g.id).collect();
    for g in group_ids {
        let pinned = w.tabs.iter().find(|t| t.group == Some(g)).map(|t| t.pinned);
        if let Some(pinned) = pinned {
            for t in w.tabs.iter_mut().filter(|t| t.group == Some(g)) {
                t.pinned = pinned;
            }
        }
    }
    let used: HashSet<_> = w.tabs.iter().filter_map(|t| t.group).collect();
    w.groups.retain(|g| used.contains(&g.id));

    let live: HashSet<TabId> = w.tabs.iter().map(|t| t.id).collect();
    let mut seen = HashSet::new();
    w.mru.retain(|id| live.contains(id) && seen.insert(*id));
    if w.active.is_none_or(|a| !live.contains(&a)) {
        w.active = w.tabs.first().map(|t| t.id);
    }
    let order = w.canonical_order();
    let mut rest = std::mem::take(&mut w.tabs);
    w.tabs = order
        .iter()
        .filter_map(|id| {
            let at = rest.iter().position(|t| t.id == *id)?;
            Some(rest.remove(at))
        })
        .collect();
}

/// Keeps the workspaces that have an id and a name nobody else has, each folder once.
fn repair_workspaces(store: &mut Store, workspaces: Vec<Workspace>, notes: &mut Vec<String>) {
    let mut ids = HashSet::new();
    let mut names = HashSet::new();
    for mut ws in workspaces {
        ws.name = ws.name.trim().to_string();
        if ws.name.is_empty() || !ids.insert(ws.id) || !names.insert(workspace_key(&ws.name)) {
            notes.push(format!(
                "dropped workspace {}: no name, or a repeat",
                ws.id.0
            ));
            continue;
        }
        let mut uris = HashSet::new();
        ws.locations.retain(|l| uris.insert(l.uri.clone()));
        store.workspaces.push(ws);
    }
}

/// Keeps the Shelf items that have an id and a location nobody else has, up to the limit.
fn repair_shelf(store: &mut Store, shelf: Vec<ShelfItem>, notes: &mut Vec<String>) {
    let mut ids = HashSet::new();
    let mut uris = HashSet::new();
    for item in shelf {
        if !ids.insert(item.id) || !uris.insert(item.location.uri.clone()) {
            notes.push(format!("dropped Shelf item {}: a repeat", item.id.0));
        } else if store.shelf.len() >= SHELF_LIMIT {
            notes.push(format!("dropped Shelf item {}: past the limit", item.id.0));
        } else {
            store.shelf.push(item);
        }
    }
}

/// Makes every counter exceed every id in use, so new ids never collide with restored ones.
fn raise_counters(store: &mut Store) {
    let mut tab = 0;
    let mut group = 0;
    let mut pair = 0;
    let mut window = 0;
    for w in &store.windows {
        tab = tab.max(w.tabs.iter().map(|t| t.id.0).max().unwrap_or(0));
        group = group.max(w.groups.iter().map(|g| g.id.0).max().unwrap_or(0));
        pair = pair.max(w.pairs.iter().map(|p| p.id.0).max().unwrap_or(0));
        if let Some(n) = w
            .label
            .strip_prefix("main-")
            .and_then(|n| n.parse::<u32>().ok())
        {
            window = window.max(n);
        }
    }
    tab = tab.max(store.closed.iter().map(|c| c.tab.id.0).max().unwrap_or(0));
    store.next_tab = store.next_tab.max(tab + 1);
    store.next_group = store.next_group.max(group + 1);
    store.next_pair = store.next_pair.max(pair + 1);
    store.next_window = store.next_window.max(window + 1);
    let workspace = store.workspaces.iter().map(|w| w.id.0).max().unwrap_or(0);
    store.next_workspace = store.next_workspace.max(workspace + 1);
    let shelf = store.shelf.iter().map(|i| i.id.0).max().unwrap_or(0);
    store.next_shelf = store.next_shelf.max(shelf + 1);
}
