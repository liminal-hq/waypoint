// Composes the operations plugin: the seams it is injected with, adapted over the other plugins
// and `tauri-plugin-store`
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// `tauri-plugin-waypoint-ops` calls no other plugin (A4). This module is where the Trash plugin and
// the vfs plugin's listings are adapted to the `Trash` and `SelectionResolver` traits, where the
// undo journal is kept in `ops-journal.json` (the latest document under `journal`, the one before
// it under `previous`, an unreadable file copied aside as `ops-journal.json.corrupt-{unix}`, as the
// session does), and where the settings are kept (`settings.json`, key `ops`).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::Value;
use tauri::{AppHandle, Manager, Wry};
use tauri_plugin_trash::{RestoreTarget, TrashError, TrashExt, FEATURE_TRASH};
use tauri_plugin_waypoint_ops::{OpsDeps, SettingsStorage};
use tauri_plugin_waypoint_vfs::Vfs;
use waypoint_ops::{
    JournalDocument, JournalStorage, Loaded, OpsError, OpsSettings, Protected, Providers,
    SelectionResolver, StorageError, SystemClock, Trash, TrashReceipt,
};
use waypoint_path::{FilePath, VfsPath};
use waypoint_protocol::{Location, VfsError};
use waypoint_vfs::{ListingHandle, LocalProvider, SelectionSpec};

use crate::storage::{FileKeyValue, KeyValue};

/// The journal's store file, relative to the app data directory.
pub const JOURNAL_FILE: &str = "ops-journal.json";
const JOURNAL: &str = "journal";
const PREVIOUS: &str = "previous";

/// The settings store file and the key the operations settings live under.
pub const SETTINGS_FILE: &str = "settings.json";
const SETTINGS_KEY: &str = "ops";

/// The operations plugin's dependencies, made once the app exists.
pub fn deps(app: &AppHandle<Wry>) -> OpsDeps {
    let journal_storage: Arc<dyn JournalStorage> = match FileKeyValue::open_file(app, JOURNAL_FILE)
    {
        Ok(kv) => Arc::new(JournalPersistence::new(kv)),
        Err(e) => {
            log::warn!("could not open the journal file, undo will not survive a restart: {e}");
            Arc::new(JournalPersistence::new(MemoryKv::default()))
        }
    };
    let settings: Arc<dyn SettingsStorage> = match FileKeyValue::open_file(app, SETTINGS_FILE) {
        Ok(kv) => Arc::new(SettingsPersistence::new(kv)),
        Err(e) => {
            log::warn!("could not open the settings file, settings will not be saved: {e}");
            Arc::new(tauri_plugin_waypoint_ops::MemorySettings::default())
        }
    };
    OpsDeps::new(
        Providers::single(Arc::new(LocalProvider::new())),
        Arc::new(TrashAdapter { app: app.clone() }),
        Arc::new(VfsSelectionResolver { app: app.clone() }),
        journal_storage,
        settings,
        Arc::new(SystemClock),
        protected(app),
    )
}

/// A key-value map that is never written to disk, for when a store file cannot be opened.
#[derive(Default)]
struct MemoryKv {
    values: Mutex<std::collections::HashMap<String, Value>>,
}

impl KeyValue for MemoryKv {
    fn get(&self, key: &str) -> Option<Value> {
        self.values
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(key)
            .cloned()
    }

    fn set(&self, key: &str, value: Value) {
        self.values
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(key.to_owned(), value);
    }

    fn save(&self) -> Result<(), String> {
        Ok(())
    }

    fn set_aside(&self) -> Option<String> {
        None
    }
}

// ---- the Trash ----

/// The `trash` plugin behind the engine's `Trash`. The engine's workers are plain threads, so a
/// call blocks one of them on the plugin's async API without touching the async runtime's threads.
pub struct TrashAdapter {
    app: AppHandle<Wry>,
}

fn file_path(location: &Location) -> Result<PathBuf, OpsError> {
    match VfsPath::from_location(location) {
        Ok(VfsPath::File(path)) => Ok(path.as_path().to_path_buf()),
        _ => Err(OpsError::Unsupported {
            what: "the Trash for something that is not a local file".to_owned(),
        }),
    }
}

