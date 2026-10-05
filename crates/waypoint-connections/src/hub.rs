// The one writer of the saved connections: edits, saving, change events, and the manager beside them
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The vfs plugin holds one `ConnectionsHub`. Every edit goes through it: the store changes under
// its lock, the document is saved, and the change goes to the windows (the sink) and to whatever
// the app wired to the saved connections (observers: the SSH options of a saved host, a provider's
// tuning). It holds no Tauri types, so all of it is tested headlessly.

use std::sync::{Arc, Mutex, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

use waypoint_path::ConnectionKey;
use waypoint_protocol::VfsError;
use waypoint_vfs::{CancelToken, ConnectAnswer};

use crate::credentials::KeyringUnavailable;
use crate::manager::{ConnectionManager, Remembered};
use crate::model::{ConnectionDraft, ConnectionEntry, SavedConnection};
use crate::storage::ConnectionStorage;
use crate::store::{
    root_of, Connections, ConnectionsChanged, ConnectionsDocument, ConnectionsError,
    ConnectionsSnapshot,
};

type ChangeSink = Arc<dyn Fn(&ConnectionsChanged) + Send + Sync>;
type Observer = Arc<dyn Fn(&[SavedConnection]) + Send + Sync>;

/// What a saved connection adds to the SSH options of its host: the key file and the jump host
/// the Connect dialog set. `~/.ssh/config` still applies to everything else.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HostOverride {
    pub key_file: Option<String>,
    pub jump_host: Option<String>,
}

/// The saved connections, their storage and the connection manager.
pub struct ConnectionsHub {
    store: Mutex<Connections>,
    storage: Arc<dyn ConnectionStorage>,
    manager: Arc<ConnectionManager>,
    sink: RwLock<Option<ChangeSink>>,
    observers: RwLock<Vec<Observer>>,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

impl ConnectionsHub {
    /// Loads what `storage` holds. A document that cannot be read leaves the list empty, with a
    /// warning: saved connections must never stop the app starting.
    pub fn new(storage: Arc<dyn ConnectionStorage>, manager: Arc<ConnectionManager>) -> Self {
        let store = match storage.load() {
            Ok(Some(document)) => Connections::from_document(document),
            Ok(None) => Connections::new(),
            Err(error) => {
                log::warn!("could not read the saved connections: {error}");
                Connections::new()
            }
        };
        Self {
            store: Mutex::new(store),
            storage,
            manager,
            sink: RwLock::new(None),
            observers: RwLock::new(Vec::new()),
        }
    }

    pub fn manager(&self) -> &Arc<ConnectionManager> {
        &self.manager
    }

    /// Where change events go (the plugin's event to every window).
    pub fn set_sink(&self, sink: impl Fn(&ConnectionsChanged) + Send + Sync + 'static) {
        *self.sink.write().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(sink));
    }

