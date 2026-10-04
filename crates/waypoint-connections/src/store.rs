// The saved connections and the recent servers: one writer, a revision, and the changes each edit made
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The plugin holds one `Connections` behind its lock. Every edit checks the draft, bumps the
// revision by one and returns a `ConnectionsChanged` naming exactly the connections it added,
// changed or removed (and the order, when it moved), which the plugin sends to every window. The
// store never holds a secret, so it can be saved, exported and logged in full.

use serde::{Deserialize, Serialize};
use thiserror::Error;
use ts_rs::TS;
use waypoint_path::{ConnectionKey, RemotePath, VfsPath};

use crate::model::{
    check_draft, ConnectionDraft, ConnectionEntry, DraftError, RecentServer, SavedConnection,
};

/// The most connections that can be saved.
pub const MAX_CONNECTIONS: usize = 500;

/// The most recent servers that are remembered; the oldest go first.
pub const MAX_RECENT: usize = 10;

/// The document format this build writes and reads.
pub const CONNECTIONS_VERSION: u32 = 1;

/// The saved form: the connections in the order the Network section shows them, the recent
/// servers newest first, and the next id to hand out (so an id is never reused).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ConnectionsDocument {
    pub version: u32,
    pub next_id: u64,
    pub connections: Vec<SavedConnection>,
    pub recent: Vec<RecentServer>,
}

impl ConnectionsDocument {
    pub fn new(next_id: u64, connections: Vec<SavedConnection>, recent: Vec<RecentServer>) -> Self {
        Self {
            version: CONNECTIONS_VERSION,
            next_id,
            connections,
            recent,
        }
    }
}

/// One connection an edit touched: its entry now, or `None` when it was removed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct ConnectionChange {
    pub id: String,
    pub entry: Option<ConnectionEntry>,
}

/// Every saved connection and recent server, and the revision they are at.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct ConnectionsSnapshot {
    #[ts(type = "number")]
    pub revision: u64,
    pub connections: Vec<ConnectionEntry>,
    pub recent: Vec<RecentServer>,
}

/// Sent to every window after an edit: the revision it made, the connections it touched, the new
/// order when it changed, and the recent servers when they changed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct ConnectionsChanged {
    #[ts(type = "number")]
    pub revision: u64,
    pub changes: Vec<ConnectionChange>,
    /// The ids in order, when the order changed (an add, a removal or a move).
    pub order: Option<Vec<String>>,
    pub recent: Option<Vec<RecentServer>>,
}

/// Why an edit was refused.
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum ConnectionsError {
    #[error("{error}")]
    Draft { error: DraftError },
    #[error("there is no saved connection `{id}`")]
    NotFound { id: String },
    #[error("at most {MAX_CONNECTIONS} connections can be saved")]
    TooMany,
    #[error("`{id}` is listed more than once")]
    Duplicate { id: String },
}

impl From<DraftError> for ConnectionsError {
    fn from(error: DraftError) -> Self {
        Self::Draft { error }
    }
}

/// The saved connections. Not shared: the plugin owns one behind its writer lock.
#[derive(Debug, Clone, Default)]
pub struct Connections {
    connections: Vec<SavedConnection>,
    recent: Vec<RecentServer>,
    next_id: u64,
    revision: u64,
}

