// Owns the plugin's state: probe results, registered regions, and the drag in progress with its follow thread
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, MutexGuard,
    },
    thread,
    time::Instant,
};

use log::{debug, warn};
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager, Runtime, WebviewWindow};

use crate::{
    follow::{Action, FollowMachine, StartGuard, STALE_AFTER, TICK, TIMEOUT},
    ghost, main_thread,
    models::{
        BeginReport, BeginState, DropReport, Options, Outcome, PluginStatus, Point, Region, Size,
        CURSOR_STALE_EVENT, FEATURE_CURSOR_FOLLOW, FEATURE_GHOST, FEATURE_HIT_TEST, PAYLOAD_EVENT,
        TIMEOUT_EVENT,
    },
    platform,
    regions::{self, WindowGeometry},
    status::{self, Probes},
};

/// A drag in progress.
#[derive(Clone)]
struct Active {
    source: String,
    stop: Arc<AtomicBool>,
    machine: Arc<Mutex<FollowMachine>>,
}

/// What `end` found.
enum EndTake {
    /// The caller's drag, now ended.
    Ended(Active),
    /// Another window's drag, left running.
    NotYours,
    /// No drag in progress.
    Nothing,
}

/// The state the plugin keeps for the whole session.
pub struct Tearoff {
    options: Options,
    probes: tokio::sync::Mutex<Option<Probes>>,
    guard: StartGuard,
    regions: Mutex<HashMap<String, Vec<Region>>>,
    active: Mutex<Option<Active>>,
    payload: Mutex<Option<Value>>,
}

/// Locks, recovering the data if another thread panicked while holding the lock: the state stays usable and a drag can still be ended.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn has(status: &PluginStatus, feature: &str) -> bool {
    status.features.iter().any(|name| name == feature)
}

fn native_cursor<R: Runtime>(app: &AppHandle<R>) -> Option<Point> {
    app.cursor_position().ok().map(|p| Point { x: p.x, y: p.y })
}

impl Tearoff {
    pub fn new(options: Options) -> Self {
        Self {
            options,
            probes: tokio::sync::Mutex::new(None),
            guard: StartGuard::default(),
            regions: Mutex::new(HashMap::new()),
            active: Mutex::new(None),
            payload: Mutex::new(None),
        }
    }

    /// Creates the shared ghost window; see `ghost::create`.
    pub fn create_ghost<R: Runtime>(&self, app: &AppHandle<R>) {
        ghost::create(app, &self.options);
    }

    pub fn ghost_label(&self) -> &str {
        &self.options.ghost_label
    }

    /// What this system can do, probed once and cached for the session. While a drag owns the ghost the probe is skipped and a pessimistic answer is returned without caching it.
    pub async fn probes<R: Runtime>(&self, app: &AppHandle<R>) -> Probes {
        let mut cached = self.probes.lock().await;
        if let Some(probes) = *cached {
            return probes;
        }
        if self.guard.is_active() {
            return Probes {
                platform: status::Platform::Unsupported,
                ghost_created: false,
                window_position: false,
                hit_test: false,
            };
        }
        let probes = ghost::probe(app, self.ghost_label()).await;
        *cached = Some(probes);
        probes
    }

    pub async fn status<R: Runtime>(&self, app: &AppHandle<R>) -> PluginStatus {
        status::status_for(&self.probes(app).await)
    }

    /// The native cursor in physical pixels, or `None` where the system does not report a usable one.
    pub async fn cursor<R: Runtime>(&self, app: &AppHandle<R>) -> Option<Point> {
        let status = self.status(app).await;
        if !has(&status, FEATURE_CURSOR_FOLLOW) {
            return None;
        }
        let app = app.clone();
        main_thread::run(&app.clone(), move || native_cursor(&app))
            .await
            .flatten()
    }

