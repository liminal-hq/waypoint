// The settings document as a versioned file value, with one earlier generation and recovery
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The document lives in a key-value file under two keys: `settings`, the latest document, and
// `settingsPrevious`, the one that was current when this run started. The first save of each run
// rotates `settings` into `settingsPrevious`, so a bad write costs at most one run's changes.
// Loading tries `settings`, then `settingsPrevious`; a value that does not parse (or that a newer
// build wrote) is set aside by copying the whole file once, and loading falls through, ending at
// the defaults rather than failing: settings that cannot be read must never stop the app starting.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use ts_rs::TS;

use crate::model::Settings;

/// The document format this build writes and reads.
pub const DOCUMENT_VERSION: u32 = 1;

/// The key the latest document is stored under.
pub const SETTINGS_KEY: &str = "settings";

/// The key the document of the run before is kept under.
pub const PREVIOUS_KEY: &str = "settingsPrevious";

/// The settings as a file value: a version so a later build can migrate, and the body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct SettingsDocument {
    pub version: u32,
    pub body: Settings,
}

impl SettingsDocument {
    pub fn new(body: Settings) -> Self {
        Self {
            version: DOCUMENT_VERSION,
            body,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum StorageError {
    #[error("settings storage failed: {0}")]
    Io(String),
}

/// Where the document lives. The app implements it over its key-value file (see `Persistence`).
pub trait SettingsStorage: Send + Sync {
    /// The saved document, or `None` when nothing usable was saved.
    fn load(&self) -> Result<Option<SettingsDocument>, StorageError>;
    fn save(&self, document: &SettingsDocument) -> Result<(), StorageError>;

    /// Keeps the saved document as the previous copy now, so the next `save` replaces a document
    /// whose predecessor is the one being replaced. Saving alone keeps only the document the run
    /// started with; an import calls this first so one step back is always the document it
    /// replaced. Storage with no previous copy does nothing.
    fn keep_as_previous(&self) {}
}

/// Settings kept in memory: for tests, and for an app that cannot open its settings file.
#[derive(Default)]
pub struct MemoryStorage {
    saved: Mutex<Option<SettingsDocument>>,
}

impl MemoryStorage {
    pub fn saved(&self) -> Option<SettingsDocument> {
        self.saved.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
}

impl SettingsStorage for MemoryStorage {
    fn load(&self) -> Result<Option<SettingsDocument>, StorageError> {
        Ok(self.saved())
    }

    fn save(&self, document: &SettingsDocument) -> Result<(), StorageError> {
        *self.saved.lock().unwrap_or_else(|e| e.into_inner()) = Some(document.clone());
        Ok(())
    }
}

/// The few operations the persistence needs from a key-value file, so the recovery logic runs in
/// tests without Tauri.
pub trait KeyValue: Send + Sync {
    fn get(&self, key: &str) -> Option<Value>;
    fn set(&self, key: &str, value: Value);
    /// Writes the values to disk.
    fn save(&self) -> Result<(), String>;
    /// Copies the backing file next to itself as `…corrupt-{unix}`; the new name, when it worked.
    fn set_aside(&self) -> Option<String>;
    /// The file existed but was not readable as a store at all (and was already set aside when it
    /// opened).
    fn was_unreadable(&self) -> bool {
        false
    }
}

/// The document storage: rotation and recovery over any `KeyValue`.
pub struct Persistence<K: KeyValue> {
    kv: K,
    rotated: AtomicBool,
    set_aside: AtomicBool,
}

impl<K: KeyValue> Persistence<K> {
    pub fn new(kv: K) -> Self {
        Self {
            kv,
            rotated: AtomicBool::new(false),
            set_aside: AtomicBool::new(false),
        }
    }

    /// Whether loading had to give up on a stored copy.
    pub fn recovered(&self) -> bool {
        self.set_aside.load(Ordering::Acquire)
    }

    fn corrupt(&self, key: &str, why: &str) {
        log::warn!("the stored `{key}` settings are unusable ({why}); trying the next copy");
        if !self.set_aside.swap(true, Ordering::AcqRel) {
            match self.kv.set_aside() {
                Some(name) => log::warn!("kept a copy of the settings file as `{name}`"),
                None => log::warn!("could not keep a copy of the unreadable settings file"),
            }
        }
    }

    fn read(&self, key: &str) -> Option<Result<SettingsDocument, String>> {
        let value = self.kv.get(key)?;
        Some(
            serde_json::from_value::<SettingsDocument>(value)
                .map_err(|e| e.to_string())
                .and_then(|doc| {
                    if doc.version > DOCUMENT_VERSION {
                        Err(format!(
                            "the document is version {}, this build reads version {DOCUMENT_VERSION}",
                            doc.version
                        ))
                    } else {
                        Ok(doc)
                    }
                }),
        )
    }
}

impl<K: KeyValue> SettingsStorage for Persistence<K> {
    fn load(&self) -> Result<Option<SettingsDocument>, StorageError> {
        if self.kv.was_unreadable() {
            self.set_aside.store(true, Ordering::Release);
        }
        for key in [SETTINGS_KEY, PREVIOUS_KEY] {
            match self.read(key) {
                None => {}
                Some(Ok(document)) => return Ok(Some(document)),
                Some(Err(why)) => self.corrupt(key, &why),
            }
        }
        Ok(None)
    }

    fn keep_as_previous(&self) {
        if let Some(Ok(_)) = self.read(SETTINGS_KEY) {
            if let Some(current) = self.kv.get(SETTINGS_KEY) {
                self.kv.set(PREVIOUS_KEY, current);
            }
        }
        // The rotation of this run has happened: the first save must not rotate over it.
        self.rotated.store(true, Ordering::Release);
    }

    fn save(&self, document: &SettingsDocument) -> Result<(), StorageError> {
        let value = serde_json::to_value(document).map_err(|e| StorageError::Io(e.to_string()))?;
        if !self.rotated.swap(true, Ordering::AcqRel) {
            // The first save of the run: what is on disk now is how the run started. An unusable
            // copy is not worth keeping over an earlier good `previous`.
            if let Some(Ok(_)) = self.read(SETTINGS_KEY) {
                if let Some(current) = self.kv.get(SETTINGS_KEY) {
                    self.kv.set(PREVIOUS_KEY, current);
                }
            }
        }
        self.kv.set(SETTINGS_KEY, value);
        self.kv.save().map_err(StorageError::Io)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::Arc;

    use serde_json::json;

    use super::*;
    use crate::model::{DefaultView, DropActionRule};

    /// A map that counts what the persistence does to it.
    #[derive(Default)]
    struct Memory {
        values: Mutex<HashMap<String, Value>>,
        saves: Mutex<u32>,
        aside: Mutex<u32>,
        unreadable: AtomicBool,
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
            Some("settings.json.corrupt-1".into())
        }
        fn was_unreadable(&self) -> bool {
            self.unreadable.load(Ordering::Acquire)
        }
    }

    fn setup() -> (Arc<Memory>, Persistence<Arc<Memory>>) {
        let memory = Arc::new(Memory::default());
        (memory.clone(), Persistence::new(memory))
    }

    fn with_view(view: DefaultView) -> SettingsDocument {
        let mut body = Settings::default();
        body.general.default_view = view;
        SettingsDocument::new(body)
    }

    #[test]
    fn a_document_from_before_the_ui_section_loads_with_the_action_bar_on() {
        let (memory, storage) = setup();
        memory.set(
            SETTINGS_KEY,
            serde_json::json!({"version": 1, "body": {"general": {"defaultView": "grid"}}}),
        );
        let loaded = storage.load().unwrap().unwrap();
        assert_eq!(loaded.body.general.default_view, DefaultView::Grid);
        assert!(loaded.body.ui.action_bar);
        assert!(loaded.body.ui.action_bar_labels);
    }

    #[test]
    fn nothing_stored_loads_nothing_quietly() {
        let (memory, storage) = setup();
        assert_eq!(storage.load().unwrap(), None);
        assert!(!storage.recovered());
        assert_eq!(*memory.aside.lock().unwrap(), 0);
    }

    #[test]
    fn a_saved_document_loads_back_under_the_settings_key() {
        let (memory, storage) = setup();
        storage.save(&with_view(DefaultView::Grid)).unwrap();
        assert_eq!(storage.load().unwrap(), Some(with_view(DefaultView::Grid)));
        assert_eq!(memory.get(SETTINGS_KEY).unwrap()["version"], 1);
        assert_eq!(
            memory.get(SETTINGS_KEY).unwrap()["body"]["general"]["defaultView"],
            "grid"
        );
        assert_eq!(*memory.saves.lock().unwrap(), 1);
    }

    #[test]
    fn the_first_save_of_a_run_keeps_what_the_run_started_with_as_previous() {
        let (memory, storage) = setup();
        memory.set(
            SETTINGS_KEY,
            serde_json::to_value(with_view(DefaultView::Grid)).unwrap(),
        );
        storage.save(&with_view(DefaultView::List)).unwrap();
        storage.save(&with_view(DefaultView::Grid)).unwrap();
        // The second save of the run does not rotate again.
        let previous: SettingsDocument =
            serde_json::from_value(memory.get(PREVIOUS_KEY).unwrap()).unwrap();
        assert_eq!(previous, with_view(DefaultView::Grid));
        let mut changed = Settings::default();
        changed.dnd.default_action_rule = DropActionRule::AlwaysCopy;
        storage.save(&SettingsDocument::new(changed)).unwrap();
        let previous: SettingsDocument =
            serde_json::from_value(memory.get(PREVIOUS_KEY).unwrap()).unwrap();
        assert_eq!(previous, with_view(DefaultView::Grid));
    }

    #[test]
    fn keeping_as_previous_makes_the_replaced_document_the_one_step_back_even_after_a_rotation() {
        let (memory, storage) = setup();
        memory.set(
            SETTINGS_KEY,
            serde_json::to_value(with_view(DefaultView::Grid)).unwrap(),
        );
        // The run's own rotation keeps the document the run started with…
        storage.save(&with_view(DefaultView::List)).unwrap();
        let previous = |memory: &Arc<Memory>| -> SettingsDocument {
            serde_json::from_value(memory.get(PREVIOUS_KEY).unwrap()).unwrap()
        };
        assert_eq!(previous(&memory), with_view(DefaultView::Grid));
        // …and an import steps in front of it: the document it replaces is the previous one.
        storage.keep_as_previous();
        let mut imported = Settings::default();
        imported.dnd.default_action_rule = DropActionRule::AlwaysCopy;
        storage
            .save(&SettingsDocument::new(imported.clone()))
            .unwrap();
        assert_eq!(previous(&memory), with_view(DefaultView::List));
        assert_eq!(
            storage.load().unwrap(),
            Some(SettingsDocument::new(imported))
        );
    }

    #[test]
    fn keeping_as_previous_before_any_save_rotates_once_and_a_bad_copy_is_not_kept() {
        let (memory, storage) = setup();
        storage.keep_as_previous();
        assert!(memory.get(PREVIOUS_KEY).is_none(), "nothing to keep");
        memory.set(SETTINGS_KEY, json!("nonsense"));
        storage.keep_as_previous();
        assert!(
            memory.get(PREVIOUS_KEY).is_none(),
            "an unusable copy is not kept"
        );
    }

    #[test]
    fn an_unparseable_latest_falls_back_to_previous_and_is_set_aside_once() {
        let (memory, storage) = setup();
        memory.set(
            SETTINGS_KEY,
            json!({ "version": 1, "body": { "general": 7 } }),
        );
        memory.set(
            PREVIOUS_KEY,
            serde_json::to_value(with_view(DefaultView::Grid)).unwrap(),
        );
        assert_eq!(storage.load().unwrap(), Some(with_view(DefaultView::Grid)));
        assert!(storage.recovered());
        assert_eq!(*memory.aside.lock().unwrap(), 1);
        storage.load().unwrap();
        assert_eq!(*memory.aside.lock().unwrap(), 1, "kept aside only once");
    }

    #[test]
    fn two_unusable_copies_load_nothing_so_the_defaults_apply() {
        let (memory, storage) = setup();
        memory.set(SETTINGS_KEY, json!("nonsense"));
        memory.set(PREVIOUS_KEY, json!(42));
        assert_eq!(storage.load().unwrap(), None);
        assert!(storage.recovered());
        assert_eq!(*memory.aside.lock().unwrap(), 1);
    }

    #[test]
    fn a_document_from_a_newer_build_is_not_read_and_not_rotated_over_a_good_previous() {
        let (memory, storage) = setup();
        memory.set(SETTINGS_KEY, json!({ "version": 2, "body": {} }));
        memory.set(
            PREVIOUS_KEY,
            serde_json::to_value(with_view(DefaultView::Grid)).unwrap(),
        );
        assert_eq!(storage.load().unwrap(), Some(with_view(DefaultView::Grid)));
        storage.save(&with_view(DefaultView::List)).unwrap();
        let previous: SettingsDocument =
            serde_json::from_value(memory.get(PREVIOUS_KEY).unwrap()).unwrap();
        assert_eq!(previous, with_view(DefaultView::Grid));
    }

    #[test]
    fn a_file_that_was_not_a_store_at_all_counts_as_recovered() {
        let (memory, storage) = setup();
        memory.unreadable.store(true, Ordering::Release);
        assert_eq!(storage.load().unwrap(), None);
        assert!(storage.recovered());
    }

    #[test]
    fn a_missing_body_section_loads_with_defaults() {
        let (memory, storage) = setup();
        memory.set(SETTINGS_KEY, json!({ "version": 1, "body": {} }));
        assert_eq!(
            storage.load().unwrap(),
            Some(SettingsDocument::new(Settings::default()))
        );
    }

    /// A key-value file in a directory, the way the app's own is a JSON object on disk.
    struct JsonFile {
        path: PathBuf,
        values: Mutex<serde_json::Map<String, Value>>,
    }

    impl JsonFile {
        fn open(path: PathBuf) -> Self {
            let values = std::fs::read(&path)
                .ok()
                .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
                .and_then(|v| v.as_object().cloned())
                .unwrap_or_default();
            Self {
                path,
                values: Mutex::new(values),
            }
        }
    }

    impl KeyValue for JsonFile {
        fn get(&self, key: &str) -> Option<Value> {
            self.values.lock().unwrap().get(key).cloned()
        }
        fn set(&self, key: &str, value: Value) {
            self.values.lock().unwrap().insert(key.to_string(), value);
        }
        fn save(&self) -> Result<(), String> {
            let text =
                serde_json::to_vec(&*self.values.lock().unwrap()).map_err(|e| e.to_string())?;
            std::fs::write(&self.path, text).map_err(|e| e.to_string())
        }
        fn set_aside(&self) -> Option<String> {
            let name = format!("{}.corrupt", self.path.file_name()?.to_string_lossy());
            std::fs::copy(&self.path, self.path.with_file_name(&name)).ok()?;
            Some(name)
        }
    }

    #[test]
    fn settings_survive_a_restart_in_a_real_file_and_other_keys_are_left_alone() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, r#"{"ops":{"concurrency":4}}"#).unwrap();
        {
            let storage = Persistence::new(JsonFile::open(path.clone()));
            assert_eq!(storage.load().unwrap(), None);
            storage.save(&with_view(DefaultView::Grid)).unwrap();
        }
        let storage = Persistence::new(JsonFile::open(path.clone()));
        assert_eq!(storage.load().unwrap(), Some(with_view(DefaultView::Grid)));
        let on_disk: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(
            on_disk["ops"]["concurrency"], 4,
            "the operations key is the ops plugin's"
        );
    }

    #[test]
    fn a_corrupt_file_on_disk_is_copied_aside_and_loading_ends_at_the_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, r#"{"settings":"garbage"}"#).unwrap();
        let storage = Persistence::new(JsonFile::open(path.clone()));
        assert_eq!(storage.load().unwrap(), None);
        assert!(dir.path().join("settings.json.corrupt").exists());
    }

    #[test]
    fn memory_storage_round_trips() {
        let storage = MemoryStorage::default();
        assert_eq!(storage.load().unwrap(), None);
        storage.save(&with_view(DefaultView::Grid)).unwrap();
        assert_eq!(storage.saved(), Some(with_view(DefaultView::Grid)));
    }
}
