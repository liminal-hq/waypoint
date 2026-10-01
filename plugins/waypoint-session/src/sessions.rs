// The session registry held in Tauri state: one `Store` for every window behind one lock.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use tauri::{AppHandle, Emitter, Manager, Runtime};
use waypoint_protocol::WindowKind;
use waypoint_session::{
    Command, MoveTo, Outcome, SessionEvent, SessionSnapshot, SessionStorage, Store, TabId,
};

use crate::deps::SessionDeps;
use crate::error::Error;
use crate::EVENT;

type Hook = Arc<dyn Fn(&str, TabId) + Send + Sync>;

fn locked<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    // A poisoned lock only means a hook panicked; the state is still consistent.
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

/// Every window's session as one `Store` with one writer. A command locks the store, applies
/// itself, makes any window it created, delivers each event to the window it belongs to and
/// fires the hooks, all before unlocking, so events reach a window in revision order.
///
/// The composition root subscribes to tab closes with `on_tab_closed` (for example to close the
/// tab's listing handle); the plugin itself knows nothing about listings.
pub struct Sessions<R: Runtime> {
    store: Arc<Mutex<Store>>,
    hooks: Mutex<Vec<Hook>>,
    deps: SessionDeps<R>,
    change_pending: Arc<AtomicBool>,
}