impl Connections {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            ..Self::default()
        }
    }

    /// Loads a saved document. What cannot be right is left out rather than refused: a connection
    /// whose fields no longer check, a repeated id (the first wins), a recent server that is not a
    /// server address, and anything over the bounds.
    pub fn from_document(document: ConnectionsDocument) -> Self {
        let mut connections: Vec<SavedConnection> = Vec::new();
        let mut highest = 0;
        for saved in document.connections {
            if connections.iter().any(|c| c.id == saved.id) || connections.len() >= MAX_CONNECTIONS
            {
                continue;
            }
            let Ok(checked) = check_draft(&saved.draft) else {
                log::warn!(
                    "left out the saved connection `{}`: it no longer checks",
                    saved.id
                );
                continue;
            };
            highest = highest.max(id_number(&saved.id).unwrap_or(0));
            connections.push(SavedConnection {
                id: saved.id,
                draft: checked.draft,
            });
        }
        let recent = document
            .recent
            .into_iter()
            .filter(|server| root_of(&server.key).is_some())
            .take(MAX_RECENT)
            .collect();
        Self {
            connections,
            recent,
            next_id: document.next_id.max(highest + 1).max(1),
            revision: 0,
        }
    }

    pub fn to_document(&self) -> ConnectionsDocument {
        ConnectionsDocument::new(self.next_id, self.connections.clone(), self.recent.clone())
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn len(&self) -> usize {
        self.connections.len()
    }

    pub fn is_empty(&self) -> bool {
        self.connections.is_empty()
    }

    pub fn get(&self, id: &str) -> Option<&SavedConnection> {
        self.connections.iter().find(|c| c.id == id)
    }

    pub fn connections(&self) -> &[SavedConnection] {
        &self.connections
    }

    /// The saved connection of a login, when one is saved (the first, when several are).
    pub fn for_key(&self, key: &ConnectionKey) -> Option<&SavedConnection> {
        self.connections
            .iter()
            .find(|c| c.key().as_ref() == Some(key))
    }

    pub fn snapshot(&self) -> ConnectionsSnapshot {
        ConnectionsSnapshot {
            revision: self.revision,
            connections: self
                .connections
                .iter()
                .filter_map(SavedConnection::entry)
                .collect(),
            recent: self.recent.clone(),
        }
    }

    fn order(&self) -> Vec<String> {
        self.connections.iter().map(|c| c.id.clone()).collect()
    }

    fn changed(
        &mut self,
        changes: Vec<ConnectionChange>,
        order: bool,
        recent: bool,
    ) -> ConnectionsChanged {
        self.revision += 1;
        ConnectionsChanged {
            revision: self.revision,
            changes,
            order: order.then(|| self.order()),
            recent: recent.then(|| self.recent.clone()),
        }
    }

    fn change_of(&self, id: &str) -> ConnectionChange {
        ConnectionChange {
            id: id.to_owned(),
            entry: self.get(id).and_then(SavedConnection::entry),
        }
    }

    /// Saves a new connection at the end of the list.
    pub fn add(
        &mut self,
        draft: &ConnectionDraft,
    ) -> Result<(String, ConnectionsChanged), ConnectionsError> {
        if self.connections.len() >= MAX_CONNECTIONS {
            return Err(ConnectionsError::TooMany);
        }
        let checked = check_draft(draft)?;
        let id = format!("c{}", self.next_id);
        self.next_id += 1;
        self.connections.push(SavedConnection {
            id: id.clone(),
            draft: checked.draft,
        });
        // A server saved is no longer only recent.
        let key = checked.root.connection_key();
        let before = self.recent.len();
        self.recent.retain(|server| server.key != key.as_str());
        let recent = self.recent.len() != before;
        let change = self.change_of(&id);
        Ok((id, self.changed(vec![change], true, recent)))
    }

    /// Replaces what a saved connection holds. Nothing changes, and `None` comes back, when the
    /// draft saves the same as what is there.
    pub fn update(
        &mut self,
        id: &str,
        draft: &ConnectionDraft,
    ) -> Result<Option<ConnectionsChanged>, ConnectionsError> {
        let checked = check_draft(draft)?;
        let saved = self
            .connections
            .iter_mut()
            .find(|c| c.id == id)
            .ok_or_else(|| ConnectionsError::NotFound { id: id.to_owned() })?;
        if saved.draft == checked.draft {
            return Ok(None);
        }
        saved.draft = checked.draft;
        let change = self.change_of(id);
        Ok(Some(self.changed(vec![change], false, false)))
    }

    /// Saves a copy of a connection right after it, named "… (copy)" by the caller's `name`.
    pub fn duplicate(
        &mut self,
        id: &str,
        name: impl FnOnce(&str) -> String,
    ) -> Result<(String, ConnectionsChanged), ConnectionsError> {
        if self.connections.len() >= MAX_CONNECTIONS {
            return Err(ConnectionsError::TooMany);
        }
        let at = self
            .connections
            .iter()
            .position(|c| c.id == id)
            .ok_or_else(|| ConnectionsError::NotFound { id: id.to_owned() })?;
        let original = &self.connections[at];
        let label = original
            .entry()
            .map(|entry| entry.label)
            .unwrap_or_else(|| original.draft.host.clone());
        let mut draft = original.draft.clone();
        draft.name = name(&label);
        let draft = check_draft(&draft)?.draft;
        let copy = format!("c{}", self.next_id);
        self.next_id += 1;
        self.connections.insert(
            at + 1,
            SavedConnection {
                id: copy.clone(),
                draft,
            },
        );
        let change = self.change_of(&copy);
        Ok((copy, self.changed(vec![change], true, false)))
    }

    /// Forgets a saved connection and returns what it was.
    pub fn remove(
        &mut self,
        id: &str,
    ) -> Result<(SavedConnection, ConnectionsChanged), ConnectionsError> {
        let at = self
            .connections
            .iter()
            .position(|c| c.id == id)
            .ok_or_else(|| ConnectionsError::NotFound { id: id.to_owned() })?;
        let removed = self.connections.remove(at);
        let change = ConnectionChange {
            id: id.to_owned(),
            entry: None,
        };
        Ok((removed, self.changed(vec![change], true, false)))
    }

    /// Moves a saved connection to position `to` (clamped to the end).
    pub fn move_to(
        &mut self,
        id: &str,
        to: usize,
    ) -> Result<Option<ConnectionsChanged>, ConnectionsError> {
        let at = self
            .connections
            .iter()
            .position(|c| c.id == id)
            .ok_or_else(|| ConnectionsError::NotFound { id: id.to_owned() })?;
        let to = to.min(self.connections.len() - 1);
        if to == at {
            return Ok(None);
        }
        let moved = self.connections.remove(at);
        self.connections.insert(to, moved);
        Ok(Some(self.changed(Vec::new(), true, false)))
    }

    /// Notes that `key` connected at `at_ms`. A server that is not saved goes to the top of the
    /// recent servers; a saved one changes nothing. `None` when nothing changed.
    pub fn note_connected(
        &mut self,
        key: &ConnectionKey,
        at_ms: u64,
    ) -> Option<ConnectionsChanged> {
        if self.for_key(key).is_some() {
            return None;
        }
        let root = root_of(key.as_str())?;
        self.recent.retain(|server| server.key != key.as_str());
        self.recent.insert(
            0,
            RecentServer {
                key: key.as_str().to_owned(),
                location: VfsPath::Remote(root).to_location(),
                at_ms,
            },
        );
        self.recent.truncate(MAX_RECENT);
        Some(self.changed(Vec::new(), false, true))
    }

    /// Forgets one recent server, or all of them with `None`.
    pub fn forget_recent(&mut self, key: Option<&str>) -> Option<ConnectionsChanged> {
        let before = self.recent.len();
        match key {
            Some(key) => self.recent.retain(|server| server.key != key),
            None => self.recent.clear(),
        }
        (self.recent.len() != before).then(|| self.changed(Vec::new(), false, true))
    }

    /// Puts a whole document in force (an import), as one change touching every connection that
    /// differs. `None` when nothing differs.
    pub fn replace_all(
        &mut self,
        document: ConnectionsDocument,
    ) -> Result<Option<ConnectionsChanged>, ConnectionsError> {
        let mut seen: Vec<&str> = Vec::new();
        for saved in &document.connections {
            if seen.contains(&saved.id.as_str()) {
                return Err(ConnectionsError::Duplicate {
                    id: saved.id.clone(),
                });
            }
            seen.push(&saved.id);
            check_draft(&saved.draft)?;
        }
        if document.connections.len() > MAX_CONNECTIONS {
            return Err(ConnectionsError::TooMany);
        }
        let incoming = Connections::from_document(document);
        let mut changes = Vec::new();
        for old in &self.connections {
            if incoming.get(&old.id).is_none() {
                changes.push(ConnectionChange {
                    id: old.id.clone(),
                    entry: None,
                });
            }
        }
        for new in &incoming.connections {
            if self.get(&new.id) != Some(new) {
                changes.push(ConnectionChange {
                    id: new.id.clone(),
                    entry: new.entry(),
                });
            }
        }
        let order = self.order() != incoming.order();
        let recent = self.recent != incoming.recent;
        if changes.is_empty() && !order && !recent {
            return Ok(None);
        }
        self.connections = incoming.connections;
        self.recent = incoming.recent;
        self.next_id = self.next_id.max(incoming.next_id);
        Ok(Some(self.changed(changes, order, recent)))
    }
}

