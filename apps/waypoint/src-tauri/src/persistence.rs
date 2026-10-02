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
use waypoint_settings::Settings;

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

/// What the saved store becomes at start-up under `settings`. The session's windows come back
/// only when the start-up setting says so; with "Open Home" everything that is not a window (the
/// saved workspaces, the closed tabs, the Shelf, the id counters) is carried over, because the
/// first save of the run replaces the document and must not empty it. The Shelf is emptied when
/// it is not kept between sessions, and the ids it has used stay used either way, so a later item
/// never takes a forgotten one's id.
pub(crate) fn restored_for(mut store: Store, settings: &Settings) -> Store {
    if !crate::settings::restores_session(settings) {
        store = store.without_windows();
    }
    if !settings.dnd.shelf_persist {
        store.forget_shelf();
    }
    // A start that restores no windows opens one fresh window, and the Shelf docks in it.
    if !crate::settings::restores_session(settings) {
        store.dock_shelf();
    }
    store
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
    // "Open Home" (General settings) starts with the one window a first run gets, but the saved
    // document is always read: its workspaces, closed tabs and Shelf carry over into the run's
    // first save (`restored_for` decides what is kept), so a start that restores nothing cannot
    // empty them.
    let settings = crate::settings::current(app);
    if !crate::settings::restores_session(&settings) {
        log::info!("the start-up setting is Home: not restoring the last session's windows");
    }
    match saver.storage.load() {
        Ok(Some(document)) => match Store::from_document(document) {
            Ok((store, notes)) => {
                for note in notes {
                    log::info!("repaired the saved session: {note}");
                }
                sessions.restore(restored_for(store, &settings));
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
    restore_shelf_window(app, &sessions);
    log::info!("session restored: {created} window(s)");
}

/// Brings the Shelf window back when the session left the Shelf undocked. When it cannot be made
/// the Shelf docks again, so no main window is left without a dock and without a Shelf window.
fn restore_shelf_window(app: &AppHandle, sessions: &Sessions<Wry>) {
    let shelf = sessions.with_store(|s| *s.shelf_window());
    if !shelf.undocked {
        return;
    }
    if let Err(e) = crate::shelf_window::build(app, &shelf) {
        log::warn!("could not restore the Shelf window: {e}; docking the Shelf");
        if let Err(e) = sessions.run_existing(
            app,
            waypoint_protocol::SHELF_LABEL,
            waypoint_session::Command::SetShelfUndocked { undocked: false },
        ) {
            log::warn!("could not dock the Shelf: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use waypoint_session::{Command, Workspace, WorkspaceId};
    use waypoint_settings::StartupMode;

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

    /// A saved session with one window and one workspace, as the file holds it.
    fn saved_document() -> Document {
        let mut document = document(1).unwrap();
        document.body.workspaces.push(Workspace {
            id: WorkspaceId(1),
            name: "Work".to_string(),
            locations: Vec::new(),
        });
        document.body.next_workspace = 2;
        document
    }

    /// One start-up: the document is loaded, restored for `settings`, and saved as the first save of the run.
    fn start_up(document: Document, settings: &Settings) -> Document {
        let (store, _) = Store::from_document(document).unwrap();
        restored_for(store, settings).to_document()
    }

    #[test]
    fn open_home_keeps_the_workspaces_across_start_ups_but_not_the_windows() {
        let mut settings = Settings::default();
        settings.general.startup = StartupMode::Home;
        // Two Home start-ups in a row, each saving what it started with.
        let first = start_up(saved_document(), &settings);
        let second = start_up(first, &settings);
        assert!(second.body.windows.is_empty());
        assert_eq!(second.body.workspaces.len(), 1);
        assert_eq!(second.body.workspaces[0].name, "Work");
        assert_eq!(second.body.next_workspace, 2);
    }

    #[test]
    fn restoring_the_session_leaves_the_store_as_it_was_saved() {
        let mut settings = Settings::default();
        settings.general.startup = StartupMode::RestoreSession;
        let (store, _) = Store::from_document(saved_document()).unwrap();
        assert_eq!(restored_for(store.clone(), &settings), store);
    }

    fn undocked_document() -> Document {
        let mut store = Store::new();
        store
            .dispatch(
                "main-1",
                Command::OpenWindow {
                    location: Some(waypoint_protocol::Location::new("/", "file:///")),
                    geometry: None,
                },
            )
            .unwrap();
        store
            .dispatch("main-1", Command::SetShelfUndocked { undocked: true })
            .unwrap();
        store
            .dispatch("shelf", Command::SetShelfOnTop { on_top: true })
            .unwrap();
        store.to_document()
    }

    #[test]
    fn an_undocked_shelf_comes_back_undocked_when_the_session_is_restored() {
        let mut settings = Settings::default();
        settings.general.startup = StartupMode::RestoreSession;
        let (store, _) = Store::from_document(undocked_document()).unwrap();
        let restored = restored_for(store, &settings);
        assert!(restored.shelf_window().undocked && restored.shelf_window().on_top);
    }

    #[test]
    fn a_start_that_restores_no_windows_docks_the_shelf_and_keeps_its_choices() {
        let mut settings = Settings::default();
        settings.general.startup = StartupMode::Home;
        let (store, _) = Store::from_document(undocked_document()).unwrap();
        let restored = restored_for(store, &settings);
        assert!(!restored.shelf_window().undocked);
        assert!(restored.shelf_window().on_top);
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

    fn store_with_shelf() -> Store {
        let mut store = Store::new();
        store
            .dispatch(
                "main-1",
                Command::OpenWindow {
                    location: Some(waypoint_protocol::Location::new("/", "file:///")),
                    geometry: None,
                },
            )
            .unwrap();
        store
            .dispatch(
                "main-1",
                Command::AddToShelf {
                    locations: vec![
                        waypoint_protocol::Location::new("/a", "file:///a"),
                        waypoint_protocol::Location::new("/b", "file:///b"),
                    ],
                    added_ms: 1,
                },
            )
            .unwrap();
        // Save and load, as a restart does.
        Store::from_document(store.to_document()).unwrap().0
    }

    #[test]
    fn the_shelf_comes_back_by_default() {
        let restored = restored_for(store_with_shelf(), &Settings::default());
        assert_eq!(restored.shelf().len(), 2);
        assert_eq!(restored.windows().len(), 1);
    }

    #[test]
    fn a_shelf_that_is_not_kept_is_cleared_at_start_up_and_its_ids_stay_used() {
        let mut settings = Settings::default();
        settings.dnd.shelf_persist = false;
        let mut restored = restored_for(store_with_shelf(), &settings);
        assert!(restored.shelf().is_empty());
        assert_eq!(restored.windows().len(), 1, "the session is unaffected");
        restored
            .dispatch(
                "main-1",
                Command::AddToShelf {
                    locations: vec![waypoint_protocol::Location::new("/c", "file:///c")],
                    added_ms: 2,
                },
            )
            .unwrap();
        assert_eq!(restored.shelf()[0].id.0, 3);
    }

    #[test]
    fn open_home_keeps_the_workspaces_across_start_ups_whether_or_not_the_shelf_is_kept() {
        for shelf_persist in [true, false] {
            let mut settings = Settings::default();
            settings.general.startup = waypoint_settings::StartupMode::Home;
            settings.dnd.shelf_persist = shelf_persist;
            let mut document = store_with_shelf().to_document();
            document.body.workspaces.push(waypoint_session::Workspace {
                id: waypoint_session::WorkspaceId(1),
                name: "Work".to_string(),
                locations: Vec::new(),
            });
            document.body.next_workspace = 2;
            // Two Home start-ups in a row, each saving what it started with.
            for _ in 0..2 {
                let (store, _) = Store::from_document(document).unwrap();
                document = restored_for(store, &settings).to_document();
            }
            assert!(document.body.windows.is_empty());
            assert_eq!(
                document.body.workspaces.len(),
                1,
                "shelf_persist {shelf_persist}"
            );
            assert_eq!(document.body.next_workspace, 2);
            assert_eq!(document.body.shelf.len(), if shelf_persist { 2 } else { 0 });
        }
    }

    #[test]
    fn opening_home_still_keeps_the_shelf_but_not_the_windows() {
        let mut settings = Settings::default();
        settings.general.startup = waypoint_settings::StartupMode::Home;
        let restored = restored_for(store_with_shelf(), &settings);
        assert_eq!(restored.shelf().len(), 2);
        assert!(restored.windows().is_empty());
        settings.dnd.shelf_persist = false;
        let restored = restored_for(store_with_shelf(), &settings);
        assert!(restored.shelf().is_empty() && restored.windows().is_empty());
    }
}
