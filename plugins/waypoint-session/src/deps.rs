// What the composition root injects into the session plugin: window creation, storage, the
// store's policy and the hooks.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::sync::{Arc, Mutex};
use std::time::Duration;

use tauri::{AppHandle, Runtime, Wry};
use thiserror::Error;
use waypoint_session::{Document, Geometry, SessionStorage, StorageError, Store, StorePolicy};

/// How long after a change the `on_change` hook runs, so a burst of changes makes one call.
pub const CHANGE_DELAY: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum WindowError {
    /// The app cannot create windows yet (before the window factory exists).
    #[error("creating windows is not available yet")]
    NotAvailable,
    #[error("{0}")]
    Failed(String),
}

/// Creates the webview for a window the store has just made. The plugin calls it after the store
/// change is made and before any event is sent, with the store locked; an error undoes the change.
/// Implementations must not call back into the session commands, and should return once the
/// window exists, not once its page has loaded (the page reads its own state with `get_snapshot`).
pub trait WindowFactory<R: Runtime = Wry>: Send + Sync {
    fn create(
        &self,
        app: &AppHandle<R>,
        label: &str,
        geometry: Option<&Geometry>,
    ) -> Result<(), WindowError>;
}

/// Runs when the last window of the store closes.
pub type LastWindowHook<R> = Arc<dyn Fn(&AppHandle<R>) + Send + Sync>;

/// Runs with a copy of the store `change_delay` after a change, once per burst of changes.
pub type ChangeHook = Arc<dyn Fn(&Store) + Send + Sync>;

/// Everything the plugin needs from the app.
pub struct SessionDeps<R: Runtime = Wry> {
    pub create_window: Arc<dyn WindowFactory<R>>,
    /// Where the store persists. The plugin only holds it (`Sessions::storage`); loading at start
    /// and saving from `on_change` belong to the app's persistence wiring.
    pub storage: Arc<dyn SessionStorage>,
    pub policy: StorePolicy,
    pub on_last_window_closed: Option<LastWindowHook<R>>,
    /// The hook persistence subscribes to. It is called from a helper thread, never while the
    /// store is locked.
    pub on_change: Option<ChangeHook>,
    pub change_delay: Duration,
}

impl<R: Runtime> SessionDeps<R> {
    /// The given factory, storage and policy with no hooks and the default change delay.
    pub fn new(
        create_window: Arc<dyn WindowFactory<R>>,
        storage: Arc<dyn SessionStorage>,
        policy: StorePolicy,
    ) -> Self {
        Self {
            create_window,
            storage,
            policy,
            on_last_window_closed: None,
            on_change: None,
            change_delay: CHANGE_DELAY,
        }
    }
}

/// A `SessionStorage` that keeps the document in memory: for tests and for an app that does not
/// persist yet.
#[derive(Default)]
pub struct MemoryStorage {
    document: Mutex<Option<Document>>,
}

impl SessionStorage for MemoryStorage {
    fn load(&self) -> Result<Option<Document>, StorageError> {
        Ok(self
            .document
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone())
    }

    fn save(&self, document: &Document) -> Result<(), StorageError> {
        *self.document.lock().unwrap_or_else(|e| e.into_inner()) = Some(document.clone());
        Ok(())
    }
}