fn location_of(path: &Path) -> Result<Location, OpsError> {
    FilePath::from_path(path)
        .map(|p| p.to_location())
        .map_err(|_| OpsError::Io {
            message: format!("{} is not a usable path", path.display()),
        })
}

/// What the engine says for something the `trash` plugin refused, at `at`.
pub fn ops_error(error: TrashError, at: &Location) -> OpsError {
    match error {
        TrashError::NotFound => OpsError::NotFound {
            location: at.clone(),
        },
        TrashError::PermissionDenied => OpsError::PermissionDenied {
            location: at.clone(),
        },
        TrashError::TrashUnavailable { reason } => OpsError::TrashUnavailable { reason },
        TrashError::Unsupported => OpsError::TrashUnavailable {
            reason: "the Trash is not supported on this system".to_owned(),
        },
        TrashError::Refused { .. } => OpsError::Protected {
            location: at.clone(),
        },
        TrashError::OriginExists { path } => OpsError::NameInUse {
            location: location_of(&path).unwrap_or_else(|_| at.clone()),
        },
        TrashError::OriginMissingParent { path } => OpsError::NotFound {
            location: location_of(&path).unwrap_or_else(|_| at.clone()),
        },
        TrashError::Io { message } => OpsError::Io { message },
    }
}

fn engine_receipt(receipt: tauri_plugin_trash::TrashReceipt) -> Result<TrashReceipt, OpsError> {
    Ok(TrashReceipt {
        id: receipt.trash_id,
        original: location_of(&receipt.original_path)?,
        deleted_at: receipt.deleted_at.saturating_mul(1000),
    })
}

fn plugin_receipt(receipt: &TrashReceipt) -> Result<tauri_plugin_trash::TrashReceipt, OpsError> {
    Ok(tauri_plugin_trash::TrashReceipt {
        trash_id: receipt.id.clone(),
        original_path: file_path(&receipt.original)?,
        deleted_at: receipt.deleted_at / 1000,
    })
}

impl Trash for TrashAdapter {
    fn available(&self) -> Result<(), String> {
        let status = self.app.trash().get_status();
        match status.features.iter().find(|f| f.name == FEATURE_TRASH) {
            Some(feature) if feature.available => Ok(()),
            Some(feature) => Err(feature
                .reason
                .clone()
                .unwrap_or_else(|| "the Trash is not available".to_owned())),
            None => Err("the Trash is not available".to_owned()),
        }
    }

    fn trash(&self, items: &[Location]) -> Vec<Result<TrashReceipt, OpsError>> {
        // Items that are not local files are refused one by one; the rest go as one batch.
        let paths: Vec<Result<PathBuf, OpsError>> = items.iter().map(file_path).collect();
        let batch: Vec<PathBuf> = paths
            .iter()
            .filter_map(|p| p.as_ref().ok().cloned())
            .collect();
        let mut results = tauri::async_runtime::block_on(self.app.trash().trash(batch)).into_iter();
        items
            .iter()
            .zip(paths)
            .map(|(item, path)| {
                path?;
                match results.next() {
                    Some(Ok(receipt)) => engine_receipt(receipt),
                    Some(Err(error)) => Err(ops_error(error, item)),
                    None => Err(OpsError::Io {
                        message: "the Trash reported nothing for this item".to_owned(),
                    }),
                }
            })
            .collect()
    }

    fn restore(&self, receipt: &TrashReceipt) -> Result<Location, OpsError> {
        let given = plugin_receipt(receipt)?;
        let restored = tauri::async_runtime::block_on(
            self.app.trash().restore(&given, RestoreTarget::Original),
        )
        .map_err(|e| ops_error(e, &receipt.original))?;
        location_of(&restored.original_path)
    }

    fn delete(&self, receipt: &TrashReceipt) -> Result<(), OpsError> {
        let given = plugin_receipt(receipt)?;
        tauri::async_runtime::block_on(self.app.trash().delete(&given))
            .map_err(|e| ops_error(e, &receipt.original))
    }

    fn empty(&self, older_than_days: Option<u32>) -> Result<u64, OpsError> {
        let at = Location::new("", "");
        let report = tauri::async_runtime::block_on(self.app.trash().empty(older_than_days))
            .map_err(|e| ops_error(e, &at))?;
        Ok(u64::from(report.removed))
    }