fn id_number(id: &str) -> Option<u64> {
    id.strip_prefix('c')?.parse().ok()
}

/// The root of a login's server, when `key` is a server address.
pub(crate) fn root_of(key: &str) -> Option<RemotePath> {
    match VfsPath::from_uri(&format!("{key}/")) {
        Ok(VfsPath::Remote(root)) if root.connection_key().as_str() == key => Some(root),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft(host: &str) -> ConnectionDraft {
        ConnectionDraft {
            scheme: "sftp".into(),
            host: host.into(),
            user: Some("me".into()),
            ..ConnectionDraft::default()
        }
    }

    #[test]
    fn edits_bump_the_revision_and_name_what_they_touched() {
        let mut store = Connections::new();
        let (a, added) = store.add(&draft("a.lan")).unwrap();
        assert_eq!((a.as_str(), added.revision), ("c1", 1));
        assert_eq!(added.order, Some(vec!["c1".to_owned()]));
        assert_eq!(
            added.changes[0].entry.as_ref().unwrap().key,
            "sftp://me@a.lan"
        );

        let (b, _) = store.add(&draft("b.lan")).unwrap();
        let updated = store
            .update(
                &a,
                &ConnectionDraft {
                    name: "Work".into(),
                    ..draft("a.lan")
                },
            )
            .unwrap()
            .unwrap();
        assert_eq!(updated.revision, 3);
        assert_eq!(updated.order, None);
        assert_eq!(updated.changes[0].entry.as_ref().unwrap().label, "Work");
        assert_eq!(
            store
                .update(
                    &a,
                    &ConnectionDraft {
                        name: " Work ".into(),
                        ..draft("A.LAN")
                    }
                )
                .unwrap(),
            None,
            "the same connection written differently changes nothing"
        );

        let moved = store.move_to(&b, 0).unwrap().unwrap();
        assert_eq!(moved.order, Some(vec![b.clone(), a.clone()]));
        assert_eq!(store.move_to(&b, 0).unwrap(), None);

        let (copy, duplicated) = store
            .duplicate(&a, |name| format!("{name} (copy)"))
            .unwrap();
        assert_eq!(copy, "c3");
        assert_eq!(
            duplicated.order,
            Some(vec![b.clone(), a.clone(), copy.clone()])
        );
        assert_eq!(store.get(&copy).unwrap().draft.name, "Work (copy)");

        let (removed, change) = store.remove(&a).unwrap();
        assert_eq!(removed.draft.name, "Work");
        assert_eq!(change.changes[0].entry, None);
        assert!(matches!(
            store.remove(&a),
            Err(ConnectionsError::NotFound { .. })
        ));
        // Ids are never reused, even after a removal and a reload.
        let reloaded = Connections::from_document(store.to_document());
        let mut reloaded = reloaded;
        let (next, _) = reloaded.add(&draft("c.lan")).unwrap();
        assert_eq!(next, "c4");
    }

    #[test]
    fn a_bad_draft_is_refused_with_its_field() {
        let mut store = Connections::new();
        assert_eq!(
            store.add(&draft("")).unwrap_err(),
            ConnectionsError::Draft {
                error: DraftError::Host
            }
        );
        assert_eq!(store.revision(), 0);
    }

    #[test]
    fn recent_servers_are_unsaved_newest_first_and_bounded() {
        let mut store = Connections::new();
        let key = |host: &str| check_draft(&draft(host)).unwrap().root.connection_key();
        for n in 0..12 {
            store.note_connected(&key(&format!("h{n}.lan")), n).unwrap();
        }
        let recent = store.snapshot().recent;
        assert_eq!(recent.len(), MAX_RECENT);
        assert_eq!(recent[0].key, "sftp://me@h11.lan");
        assert_eq!(recent[0].location.uri, "sftp://me@h11.lan/");
        // Connecting again moves it to the top without repeating it.
        store.note_connected(&key("h5.lan"), 99).unwrap();
        let recent = store.snapshot().recent;
        assert_eq!(recent[0].key, "sftp://me@h5.lan");
        assert_eq!(
            recent
                .iter()
                .filter(|s| s.key == "sftp://me@h5.lan")
                .count(),
            1
        );
        // Saving a server takes it out of the recent ones, and a saved one is never added.
        let (_, change) = store.add(&draft("h5.lan")).unwrap();
        assert!(change.recent.is_some());
        assert!(store.note_connected(&key("h5.lan"), 100).is_none());
        assert!(store.forget_recent(Some("sftp://me@h11.lan")).is_some());
        assert!(store.forget_recent(None).is_some());
        assert!(store.forget_recent(None).is_none());
    }

    #[test]
    fn loading_leaves_out_what_cannot_be_right() {
        let good = SavedConnection {
            id: "c7".into(),
            draft: draft("a.lan"),
        };
        let document = ConnectionsDocument {
            version: 1,
            next_id: 2,
            connections: vec![
                good.clone(),
                SavedConnection {
                    id: "c7".into(),
                    draft: draft("b.lan"),
                },
                SavedConnection {
                    id: "c8".into(),
                    draft: draft(""),
                },
            ],
            recent: vec![RecentServer {
                key: "not a server".into(),
                location: good.entry().unwrap().location,
                at_ms: 0,
            }],
        };
        let mut store = Connections::from_document(document);
        assert_eq!(store.len(), 1);
        assert!(store.snapshot().recent.is_empty());
        let (next, _) = store.add(&draft("c.lan")).unwrap();
        assert_eq!(next, "c8", "the next id is past every id loaded");
    }

    #[test]
    fn replacing_everything_reports_each_difference_once() {
        let mut store = Connections::new();
        let (a, _) = store.add(&draft("a.lan")).unwrap();
        let (b, _) = store.add(&draft("b.lan")).unwrap();
        let mut document = store.to_document();
        document.connections.remove(0);
        document.connections[0].draft.name = "B".into();
        let change = store.replace_all(document.clone()).unwrap().unwrap();
        assert_eq!(change.changes.len(), 2);
        assert_eq!(change.changes[0].id, a);
        assert_eq!(change.changes[0].entry, None);
        assert_eq!(change.changes[1].id, b);
        assert_eq!(change.order, Some(vec![b.clone()]));
        assert_eq!(store.replace_all(document.clone()).unwrap(), None);
        document.connections.push(document.connections[0].clone());
        assert!(matches!(
            store.replace_all(document),
            Err(ConnectionsError::Duplicate { .. })
        ));
    }
}
