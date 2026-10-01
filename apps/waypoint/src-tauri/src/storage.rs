// Persists the session over `tauri-plugin-store`, keeping one earlier copy and setting aside what cannot be read
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The store file `session.json` lives in the app data directory and holds two keys: `session`,
// the latest document, and `previous`, the document that was current when this run started. The
// first save of each run rotates `session` into `previous`, so a bad write (or a crash) can cost
// at most the changes of one run. Loading tries `session`, then `previous`; a value that does not
// parse as a `Document`, or that `Store::from_document` rejects, is corrupt: the whole file is
// copied to `session.json.corrupt-{unix}` once, a warning is logged, and loading falls through.
// When something was set aside the person is told ("Your last session could not be restored")
// through `Persistence::take_notice`, which the app hands to the page once.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use serde_json::Value;
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_store::StoreExt;
use waypoint_session::{Document, SessionStorage, StorageError, Store};

/// The store file, relative to the app data directory.
pub const FILE: &str = "session.json";
const SESSION: &str = "session";
const PREVIOUS: &str = "previous";

/// The few operations the persistence needs from a key-value file, so the recovery logic runs
/// in tests without Tauri.
pub trait KeyValue: Send + Sync {
    fn get(&self, key: &str) -> Option<Value>;
    fn set(&self, key: &str, value: Value);
    /// Writes the values to disk.
    fn save(&self) -> Result<(), String>;
    /// Copies the backing file next to itself as `…corrupt-{unix}`; the new name, when it worked.
    fn set_aside(&self) -> Option<String>;
    /// The file existed but was not readable as a store at all (and was already set aside when
    /// the store opened).
    fn was_unreadable(&self) -> bool {
        false
    }
}

/// The document storage: rotation, recovery and the one-time notice, over any `KeyValue`.
pub struct Persistence<K: KeyValue> {
    kv: K,
    rotated: AtomicBool,
    set_aside: AtomicBool,
    notice: Mutex<Option<String>>,
}

/// The sentence the page shows when the last session could not be restored.
pub const RESTORE_FAILED: &str = "Your last session could not be restored";

impl<K: KeyValue> Persistence<K> {
    pub fn new(kv: K) -> Self {
        Self {
            kv,
            rotated: AtomicBool::new(false),
            set_aside: AtomicBool::new(false),
            notice: Mutex::new(None),
        }
    }

    /// The notice to show the person, once: `Some` only the first time after a load that had to
    /// give up on a stored session.
    pub fn take_notice(&self) -> Option<String> {
        self.notice.lock().unwrap_or_else(|e| e.into_inner()).take()
    }

    fn corrupt(&self, key: &str, why: &str) {
        log::warn!("the stored `{key}` session is unusable ({why}); trying the next copy");
        if !self.set_aside.swap(true, Ordering::AcqRel) {
            match self.kv.set_aside() {
                Some(name) => log::warn!("kept a copy of the session file as `{name}`"),
                None => log::warn!("could not keep a copy of the unreadable session file"),
            }
        }
        *self.notice.lock().unwrap_or_else(|e| e.into_inner()) = Some(RESTORE_FAILED.to_string());
    }

    /// The document stored under `key` when it parses and passes the store's own checks.
    fn read(&self, key: &str) -> Option<Result<Document, String>> {
        let value = self.kv.get(key)?;
        Some(
            serde_json::from_value::<Document>(value)
                .map_err(|e| e.to_string())
                .and_then(|doc| {
                    Store::from_document(doc.clone())
                        .map(|_| doc)
                        .map_err(|e| e.to_string())
                }),
        )
    }
}

impl<K: KeyValue> SessionStorage for Persistence<K> {
    fn load(&self) -> Result<Option<Document>, StorageError> {
        if self.kv.was_unreadable() {
            *self.notice.lock().unwrap_or_else(|e| e.into_inner()) =
                Some(RESTORE_FAILED.to_string());
            self.set_aside.store(true, Ordering::Release);
        }
        for key in [SESSION, PREVIOUS] {
            match self.read(key) {
                None => {}
                Some(Ok(document)) => return Ok(Some(document)),
                Some(Err(why)) => self.corrupt(key, &why),
            }
        }
        Ok(None)
    }

    fn save(&self, document: &Document) -> Result<(), StorageError> {
        let value = serde_json::to_value(document).map_err(|e| StorageError::Io(e.to_string()))?;
        if !self.rotated.swap(true, Ordering::AcqRel) {
            // The first save of the run: what is on disk now is how the run started. A copy that
            // is unusable is not worth keeping over an earlier good `previous`.
            if let Some(Ok(_)) = self.read(SESSION) {
                if let Some(current) = self.kv.get(SESSION) {
                    self.kv.set(PREVIOUS, current);
                }
            }
        }
        self.kv.set(SESSION, value);
        self.kv.save().map_err(StorageError::Io)
    }
}