    /// Calls `observer` with the saved connections now and after every change.
    pub fn observe(&self, observer: impl Fn(&[SavedConnection]) + Send + Sync + 'static) {
        let observer: Observer = Arc::new(observer);
        observer(self.lock().connections());
        self.observers
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .push(observer);
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Connections> {
        self.store.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Saves, then tells the windows and the observers. Called with the store's lock released.
    fn commit(
        &self,
        change: ConnectionsChanged,
        document: ConnectionsDocument,
        saved: Vec<SavedConnection>,
    ) {
        if let Err(error) = self.storage.save(&document) {
            log::warn!("could not save the connections: {error}");
        }
        let sink = self.sink.read().unwrap_or_else(|e| e.into_inner()).clone();
        if let Some(sink) = sink {
            sink(&change);
        }
        let observers = self
            .observers
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        for observer in observers {
            observer(&saved);
        }
    }

    /// Runs one edit under the lock and commits what it changed.
    fn edit<T>(
        &self,
        apply: impl FnOnce(
            &mut Connections,
        ) -> Result<(T, Option<ConnectionsChanged>), ConnectionsError>,
    ) -> Result<T, ConnectionsError> {
        let (value, committed) = {
            let mut store = self.lock();
            let (value, change) = apply(&mut store)?;
            let committed =
                change.map(|change| (change, store.to_document(), store.connections().to_vec()));
            (value, committed)
        };
        if let Some((change, document, saved)) = committed {
            self.commit(change, document, saved);
        }
        Ok(value)
    }

    pub fn snapshot(&self) -> ConnectionsSnapshot {
        self.lock().snapshot()
    }

    /// The document as it is exported.
    pub fn export(&self) -> ConnectionsDocument {
        self.lock().to_document()
    }

    /// A probe of the store, for planning an import without touching it.
    pub fn probe(&self) -> Connections {
        self.lock().clone()
    }

    pub fn get(&self, id: &str) -> Option<ConnectionEntry> {
        self.lock().get(id).and_then(SavedConnection::entry)
    }

    /// The saved connection of a login, when one is saved.
    pub fn for_key(&self, key: &ConnectionKey) -> Option<ConnectionEntry> {
        self.lock().for_key(key).and_then(SavedConnection::entry)
    }

    pub fn add(&self, draft: &ConnectionDraft) -> Result<ConnectionEntry, ConnectionsError> {
        self.edit(|store| {
            let (id, change) = store.add(draft)?;
            let entry = store.get(&id).and_then(SavedConnection::entry);
            Ok((entry, Some(change)))
        })?
        .ok_or(ConnectionsError::NotFound { id: String::new() })
    }

    pub fn update(
        &self,
        id: &str,
        draft: &ConnectionDraft,
    ) -> Result<ConnectionEntry, ConnectionsError> {
        self.edit(|store| {
            let change = store.update(id, draft)?;
            let entry = store.get(id).and_then(SavedConnection::entry);
            Ok((entry, change))
        })?
        .ok_or_else(|| ConnectionsError::NotFound { id: id.to_owned() })
    }

    /// Saves a copy of a connection after it; `name` words the copy's name from the original's.
    pub fn duplicate(
        &self,
        id: &str,
        name: impl FnOnce(&str) -> String,
    ) -> Result<ConnectionEntry, ConnectionsError> {
        self.edit(|store| {
            let (copy, change) = store.duplicate(id, name)?;
            let entry = store.get(&copy).and_then(SavedConnection::entry);
            Ok((entry, Some(change)))
        })?
        .ok_or_else(|| ConnectionsError::NotFound { id: id.to_owned() })
    }

    /// Forgets a saved connection. With `forget_secrets` the keyring's secrets of its login go
    /// too, unless another saved connection uses the same login; the session's answers always
    /// stay until the app quits or the login is forgotten.
    pub fn remove(
        &self,
        id: &str,
        forget_secrets: bool,
    ) -> Result<Option<KeyringUnavailable>, ConnectionsError> {
        let (removed, still_used) = self.edit(|store| {
            let (removed, change) = store.remove(id)?;
            let key = removed.key();
            let still_used = key.as_ref().is_some_and(|key| store.for_key(key).is_some());
            Ok(((removed, still_used), Some(change)))
        })?;
        if !forget_secrets || still_used {
            return Ok(None);
        }
        Ok(removed.key().and_then(|key| self.forget_login(&key).err()))
    }

    /// Forgets every remembered secret of a login, in the keyring and in this session.
    pub fn forget_login(&self, key: &ConnectionKey) -> Result<usize, KeyringUnavailable> {
        self.manager.credentials().forget_session(key);
        self.manager.credentials().keyring().forget(key)
    }

    pub fn move_to(&self, id: &str, to: usize) -> Result<(), ConnectionsError> {
        self.edit(|store| Ok(((), store.move_to(id, to)?)))
    }

    /// Forgets one recent server, or all of them with `None`.
    pub fn forget_recent(&self, key: Option<&str>) {
        let _ = self.edit(|store| Ok(((), store.forget_recent(key))));
    }

    /// Puts an imported document in force.
    pub fn replace_all(&self, document: ConnectionsDocument) -> Result<(), ConnectionsError> {
        self.edit(|store| Ok(((), store.replace_all(document)?)))
    }

    /// Connects a login now with the person's answer (see `ConnectionManager::connect`), and adds a
    /// server that is not saved to the recent servers once it connects.
    pub fn connect(
        &self,
        key: &ConnectionKey,
        answer: Option<ConnectAnswer>,
        remember: bool,
        cancel: &CancelToken,
    ) -> Result<Remembered, VfsError> {
        let remembered = self.manager.connect(key, answer, remember, cancel)?;
        self.note_connected(key);
        Ok(remembered)
    }

    /// Adds a login that connected to the recent servers, when it is not saved.
    pub fn note_connected(&self, key: &ConnectionKey) {
        if root_of(key.as_str()).is_none() {
            return;
        }
        let _ = self.edit(|store| Ok(((), store.note_connected(key, now_ms()))));
    }

    /// The label the breadcrumbs give a server's root: the saved connection's name, when the
    /// login is saved under one.
    pub fn label_for(&self, key: &ConnectionKey) -> Option<String> {
        let store = self.lock();
        let saved = store.for_key(key)?;
        (!saved.draft.name.is_empty()).then(|| saved.draft.name.clone())
    }

    /// What the saved connections of `host` add to its SSH options (the first one that sets
    /// anything, in the Network section's order).
    pub fn host_override(&self, host: &str) -> Option<HostOverride> {
        host_override(self.lock().connections(), host)
    }
}

/// `ConnectionsHub::host_override` over a list, for an observer that keeps its own copy.
pub fn host_override(connections: &[SavedConnection], host: &str) -> Option<HostOverride> {
    let host = host.to_ascii_lowercase();
    connections
        .iter()
        .filter(|c| c.draft.scheme == "sftp" && c.draft.host == host)
        .map(|c| HostOverride {
            key_file: c.draft.key_file.clone(),
            jump_host: c.draft.jump_host.clone(),
        })
        .find(|o| o.key_file.is_some() || o.jump_host.is_some())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::credentials::{Credentials, MemorySecrets, SecretKind, SecretStore};
    use crate::storage::MemoryConnections;
    use std::sync::Mutex as StdMutex;
    use waypoint_path::{CaseRule, RemoteScheme};
    use waypoint_vfs::{FakeRemoteProvider, ProviderRegistry, Secret};

    struct Fixture {
        hub: ConnectionsHub,
        storage: Arc<MemoryConnections>,
        keyring: Arc<MemorySecrets>,
        changes: Arc<StdMutex<Vec<ConnectionsChanged>>>,
    }

    fn fixture() -> Fixture {
        let keyring = Arc::new(MemorySecrets::new());
        let credentials = Arc::new(Credentials::new(keyring.clone()));
        let server = FakeRemoteProvider::new(RemoteScheme::Sftp, CaseRule::Sensitive)
            .with_credentials(credentials.clone());
        let registry = ProviderRegistry::new();
        registry.register(Arc::new(server));
        let manager = Arc::new(ConnectionManager::new(Arc::new(registry), credentials));
        let storage = Arc::new(MemoryConnections::default());
        let hub = ConnectionsHub::new(storage.clone(), manager);
        let changes = Arc::new(StdMutex::new(Vec::new()));
        let seen = changes.clone();
        hub.set_sink(move |change| seen.lock().unwrap().push(change.clone()));
        Fixture {
            hub,
            storage,
            keyring,
            changes,
        }
    }

    fn draft(host: &str) -> ConnectionDraft {
        ConnectionDraft {
            scheme: "sftp".into(),
            host: host.into(),
            user: Some("me".into()),
            ..ConnectionDraft::default()
        }
    }

    #[test]
    fn every_edit_is_saved_and_sent() {
        let fx = fixture();
        let a = fx.hub.add(&draft("a.lan")).unwrap();
        fx.hub
            .update(
                &a.connection.id,
                &ConnectionDraft {
                    name: "A".into(),
                    ..draft("a.lan")
                },
            )
            .unwrap();
        let copy = fx
            .hub
            .duplicate(&a.connection.id, |n| format!("{n} (copy)"))
            .unwrap();
        assert_eq!(copy.label, "A (copy)");
        fx.hub.move_to(&copy.connection.id, 0).unwrap();
        assert_eq!(fx.changes.lock().unwrap().len(), 4);
        let saved = fx.storage.saved().unwrap();
        assert_eq!(saved.connections[0].id, copy.connection.id);
        // A failed edit sends and saves nothing.
        assert!(fx.hub.update("c99", &draft("x")).is_err());
        assert_eq!(fx.changes.lock().unwrap().len(), 4);
    }

    #[test]
    fn removing_can_forget_the_keyring_secret_unless_another_connection_shares_the_login() {
        let fx = fixture();
        let a = fx.hub.add(&draft("a.lan")).unwrap();
        let b = fx
            .hub
            .add(&ConnectionDraft {
                name: "again".into(),
                ..draft("a.lan")
            })
            .unwrap();
        let key = waypoint_path::VfsPath::from_location(&a.location)
            .unwrap()
            .connection_key()
            .unwrap();
        fx.keyring
            .store(&key, SecretKind::Password, Secret::from("pw"))
            .unwrap();
        fx.hub.remove(&a.connection.id, true).unwrap();
        assert_eq!(fx.keyring.len(), 1, "the other connection still uses it");
        fx.hub.remove(&b.connection.id, true).unwrap();
        assert!(fx.keyring.is_empty());
    }

    #[test]
    fn a_server_that_connects_unsaved_becomes_recent() {
        let fx = fixture();
        let key = crate::store::root_of("sftp://me@b.lan")
            .unwrap()
            .connection_key();
        fx.hub
            .connect(&key, None, false, &CancelToken::new())
            .unwrap();
        let snapshot = fx.hub.snapshot();
        assert_eq!(snapshot.recent[0].key, "sftp://me@b.lan");
        assert!(fx.storage.saved().unwrap().recent.len() == 1);
    }

    #[test]
    fn observers_and_host_overrides_follow_the_saved_connections() {
        let fx = fixture();
        let seen = Arc::new(StdMutex::new(0usize));
        let count = seen.clone();
        fx.hub
            .observe(move |all| *count.lock().unwrap() = all.len());
        assert_eq!(*seen.lock().unwrap(), 0);
        fx.hub
            .add(&ConnectionDraft {
                key_file: Some("~/.ssh/work".into()),
                jump_host: Some("bastion".into()),
                name: "Work".into(),
                ..draft("NAS.lan")
            })
            .unwrap();
        assert_eq!(*seen.lock().unwrap(), 1);
        let found = fx.hub.host_override("nas.lan").unwrap();
        assert_eq!(found.key_file.as_deref(), Some("~/.ssh/work"));
        assert_eq!(found.jump_host.as_deref(), Some("bastion"));
        assert_eq!(fx.hub.host_override("other.lan"), None);
        let key = crate::store::root_of("sftp://me@nas.lan")
            .unwrap()
            .connection_key();
        assert_eq!(fx.hub.label_for(&key).as_deref(), Some("Work"));
    }
}