    /// Whether the item is still in the Trash, by its id, from the plugin's list.
    fn contains(&self, receipt: &TrashReceipt) -> Result<bool, OpsError> {
        let at = &receipt.original;
        let items = tauri::async_runtime::block_on(self.app.trash().list())
            .map_err(|e| ops_error(e, at))?;
        Ok(items.iter().any(|i| i.receipt.trash_id == receipt.id))
    }

    fn receipt_for(&self, trashed: &Location) -> Result<TrashReceipt, OpsError> {
        // The Trash has no location of its own yet (the browsable Trash view is a later slice), so
        // there is nothing a location could name.
        let _ = trashed;
        Err(OpsError::Unsupported {
            what: "restoring from a location in the Trash".to_owned(),
        })
    }
}

// ---- selections ----

/// The vfs plugin's listings behind the engine's `SelectionResolver` (A47): a selection is a
/// handle and a range spec, never a list of paths.
pub struct VfsSelectionResolver {
    app: AppHandle<Wry>,
}

impl SelectionResolver for VfsSelectionResolver {
    fn resolve(
        &self,
        handle: ListingHandle,
        spec: &SelectionSpec,
        window: &str,
    ) -> Result<Vec<Location>, OpsError> {
        let vfs = self
            .app
            .try_state::<Vfs>()
            .ok_or_else(|| OpsError::Unsupported {
                what: "listings are not available".to_owned(),
            })?;
        vfs.resolve_selection(window, handle, spec)
            .map_err(|e: VfsError| OpsError::from(e))
    }
}

// ---- protected paths ----

/// The paths no operation removes or moves: the home folder and every mount point (roots are
/// protected by the engine itself).
fn protected(app: &AppHandle<Wry>) -> Protected {
    let mut paths: Vec<PathBuf> = Vec::new();
    if let Ok(home) = app.path().home_dir() {
        paths.push(home);
    }
    #[cfg(target_os = "linux")]
    if let Ok(text) = std::fs::read_to_string("/proc/self/mounts") {
        paths.extend(mount_points(&text));
    }
    Protected::new(
        paths
            .iter()
            .filter_map(|p| FilePath::from_path(p).ok())
            .map(VfsPath::File)
            .collect(),
    )
}

/// The mount points a `/proc/self/mounts` table lists. A space in a path is written `\040`.
#[cfg(any(test, target_os = "linux"))]
pub fn mount_points(table: &str) -> Vec<PathBuf> {
    table
        .lines()
        .filter_map(|line| line.split_whitespace().nth(1))
        .filter(|target| target.starts_with('/') && *target != "/")
        .map(|target| {
            PathBuf::from(
                target
                    .replace("\\040", " ")
                    .replace("\\011", "\t")
                    .replace("\\012", "\n")
                    .replace("\\134", "\\"),
            )
        })
        .collect()
}

// ---- the journal ----

/// The journal over any `KeyValue`: two generations, rotated on the first save of each run, with an
/// unreadable one copied aside once.
pub struct JournalPersistence<K: KeyValue> {
    kv: K,
    rotated: AtomicBool,
    set_aside: Mutex<Option<String>>,
}

impl<K: KeyValue> JournalPersistence<K> {
    pub fn new(kv: K) -> Self {
        Self {
            kv,
            rotated: AtomicBool::new(false),
            set_aside: Mutex::new(None),
        }
    }

    /// Copies the stored file aside, once per run, and says where.
    fn keep_copy(&self) -> Option<String> {
        let mut kept = self.set_aside.lock().unwrap_or_else(|e| e.into_inner());
        if kept.is_none() {
            *kept = self.kv.set_aside();
            match &*kept {
                Some(name) => log::warn!("kept a copy of the undo journal as `{name}`"),
                None => log::warn!("could not keep a copy of the unreadable undo journal"),
            }
        }
        kept.clone()
    }

    fn read(&self, key: &str) -> Option<Result<JournalDocument, String>> {
        let value = self.kv.get(key)?;
        Some(serde_json::from_value::<JournalDocument>(value).map_err(|e| e.to_string()))
    }
}

