// What the composition root injects into the operations plugin: the providers, the Trash, the
// selection resolver, where the journal and the settings are kept, and the policy around them.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::sync::{Arc, Mutex};
use std::time::Duration;

use waypoint_ops::{
    Clock, JournalStorage, OpsEvent, OpsSettings, Protected, Providers, SelectionResolver, Trash,
};

/// How long after a change the journal is written, so a burst of changes makes one write. The
/// write-ahead record of a job that is about to run is not delayed.
pub const SAVE_DELAY: Duration = Duration::from_secs(1);

/// How long quitting waits for running jobs to unwind after they are cancelled.
pub const EXIT_WAIT: Duration = Duration::from_secs(3);

/// Where the operations settings are kept. The plugin loads once at start and saves on every
/// change; the Settings window owns what the values mean.
pub trait SettingsStorage: Send + Sync {
    /// The saved settings, or `None` when none were saved yet.
    fn load(&self) -> Result<Option<OpsSettings>, String>;
    fn save(&self, settings: &OpsSettings) -> Result<(), String>;
}

/// Settings kept in memory: for tests, and for an app that does not persist them yet.
#[derive(Default)]
pub struct MemorySettings {
    saved: Mutex<Option<OpsSettings>>,
}

impl MemorySettings {
    /// What was saved last.
    pub fn saved(&self) -> Option<OpsSettings> {
        *self.saved.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl SettingsStorage for MemorySettings {
    fn load(&self) -> Result<Option<OpsSettings>, String> {
        Ok(self.saved())
    }

    fn save(&self, settings: &OpsSettings) -> Result<(), String> {
        *self.saved.lock().unwrap_or_else(|e| e.into_inner()) = Some(*settings);
        Ok(())
    }
}

/// Runs with the events of each change to the queue or the journal, while the queue is locked, so it
/// must be quick and must not call back into the plugin's commands.
pub type ChangeHook = Arc<dyn Fn(&[OpsEvent]) + Send + Sync>;

/// Everything the plugin needs from the app. It never calls another plugin: the app adapts the
/// Trash and the vfs listings to these traits.
pub struct OpsDeps {
    pub providers: Providers,
    pub trash: Arc<dyn Trash>,
    pub resolver: Arc<dyn SelectionResolver>,
    pub journal_storage: Arc<dyn JournalStorage>,
    pub settings: Arc<dyn SettingsStorage>,
    pub clock: Arc<dyn Clock>,
    pub protected: Protected,
    pub on_change: Option<ChangeHook>,
    /// The delay before the journal is written after a change.
    pub save_delay: Duration,
    /// How long quitting waits for cancelled jobs to unwind before the journal is written.
    pub exit_wait: Duration,
}

impl OpsDeps {
    /// The given seams with no hook and the default delays.
    pub fn new(
        providers: Providers,
        trash: Arc<dyn Trash>,
        resolver: Arc<dyn SelectionResolver>,
        journal_storage: Arc<dyn JournalStorage>,
        settings: Arc<dyn SettingsStorage>,
        clock: Arc<dyn Clock>,
        protected: Protected,
    ) -> Self {
        Self {
            providers,
            trash,
            resolver,
            journal_storage,
            settings,
            clock,
            protected,
            on_change: None,
            save_delay: SAVE_DELAY,
            exit_wait: EXIT_WAIT,
        }
    }
}