    #[cfg(test)]
    pub fn payload_for_test(&self) -> MutexGuard<'_, Option<Value>> {
        lock(&self.payload)
    }

    pub fn payload(&self) -> Option<Value> {
        lock(&self.payload).clone()
    }

    pub fn set_regions(&self, label: &str, regions: Vec<Region>) {
        lock(&self.regions).insert(label.to_string(), regions);
    }

    /// Forgets a destroyed window's regions, and ends the drag if that window began it.
    pub fn window_destroyed<R: Runtime>(&self, app: &AppHandle<R>, label: &str) {
        lock(&self.regions).remove(label);
        if let EndTake::Ended(_) = self.take_for_end(label) {
            self.clear_drag(app);
        }
    }

    /// Claims the single drag slot for `source` and registers the drag, before anything is awaited,
    /// so an `end` that arrives while `begin` is still working finds the drag and cancels it.
    /// `None` when a drag is already running.
    fn claim(&self, source: &str) -> Option<Active> {
        if !self.guard.try_start() {
            return None;
        }
        let active = Active {
            source: source.to_string(),
            stop: Arc::new(AtomicBool::new(false)),
            machine: Arc::new(Mutex::new(FollowMachine::new(TIMEOUT, STALE_AFTER))),
        };
        *lock(&self.active) = Some(active.clone());
        Some(active)
    }

    /// Ends the drag if `source` owns it: stops its follow loop and frees the slot. A drag belongs
    /// to the window that began it, so another window's call leaves it running.
    fn take_for_end(&self, source: &str) -> EndTake {
        let mut slot = lock(&self.active);
        match slot.as_ref() {
            None => EndTake::Nothing,
            Some(current) if current.source != source => EndTake::NotYours,
            Some(_) => {
                let ended = slot.take().expect("checked above");
                ended.stop.store(true, Ordering::Release);
                self.guard.finish();
                EndTake::Ended(ended)
            }
        }
    }

    /// Ends the drag identified by its stop token (the follow thread's timeout, or a `begin` that
    /// failed), unless `end` already did.
    fn take_by_token(&self, stop: &Arc<AtomicBool>) -> Option<Active> {
        let mut slot = lock(&self.active);
        match slot.as_ref() {
            Some(current) if Arc::ptr_eq(&current.stop, stop) => {
                let ended = slot.take();
                stop.store(true, Ordering::Release);
                self.guard.finish();
                ended
            }
            _ => None,
        }
    }

    /// After a drag: hides the ghost, forgets the payload and tells the ghost page to clear its
    /// card, so the next drag never shows the last one.
    fn clear_drag<R: Runtime>(&self, app: &AppHandle<R>) {
        self.hide_ghost(app);
        *lock(&self.payload) = None;
        self.emit_payload(app, &Value::Null);
    }

    fn hide_ghost<R: Runtime>(&self, app: &AppHandle<R>) {
        let label = self.options.ghost_label.clone();
        let handle = app.clone();
        let _ = app.run_on_main_thread(move || {
            if let Some(ghost) = handle.get_webview_window(&label) {
                let _ = ghost.hide();
            }
        });
    }

    fn emit_payload<R: Runtime>(&self, app: &AppHandle<R>, payload: &Value) {
        if let Err(error) = app.emit_to(&self.options.ghost_label, PAYLOAD_EVENT, payload) {
            warn!("window-tearoff: cannot send the payload to the ghost: {error}");
        }
    }

    /// Shows the ghost under the cursor and starts following it. Returns `NoGhost`, without starting anything, where the system cannot place a ghost or report the cursor.
    pub async fn begin<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        source: &WebviewWindow<R>,
        payload: Value,
        grab_offset: Point,
        size: Size,
    ) -> BeginReport {
        let status = self.status(app).await;
        let report = |state| BeginReport { state };
        if !has(&status, FEATURE_GHOST) || !has(&status, FEATURE_CURSOR_FOLLOW) {
            return report(BeginState::NoGhost);
        }
        let Some(active) = self.claim(source.label()) else {
            return report(BeginState::AlreadyActive);
        };

        // The payload goes out before the ghost is shown, so the page never draws the last drag's.
        *lock(&self.payload) = Some(payload.clone());
        self.emit_payload(app, &payload);

        let scale = source.scale_factor().unwrap_or(1.0);
        let ghost_label = self.options.ghost_label.clone();
        let started = main_thread::run(app, {
            let app = app.clone();
            move || {
                let cursor = native_cursor(&app)?;
                let ghost = app.get_webview_window(&ghost_label)?;
                let position = regions::placement(cursor, grab_offset, scale);
                ghost::show_at(&ghost, Some((size.width, size.height)), position);
                Some(cursor)
            }
        })
        .await
        .flatten();
        // An `end` that came in while the ghost was being shown has already freed the slot and
        // cleared the payload; the ghost it hid may have been shown again since, so hide it once more.
        if active.stop.load(Ordering::Acquire) {
            if !self.guard.is_active() {
                self.clear_drag(app);
            }
            return report(BeginState::NoGhost);
        }
        let Some(cursor) = started else {
            if self.take_by_token(&active.stop).is_some() {
                self.clear_drag(app);
            }
            return report(BeginState::NoGhost);
        };

        lock(&active.machine).tick(std::time::Duration::ZERO, Some(cursor), true);
        spawn_follow(
            app.clone(),
            FollowJob {
                source: source.label().to_string(),
                ghost: self.options.ghost_label.clone(),
                grab_offset,
                scale,
                stop: active.stop,
                machine: active.machine,
            },
        );
        report(BeginState::Following)
    }

    /// Sends a new payload to the ghost while a drag is in progress; ignored otherwise.
    pub fn update<R: Runtime>(&self, app: &AppHandle<R>, payload: Value) {
        if !self.guard.is_active() {
            return;
        }
        *lock(&self.payload) = Some(payload.clone());
        self.emit_payload(app, &payload);
    }

    /// Ends the drag, hides the ghost and reports where the cursor was and which region it was over. Works without a drag in progress, which is how a caller that got `NoGhost` finds out where the drop landed.
    pub async fn end<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        source: &WebviewWindow<R>,
        outcome: Outcome,
    ) -> DropReport {
        // Only the window that began the drag can end it. Another window's call still reports the
        // cursor and the hit, as a call with no drag does, but leaves the drag and its ghost alone.
        let ended = match self.take_for_end(source.label()) {
            EndTake::Ended(ended) => {
                self.clear_drag(app);
                Some(ended)
            }
            EndTake::Nothing => {
                *lock(&self.payload) = None;
                None
            }
            EndTake::NotYours => None,
        };

        let scale_factor = source.scale_factor().unwrap_or(1.0);
        let probes = *self.probes.lock().await;
        let status = probes.as_ref().map(status::status_for);
        let can_follow = status
            .as_ref()
            .is_some_and(|status| has(status, FEATURE_CURSOR_FOLLOW));
        let can_hit = status
            .as_ref()
            .is_some_and(|status| has(status, FEATURE_HIT_TEST));

        let (last, stale) = ended
            .as_ref()
            .map(|ended| {
                let machine = lock(&ended.machine);
                (machine.last_cursor(), machine.is_stale())
            })
            .unwrap_or((None, false));

        let ghost_label = self.options.ghost_label.clone();
        let regions = lock(&self.regions).clone();
        let snapshot = main_thread::run(app, {
            let app = app.clone();
            move || {
                let cursor = if can_follow {
                    native_cursor(&app)
                } else {
                    None
                };
                let windows = if can_hit && outcome == Outcome::Drop {
                    geometry(&app, &ghost_label, &regions)
                } else {
                    Vec::new()
                };
                (cursor, windows)
            }
        })
        .await;
        let (read, windows) = snapshot.unwrap_or((None, Vec::new()));
        let cursor = read.or(if can_follow { last } else { None });
        let hit = match (outcome, cursor) {
            (Outcome::Drop, Some(cursor)) => regions::hit_test(cursor, &windows),
            _ => None,
        };
        debug!("window-tearoff: ended {outcome:?} at {cursor:?}, hit {hit:?}");
        DropReport {
            cursor,
            scale_factor,
            cursor_stale: stale,
            hit,
        }
    }
}