impl<K: KeyValue> JournalStorage for JournalPersistence<K> {
    fn load(&self) -> Result<Loaded, StorageError> {
        let mut loaded = Loaded::default();
        let mut bad: Option<String> = self
            .kv
            .was_unreadable()
            .then(|| "the journal file is not a JSON object".to_owned());
        for key in [JOURNAL, PREVIOUS] {
            match self.read(key) {
                None => {}
                Some(Ok(document)) => {
                    loaded.document = Some(document);
                    loaded.from_previous = key == PREVIOUS && bad.is_some();
                    break;
                }
                Some(Err(why)) => {
                    log::warn!("the stored undo journal `{key}` is unusable ({why})");
                    bad = Some(why);
                }
            }
        }
        if bad.is_some() {
            // A copy of the whole file, so the unreadable generation can be looked at.
            if !self.kv.was_unreadable() {
                loaded.set_aside = self.keep_copy();
            }
            if loaded.document.is_none() {
                loaded.unreadable = bad;
            }
        }
        Ok(loaded)
    }

    fn save(&self, document: &JournalDocument) -> Result<(), StorageError> {
        let value = serde_json::to_value(document).map_err(|e| StorageError::Io(e.to_string()))?;
        if !self.rotated.swap(true, Ordering::AcqRel) {
            // The first save of the run: what is on disk now is how the run started. A copy that
            // is unusable is not worth keeping over an earlier good `previous`.
            if let Some(Ok(_)) = self.read(JOURNAL) {
                if let Some(current) = self.kv.get(JOURNAL) {
                    self.kv.set(PREVIOUS, current);
                }
            }
        }
        self.kv.set(JOURNAL, value);
        self.kv.save().map_err(StorageError::Io)
    }

    fn set_aside(&self) -> Option<String> {
        self.keep_copy()
    }
}

// ---- the settings ----

/// The operations settings over any `KeyValue`, under the key `ops`.
pub struct SettingsPersistence<K: KeyValue> {
    kv: K,
}

impl<K: KeyValue> SettingsPersistence<K> {
    pub fn new(kv: K) -> Self {
        Self { kv }
    }
}

impl<K: KeyValue> SettingsStorage for SettingsPersistence<K> {
    fn load(&self) -> Result<Option<OpsSettings>, String> {
        match self.kv.get(SETTINGS_KEY) {
            None => Ok(None),
            Some(value) => serde_json::from_value::<OpsSettings>(value)
                .map(Some)
                .map_err(|e| e.to_string()),
        }
    }

    fn save(&self, settings: &OpsSettings) -> Result<(), String> {
        let value: Value = serde_json::to_value(settings).map_err(|e| e.to_string())?;
        self.kv.set(SETTINGS_KEY, value);
        self.kv.save()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use serde_json::json;
    use waypoint_ops::JournalBody;

    #[derive(Default)]
    struct Memory {
        values: Mutex<HashMap<String, Value>>,
        saves: Mutex<u32>,
        aside: Mutex<u32>,
        unreadable: bool,
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
            Some("ops-journal.json.corrupt-1".into())
        }
        fn was_unreadable(&self) -> bool {
            self.unreadable
        }
    }

    fn document(revision: u64) -> JournalDocument {
        JournalDocument::new(JournalBody {
            revision,
            next_id: 1,
            ..JournalBody::default()
        })
    }

    fn setup() -> (Arc<Memory>, JournalPersistence<Arc<Memory>>) {
        let memory = Arc::new(Memory::default());
        (memory.clone(), JournalPersistence::new(memory))
    }

    fn put(memory: &Memory, key: &str, value: Value) {
        memory.values.lock().unwrap().insert(key.into(), value);
    }

    #[test]
    fn nothing_stored_loads_nothing_quietly() {
        let (memory, storage) = setup();
        assert_eq!(storage.load().unwrap(), Loaded::default());
        assert_eq!(*memory.aside.lock().unwrap(), 0);
    }

    #[test]
    fn a_saved_journal_loads_back() {
        let (_, storage) = setup();
        storage.save(&document(3)).unwrap();
        let loaded = storage.load().unwrap();
        assert_eq!(loaded.document, Some(document(3)));
        assert!(!loaded.from_previous && loaded.unreadable.is_none());
    }

