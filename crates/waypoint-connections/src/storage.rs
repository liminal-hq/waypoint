// Where the saved connections live: a rotating document with one earlier generation, and the import plan of the settings export
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use serde_json::Value;
use waypoint_settings::{
    unknown_paths, BundleError, ChangeGroup, FilePlan, ImportWarning, KeyValue, StorageError,
};

use crate::store::{Connections, ConnectionsDocument, CONNECTIONS_VERSION};

/// The id the saved connections are exported under.
pub const CONNECTIONS_FILE_ID: &str = "connections";

/// The file the saved connections are kept in, in the app data directory.
pub const CONNECTIONS_FILE: &str = "connections.json";

/// The key the latest document is stored under.
pub const CONNECTIONS_KEY: &str = "connections";

/// The key the document of the run before is kept under.
pub const CONNECTIONS_PREVIOUS_KEY: &str = "connectionsPrevious";

/// Where the saved connections live. The app implements it over its key-value file.
pub trait ConnectionStorage: Send + Sync {
    /// The saved document, or `None` when nothing usable was saved.
    fn load(&self) -> Result<Option<ConnectionsDocument>, StorageError>;
    fn save(&self, document: &ConnectionsDocument) -> Result<(), StorageError>;
}

/// Connections kept in memory: for tests, and for an app that cannot open the file.
#[derive(Default)]
pub struct MemoryConnections {
    saved: Mutex<Option<ConnectionsDocument>>,
}

impl MemoryConnections {
    pub fn saved(&self) -> Option<ConnectionsDocument> {
        self.saved.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
}

impl ConnectionStorage for MemoryConnections {
    fn load(&self) -> Result<Option<ConnectionsDocument>, StorageError> {
        Ok(self.saved())
    }

    fn save(&self, document: &ConnectionsDocument) -> Result<(), StorageError> {
        *self.saved.lock().unwrap_or_else(|e| e.into_inner()) = Some(document.clone());
        Ok(())
    }
}

/// The document storage: the first save of each run rotates the latest document into the
/// previous key, loading tries the latest then the previous, and a copy that cannot be read (or
/// that a newer build wrote) is set aside once. Saved connections that cannot be read are never
/// fatal: the Network section is just empty.
pub struct ConnectionsPersistence<K: KeyValue> {
    kv: K,
    rotated: AtomicBool,
    set_aside: AtomicBool,
}

impl<K: KeyValue> ConnectionsPersistence<K> {
    pub fn new(kv: K) -> Self {
        Self {
            kv,
            rotated: AtomicBool::new(false),
            set_aside: AtomicBool::new(false),
        }
    }

    fn corrupt(&self, key: &str, why: &str) {
        log::warn!("the stored `{key}` connections are unusable ({why}); trying the next copy");
        if !self.set_aside.swap(true, Ordering::AcqRel) {
            match self.kv.set_aside() {
                Some(name) => log::warn!("kept a copy of the connections file as `{name}`"),
                None => log::warn!("could not keep a copy of the unreadable connections file"),
            }
        }
    }

    fn read(&self, key: &str) -> Option<Result<ConnectionsDocument, String>> {
        let value = self.kv.get(key)?;
        Some(
            serde_json::from_value::<ConnectionsDocument>(value)
                .map_err(|e| e.to_string())
                .and_then(|doc| {
                    if doc.version > CONNECTIONS_VERSION {
                        Err(format!(
                            "the document is version {}, this build reads version {CONNECTIONS_VERSION}",
                            doc.version
                        ))
                    } else {
                        Ok(doc)
                    }
                }),
        )
    }
}

impl<K: KeyValue> ConnectionStorage for ConnectionsPersistence<K> {
    fn load(&self) -> Result<Option<ConnectionsDocument>, StorageError> {
        if self.kv.was_unreadable() {
            self.set_aside.store(true, Ordering::Release);
        }
        for key in [CONNECTIONS_KEY, CONNECTIONS_PREVIOUS_KEY] {
            match self.read(key) {
                None => {}
                Some(Ok(document)) => return Ok(Some(document)),
                Some(Err(why)) => self.corrupt(key, &why),
            }
        }
        Ok(None)
    }