/// Every visible window's inner geometry with its registered regions, except the ghost. Must run on the main thread.
fn geometry<R: Runtime>(
    app: &AppHandle<R>,
    ghost_label: &str,
    regions: &HashMap<String, Vec<Region>>,
) -> Vec<WindowGeometry> {
    let mut windows: Vec<_> = app
        .webview_windows()
        .into_iter()
        .filter(|(label, window)| {
            label != ghost_label
                && window.is_visible().unwrap_or(true)
                && !window.is_minimized().unwrap_or(false)
        })
        .filter_map(|(label, window)| {
            let position = window.inner_position().ok()?;
            let size = window.inner_size().ok()?;
            Some(WindowGeometry {
                regions: regions.get(&label).cloned().unwrap_or_default(),
                label,
                inner_position: (position.x, position.y),
                inner_size: (size.width, size.height),
                scale_factor: window.scale_factor().unwrap_or(1.0),
            })
        })
        .collect();
    // `webview_windows` is unordered; a stable order makes the tie-break repeatable.
    windows.sort_by(|a, b| a.label.cmp(&b.label));
    windows
}

struct FollowJob {
    source: String,
    ghost: String,
    grab_offset: Point,
    scale: f64,
    stop: Arc<AtomicBool>,
    machine: Arc<Mutex<FollowMachine>>,
}