/// `session.json` in the app data directory, through `tauri-plugin-store`.
pub struct FileKeyValue<R: Runtime> {
    store: std::sync::Arc<tauri_plugin_store::Store<R>>,
    path: PathBuf,
    unreadable: bool,
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Copies `path` to `{path}.corrupt-{unix}`, a new name each time.
fn copy_aside(path: &Path) -> Option<String> {
    let name = path.file_name()?.to_string_lossy().into_owned();
    let now = unix_now();
    for attempt in 0..100u32 {
        let suffix = if attempt == 0 {
            format!("{name}.corrupt-{now}")
        } else {
            format!("{name}.corrupt-{now}-{attempt}")
        };
        let target = path.with_file_name(&suffix);
        if !target.exists() {
            return std::fs::copy(path, &target).ok().map(|_| suffix);
        }
    }
    None
}

impl<R: Runtime> FileKeyValue<R> {
    /// Opens the store. `tauri-plugin-store` ignores a file it cannot parse and the next save
    /// would overwrite it, so a file that is not a JSON object is copied aside first.
    pub fn open(app: &AppHandle<R>) -> Result<Self, String> {
        let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
        let path = dir.join(FILE);
        let unreadable = std::fs::read(&path).is_ok_and(|bytes| {
            !matches!(
                serde_json::from_slice::<Value>(&bytes),
                Ok(Value::Object(_))
            )
        });
        if unreadable {
            log::warn!("the session file {} is not a JSON object", path.display());
            match copy_aside(&path) {
                Some(name) => log::warn!("kept a copy of the session file as `{name}`"),
                None => log::warn!("could not keep a copy of the unreadable session file"),
            }
        }
        let store = app.store(FILE).map_err(|e| e.to_string())?;
        Ok(Self {
            store,
            path,
            unreadable,
        })
    }
}

impl<R: Runtime> KeyValue for FileKeyValue<R> {
    fn get(&self, key: &str) -> Option<Value> {
        self.store.get(key)
    }

    fn set(&self, key: &str, value: Value) {
        self.store.set(key, value);
    }

    fn save(&self) -> Result<(), String> {
        self.store.save().map_err(|e| e.to_string())
    }

    fn set_aside(&self) -> Option<String> {
        copy_aside(&self.path)
    }

