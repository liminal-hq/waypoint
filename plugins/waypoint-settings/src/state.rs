// The settings state: one document, one writer, a revision and a change event to every window
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::sync::{Arc, Mutex, MutexGuard};

use tauri::{AppHandle, Emitter, Runtime};
use waypoint_settings::{
    Settings, SettingsDocument, SettingsSnapshot, SettingsStorage, UiSettings,
};

use crate::error::Error;
use crate::EVENT;

/// Runs with the new settings after every change, outside the plugin's locks. The app subscribes
/// to push what other state depends on (the session's default view) without the plugin knowing.
pub type ChangeHook = Arc<dyn Fn(&Settings) + Send + Sync>;

fn locked<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    // A poisoned lock only means a hook panicked; the state is still consistent.
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

/// The application settings every window reads. Rust owns them: a window asks for a change and
/// hears the answer as an event carrying the new revision.
pub struct SettingsStore<R: Runtime> {
    app: AppHandle<R>,
    storage: Arc<dyn SettingsStorage>,
    current: Mutex<SettingsSnapshot>,
    /// Held across a whole change, so two changes never interleave their save, event and hooks.
    writer: Mutex<()>,
    hooks: Mutex<Vec<ChangeHook>>,
}

impl<R: Runtime> SettingsStore<R> {
    /// Loads what the storage holds. Settings that cannot be read are never fatal: the defaults
    /// apply, and an out-of-range value from a hand-edited file is brought into range.
    pub fn new(app: AppHandle<R>, storage: Arc<dyn SettingsStorage>) -> Self {
        let settings = match storage.load() {
            Ok(Some(document)) => document.body.clamped(),
            Ok(None) => Settings::default(),
            Err(why) => {
                log::warn!("could not load the settings, using the defaults: {why}");
                Settings::default()
            }
        };
        Self {
            app,
            storage,
            current: Mutex::new(SettingsSnapshot {
                revision: 0,
                settings,
            }),
            writer: Mutex::new(()),
            hooks: Mutex::new(Vec::new()),
        }
    }

    /// The settings in force and the revision they are at.
    pub fn snapshot(&self) -> SettingsSnapshot {
        locked(&self.current).clone()
    }

    pub fn get(&self) -> Settings {
        self.snapshot().settings
    }

    /// Runs `hook` after every change. It is not run for the settings loaded at start-up: the
    /// caller reads those with `get`.
    pub fn on_change(&self, hook: impl Fn(&Settings) + Send + Sync + 'static) {
        locked(&self.hooks).push(Arc::new(hook));
    }

    /// Replaces the settings: validated, saved, then in force and announced to every window. A
    /// value out of range, or a save that fails, changes nothing. Setting what is already in
    /// force changes nothing either: no revision, no event.
    pub fn set(&self, settings: Settings) -> Result<SettingsSnapshot, Error> {
        settings.validate()?;
        let _writer = locked(&self.writer);
        self.commit(settings)
    }

    /// Changes only the `ui` settings, read and written under the writer lock: whatever else is
    /// in force (a change another window just made) is kept, so a window that only knows about the
    /// Action bar cannot turn the rest of the document back. Same save, event and hooks as `set`.
    pub fn update_ui(
        &self,
        change: impl FnOnce(&mut UiSettings),
    ) -> Result<SettingsSnapshot, Error> {
        let _writer = locked(&self.writer);
        let mut settings = self.get();
        change(&mut settings.ui);
        settings.validate()?;
        self.commit(settings)
    }

    /// Saves, applies and announces `settings`; the caller holds the writer lock.
    fn commit(&self, settings: Settings) -> Result<SettingsSnapshot, Error> {
        let before = self.snapshot();
        if before.settings == settings {
            return Ok(before);
        }
        self.storage
            .save(&SettingsDocument::new(settings.clone()))
            .map_err(|e| Error::Storage(e.to_string()))?;
        let after = SettingsSnapshot {
            revision: before.revision + 1,
            settings: settings.clone(),
        };
        *locked(&self.current) = after.clone();
        if let Err(e) = self.app.emit(EVENT, after.clone()) {
            log::warn!("could not announce the settings change: {e}");
        }
        let hooks: Vec<ChangeHook> = locked(&self.hooks).clone();
        for hook in hooks {
            hook(&settings);
        }
        Ok(after)
    }
}
