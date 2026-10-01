// Keeps the last-known time format and emits an event when the 12/24-hour answer flips
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::{
    sync::{Mutex, MutexGuard, PoisonError},
    time::Duration,
};

use tauri::{AppHandle, Emitter, Manager, Runtime};
use tokio::sync::{mpsc, oneshot};

use crate::{
    models::TimeFormat,
    parse::{Observation, Tracker},
    platform,
};

/// Event emitted to all windows with the new `TimeFormat` when the answer flips.
pub const CHANGED_EVENT: &str = "os-prefs://time-format-changed";

/// How long to let a burst of change notifications settle before re-reading.
const DEBOUNCE: Duration = Duration::from_millis(150);

/// How long startup waits for a watcher to begin listening before it reads the baseline anyway.
const READY_TIMEOUT: Duration = Duration::from_secs(5);

/// How long after the baseline a watcher that cannot confirm it is listening gets before the
/// time format is read once more.
const SETTLE: Duration = Duration::from_millis(750);

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// One platform reading: the answer, and why it is not the user's own setting when it is not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reading {
    pub format: TimeFormat,
    /// Present when a better source failed or does not exist on this system.
    pub note: Option<String>,
}

/// Resolves once a watcher is listening: `Err` carries why it could not start.
pub type ReadySignal = oneshot::Receiver<Result<(), String>>;

/// How a platform watcher says it is listening.
// Only some platforms use each variant.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub enum Readiness {
    /// Listening by the time `watch` returns.
    Listening,
    /// Nothing can be watched here; the string says why.
    Unavailable(String),
    /// Listening once this resolves with `Ok`; `Err` (or a dropped sender) means the watcher
    /// could not start.
    Signal(ReadySignal),
    /// A child process was started and its subscription cannot be observed, so the baseline is
    /// read again after `SETTLE` to catch a change made while it was starting.
    Unconfirmed,
}

/// Whether changes are being pushed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WatchState {
    Starting,
    Listening,
    Unavailable(String),
}

/// Plugin state: the last reading, the change tracker and the platform's watcher.
pub struct Service {
    /// Serialises reads so a slow older reading cannot overwrite a newer one.
    gate: tokio::sync::Mutex<()>,
    tracker: Mutex<Tracker>,
    note: Mutex<Option<String>>,
    watch: Mutex<WatchState>,
    watcher: Mutex<Option<platform::Watcher>>,
}

impl Default for Service {
    fn default() -> Self {
        Self {
            gate: tokio::sync::Mutex::new(()),
            tracker: Mutex::new(Tracker::default()),
            note: Mutex::new(None),
            watch: Mutex::new(WatchState::Starting),
            watcher: Mutex::new(None),
        }
    }
}

impl Service {
    /// Reads the platform now, remembers the result and emits the change event if the answer
    /// flipped since the previous reading. The first reading only sets the baseline.
    pub async fn refresh<R: Runtime>(&self, app: &AppHandle<R>) -> TimeFormat {
        let _turn = self.gate.lock().await;
        let reading = platform::read().await;
        let observation = lock(&self.tracker).observe(reading.format);
        *lock(&self.note) = reading.note;
        if observation == Observation::Changed {
            if let Err(error) = app.emit(CHANGED_EVENT, reading.format) {
                log::warn!("failed to emit change event: {error}");
            }
        }
        reading.format
    }

    pub fn note(&self) -> Option<String> {
        lock(&self.note).clone()
    }

    pub fn watch_state(&self) -> WatchState {
        lock(&self.watch).clone()
    }