    #[test]
    fn the_first_save_of_a_run_rotates_the_stored_document_into_previous() {
        let (memory, storage) = setup();
        put(&memory, JOURNAL, serde_json::to_value(document(1)).unwrap());
        storage.save(&document(2)).unwrap();
        storage.save(&document(3)).unwrap();
        let previous: JournalDocument =
            serde_json::from_value(memory.get(PREVIOUS).unwrap()).unwrap();
        assert_eq!(previous, document(1), "only the first save rotates");
    }

    #[test]
    fn a_corrupt_journal_falls_back_to_previous_and_is_set_aside_once() {
        let (memory, storage) = setup();
        put(&memory, JOURNAL, json!({ "nonsense": true }));
        put(
            &memory,
            PREVIOUS,
            serde_json::to_value(document(7)).unwrap(),
        );
        let loaded = storage.load().unwrap();
        assert_eq!(loaded.document, Some(document(7)));
        assert!(loaded.from_previous);
        assert_eq!(
            loaded.set_aside.as_deref(),
            Some("ops-journal.json.corrupt-1")
        );
        assert!(loaded.unreadable.is_none());
        let _ = storage.load().unwrap();
        assert_eq!(*memory.aside.lock().unwrap(), 1, "kept once per run");
    }

    #[test]
    fn corrupt_journal_and_previous_load_nothing_and_say_why() {
        let (memory, storage) = setup();
        put(&memory, JOURNAL, json!("not a document"));
        put(&memory, PREVIOUS, json!([1, 2]));
        let loaded = storage.load().unwrap();
        assert!(loaded.document.is_none());
        assert!(loaded.unreadable.is_some());
        assert!(loaded.set_aside.is_some());
        assert_eq!(*memory.aside.lock().unwrap(), 1);
    }

    #[test]
    fn a_file_that_was_not_a_store_at_all_is_reported_without_a_second_copy() {
        let memory = Arc::new(Memory {
            unreadable: true,
            ..Memory::default()
        });
        let storage = JournalPersistence::new(memory.clone());
        let loaded = storage.load().unwrap();
        assert!(loaded.document.is_none());
        assert!(loaded.unreadable.is_some());
        assert_eq!(*memory.aside.lock().unwrap(), 0, "open() already copied it");
    }

    #[test]
    fn an_unusable_latest_copy_is_not_rotated_over_a_good_previous() {
        let (memory, storage) = setup();
        put(&memory, JOURNAL, json!("garbage"));
        put(
            &memory,
            PREVIOUS,
            serde_json::to_value(document(5)).unwrap(),
        );
        storage.save(&document(6)).unwrap();
        let previous: JournalDocument =
            serde_json::from_value(memory.get(PREVIOUS).unwrap()).unwrap();
        assert_eq!(previous, document(5));
    }

    #[test]
    fn settings_save_and_load_under_the_ops_key() {
        let memory = Arc::new(Memory::default());
        let storage = SettingsPersistence::new(memory.clone());
        assert_eq!(storage.load().unwrap(), None);
        let settings = OpsSettings {
            concurrency: 4,
            ..OpsSettings::default()
        };
        storage.save(&settings).unwrap();
        assert_eq!(storage.load().unwrap(), Some(settings));
        assert_eq!(memory.get(SETTINGS_KEY).unwrap()["concurrency"], 4);
        put(&memory, SETTINGS_KEY, json!("nonsense"));
        assert!(storage.load().is_err());
    }

    #[test]
    fn the_mount_table_gives_mount_points_without_the_root() {
        let table = "/dev/sda1 / ext4 rw 0 0\n\
                     tmpfs /run/user/1000 tmpfs rw 0 0\n\
                     /dev/sdb1 /mnt/my\\040disk ext4 rw 0 0\n\
                     proc proc proc rw 0 0\n";
        assert_eq!(
            mount_points(table),
            vec![
                PathBuf::from("/run/user/1000"),
                PathBuf::from("/mnt/my disk")
            ]
        );
    }

    #[test]
    fn the_trash_plugins_refusals_become_the_engines() {
        let at = Location::new("/a", "file:///a");
        assert_eq!(
            ops_error(TrashError::NotFound, &at),
            OpsError::NotFound {
                location: at.clone()
            }
        );
        assert_eq!(
            ops_error(
                TrashError::Refused {
                    reason: "home".into()
                },
                &at
            ),
            OpsError::Protected {
                location: at.clone()
            }
        );
        assert!(matches!(
            ops_error(
                TrashError::OriginExists {
                    path: PathBuf::from("/a")
                },
                &at
            ),
            OpsError::NameInUse { .. }
        ));
        assert!(matches!(
            ops_error(TrashError::Unsupported, &at),
            OpsError::TrashUnavailable { .. }
        ));
    }