    fn save(&self, document: &ConnectionsDocument) -> Result<(), StorageError> {
        let value = serde_json::to_value(document).map_err(|e| StorageError::Io(e.to_string()))?;
        if !self.rotated.swap(true, Ordering::AcqRel) {
            // The first save of the run: what is on disk now is how the run started. An unusable
            // copy is not worth keeping over an earlier good `previous`.
            if let Some(Ok(_)) = self.read(CONNECTIONS_KEY) {
                if let Some(current) = self.kv.get(CONNECTIONS_KEY) {
                    self.kv.set(CONNECTIONS_PREVIOUS_KEY, current);
                }
            }
        }
        self.kv.set(CONNECTIONS_KEY, value);
        self.kv.save().map_err(StorageError::Io)
    }
}

/// Plans importing a document of saved connections over `current`: the checks of a normal save, a
/// version from a newer build refused, keys this build does not have left out and noted. Two
/// groups: `connections` counts the connections the import would add, change or forget, and
/// `recent` is one change when the recent servers differ.
pub fn plan_connections(current: &Connections, incoming: &Value) -> Result<FilePlan, BundleError> {
    let invalid = |message: String| BundleError::Invalid {
        file: CONNECTIONS_FILE_ID.to_owned(),
        message,
    };
    let object = incoming
        .as_object()
        .ok_or_else(|| invalid("it is not an object".to_owned()))?;
    let version = object
        .get("version")
        .and_then(Value::as_u64)
        .ok_or_else(|| invalid("it has no version".to_owned()))?;
    if version > u64::from(CONNECTIONS_VERSION) {
        return Err(BundleError::NewerDocument {
            file: CONNECTIONS_FILE_ID.to_owned(),
            found: version,
            supported: CONNECTIONS_VERSION,
        });
    }
    let parsed: ConnectionsDocument =
        serde_json::from_value(incoming.clone()).map_err(|e| invalid(e.to_string()))?;
    let document = ConnectionsDocument::new(parsed.next_id, parsed.connections, parsed.recent);
    let mut probe = current.clone();
    let change = probe
        .replace_all(document.clone())
        .map_err(|e| invalid(e.to_string()))?;
    let document_value =
        serde_json::to_value(probe.to_document()).map_err(|e| invalid(e.to_string()))?;
    let mut changes = Vec::new();
    if let Some(change) = change {
        if !change.changes.is_empty() || change.order.is_some() {
            changes.push(ChangeGroup {
                file: CONNECTIONS_FILE_ID.to_owned(),
                group: "connections".to_owned(),
                count: change.changes.len().max(1) as u32,
            });
        }
        if change.recent.is_some() {
            changes.push(ChangeGroup {
                file: CONNECTIONS_FILE_ID.to_owned(),
                group: "recent".to_owned(),
                count: 1,
            });
        }
    }
    let unknown = unknown_paths(
        incoming,
        &serde_json::to_value(&document).unwrap_or_default(),
    );
    Ok(FilePlan {
        document: document_value,
        changes,
        warnings: if unknown.is_empty() {
            Vec::new()
        } else {
            vec![ImportWarning::UnknownKeys {
                file: CONNECTIONS_FILE_ID.to_owned(),
                keys: unknown,
            }]
        },
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use serde_json::json;

    use super::*;
    use crate::model::ConnectionDraft;

    #[derive(Default)]
    struct Memory {
        values: Mutex<HashMap<String, Value>>,
        aside: Mutex<u32>,
    }

    #[derive(Clone)]
    struct Kv(Arc<Memory>);

    impl KeyValue for Kv {
        fn get(&self, key: &str) -> Option<Value> {
            self.0.values.lock().unwrap().get(key).cloned()
        }
        fn set(&self, key: &str, value: Value) {
            self.0.values.lock().unwrap().insert(key.to_owned(), value);
        }
        fn save(&self) -> Result<(), String> {
            Ok(())
        }
        fn set_aside(&self) -> Option<String> {
            *self.0.aside.lock().unwrap() += 1;
            Some("connections.json.corrupt-1".into())
        }
    }

    fn draft(host: &str) -> ConnectionDraft {
        ConnectionDraft {
            scheme: "sftp".into(),
            host: host.into(),
            ..ConnectionDraft::default()
        }
    }

    #[test]
    fn the_first_save_of_a_run_keeps_the_last_run_as_previous() {
        let memory = Kv(Arc::new(Memory::default()));
        let mut store = Connections::new();
        store.add(&draft("a.lan")).unwrap();
        ConnectionsPersistence::new(memory.clone())
            .save(&store.to_document())
            .unwrap();
        // The next run.
        let storage = ConnectionsPersistence::new(memory.clone());
        let loaded = storage.load().unwrap().unwrap();
        assert_eq!(loaded.connections.len(), 1);
        store.add(&draft("b.lan")).unwrap();
        storage.save(&store.to_document()).unwrap();
        storage.save(&store.to_document()).unwrap();
        let previous: ConnectionsDocument =
            serde_json::from_value(memory.get(CONNECTIONS_PREVIOUS_KEY).unwrap()).unwrap();
        assert_eq!(previous.connections.len(), 1);
    }

    #[test]
    fn an_unreadable_or_newer_copy_falls_back_and_is_set_aside_once() {
        let memory = Kv(Arc::new(Memory::default()));
        memory.set(CONNECTIONS_KEY, json!({ "version": 99 }));
        memory.set(CONNECTIONS_PREVIOUS_KEY, json!("nonsense"));
        let storage = ConnectionsPersistence::new(memory.clone());
        assert_eq!(storage.load().unwrap(), None);
        assert_eq!(*memory.0.aside.lock().unwrap(), 1);
    }

    #[test]
    fn the_saved_file_holds_no_secret_field() {
        let mut store = Connections::new();
        store.add(&draft("a.lan")).unwrap();
        let text = serde_json::to_string(&store.to_document()).unwrap();
        for word in ["password", "passphrase", "secret"] {
            assert!(!text.to_lowercase().contains(word), "{word} in {text}");
        }
    }

    #[test]
    fn an_import_is_planned_without_touching_the_store() {
        let mut store = Connections::new();
        store.add(&draft("a.lan")).unwrap();
        let mut other = Connections::new();
        other.add(&draft("b.lan")).unwrap();
        other.add(&draft("c.lan")).unwrap();
        let mut incoming = serde_json::to_value(other.to_document()).unwrap();
        incoming["future"] = json!(1);
        let plan = plan_connections(&store, &incoming).unwrap();
        assert_eq!(store.len(), 1);
        assert_eq!(plan.changes[0].group, "connections");
        assert_eq!(plan.changes[0].count, 2);
        assert!(matches!(
            &plan.warnings[0],
            ImportWarning::UnknownKeys { keys, .. } if keys == &["future".to_owned()]
        ));
        let newer = json!({ "version": 2, "connections": [] });
        assert!(matches!(
            plan_connections(&store, &newer),
            Err(BundleError::NewerDocument { .. })
        ));
        let bad =
            json!({ "version": 1, "connections": [{ "id": "c1", "scheme": "sftp", "host": "" }] });
        assert!(matches!(
            plan_connections(&store, &bad),
            Err(BundleError::Invalid { .. })
        ));
    }
}
