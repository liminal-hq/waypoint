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
use std::time::{Duration, Instant};

use serde_json::Value;
use tauri::{AppHandle, Manager, Runtime, Wry};
use tauri_plugin_trash::{
    RestoreTarget, TrashError, TrashExt, FEATURE_EXPIRY, FEATURE_LIST, FEATURE_TRASH,
};
use tauri_plugin_waypoint_ops::{Ops, OpsDeps, SettingsStorage};
use tauri_plugin_waypoint_vfs::Vfs;
use waypoint_ops::{
    JobId, JobKind, JobOptions, JobRequest, JournalDocument, JournalStorage, Loaded, OpsError,
    OpsSettings, Protected, Providers, SelectionResolver, Sources, StorageError, SystemClock,
    Trash, TrashReceipt,
};
use waypoint_path::{FilePath, TrashPath, VfsPath};
use waypoint_protocol::{Location, VfsError};
use waypoint_vfs::{
    ListingHandle, LocalProvider, SelectionSpec, TrashProvider, TrashSource, TrashedItem,
};

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
    compose(app, journal_storage, settings, protected(app))
}

/// The operations plugin's dependencies over the plugins that are set up in `app`, with the given
/// journal, settings and protected paths. One `TrashAdapter` serves the engine (`Trash`) and the
/// Trash view (`TrashSource`), so they share its short-lived list; the vfs plugin was set up first
/// and is given the view's half here.
pub fn compose<R: Runtime>(
    app: &AppHandle<R>,
    journal_storage: Arc<dyn JournalStorage>,
    settings: Arc<dyn SettingsStorage>,
    protected: Protected,
) -> OpsDeps {
    let trash = Arc::new(TrashAdapter::new(app.clone()));
    let mut providers = Providers::single(Arc::new(LocalProvider::new()));
    match app.try_state::<Vfs>() {
        Some(vfs) => {
            vfs.set_trash_source(trash.clone());
            if let Some(provider) = vfs.trash_provider() {
                providers.register(provider);
            }
        }
        None => {
            log::warn!("the file system plugin is not set up, so the Trash cannot be browsed");
            providers.register(Arc::new(TrashProvider::new(trash.clone())));
        }
    }
    OpsDeps::new(
        providers,
        trash,
        Arc::new(VfsSelectionResolver { app: app.clone() }),
        journal_storage,
        settings,
        Arc::new(SystemClock),
        protected,
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

/// How long the list of the Trash is kept for lookups, so restoring a thousand items is one list and
/// not a thousand. Anything this app does to the Trash drops it, and another program's change shows
/// within a second.
const LIST_TTL: Duration = Duration::from_secs(1);

/// The `trash` plugin behind the engine's `Trash` and the Trash view's `TrashSource` (A4: this is
/// where the plugins meet). The engine's workers are plain threads, so a call blocks one of them on
/// the plugin's async API without touching the async runtime's threads.
pub struct TrashAdapter<R: Runtime = Wry> {
    app: AppHandle<R>,
    listed: Mutex<Option<(Instant, Arc<Vec<tauri_plugin_trash::TrashedItem>>)>>,
}

impl<R: Runtime> TrashAdapter<R> {
    pub fn new(app: AppHandle<R>) -> Self {
        Self {
            app,
            listed: Mutex::new(None),
        }
    }

    fn forget_list(&self) {
        *self.listed.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }

    /// Everything in the Trash, from the plugin or from a list taken a moment ago.
    fn items(&self) -> Result<Arc<Vec<tauri_plugin_trash::TrashedItem>>, TrashError> {
        {
            let listed = self.listed.lock().unwrap_or_else(|e| e.into_inner());
            if let Some((at, items)) = &*listed {
                if at.elapsed() < LIST_TTL {
                    return Ok(items.clone());
                }
            }
        }
        let items = Arc::new(tauri::async_runtime::block_on(self.app.trash().list())?);
        *self.listed.lock().unwrap_or_else(|e| e.into_inner()) =
            Some((Instant::now(), items.clone()));
        Ok(items)
    }

    /// Whether a feature of the plugin works here, and if not why.
    fn feature(&self, name: &str) -> Result<(), String> {
        let status = self.app.trash().get_status();
        match status.features.iter().find(|f| f.name == name) {
            Some(feature) if feature.available => Ok(()),
            Some(feature) => Err(feature
                .reason
                .clone()
                .unwrap_or_else(|| format!("the Trash cannot do this here ({name})"))),
            None => Err("the Trash is not available".to_owned()),
        }
    }
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
        TrashError::OriginMissingParent { path } => OpsError::OriginMissingParent {
            location: location_of(&path).unwrap_or_else(|_| at.clone()),
        },
        TrashError::Io { message } => OpsError::Io { message },
    }
}

/// What the Trash view says for something the plugin refused, for the item at `at`.
fn vfs_error(error: TrashError, at: &Location) -> VfsError {
    match error {
        TrashError::NotFound => VfsError::NotFound {
            location: at.clone(),
        },
        TrashError::PermissionDenied => VfsError::PermissionDenied {
            location: at.clone(),
        },
        TrashError::OriginExists { path } => VfsError::AlreadyExists {
            location: location_of(&path).unwrap_or_else(|_| at.clone()),
        },
        TrashError::OriginMissingParent { path } => VfsError::NotFound {
            location: location_of(&path).unwrap_or_else(|_| at.clone()),
        },
        TrashError::TrashUnavailable { reason } => VfsError::Unsupported { what: reason },
        TrashError::Unsupported => VfsError::Unsupported {
            what: "the Trash is not supported on this system".to_owned(),
        },
        TrashError::Refused { reason } => VfsError::Unsupported { what: reason },
        TrashError::Io { message } => VfsError::Io {
            message,
            location: Some(at.clone()),
        },
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

/// The location the Trash view gives the item with this id.
fn trashed_location(id: &str) -> Location {
    VfsPath::Trash(TrashPath::Item(id.to_owned())).to_location()
}

/// The item as the Trash view lists it: its original name, and the folder it was in.
fn view_item(item: &tauri_plugin_trash::TrashedItem) -> TrashedItem {
    TrashedItem {
        id: item.receipt.trash_id.clone(),
        name: item.name.clone(),
        original_path: item
            .original_path
            .parent()
            .map(|folder| folder.to_string_lossy().into_owned())
            .unwrap_or_default(),
        deleted_ms: item.deleted_at.saturating_mul(1000),
        size: item.size,
        is_dir: item.is_dir,
    }
}

impl<R: Runtime> Trash for TrashAdapter<R> {
    fn available(&self) -> Result<(), String> {
        self.feature(FEATURE_TRASH)
    }

    fn trash(&self, items: &[Location]) -> Vec<Result<TrashReceipt, OpsError>> {
        // Items that are not local files are refused one by one; the rest go as one batch.
        let paths: Vec<Result<PathBuf, OpsError>> = items.iter().map(file_path).collect();
        let batch: Vec<PathBuf> = paths
            .iter()
            .filter_map(|p| p.as_ref().ok().cloned())
            .collect();
        let mut results = tauri::async_runtime::block_on(self.app.trash().trash(batch)).into_iter();
        self.forget_list();
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
        );
        self.forget_list();
        let restored = restored.map_err(|e| ops_error(e, &receipt.original))?;
        location_of(&restored.original_path)
    }

    fn restore_to(&self, receipt: &TrashReceipt, target: &Location) -> Result<Location, OpsError> {
        let given = plugin_receipt(receipt)?;
        let path = file_path(target)?;
        let restored = tauri::async_runtime::block_on(
            self.app
                .trash()
                .restore(&given, RestoreTarget::Path { path }),
        );
        self.forget_list();
        let restored = restored.map_err(|e| ops_error(e, target))?;
        location_of(&restored.original_path)
    }

    fn delete(&self, receipt: &TrashReceipt) -> Result<(), OpsError> {
        let given = plugin_receipt(receipt)?;
        let deleted = tauri::async_runtime::block_on(self.app.trash().delete(&given));
        self.forget_list();
        deleted.map_err(|e| ops_error(e, &trashed_location(&receipt.id)))
    }

    fn empty(&self, older_than_days: Option<u32>) -> Result<u64, OpsError> {
        let at = trashed_location("");
        let report = tauri::async_runtime::block_on(self.app.trash().empty(older_than_days));
        self.forget_list();
        let report = report.map_err(|e| ops_error(e, &at))?;
        for failed in &report.failed {
            log::warn!(
                "the Trash could not remove `{}`: {}",
                failed.trash_id,
                failed.error
            );
        }
        Ok(u64::from(report.removed))
    }

    /// Whether the item is still in the Trash, by its id, from the plugin's list.
    fn contains(&self, receipt: &TrashReceipt) -> Result<bool, OpsError> {
        let at = &receipt.original;
        let items = self.items().map_err(|e| ops_error(e, at))?;
        Ok(items.iter().any(|i| i.receipt.trash_id == receipt.id))
    }

    /// The receipt of the item the Trash view shows at `trash:/{id}`, read from the Trash itself so
    /// it carries the item's real original path and date.
    fn receipt_for(&self, trashed: &Location) -> Result<TrashReceipt, OpsError> {
        let Ok(VfsPath::Trash(TrashPath::Item(id))) = VfsPath::from_location(trashed) else {
            return Err(OpsError::NotFound {
                location: trashed.clone(),
            });
        };
        let items = self.items().map_err(|e| ops_error(e, trashed))?;
        match items.iter().find(|item| item.receipt.trash_id == id) {
            Some(item) => engine_receipt(item.receipt.clone()),
            None => Err(OpsError::NotFound {
                location: trashed.clone(),
            }),
        }
    }

    fn is_trashed(&self, location: &Location) -> bool {
        matches!(
            VfsPath::from_location(location),
            Ok(VfsPath::Trash(TrashPath::Item(_)))
        )
    }
}

impl<R: Runtime> TrashSource for TrashAdapter<R> {
    /// The Trash view needs the plugin to list; where it cannot (a sandbox's portal only trashes)
    /// the reason is the plugin's own.
    fn available(&self) -> Result<(), String> {
        self.feature(FEATURE_LIST)
    }

    fn list(&self) -> Result<Vec<TrashedItem>, VfsError> {
        let at = trashed_location("");
        let items = self.items().map_err(|e| vfs_error(e, &at))?;
        Ok(items.iter().map(view_item).collect())
    }

    fn restore(&self, id: &str) -> Result<Location, VfsError> {
        let at = trashed_location(id);
        let receipt = self.receipt_for(&at).map_err(|_| VfsError::NotFound {
            location: at.clone(),
        })?;
        Trash::restore(self, &receipt).map_err(|e| match e {
            OpsError::NameInUse { location } => VfsError::AlreadyExists { location },
            OpsError::NotFound { location } | OpsError::OriginMissingParent { location } => {
                VfsError::NotFound { location }
            }
            other => VfsError::Io {
                message: other.to_string(),
                location: Some(at.clone()),
            },
        })
    }

    fn delete(&self, id: &str) -> Result<(), VfsError> {
        let at = trashed_location(id);
        let receipt = self.receipt_for(&at).map_err(|_| VfsError::NotFound {
            location: at.clone(),
        })?;
        Trash::delete(self, &receipt).map_err(|e| VfsError::Io {
            message: e.to_string(),
            location: Some(at),
        })
    }

    fn empty(&self, older_than_days: Option<u32>) -> Result<u64, VfsError> {
        Trash::empty(self, older_than_days).map_err(|e| VfsError::Io {
            message: e.to_string(),
            location: None,
        })
    }
}

/// The job that empties what has been in the Trash for `days` days or more.
pub fn sweep_request(days: u32) -> JobRequest {
    JobRequest {
        kind: JobKind::EmptyTrash {
            older_than_days: Some(days),
        },
        sources: Sources::Locations { locations: vec![] },
        destination: None,
        name: None,
        options: JobOptions::default(),
        origin_window: SWEEP_ORIGIN.to_owned(),
    }
}

/// The label a job the app starts for itself comes from. No window has it.
const SWEEP_ORIGIN: &str = "app";

/// Queues the Trash sweep when the setting asks for one, and says whether it did. It is an
/// ordinary job: it shows in the queue, can be cancelled, and reports its own failure.
pub fn submit_sweep<R: Runtime>(ops: &Ops<R>) -> Option<JobId> {
    let days = ops.settings().trash_expiry_days?;
    match ops.submit(SWEEP_ORIGIN, sweep_request(days)) {
        Ok(id) => Some(id),
        Err(error) => {
            log::warn!("could not queue the Trash sweep: {error}");
            None
        }
    }
}

/// How long start-up waits for a window to be on screen before the sweep goes ahead anyway.
const SWEEP_WAIT: Duration = Duration::from_secs(20);

/// Runs the Trash sweep once, after the first window is shown, on a thread of its own so nothing
/// waits for it. Does nothing when the setting is off or this system cannot expire items.
pub fn start_trash_sweep(app: &AppHandle<Wry>) {
    let app = app.clone();
    let spawned = std::thread::Builder::new()
        .name("trash-sweep".to_owned())
        .spawn(move || {
            let Some(ops) = app.try_state::<Ops<Wry>>() else {
                return;
            };
            if ops.settings().trash_expiry_days.is_none() {
                return;
            }
            let started = Instant::now();
            while started.elapsed() < SWEEP_WAIT
                && !app
                    .webview_windows()
                    .values()
                    .any(|w| w.is_visible().unwrap_or(false))
            {
                std::thread::sleep(Duration::from_millis(250));
            }
            let status = app.trash().get_status();
            let can_expire = status
                .features
                .iter()
                .any(|f| f.name == FEATURE_EXPIRY && f.available);
            if !can_expire {
                log::info!("the Trash sweep is on but this system cannot expire Trash items");
                return;
            }
            if let Some(id) = submit_sweep(&ops) {
                log::info!("queued the Trash sweep as job {}", id.0);
            }
        });
    if let Err(error) = spawned {
        log::warn!("could not start the Trash sweep: {error}");
    }
}

// ---- selections ----

/// The vfs plugin's listings behind the engine's `SelectionResolver` (A47): a selection is a
/// handle and a range spec, never a list of paths.
pub struct VfsSelectionResolver<R: Runtime = Wry> {
    app: AppHandle<R>,
}

impl<R: Runtime> SelectionResolver for VfsSelectionResolver<R> {
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

#[cfg(all(test, target_os = "linux"))]
#[path = "ops_trash_tests.rs"]
mod trash_tests;

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