/// Runs the follow loop on its own thread: every tick the cursor read, the button state and the ghost move happen together on the main thread, and the loop ends when the drag does.
fn spawn_follow<R: Runtime>(app: AppHandle<R>, job: FollowJob) {
    let spawned = thread::Builder::new()
        .name("window-tearoff-follow".into())
        .spawn(move || {
            let started = Instant::now();
            while !job.stop.load(Ordering::Acquire) {
                thread::sleep(TICK);
                if job.stop.load(Ordering::Acquire) {
                    break;
                }
                let action = main_thread::run_blocking(&app, {
                    let app = app.clone();
                    let machine = job.machine.clone();
                    let ghost = job.ghost.clone();
                    let (grab_offset, scale) = (job.grab_offset, job.scale);
                    move || {
                        let action = lock(&machine).tick(
                            started.elapsed(),
                            native_cursor(&app),
                            platform::buttons_held(),
                        );
                        if let Action::Move(cursor) | Action::Recovered(cursor) = action {
                            if let Some(ghost) = app.get_webview_window(&ghost) {
                                let (x, y) = regions::placement(cursor, grab_offset, scale);
                                let _ = ghost.set_position(tauri::PhysicalPosition::new(x, y));
                            }
                        }
                        action
                    }
                });
                match action {
                    None => break,
                    Some(Action::Timeout) => {
                        finish_on_timeout(&app, &job);
                        break;
                    }
                    Some(Action::Stale) => {
                        let _ = app.emit_to(job.source.as_str(), CURSOR_STALE_EVENT, true);
                    }
                    Some(Action::Recovered(_)) => {
                        let _ = app.emit_to(job.source.as_str(), CURSOR_STALE_EVENT, false);
                    }
                    Some(Action::Move(_) | Action::Idle) => {}
                }
            }
        });
    if let Err(error) = spawned {
        warn!("window-tearoff: cannot start the follow thread: {error}");
    }
}

/// Ends a drag that ran too long, unless `end` already did.
fn finish_on_timeout<R: Runtime>(app: &AppHandle<R>, job: &FollowJob) {
    let Some(state) = app.try_state::<Tearoff>() else {
        return;
    };
    if state.take_by_token(&job.stop).is_some() {
        state.clear_drag(app);
        let _ = app.emit_to(job.source.as_str(), TIMEOUT_EVENT, ());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn tearoff() -> Tearoff {
        Tearoff::new(Options::default())
    }

    #[test]
    fn an_end_during_a_pending_begin_cancels_it() {
        let state = tearoff();
        // `begin` claims the slot before it awaits the main thread ...
        let pending = state.claim("main-1").expect("the slot is free");
        assert!(state.guard.is_active());
        assert!(state.claim("main-2").is_none(), "one drag at a time");
        // ... so an `end` arriving meanwhile finds the drag and stops it.
        assert!(matches!(
            state.take_for_end("main-1"),
            EndTake::Ended(ended) if Arc::ptr_eq(&ended.stop, &pending.stop)
        ));
        assert!(
            pending.stop.load(Ordering::Acquire),
            "the begin sees the cancellation when it resumes"
        );
        assert!(!state.guard.is_active());
        assert!(state.claim("main-2").is_some(), "the slot is free again");
    }

    #[test]
    fn only_the_window_that_began_a_drag_can_end_it() {
        let state = tearoff();
        let drag = state.claim("main-1").unwrap();
        assert!(matches!(state.take_for_end("main-2"), EndTake::NotYours));
        assert!(state.guard.is_active());
        assert!(!drag.stop.load(Ordering::Acquire));
        assert!(matches!(state.take_for_end("main-1"), EndTake::Ended(_)));
        assert!(matches!(state.take_for_end("main-1"), EndTake::Nothing));
    }

    #[test]
    fn the_timeout_ends_only_its_own_drag() {
        let state = tearoff();
        let first = state.claim("main-1").unwrap();
        assert!(matches!(state.take_for_end("main-1"), EndTake::Ended(_)));
        let second = state.claim("main-1").unwrap();
        assert!(state.take_by_token(&first.stop).is_none());
        assert!(state.take_by_token(&second.stop).is_some());
        assert!(!state.guard.is_active());
    }

    #[test]
    fn a_destroyed_window_ends_its_drag_and_forgets_its_payload() {
        use tauri::test::{mock_builder, mock_context, noop_assets};
        let app = mock_builder()
            .build(mock_context(noop_assets()))
            .expect("the mock app builds");
        let state = tearoff();
        state.claim("main-1").unwrap();
        *lock(&state.payload) = Some(json!({ "title": "Documents" }));
        state.window_destroyed(app.handle(), "main-2");
        assert!(
            state.payload().is_some(),
            "another window's destruction changes nothing"
        );
        state.window_destroyed(app.handle(), "main-1");
        assert_eq!(state.payload(), None);
        assert!(!state.guard.is_active());
    }
}
