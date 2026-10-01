// Wires the session to its file: restores at start, saves on change, and flushes as the app quits
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Saves come from three places: the session plugin's debounced `on_change` hook (a second after a
// burst of changes), `window_closing` when a main window asks to close (the last one freezes the session as it is), and `finish` when the app quits.
// They share one lock, so the last writer is always the newest document, and `finish` freezes the
// saver: windows closing while the app exits must not overwrite the session being kept.

use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use tauri::{AppHandle, Manager, Wry};
use tauri_plugin_waypoint_session::Sessions;
use waypoint_session::{Document, SessionStorage, StorageError, Store};

use crate::storage::{FileKeyValue, Persistence};
use crate::windows::build_main_window;

/// How long the main thread waits for the store's lock when it flushes: a window factory holds it
/// while it waits for that thread, so waiting longer could not help.
const LOCK_WAIT: Duration = Duration::from_millis(300);

/// How long quitting waits for the session plugin to end the last window's session.
const SETTLE_WAIT: Duration = Duration::from_secs(1);

type FileStorage = Persistence<FileKeyValue<Wry>>;

/// The storage handed to the session plugin before the app exists; `setup` opens the file.
#[derive(Default)]
pub struct LazyStorage {
    inner: OnceLock<FileStorage>,
}

impl LazyStorage {
    fn get(&self) -> Result<&FileStorage, StorageError> {
        self.inner
            .get()
            .ok_or_else(|| StorageError::Io("the session file is not open yet".into()))
    }
}

impl SessionStorage for LazyStorage {
    fn load(&self) -> Result<Option<Document>, StorageError> {
        self.get()?.load()
    }

    fn save(&self, document: &Document) -> Result<(), StorageError> {
        self.get()?.save(document)
    }
}

/// Saves the session and remembers whether the app is finishing.
pub struct Saver {
    storage: Arc<LazyStorage>,
    /// `true` once the final save is done; later saves are ignored.
    frozen: Mutex<bool>,
}

impl Saver {
    pub fn new() -> Self {
        Self {
            storage: Arc::new(LazyStorage::default()),
            frozen: Mutex::new(false),
        }
    }

    pub fn storage(&self) -> Arc<dyn SessionStorage> {
        self.storage.clone()
    }

    pub fn take_notice(&self) -> Option<String> {
        self.storage.inner.get()?.take_notice()
    }

    /// Saves the document `read` returns, unless the app already finished. `read` runs under the
    /// saver's lock, so a newer document is never overwritten by an older one.
    pub fn save(&self, read: impl FnOnce() -> Option<Document>, freeze: bool) {
        let mut frozen = self.frozen.lock().unwrap_or_else(|e| e.into_inner());
        if *frozen {
            return;
        }
        if let Some(document) = read() {
            if let Err(e) = self.storage.save(&document) {
                log::warn!("could not save the session: {e}");
            }
        }
        if freeze {
            *frozen = true;
        }
    }

    fn current(app: &AppHandle, wait: Duration) -> Option<Document> {
        let sessions = app.try_state::<Sessions<Wry>>()?;
        let store = sessions.try_clone_store(wait);
        if store.is_none() {
            log::warn!("the session store was busy; the debounced save will cover this one");
        }
        store.map(|s| s.to_document())
    }

    /// A main window asked to close. Closing the last one quits the app, so it is the session to
    /// bring back next time: it is saved as it is, with that window's tabs, and the saver freezes
    /// before the window's session ends. Closing any other window just saves.
    pub fn window_closing(&self, app: &AppHandle) {
        self.save(|| Self::current(app, LOCK_WAIT), Self::is_last_window(app));
    }

    fn is_last_window(app: &AppHandle) -> bool {
        app.try_state::<Sessions<Wry>>()
            .and_then(|s| s.try_clone_store(LOCK_WAIT))
            .is_some_and(|s| s.windows().len() <= 1)
    }

    /// The final save as the app quits. Later saves are ignored.
    pub fn finish(&self, app: &AppHandle) {
        self.save(|| Self::current(app, LOCK_WAIT), true);
    }

    /// Quits once the session plugin has ended the last window's session (or a second has passed).
    pub fn quit_when_settled(self: &Arc<Self>, app: &AppHandle) {
        let saver = Arc::clone(self);
        let app = app.clone();
        std::thread::spawn(move || {
            let start = std::time::Instant::now();
            while start.elapsed() < SETTLE_WAIT {
                let empty = app
                    .try_state::<Sessions<Wry>>()
                    .and_then(|s| s.try_clone_store(LOCK_WAIT))
                    .is_none_or(|s: Store| s.windows().is_empty());
                if empty {
                    break;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            saver.finish(&app);
            app.exit(0);
        });
    }
}

/// Loads the saved session into the store and creates its windows; with nothing saved (the first
/// run, or a file that could not be read) one `main-1` opens and registers itself on its first
/// `get_snapshot`.
pub fn restore(app: &AppHandle, saver: &Saver) {
    match FileKeyValue::open(app) {
        Ok(kv) => {
            let _ = saver.storage.inner.set(Persistence::new(kv));
        }
        Err(e) => log::warn!("could not open the session file: {e}"),
    }
    let sessions = app.state::<Sessions<Wry>>();
    match saver.storage.load() {
        Ok(Some(document)) => match Store::from_document(document) {
            Ok((store, notes)) => {
                for note in notes {
                    log::info!("repaired the saved session: {note}");
                }
                sessions.restore(store);
            }
            // `Persistence::load` already vetted it, so this is only a race with nothing.
            Err(e) => log::warn!("the saved session was rejected: {e}"),
        },
        Ok(None) => {}
        Err(e) => log::warn!("could not read the saved session: {e}"),
    }
    let windows: Vec<_> = sessions.with_store(|s| {
        s.windows()
            .iter()
            .map(|w| (w.label.clone(), w.geometry))
            .collect()
    });
    let mut created = 0;
    for (label, geometry) in &windows {
        match build_main_window(app, label, geometry.as_ref()) {
            Ok(_) => created += 1,
            Err(e) => log::warn!("could not restore the window `{label}`: {e}"),
        }
    }
    if created == 0 {
        if let Err(e) = build_main_window(app, "main-1", None) {
            log::error!("could not open the first window: {e}");
        }
    }
    log::info!("session restored: {created} window(s)");
}
