// Keeps the last-known preferences and emits an event when they change
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::{
    sync::{Mutex, MutexGuard, PoisonError},
    time::Duration,
};

use tauri::{AppHandle, Emitter, Manager, Runtime};
use tokio::sync::mpsc;

use crate::{
    models::{Snapshot, TitlebarSnapshot},
    platform,
};

/// Event emitted to all windows with the new `TitlebarSnapshot` when the preferences change.
pub const CHANGED_EVENT: &str = "system-appearance://titlebar-preferences-changed";

/// How long to let a burst of change notifications settle before re-reading.
const DEBOUNCE: Duration = Duration::from_millis(150);

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A reading of the platform with the revision it was assigned.
#[derive(Debug, Clone)]
pub struct Reading {
    pub snapshot: Snapshot,
    pub revision: u32,
}

impl Reading {
    /// The preferences and revision as sent to the front end.
    pub fn titlebar(&self) -> TitlebarSnapshot {
        TitlebarSnapshot {
            revision: self.revision,
            preferences: self.snapshot.preferences.clone(),
        }
    }
}

/// Assigns `next` its revision given the previous reading, and says whether the preferences
/// changed since a baseline existed. The first reading is revision 1 and only sets the baseline;
/// the revision increases exactly when the preferences differ from the previous reading.
pub fn advance(previous: Option<&Reading>, next: Snapshot) -> (Reading, bool) {
    match previous {
        None => (
            Reading {
                snapshot: next,
                revision: 1,
            },
            false,
        ),
        Some(previous) if previous.snapshot.preferences == next.preferences => (
            Reading {
                snapshot: next,
                revision: previous.revision,
            },
            false,
        ),
        Some(previous) => (
            Reading {
                snapshot: next,
                revision: previous.revision + 1,
            },
            true,
        ),
    }
}

/// How long startup waits for a watcher to begin listening before it reads the baseline anyway.
const READY_TIMEOUT: Duration = Duration::from_secs(5);

/// How long after the baseline read a watcher that cannot confirm it is listening gets before the
/// preferences are read once more.
const SETTLE: Duration = Duration::from_millis(750);

/// Whether, and how, a watcher can say it has started listening.
// Only the Linux watchers need the last two variants; the other platforms listen to nothing.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub enum Readiness {
    /// Listening by the time `watch` returns.
    Listening,
    /// Listening once this signal resolves (or is dropped because the watcher could not start).
    Signal(tokio::sync::oneshot::Receiver<()>),
    /// A child process was started and its subscription cannot be observed, so the baseline is
    /// read again after `SETTLE` to catch a change made while it was starting.
    Unconfirmed,
}

/// Waits until a watcher reports that it is listening. A watcher that fails to start drops its
/// signal and one that never answers times out, so the baseline read is never held up for long.
/// Returns whether the preferences should be read again once things have settled.
async fn wait_until_ready(readiness: Readiness, timeout: Duration) -> bool {
    match readiness {
        Readiness::Listening => false,
        Readiness::Signal(ready) => {
            let _ = tokio::time::timeout(timeout, ready).await;
            false
        }
        Readiness::Unconfirmed => true,
    }
}

/// Plugin state: the last reading and the platform's change watcher.
#[derive(Default)]
pub struct Service {
    /// Serialises reads so a slow older reading cannot overwrite a newer one.
    gate: tokio::sync::Mutex<()>,
    last: Mutex<Option<Reading>>,
    watcher: Mutex<Option<platform::Watcher>>,
}

impl Service {
    /// Reads the platform now, remembers the result and emits the change event, stamped with the
    /// new revision, if the preferences differ from the previous reading. The first reading only
    /// sets the baseline.
    pub async fn refresh<R: Runtime>(&self, app: &AppHandle<R>) -> Reading {
        let _turn = self.gate.lock().await;
        let next = platform::read().await;
        let (reading, changed) = {
            let mut last = lock(&self.last);
            let (reading, changed) = advance(last.as_ref(), next);
            *last = Some(reading.clone());
            (reading, changed)
        };
        if changed {
            if let Err(error) = app.emit(CHANGED_EVENT, &reading.titlebar()) {
                log::warn!("failed to emit change event: {error}");
            }
        }
        reading
    }