    fn set_watch(&self, state: WatchState) {
        *lock(&self.watch) = state;
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
            let follow_up = wait_until_ready(readiness, READY_TIMEOUT).await;
            let service = app.state::<Service>();
            match follow_up {
                FollowUp::Done(state) => {
                    service.set_watch(state);
                    service.refresh(&app).await;
                }
                FollowUp::Settle => {
                    service.set_watch(WatchState::Listening);
                    service.refresh(&app).await;
                    tokio::time::sleep(SETTLE).await;
                    service.refresh(&app).await;
                }
                FollowUp::AwaitListening(ready) => {
                    service.refresh(&app).await;
                    // The watcher was slow: read again once it listens, to catch a change in between.
                    service.set_watch(settle_state(ready.await));
                    service.refresh(&app).await;
                }
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

/// Asks for a re-read on a fixed interval, for platforms that cannot announce changes. The
/// service compares each reading with the last, so a tick that finds nothing new is silent.
// Only Windows and macOS poll.
#[cfg_attr(not(any(target_os = "windows", target_os = "macos")), allow(dead_code))]
pub struct Poller(tauri::async_runtime::JoinHandle<()>);

#[cfg_attr(not(any(target_os = "windows", target_os = "macos")), allow(dead_code))]
impl Poller {
    pub fn start(changed: mpsc::UnboundedSender<()>, every: Duration) -> Self {
        Self(tauri::async_runtime::spawn(async move {
            let mut ticker = tokio::time::interval(every);
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            // The first tick fires at once; the baseline read already covers it.
            ticker.tick().await;
            loop {
                ticker.tick().await;
                if changed.send(()).is_err() {
                    break;
                }
            }
        }))
    }
}

impl Drop for Poller {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// What the baseline read needs after a watcher got as far as it did.
enum FollowUp {
    /// The watcher's state is known, so the baseline is complete after one read.
    Done(WatchState),
    /// Read again after `SETTLE`, to catch a change made while a child process was starting.
    Settle,
    /// The watcher had not said it was listening when the wait ran out; its signal is kept.
    AwaitListening(ReadySignal),
}

fn settle_state(signal: Result<Result<(), String>, oneshot::error::RecvError>) -> WatchState {
    match signal {
        Ok(Ok(())) => WatchState::Listening,
        Ok(Err(reason)) => WatchState::Unavailable(reason),
        Err(_) => WatchState::Unavailable("the change watcher stopped".to_string()),
    }
}

/// Waits (for at most `timeout`) until a watcher reports that it is listening. A watcher that
/// fails to start resolves or drops its signal, which ends the wait as well, so startup is never
/// held up for long.
async fn wait_until_ready(readiness: Readiness, timeout: Duration) -> FollowUp {
    match readiness {
        Readiness::Listening => FollowUp::Done(WatchState::Listening),
        Readiness::Unavailable(reason) => FollowUp::Done(WatchState::Unavailable(reason)),
        Readiness::Unconfirmed => FollowUp::Settle,
        Readiness::Signal(mut ready) => match tokio::time::timeout(timeout, &mut ready).await {
            Ok(signal) => FollowUp::Done(settle_state(signal)),
            Err(_) => FollowUp::AwaitListening(ready),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block_on<T>(future: impl std::future::Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap()
            .block_on(future)
    }

    #[test]
    fn a_watcher_that_is_already_listening_needs_no_wait() {
        block_on(async {
            let started = std::time::Instant::now();
            let follow_up = wait_until_ready(Readiness::Listening, Duration::from_secs(30)).await;
            assert!(matches!(follow_up, FollowUp::Done(WatchState::Listening)));
            assert!(started.elapsed() < Duration::from_secs(5));
        });
    }

    #[test]
    fn an_unavailable_watcher_reports_its_reason() {
        block_on(async {
            let follow_up = wait_until_ready(
                Readiness::Unavailable("nothing to watch".into()),
                Duration::from_secs(30),
            )
            .await;
            let FollowUp::Done(WatchState::Unavailable(reason)) = follow_up else {
                panic!("expected an unavailable watcher");
            };
            assert_eq!(reason, "nothing to watch");
        });
    }

    #[test]
    fn a_signal_resolves_the_wait_whether_it_succeeds_fails_or_is_dropped() {
        block_on(async {
            let (tx, rx) = oneshot::channel();
            tx.send(Ok(())).unwrap();
            let ok = wait_until_ready(Readiness::Signal(rx), Duration::from_secs(30)).await;
            assert!(matches!(ok, FollowUp::Done(WatchState::Listening)));

            let (tx, rx) = oneshot::channel();
            tx.send(Err("no portal".to_string())).unwrap();
            let failed = wait_until_ready(Readiness::Signal(rx), Duration::from_secs(30)).await;
            assert!(
                matches!(failed, FollowUp::Done(WatchState::Unavailable(reason)) if reason == "no portal")
            );

            let (tx, rx) = oneshot::channel::<Result<(), String>>();
            drop(tx);
            let dropped = wait_until_ready(Readiness::Signal(rx), Duration::from_secs(30)).await;
            assert!(matches!(
                dropped,
                FollowUp::Done(WatchState::Unavailable(_))
            ));
        });
    }

    #[test]
    fn a_slow_signal_is_kept_so_the_baseline_can_be_read_again() {
        block_on(async {
            let (tx, rx) = oneshot::channel();
            let follow_up =
                wait_until_ready(Readiness::Signal(rx), Duration::from_millis(50)).await;
            let FollowUp::AwaitListening(kept) = follow_up else {
                panic!("a signal that has not arrived must be kept");
            };
            tx.send(Ok(())).unwrap();
            assert_eq!(settle_state(kept.await), WatchState::Listening);
        });
    }

    #[test]
    fn an_unconfirmed_watcher_asks_for_a_second_read() {
        block_on(async {
            let follow_up = wait_until_ready(Readiness::Unconfirmed, Duration::from_secs(30)).await;
            assert!(matches!(follow_up, FollowUp::Settle));
        });
    }
}
