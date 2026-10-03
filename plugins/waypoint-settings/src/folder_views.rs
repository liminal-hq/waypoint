// The remembered views: one store, one writer, a revision and a change event to every window
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::sync::{Arc, Mutex, MutexGuard};

use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager, Runtime};
use waypoint_settings::{
    plan_folder_views, BundleError, ConfigFile, FilePlan, FolderViewPatch, FolderViews,
    FolderViewsChanged, FolderViewsDocument, FolderViewsSnapshot, FolderViewsStorage,
    FOLDER_VIEWS_FILE_ID,
};

use crate::error::Error;
use crate::FOLDER_VIEWS_EVENT;

fn locked<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    // A poisoned lock only means a panic elsewhere; the state is still consistent.
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

/// What each folder remembers about its view, sort and grouping. Rust owns it: a window asks for
/// a change and hears the answer as an event carrying the new revision and the folders touched.
pub struct FolderViewsStore<R: Runtime> {
    app: AppHandle<R>,
    storage: Arc<dyn FolderViewsStorage>,
    current: Mutex<FolderViews>,
    /// Held across a whole change, so two changes never interleave their save and event.
    writer: Mutex<()>,
}

impl<R: Runtime> FolderViewsStore<R> {
    /// Loads what the storage holds. Views that cannot be read are never fatal: folders then
    /// remember nothing.
    pub fn new(app: AppHandle<R>, storage: Arc<dyn FolderViewsStorage>) -> Self {
        let views = match storage.load() {
            Ok(Some(document)) => FolderViews::from_document(document),
            Ok(None) => FolderViews::new(),
            Err(why) => {
                log::warn!("could not load the folder views, starting with none: {why}");
                FolderViews::new()
            }
        };
        Self {
            app,
            storage,
            current: Mutex::new(views),
            writer: Mutex::new(()),
        }
    }

    pub fn snapshot(&self) -> FolderViewsSnapshot {
        locked(&self.current).snapshot()
    }

    pub fn document(&self) -> FolderViewsDocument {
        locked(&self.current).to_document()
    }

    /// The store as it is now, for planning an import against.
    pub fn views(&self) -> FolderViews {
        locked(&self.current).clone()
    }

    /// Remembers `patch` for the folder at `key`. Returns the revision in force.
    pub fn remember(&self, key: &str, patch: FolderViewPatch) -> Result<u64, Error> {
        self.change(|views| Ok(views.remember(key, patch)?))
    }

    /// Forgets the folder at `key`. Returns the revision in force.
    pub fn forget(&self, key: &str) -> Result<u64, Error> {
        self.change(|views| Ok(views.forget(key)))
    }

    /// Replaces everything with an imported document (validated as any write is). Returns the
    /// revision in force.
    pub fn replace(&self, document: FolderViewsDocument) -> Result<u64, Error> {
        self.change(|views| Ok(views.replace_all(document.folders)?))
    }

    /// Applies `change` to a copy, saves the copy, and only then makes it the one in force and
    /// announces it: a save that fails changes nothing.
    fn change(
        &self,
        change: impl FnOnce(&mut FolderViews) -> Result<Option<FolderViewsChanged>, Error>,
    ) -> Result<u64, Error> {
        let _writer = locked(&self.writer);
        let mut next = self.views();
        let Some(changed) = change(&mut next)? else {
            return Ok(next.revision());
        };
        self.storage
            .save(&next.to_document())
            .map_err(|e| Error::Storage(e.to_string()))?;
        *locked(&self.current) = next;
        if let Err(e) = self.app.emit(FOLDER_VIEWS_EVENT, changed.clone()) {
            log::warn!("could not announce the folder views change: {e}");
        }
        Ok(changed.revision)
    }
}

/// The remembered views as a configuration file of the settings export.
pub struct FolderViewsConfigFile<R: Runtime>(pub AppHandle<R>);

impl<R: Runtime> FolderViewsConfigFile<R> {
    fn store(&self) -> tauri::State<'_, FolderViewsStore<R>> {
        self.0.state::<FolderViewsStore<R>>()
    }
}

impl<R: Runtime> ConfigFile for FolderViewsConfigFile<R> {
    fn id(&self) -> &str {
        FOLDER_VIEWS_FILE_ID
    }

    fn export(&self) -> Result<Value, String> {
        serde_json::to_value(self.store().document()).map_err(|e| e.to_string())
    }

    fn plan(&self, incoming: &Value) -> Result<FilePlan, BundleError> {
        plan_folder_views(&self.store().views(), incoming)
    }

    fn apply(&self, document: &Value) -> Result<(), String> {
        let document: FolderViewsDocument =
            serde_json::from_value(document.clone()).map_err(|e| e.to_string())?;
        self.store()
            .replace(document)
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}