impl<R: Runtime> Sessions<R> {
    pub fn new(deps: SessionDeps<R>) -> Self {
        Self {
            store: Arc::new(Mutex::new(Store::with_policy(deps.policy))),
            hooks: Mutex::new(Vec::new()),
            deps,
            change_pending: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Runs `hook(window_label, tab)` whenever a tab is gone for good: closed, or left with its
    /// window. A tab handed to another window is not reported. It runs while the store is locked,
    /// so it must be quick and must not call back into the session commands.
    pub fn on_tab_closed(&self, hook: impl Fn(&str, TabId) + Send + Sync + 'static) {
        locked(&self.hooks).push(Arc::new(hook));
    }

    /// Where the app asked the store to persist.
    pub fn storage(&self) -> &Arc<dyn SessionStorage> {
        &self.deps.storage
    }

    /// Replaces the store with a restored one, under the plugin's policy. Called once at start,
    /// before any window exists or any command runs; it sends no events and schedules no save.
    pub fn restore(&self, mut restored: Store) {
        restored.set_policy(self.deps.policy);
        *locked(&self.store) = restored;
    }

    /// A copy of the store, waiting at most `wait` for its lock. For callers on the main thread,
    /// which must not wait on a lock that a window factory holds while it waits for that thread;
    /// `None` when the lock stayed busy.
    pub fn try_clone_store(&self, wait: std::time::Duration) -> Option<Store> {
        let start = std::time::Instant::now();
        loop {
            match self.store.try_lock() {
                Ok(store) => return Some(store.clone()),
                Err(std::sync::TryLockError::Poisoned(e)) => return Some(e.into_inner().clone()),
                Err(std::sync::TryLockError::WouldBlock) => {
                    if start.elapsed() >= wait {
                        return None;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
            }
        }
    }

    /// Reads the store under its lock.
    pub fn with_store<T>(&self, read: impl FnOnce(&Store) -> T) -> T {
        read(&locked(&self.store))
    }

    /// The window's session at the current revision. A main window that is not in the store yet
    /// (the first run, before any restore) is registered first.
    pub fn snapshot(&self, app: &AppHandle<R>, label: &str) -> Result<SessionSnapshot, Error> {
        let mut store = locked(&self.store);
        self.ensure_window(app, &mut store, label)?;
        store
            .snapshot(label)
            .ok_or_else(|| waypoint_session::SessionError::UnknownWindow(label.to_string()).into())
    }

    /// Applies `command` for the window `label` and returns what it did. See the type docs for
    /// the order of effects. When the command makes a window, the factory creates it after the
    /// store change; if the factory fails the store goes back to how it was, nothing is sent and
    /// the error is returned.
    pub fn run(&self, app: &AppHandle<R>, label: &str, command: Command) -> Result<Outcome, Error> {
        self.run_checked(app, label, command, true)
    }

    /// Like `run`, but never registers a window: a label the store does not hold is refused with
    /// `UnknownWindow` (checked under the same lock as the command). For work that follows a
    /// window's life from outside, such as the deferred geometry capture, which must not bring a
    /// window back that the store has just closed.
    pub fn run_existing(
        &self,
        app: &AppHandle<R>,
        label: &str,
        command: Command,
    ) -> Result<Outcome, Error> {
        self.run_checked(app, label, command, false)
    }

    fn run_checked(
        &self,
        app: &AppHandle<R>,
        label: &str,
        command: Command,
        register: bool,
    ) -> Result<Outcome, Error> {
        let mut store = locked(&self.store);
        if register {
            self.ensure_window(app, &mut store, label)?;
        }
        let before = store.clone();
        // Geometry has no event but is still worth saving.
        let silent_change = matches!(command, Command::SetGeometry { .. });
        let outcome = store.dispatch(label, command)?;
        for opened in outcome.windows_opened() {
            let geometry = store.window(&opened).and_then(|w| w.geometry);
            if let Err(e) = self
                .deps
                .create_window
                .create(app, &opened, geometry.as_ref())
            {
                log::warn!("could not create window `{opened}`: {e}; undoing the change");
                *store = before;
                return Err(e.into());
            }
        }
        self.publish(app, &outcome);
        drop(store);
        if silent_change {
            self.schedule_change();
        }
        self.after_unlock(app, &outcome);
        Ok(outcome)
    }

    /// Ends a destroyed window's session: its tabs go to the closed list and the hooks hear them.
    /// Does nothing for a window the store does not know (settings, properties, a window that
    /// `close_window` already closed).
    pub fn window_destroyed(&self, app: &AppHandle<R>, label: &str) {
        let mut store = locked(&self.store);
        if store.window(label).is_none() {
            return;
        }
        match store.dispatch(label, Command::CloseWindow) {
            Ok(outcome) => {
                self.publish(app, &outcome);
                drop(store);
                self.after_unlock(app, &outcome);
            }
            Err(e) => log::warn!("could not close the session of `{label}`: {e}"),
        }
    }

    /// First run: a main window the store does not hold yet is registered, empty, under its own
    /// label (`Command::RegisterWindow`), so `get_snapshot` and the first `open_tab` just work.
    /// Windows made by `OpenWindow` and `MoveTabs` are already in the store.
    fn ensure_window(
        &self,
        app: &AppHandle<R>,
        store: &mut Store,
        label: &str,
    ) -> Result<(), Error> {
        if store.window(label).is_some() {
            return Ok(());
        }
        if WindowKind::from_label(label) != Some(WindowKind::Main) {
            return Err(waypoint_session::SessionError::UnknownWindow(label.to_string()).into());
        }
        let outcome = store.dispatch(
            label,
            Command::RegisterWindow {
                label: label.to_string(),
            },
        )?;
        self.publish(app, &outcome);
        self.schedule_change();
        Ok(())
    }

    /// Sends each event to its own window, still under the lock. A window that does not exist
    /// yet (the target of a hand-off, which reads its state with `get_snapshot` when it starts)
    /// or no longer exists has nothing to receive; a failed send is logged and never fails the
    /// command, because the state has already changed.
    fn publish(&self, app: &AppHandle<R>, outcome: &Outcome) {
        for e in &outcome.events {
            if app.get_webview_window(&e.window).is_none() {
                log::debug!("no window `{}` to receive a session event", e.window);
                continue;
            }
            if let Err(err) = app.emit_to(e.window.as_str(), EVENT, &e.event) {
                log::warn!("failed to emit a session event to `{}`: {err}", e.window);
            }
        }
        let moved: HashSet<TabId> = outcome
            .events
            .iter()
            .filter_map(|e| match &e.event {
                SessionEvent::TabOpened { tab, .. } | SessionEvent::TabReopened { tab, .. } => {
                    Some(tab.id)
                }
                _ => None,
            })
            .collect();
        let hooks = locked(&self.hooks);
        for e in &outcome.events {
            if let SessionEvent::TabClosed { tab, .. } = &e.event {
                if !moved.contains(tab) {
                    for hook in hooks.iter() {
                        hook(&e.window, *tab);
                    }
                }
            }
        }
    }

    fn after_unlock(&self, app: &AppHandle<R>, outcome: &Outcome) {
        if outcome.events.is_empty() {
            return;
        }
        self.schedule_change();
        // A window the store closed by itself (its last tab closed, or its tabs were all moved
        // away) goes with it. The store already forgot it, so the destroy event finds nothing.
        for label in outcome.windows_closed() {
            if let Some(webview) = app.get_webview_window(&label) {
                if let Err(e) = webview.destroy() {
                    log::warn!("could not destroy window `{label}`: {e}");
                }
            }
        }
        if outcome.last_window_closed {
            if let Some(hook) = &self.deps.on_last_window_closed {
                hook(app);
            }
        }
    }

    /// Calls `on_change` once, `change_delay` after the first change of a burst, with the store
    /// as it is then. Saving is the app's business.
    fn schedule_change(&self) {
        let Some(hook) = self.deps.on_change.clone() else {
            return;
        };
        if self.change_pending.swap(true, Ordering::AcqRel) {
            return;
        }
        let store = Arc::clone(&self.store);
        let pending = Arc::clone(&self.change_pending);
        let delay = self.deps.change_delay;
        std::thread::spawn(move || {
            std::thread::sleep(delay);
            pending.store(false, Ordering::Release);
            let copy = locked(&store).clone();
            (*hook)(&copy);
        });
    }
}

/// The window a `MoveTabs` hands the tabs to: the label of an existing window, or the window the
/// command made.
pub(crate) fn move_target(to: &MoveTo, outcome: &Outcome) -> Option<String> {
    match to {
        MoveTo::ExistingWindow { label, .. } => Some(label.clone()),
        MoveTo::NewWindow { .. } => outcome.windows_opened().into_iter().next(),
    }
}