    fn was_unreadable(&self) -> bool {
        self.unreadable
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use super::*;
    use serde_json::json;
    use waypoint_protocol::Location;
    use waypoint_session::Command;

    /// A map that counts what the persistence does to it.
    #[derive(Default)]
    struct Memory {
        values: Mutex<HashMap<String, Value>>,
        saves: Mutex<u32>,
        aside: Mutex<u32>,
    }

    impl KeyValue for Arc<Memory> {
        fn get(&self, key: &str) -> Option<Value> {
            self.values.lock().unwrap().get(key).cloned()
        }
        fn set(&self, key: &str, value: Value) {
            self.values.lock().unwrap().insert(key.to_string(), value);
        }
        fn save(&self) -> Result<(), String> {
            *self.saves.lock().unwrap() += 1;
            Ok(())
        }
        fn set_aside(&self) -> Option<String> {
            *self.aside.lock().unwrap() += 1;
            Some("session.json.corrupt-1".into())
        }
    }

    fn document(tabs: usize) -> Document {
        let mut store = Store::new();
        store
            .dispatch(
                "main-1",
                Command::RegisterWindow {
                    label: "main-1".into(),
                },
            )
            .unwrap();
        for i in 0..tabs {
            store
                .dispatch(
                    "main-1",
                    Command::Open {
                        location: Location::new(format!("/t/{i}"), format!("file:///t/{i}")),
                        after: None,
                        activate: true,
                    },
                )
                .unwrap();
        }
        store.to_document()
    }

    fn setup() -> (Arc<Memory>, Persistence<Arc<Memory>>) {
        let memory = Arc::new(Memory::default());
        (memory.clone(), Persistence::new(memory))
    }

    fn put(memory: &Memory, key: &str, value: Value) {
        memory.values.lock().unwrap().insert(key.into(), value);
    }

    fn tab_count(document: &Document) -> usize {
        document.body.windows[0].tabs.len()
    }

    #[test]
    fn nothing_stored_loads_nothing_quietly() {
        let (memory, storage) = setup();
        assert_eq!(storage.load().unwrap(), None);
        assert_eq!(storage.take_notice(), None);
        assert_eq!(*memory.aside.lock().unwrap(), 0);
    }

    #[test]
    fn a_saved_document_loads_back() {
        let (_, storage) = setup();
        storage.save(&document(2)).unwrap();
        assert_eq!(storage.load().unwrap(), Some(document(2)));
        assert_eq!(storage.take_notice(), None);
    }

    #[test]
    fn a_corrupt_session_falls_back_to_previous_and_is_set_aside_once() {
        let (memory, storage) = setup();
        put(&memory, SESSION, json!({ "nonsense": true }));
        put(
            &memory,
            PREVIOUS,
            serde_json::to_value(document(3)).unwrap(),
        );
        let loaded = storage.load().unwrap().unwrap();
        assert_eq!(tab_count(&loaded), 3);
        assert_eq!(*memory.aside.lock().unwrap(), 1);
        assert_eq!(storage.take_notice().as_deref(), Some(RESTORE_FAILED));
        assert_eq!(storage.take_notice(), None);
    }

    #[test]
    fn corrupt_session_and_previous_load_nothing_and_set_the_file_aside_once() {
        let (memory, storage) = setup();
        put(&memory, SESSION, json!("not a document"));
        put(&memory, PREVIOUS, json!([1, 2]));
        assert_eq!(storage.load().unwrap(), None);
        assert_eq!(*memory.aside.lock().unwrap(), 1);
        assert_eq!(storage.take_notice().as_deref(), Some(RESTORE_FAILED));
    }

    #[test]
    fn a_missing_session_key_uses_previous_without_a_notice() {
        let (memory, storage) = setup();
        put(
            &memory,
            PREVIOUS,
            serde_json::to_value(document(1)).unwrap(),
        );
        assert_eq!(tab_count(&storage.load().unwrap().unwrap()), 1);
        assert_eq!(storage.take_notice(), None);
        assert_eq!(*memory.aside.lock().unwrap(), 0);
    }

    #[test]
    fn a_document_of_a_newer_version_counts_as_corrupt() {
        let (memory, storage) = setup();
        let mut newer = serde_json::to_value(document(1)).unwrap();
        newer["version"] = json!(999);
        put(&memory, SESSION, newer);
        assert_eq!(storage.load().unwrap(), None);
        assert_eq!(*memory.aside.lock().unwrap(), 1);
        assert!(storage.take_notice().is_some());
    }

    #[test]
    fn a_document_that_breaks_an_invariant_counts_as_corrupt() {
        let (memory, storage) = setup();
        let mut bad = serde_json::to_value(document(1)).unwrap();
        bad["body"]["windows"][0]["label"] = json!("settings");
        put(&memory, SESSION, bad);
        put(
            &memory,
            PREVIOUS,
            serde_json::to_value(document(2)).unwrap(),
        );
        assert_eq!(tab_count(&storage.load().unwrap().unwrap()), 2);
    }

    #[test]
    fn an_unreadable_file_is_reported_even_though_no_key_is_left() {
        struct Unreadable(Arc<Memory>);
        impl KeyValue for Unreadable {
            fn get(&self, key: &str) -> Option<Value> {
                self.0.get(key)
            }
            fn set(&self, key: &str, value: Value) {
                self.0.set(key, value)
            }
            fn save(&self) -> Result<(), String> {
                self.0.save()
            }
            fn set_aside(&self) -> Option<String> {
                self.0.set_aside()
            }
            fn was_unreadable(&self) -> bool {
                true
            }
        }
        let storage = Persistence::new(Unreadable(Arc::default()));
        assert_eq!(storage.load().unwrap(), None);
        assert_eq!(storage.take_notice().as_deref(), Some(RESTORE_FAILED));
    }

    #[test]
    fn only_the_first_save_of_a_run_rotates_into_previous() {
        let (memory, storage) = setup();
        put(&memory, SESSION, serde_json::to_value(document(1)).unwrap());
        storage.save(&document(2)).unwrap();
        storage.save(&document(3)).unwrap();
        let previous: Document = serde_json::from_value(memory.get(PREVIOUS).unwrap()).unwrap();
        let current: Document = serde_json::from_value(memory.get(SESSION).unwrap()).unwrap();
        assert_eq!(tab_count(&previous), 1, "previous is how the run started");
        assert_eq!(tab_count(&current), 3);
        assert_eq!(*memory.saves.lock().unwrap(), 2);
    }

    #[test]
    fn a_corrupt_session_is_not_rotated_over_a_good_previous() {
        let (memory, storage) = setup();
        put(&memory, SESSION, json!(7));
        put(
            &memory,
            PREVIOUS,
            serde_json::to_value(document(4)).unwrap(),
        );
        storage.save(&document(1)).unwrap();
        let previous: Document = serde_json::from_value(memory.get(PREVIOUS).unwrap()).unwrap();
        assert_eq!(tab_count(&previous), 4);
    }

    #[test]
    fn the_first_save_of_a_fresh_install_has_nothing_to_rotate() {
        let (memory, storage) = setup();
        storage.save(&document(1)).unwrap();
        assert!(memory.get(PREVIOUS).is_none());
    }
}