    // ---- who may call the plugin ----

    struct NoTrash;

    impl Trash for NoTrash {
        fn available(&self) -> Result<(), String> {
            Err("none".into())
        }
        fn trash(&self, items: &[Location]) -> Vec<Result<TrashReceipt, OpsError>> {
            items
                .iter()
                .map(|_| {
                    Err(OpsError::TrashUnavailable {
                        reason: "none".into(),
                    })
                })
                .collect()
        }
        fn restore(&self, _: &TrashReceipt) -> Result<Location, OpsError> {
            Err(OpsError::TrashUnavailable {
                reason: "none".into(),
            })
        }
        fn delete(&self, _: &TrashReceipt) -> Result<(), OpsError> {
            Err(OpsError::TrashUnavailable {
                reason: "none".into(),
            })
        }
        fn empty(&self, _: Option<u32>) -> Result<u64, OpsError> {
            Err(OpsError::TrashUnavailable {
                reason: "none".into(),
            })
        }
        fn receipt_for(&self, trashed: &Location) -> Result<TrashReceipt, OpsError> {
            Err(OpsError::NotFound {
                location: trashed.clone(),
            })
        }
    }

    struct NoSelections;

    impl SelectionResolver for NoSelections {
        fn resolve(
            &self,
            _: ListingHandle,
            _: &SelectionSpec,
            _: &str,
        ) -> Result<Vec<Location>, OpsError> {
            Ok(Vec::new())
        }
    }

    /// Calls a command of the operations plugin as the page of `window` would, through the IPC
    /// layer, so the app's real capability files decide.
    fn call(
        window: &tauri::WebviewWindow<tauri::test::MockRuntime>,
        command: &str,
    ) -> Result<tauri::ipc::InvokeResponseBody, Value> {
        tauri::test::get_ipc_response(
            window,
            tauri::webview::InvokeRequest {
                cmd: format!("plugin:waypoint-ops|{command}"),
                callback: tauri::ipc::CallbackFn(0),
                error: tauri::ipc::CallbackFn(1),
                url: if cfg!(windows) {
                    "http://tauri.localhost"
                } else {
                    "tauri://localhost"
                }
                .parse()
                .unwrap(),
                body: tauri::ipc::InvokeBody::default(),
                headers: Default::default(),
                invoke_key: tauri::test::INVOKE_KEY.to_string(),
            },
        )
    }

    #[test]
    fn only_the_main_windows_and_the_operations_window_may_call_the_plugin() {
        use tauri::{WebviewUrl, WebviewWindowBuilder};
        let deps = OpsDeps::new(
            Providers::new(),
            Arc::new(NoTrash),
            Arc::new(NoSelections),
            Arc::new(JournalPersistence::new(MemoryKv::default())),
            Arc::new(tauri_plugin_waypoint_ops::MemorySettings::default()),
            Arc::new(SystemClock),
            Protected::default(),
        );
        let app = tauri::test::mock_builder()
            .plugin(tauri_plugin_waypoint_ops::init(deps))
            .build(tauri::generate_context!())
            .expect("the mock app builds with the app's own capabilities");
        let open = |label: &str| {
            WebviewWindowBuilder::new(&app, label, WebviewUrl::App("index.html".into()))
                .build()
                .expect("the mock window opens")
        };
        for label in ["main-1", "main-7", "ops"] {
            let window = open(label);
            for command in ["get_clipboard", "get_snapshot", "get_settings"] {
                let answer = call(&window, command);
                assert!(answer.is_ok(), "{label} may call {command}: {answer:?}");
            }
        }
        for label in ["settings", "properties-1", "tear-ghost"] {
            let window = open(label);
            for command in ["get_clipboard", "submit", "undo", "set_settings"] {
                let refused = call(&window, command).expect_err("a refused call");
                assert!(
                    refused.to_string().contains("not allowed"),
                    "{label} may not call {command}: {refused}"
                );
            }
        }
    }
}
