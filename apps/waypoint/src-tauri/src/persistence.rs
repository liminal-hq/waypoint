// Wires the session to its file: restores at start, saves on change, and flushes as the app quits
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Saves come from three places: the session plugin's debounced `on_change` hook (a second after a
// burst of changes), `window_closing` when a main window asks to close (the last one keeps a session with no windows from overwriting it), and `finish` when the app quits.
// They share one lock, so the last writer is always the newest document, and `finish` freezes the
// saver: windows closing while the app exits must not overwrite the session being kept.

use std::collections::HashSet;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager, WindowEvent, Wry};
use tauri_plugin_waypoint_session::Sessions;
use waypoint_session::{Document, SessionStorage, StorageError, Store};

use crate::storage::{FileKeyValue, Persistence};
use crate::windows::build_main_window;

/// How long the main thread waits for the store's lock when it flushes: a window factory holds it
/// while it waits for that thread, so waiting longer could not help.
const LOCK_WAIT: Duration = Duration::from_millis(300);

/// How long quitting waits for the session plugin to end the last window's session.
const SETTLE_WAIT: Duration = Duration::from_secs(1);

/// The event a closing window's page answers by reporting its tab hints at once.
pub const FLUSH_HINTS_EVENT: &str = "waypoint://flush-hints";

/// How long a closing window is held so its page can report the hints it has not sent yet. The
/// webview's `pagehide` fires after the session is saved, so the page is asked first.
const FLUSH_WAIT: Duration = Duration::from_millis(150);

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

/// Why a save is happening.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Intent {
    /// A change to the session: saved unless the app finished.
    Change,
    /// A main window asked to close and it may be the last one. The close can still be cancelled,
    /// so nothing is frozen; instead a document with no windows is refused from now on, because
    /// the window going away would otherwise leave an empty session over the good one.
    LastWindowMayClose,
    /// The final save as the app quits. Later saves are ignored.
    Finish,
}

#[derive(Default)]
struct GateState {
    /// `true` once the final save is done; later saves are ignored.
    frozen: bool,
    /// A close of the last window is (or was) in flight: documents without windows are refused.
    close_in_flight: bool,
}

/// The rules of when a document may be written, apart from where it goes.
#[derive(Default)]
struct SaveGate {
    state: Mutex<GateState>,
}

impl SaveGate {
    /// Saves the document `read` returns to `storage`, subject to the gate. `read` runs under the
    /// gate's lock, so a newer document is never overwritten by an older one.
    fn save(
        &self,
        storage: &dyn SessionStorage,
        read: impl FnOnce() -> Option<Document>,
        intent: Intent,
    ) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.frozen {
            return;
        }
        if intent == Intent::LastWindowMayClose {
            state.close_in_flight = true;
        }
        if let Some(document) = read() {
            if state.close_in_flight && document.body.windows.is_empty() {
                log::debug!("not saving a session with no windows while a close is in flight");
            } else if let Err(e) = storage.save(&document) {
                log::warn!("could not save the session: {e}");
            }
        }
        if intent == Intent::Finish {
            state.frozen = true;
        }
    }
}

/// Saves the session and remembers whether the app is finishing.
pub struct Saver {
    storage: Arc<LazyStorage>,
    gate: SaveGate,
}

impl Saver {
    pub fn new() -> Self {
        Self {
            storage: Arc::new(LazyStorage::default()),
            gate: SaveGate::default(),
        }
    }

    pub fn storage(&self) -> Arc<dyn SessionStorage> {
        self.storage.clone()
    }

    pub fn take_notice(&self) -> Option<String> {
        self.storage.inner.get()?.take_notice()
    }