    /// Starts watching the platform and re-reads whenever it reports a change.
    pub fn start<R: Runtime>(&self, app: &AppHandle<R>) {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut watcher = platform::watch(tx);
        let readiness = watcher.take_readiness();
        *lock(&self.watcher) = Some(watcher);

        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            // Read the baseline only once the watcher is listening, so a change between the two
            // cannot be missed.
            let settle = wait_until_ready(readiness, READY_TIMEOUT).await;
            app.state::<Service>().refresh(&app).await;
            if settle {
                tokio::time::sleep(SETTLE).await;
                app.state::<Service>().refresh(&app).await;
            }
            while rx.recv().await.is_some() {
                tokio::time::sleep(DEBOUNCE).await;
                while rx.try_recv().is_ok() {}
                app.state::<Service>().refresh(&app).await;
            }
        });
    }

    /// Stops the platform watcher, releasing any child processes.
    pub fn stop(&self) {
        lock(&self.watcher).take();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{DesktopEnvironment, TitlebarAction, TitlebarPreferences};

    fn block_on<T>(future: impl std::future::Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap()
            .block_on(future)
    }

    fn snapshot(double_click: TitlebarAction) -> Snapshot {
        let mut preferences = TitlebarPreferences::fallback(DesktopEnvironment::Gnome);
        preferences.actions.double_click = double_click;
        Snapshot::from_source(preferences, "portal")
    }

    #[test]
    fn a_watcher_that_is_already_listening_needs_no_wait_or_second_read() {
        block_on(async {
            let started = std::time::Instant::now();
            assert!(!wait_until_ready(Readiness::Listening, Duration::from_secs(30)).await);
            assert!(started.elapsed() < Duration::from_secs(5));
        });
    }

    #[test]
    fn waiting_for_a_signal_ends_when_it_arrives_or_is_dropped() {
        block_on(async {
            let (tx, rx) = tokio::sync::oneshot::channel();
            tx.send(()).unwrap();
            let started = std::time::Instant::now();
            assert!(!wait_until_ready(Readiness::Signal(rx), Duration::from_secs(30)).await);

            let (tx, rx) = tokio::sync::oneshot::channel::<()>();
            drop(tx);
            assert!(!wait_until_ready(Readiness::Signal(rx), Duration::from_secs(30)).await);
            assert!(started.elapsed() < Duration::from_secs(5));
        });
    }

    #[test]
    fn waiting_for_a_silent_signal_gives_up_at_the_timeout() {
        block_on(async {
            let (_tx, rx) = tokio::sync::oneshot::channel::<()>();
            let started = std::time::Instant::now();
            assert!(!wait_until_ready(Readiness::Signal(rx), Duration::from_millis(50)).await);
            assert!(started.elapsed() >= Duration::from_millis(50));
        });
    }

    #[test]
    fn an_unconfirmed_watcher_asks_for_a_second_read() {
        block_on(async {
            assert!(wait_until_ready(Readiness::Unconfirmed, Duration::from_secs(30)).await);
        });
    }

    #[test]
    fn the_first_reading_is_revision_one_and_not_a_change() {
        let (reading, changed) = advance(None, snapshot(TitlebarAction::ToggleMaximise));
        assert_eq!(reading.revision, 1);
        assert!(!changed);
    }

    #[test]
    fn an_identical_reading_keeps_the_revision_and_is_not_a_change() {
        let (first, _) = advance(None, snapshot(TitlebarAction::ToggleMaximise));
        let (second, changed) = advance(Some(&first), snapshot(TitlebarAction::ToggleMaximise));
        assert_eq!(second.revision, 1);
        assert!(!changed);
    }

    #[test]
    fn a_different_reading_increases_the_revision_and_is_a_change() {
        let (first, _) = advance(None, snapshot(TitlebarAction::ToggleMaximise));
        let (second, changed) = advance(Some(&first), snapshot(TitlebarAction::Minimise));
        assert_eq!(second.revision, 2);
        assert!(changed);
        let (third, changed) = advance(Some(&second), snapshot(TitlebarAction::None));
        assert_eq!(third.revision, 3);
        assert!(changed);
    }

    #[test]
    fn the_wire_format_is_the_preferences_object_plus_a_revision() {
        let (reading, _) = advance(None, snapshot(TitlebarAction::ToggleMaximise));
        let json = serde_json::to_value(reading.titlebar()).unwrap();
        assert_eq!(json["revision"], 1);
        assert!(json["buttonLayout"].is_object());
        assert!(json["actions"].is_object());
    }
}