    /// Saves the document `read` returns, unless the app already finished.
    pub fn save(&self, read: impl FnOnce() -> Option<Document>, intent: Intent) {
        self.gate.save(self.storage.as_ref(), read, intent);
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
    /// bring back next time: it is saved as it is, with that window's tabs, and from then on a
    /// session with no windows is refused (the window's session ending must not overwrite it). The
    /// close may still be cancelled, so nothing else stops: later changes keep saving. When the
    /// store stays busy past the wait, the window counts as possibly the last one.
    pub fn window_closing(&self, app: &AppHandle) {
        let intent = if Self::is_last_window(app).unwrap_or(true) {
            Intent::LastWindowMayClose
        } else {
            Intent::Change
        };
        self.save(|| Self::current(app, LOCK_WAIT), intent);
    }

    fn is_last_window(app: &AppHandle) -> Option<bool> {
        app.try_state::<Sessions<Wry>>()
            .and_then(|s| s.try_clone_store(LOCK_WAIT))
            .map(|s| s.windows().len() <= 1)
    }

    /// The final save as the app quits. Later saves are ignored.
    pub fn finish(&self, app: &AppHandle) {
        self.save(|| Self::current(app, LOCK_WAIT), Intent::Finish);
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

/// Holds each main window's first close request for `FLUSH_WAIT` so the page can flush its tab
/// hints, then closes it again; the second request goes through. The wait runs on its own thread,
/// because the main thread has to stay free to deliver the page's reports.
#[derive(Default)]
pub struct CloseFlush {
    flushed: Mutex<HashSet<String>>,
}

impl CloseFlush {
    /// Whether this close request must be held. The first request for a label is (and is
    /// remembered); the repeat that follows the hold is not, and forgets the label.
    fn hold(&self, label: &str) -> bool {
        let mut flushed = self.flushed.lock().unwrap_or_else(|e| e.into_inner());
        !flushed.remove(label) && {
            flushed.insert(label.to_string());
            true
        }
    }

    /// Call for a main window's `CloseRequested`. Returns `true` when the close was held (it was
    /// prevented and will be repeated); the caller then does nothing more for this request.
    pub fn on_close_requested(
        self: &Arc<Self>,
        window: &tauri::Window,
        event: &WindowEvent,
    ) -> bool {
        let WindowEvent::CloseRequested { api, .. } = event else {
            return false;
        };
        let label = window.label().to_string();
        if !self.hold(&label) {
            return false;
        }
        api.prevent_close();
        if let Err(e) = window.emit_to(label.as_str(), FLUSH_HINTS_EVENT, ()) {
            log::debug!("could not ask `{label}` to flush its hints: {e}");
        }
        let app = window.app_handle().clone();
        std::thread::spawn(move || {
            std::thread::sleep(FLUSH_WAIT);
            match app.get_webview_window(&label) {
                Some(window) => {
                    if let Err(e) = window.close() {
                        log::warn!("could not close `{label}` after the hint flush: {e}");
                    }
                }
                None => log::debug!("`{label}` went away during the hint flush"),
            }
        });
        true
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
        match build_main_window(app, label, geometry.as_ref(), false) {
            Ok(_) => created += 1,
            Err(e) => log::warn!("could not restore the window `{label}`: {e}"),
        }
    }
    if created == 0 {
        if let Err(e) = build_main_window(app, "main-1", None, false) {
            log::error!("could not open the first window: {e}");
        }
    }
    log::info!("session restored: {created} window(s)");
}

#[cfg(test)]
mod tests {
    use super::*;
    use waypoint_session::Command;

    #[derive(Default)]
    struct Recording(Mutex<Vec<usize>>);

    impl SessionStorage for Recording {
        fn load(&self) -> Result<Option<Document>, StorageError> {
            Ok(None)
        }
        fn save(&self, document: &Document) -> Result<(), StorageError> {
            self.0.lock().unwrap().push(document.body.windows.len());
            Ok(())
        }
    }

    fn document(windows: usize) -> Option<Document> {
        let mut store = Store::new();
        for _ in 0..windows {
            store
                .dispatch(
                    "main-1",
                    Command::OpenWindow {
                        location: None,
                        geometry: None,
                    },
                )
                .unwrap();
        }
        Some(store.to_document())
    }

    fn saved(storage: &Recording) -> Vec<usize> {
        storage.0.lock().unwrap().clone()
    }

    #[test]
    fn a_close_is_held_once_and_the_repeat_goes_through() {
        let flush = CloseFlush::default();
        assert!(flush.hold("main-1"));
        assert!(flush.hold("main-2"), "each window is held on its own");
        assert!(!flush.hold("main-1"), "the repeat is let through");
        assert!(flush.hold("main-1"), "and a later close is held again");
    }

    #[test]
    fn a_cancelled_close_does_not_stop_later_saves() {
        let (gate, storage) = (SaveGate::default(), Recording::default());
        gate.save(&storage, || document(1), Intent::LastWindowMayClose);
        gate.save(&storage, || document(1), Intent::Change);
        assert_eq!(saved(&storage), vec![1, 1]);
    }

    #[test]
    fn a_session_with_no_windows_never_replaces_one_while_a_close_is_in_flight() {
        let (gate, storage) = (SaveGate::default(), Recording::default());
        // The store was busy, so the close counted as possibly the last, and its own save was
        // skipped; the window then went away and the debounced save found nothing left.
        gate.save(&storage, || None, Intent::LastWindowMayClose);
        gate.save(&storage, || document(0), Intent::Change);
        gate.save(&storage, || document(0), Intent::Finish);
        assert!(saved(&storage).is_empty());
    }

    #[test]
    fn without_a_close_in_flight_an_empty_session_is_saved_and_finish_freezes() {
        let (gate, storage) = (SaveGate::default(), Recording::default());
        gate.save(&storage, || document(0), Intent::Change);
        gate.save(&storage, || document(1), Intent::Finish);
        gate.save(&storage, || document(2), Intent::Change);
        assert_eq!(saved(&storage), vec![0, 1]);
    }
}
