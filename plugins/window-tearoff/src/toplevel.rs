// Tracks the toplevel drag in progress, decides how it ended and who is told, free of any windowing system
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

// Only the Linux module drives a drag; the bookkeeping is shared so it can be tested everywhere.
#![cfg_attr(not(target_os = "linux"), allow(dead_code))]

use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU32, Ordering},
        Mutex, MutexGuard,
    },
    time::{Duration, Instant},
};

use serde_json::Value;
use tauri::{AppHandle, Emitter, Runtime};

use crate::models::{
    DragLeave, PayloadDropped, ToplevelBeginReport, ToplevelBeginState, ToplevelDragEnded,
    ToplevelDragStarted, ToplevelOutcome, DRAG_LEAVE_EVENT, TOPLEVEL_DRAG_ENDED_EVENT,
};

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::{begin, cancel, install_drop_target, probe};

#[cfg(not(target_os = "linux"))]
mod unsupported;
#[cfg(not(target_os = "linux"))]
pub use unsupported::{begin, cancel, install_drop_target, probe};

/// The drag in progress.
#[derive(Debug, Clone)]
struct Active {
    /// The window the pointer is pressed in, which asked for the drag.
    source: String,
    /// The window that follows the pointer.
    window: String,
    payload: Value,
    /// The window that took the payload, once one has.
    target: Option<String>,
    /// The region of `target` the drop was over, when it was over one.
    region: Option<String>,
}

/// The least time between two `drag-hover` events to one window (about 20 a second).
pub const HOVER_INTERVAL: Duration = Duration::from_millis(50);

/// What the plugin owes the windows for one pointer movement of a toplevel drag.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Motion {
    /// A window the payload was over until now, which is to be told it has left.
    pub leave: Option<String>,
    /// Whether the window it is over now is to be told where the pointer is.
    pub hover: bool,
    /// A position was held back by the throttle and none is waiting to send it: call `HoverGate::flush` after `HOVER_INTERVAL`, since a pointer that then stops sends nothing more.
    pub trail: bool,
}

/// Throttles and de-duplicates the pointer positions a drop target reports during a toplevel drag, so the page of the window hovered hears of the pointer at about 20 Hz and only when it moved. It also remembers which window was told, so every window that was sent a `drag-hover` is sent a `drag-leave` exactly once.
#[derive(Debug, Default)]
pub struct HoverGate {
    window: Option<String>,
    sent: Option<(f64, f64, Instant)>,
    /// The latest position seen, which a throttled one leaves unsent.
    latest: (f64, f64),
    /// A flush is already owed.
    trailing: bool,
}

impl HoverGate {
    /// The pointer is at (`x`, `y`) in `label`'s content at `now`.
    pub fn motion(&mut self, label: &str, x: f64, y: f64, now: Instant) -> Motion {
        let mut motion = Motion::default();
        if self.window.as_deref() != Some(label) {
            motion.leave = self.window.take();
            self.sent = None;
            self.window = Some(label.to_string());
        }
        self.latest = (x, y);
        let due = match self.sent {
            None => true,
            Some((last_x, last_y, at)) => {
                (last_x != x || last_y != y) && now.saturating_duration_since(at) >= HOVER_INTERVAL
            }
        };
        if due {
            self.sent = Some((x, y, now));
        } else if !self.trailing && self.sent.is_some_and(|(sx, sy, _)| (sx, sy) != (x, y)) {
            self.trailing = true;
            motion.trail = true;
        }
        motion.hover = due;
        motion
    }

    /// The throttle's interval has passed: the position to send `label` now if the pointer is still over it and moved since the last one sent.
    pub fn flush(&mut self, label: &str, now: Instant) -> Option<(f64, f64)> {
        self.trailing = false;
        if self.window.as_deref() != Some(label) {
            return None;
        }
        let (x, y) = self.latest;
        match self.sent {
            Some((sx, sy, _)) if (sx, sy) != (x, y) => {
                self.sent = Some((x, y, now));
                Some((x, y))
            }
            _ => None,
        }
    }

    /// The payload left `label`, or was dropped on it. True when that window had been sent a `drag-hover` and is to be sent a `drag-leave`.
    pub fn leave(&mut self, label: &str) -> bool {
        if self.window.as_deref() == Some(label) {
            self.window = None;
            self.sent = None;
            true
        } else {
            false
        }
    }

    /// The drag ended: the window that was hovered, if any, is to be told it has left.
    pub fn end(&mut self) -> Option<String> {
        self.sent = None;
        self.window.take()
    }
}

/// How the compositor ended a drag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Finish {
    /// A target took the payload.
    Finished,
    /// Cancelled; `after_drop` when the button was released first, with nothing to take the payload.
    Cancelled {
        after_drop: bool,
    },
    Failed(String),
}

/// The windows a toplevel drag's end is sent to: the one that began it, and the one that was dragged (which owns what the drag carried and may be the only page left to act on it).
pub fn recipients(ended: &ToplevelDragEnded) -> Vec<&str> {
    let mut labels = vec![ended.source.as_str()];
    if ended.window != ended.source {
        labels.push(ended.window.as_str());
    }
    labels
}

/// Tells the windows a drag's end involves (see `recipients`).
pub fn emit_ended<R: Runtime>(app: &AppHandle<R>, ended: &ToplevelDragEnded) {
    for label in recipients(ended) {
        if let Err(error) = app.emit_to(label, TOPLEVEL_DRAG_ENDED_EVENT, ended) {
            log::warn!("window-tearoff: cannot tell `{label}` the drag ended: {error}");
        }
    }
}

/// Tells the window the payload was over, if any, that it has left, for a drag that ends (or is abandoned) with the pointer over a window.
pub fn leave_hovered<R: Runtime>(app: &AppHandle<R>, state: &State) {
    if let Some(label) = state.hover_end() {
        emit_leave(app, &label);
    }
}

/// Tells `label` the payload has left it.
pub fn emit_leave<R: Runtime>(app: &AppHandle<R>, label: &str) {
    let event = DragLeave {
        window: label.to_string(),
    };
    if let Err(error) = app.emit_to(label, DRAG_LEAVE_EVENT, &event) {
        log::warn!("window-tearoff: cannot tell `{label}` the payload left: {error}");
    }
}

/// The toplevel drag state: at most one drag at a time, and the result of each ended drag until the dragged window's page has read it (a page that is still loading when the drag ends misses the event).
#[derive(Default)]
pub struct State {
    active: Mutex<Option<Active>>,
    results: Mutex<HashMap<String, ToplevelDragEnded>>,
    seq: AtomicU32,
    hover: Mutex<HoverGate>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl State {
    /// Whether a drag is running.
    #[cfg(test)]
    pub fn is_active(&self) -> bool {
        lock(&self.active).is_some()
    }

    /// Claims the drag slot, or says why not.
    pub fn begin(
        &self,
        source: &str,
        window: &str,
        payload: &Value,
    ) -> Result<ToplevelDragStarted, ToplevelBeginReport> {
        let mut active = lock(&self.active);
        if active.is_some() {
            return Err(ToplevelBeginReport {
                state: ToplevelBeginState::AlreadyActive,
                reason: Some("a toplevel drag is already running".into()),
            });
        }
        *active = Some(Active {
            source: source.to_string(),
            window: window.to_string(),
            payload: payload.clone(),
            target: None,
            region: None,
        });
        *lock(&self.hover) = HoverGate::default();
        lock(&self.results).remove(window);
        Ok(ToplevelDragStarted {
            window: window.to_string(),
            payload: payload.clone(),
        })
    }

    /// Gives the drag slot back when the drag never started.
    pub fn abandon(&self) {
        *lock(&self.active) = None;
        lock(&self.hover).end();
    }

    /// The drag's payload when `window` is a window it can hover: a drag is running and the window is not the one being dragged (it sits under the pointer, and a hover over itself is a hover over nothing).
    pub fn hover_payload(&self, window: &str) -> Option<Value> {
        let active = lock(&self.active);
        let current = active.as_ref()?;
        (current.window != window).then(|| current.payload.clone())
    }

    /// The pointer is at (`x`, `y`) in `window`'s content; see `HoverGate::motion`.
    pub fn hover_motion(&self, window: &str, x: f64, y: f64, now: Instant) -> Motion {
        lock(&self.hover).motion(window, x, y, now)
    }

    /// The throttle's interval has passed; see `HoverGate::flush`.
    pub fn hover_flush(&self, window: &str, now: Instant) -> Option<(f64, f64)> {
        lock(&self.hover).flush(window, now)
    }

    /// The drag is over: the window it was over, if any; see `HoverGate::end`.
    pub fn hover_end(&self) -> Option<String> {
        lock(&self.hover).end()
    }

    /// The payload left `window` or is dropped on it; see `HoverGate::leave`.
    pub fn hover_leave(&self, window: &str) -> bool {
        lock(&self.hover).leave(window)
    }

    /// A window took the drag's payload at (`x`, `y`) in its content, over `region` when one. Returns what to tell that window; `None` when no drag is running, or when the window is the one being dragged (it sits under the pointer, and a drop on itself is a drop on nothing).
    pub fn payload_dropped(
        &self,
        target: &str,
        payload: Value,
        at: (f64, f64),
        region: Option<String>,
    ) -> Option<PayloadDropped> {
        let mut active = lock(&self.active);
        let current = active.as_mut()?;
        if current.window == target {
            return None;
        }
        current.target = Some(target.to_string());
        current.region = region.clone();
        Some(PayloadDropped {
            window: target.to_string(),
            payload,
            x: at.0,
            y: at.1,
            region,
        })
    }

    /// Ends the drag. Returns the result to send, once: a second end (the compositor may send `cancelled` right after `dnd_finished`) finds nothing running and returns `None`.
    pub fn finish(&self, how: Finish) -> Option<ToplevelDragEnded> {
        let current = lock(&self.active).take()?;
        lock(&self.hover).end();
        let region = current.region;
        let (outcome, target, reason) = match how {
            Finish::Finished => match current.target {
                Some(target) => (ToplevelOutcome::DroppedOnWindow, Some(target), None),
                // Taken by a surface of the app that is not a window the page can name.
                None => (ToplevelOutcome::DroppedElsewhere, None, None),
            },
            Finish::Cancelled { after_drop: true } => {
                (ToplevelOutcome::DroppedElsewhere, None, None)
            }
            Finish::Cancelled { after_drop: false } => (ToplevelOutcome::Cancelled, None, None),
            Finish::Failed(reason) => (ToplevelOutcome::Failed, None, Some(reason)),
        };
        let ended = self.record(ToplevelDragEnded {
            seq: 0,
            window: current.window.clone(),
            source: current.source,
            outcome,
            target,
            region,
            payload: current.payload,
            reason,
        });
        Some(ended)
    }

    /// Ends a drag that never started, for a caller that has already named the window it meant to drag: that window's page is told so it can put its contents back.
    pub fn fail_unstarted(
        &self,
        source: &str,
        window: &str,
        payload: &Value,
        reason: &str,
    ) -> ToplevelDragEnded {
        self.record(ToplevelDragEnded {
            seq: 0,
            window: window.to_string(),
            source: source.to_string(),
            outcome: ToplevelOutcome::Failed,
            target: None,
            region: None,
            payload: payload.clone(),
            reason: Some(reason.to_string()),
        })
    }

    /// Numbers a result and keeps it for the dragged window's page.
    fn record(&self, mut ended: ToplevelDragEnded) -> ToplevelDragEnded {
        ended.seq = self.seq.fetch_add(1, Ordering::Relaxed) + 1;
        lock(&self.results).insert(ended.window.clone(), ended.clone());
        ended
    }

    /// The labels of the windows a running drag involves, to end it when one of them is destroyed.
    pub fn involves(&self, label: &str) -> bool {
        lock(&self.active)
            .as_ref()
            .is_some_and(|current| current.source == label || current.window == label)
    }

    /// The result of the drag that moved `window`, if it has ended and nobody has read it. Reading it removes it.
    pub fn take_result(&self, window: &str) -> Option<ToplevelDragEnded> {
        lock(&self.results).remove(window)
    }

    /// Forgets a destroyed window's unread result.
    pub fn forget(&self, window: &str) {
        lock(&self.results).remove(window);
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn begun() -> State {
        let state = State::default();
        state
            .begin("main-1", "main-2", &json!({"tabs": [1, 2]}))
            .expect("the drag begins");
        state
    }

    #[test]
    fn a_drag_reports_what_was_dragged() {
        let state = State::default();
        let started = state.begin("main-1", "main-2", &json!({"a": 1})).unwrap();
        assert_eq!(started.window, "main-2");
        assert_eq!(started.payload, json!({"a": 1}));
        assert!(state.is_active());
    }

    #[test]
    fn a_second_drag_is_refused_while_one_runs() {
        let state = begun();
        let refused = state.begin("main-1", "main-3", &json!(null)).unwrap_err();
        assert_eq!(refused.state, ToplevelBeginState::AlreadyActive);
        assert!(refused.reason.is_some());
    }

    #[test]
    fn a_drag_that_never_started_gives_the_slot_back() {
        let state = begun();
        state.abandon();
        assert!(!state.is_active());
        assert!(state.begin("main-1", "main-2", &json!(null)).is_ok());
    }

    #[test]
    fn a_drop_on_another_window_names_it_and_routes_the_payload() {
        let state = begun();
        let dropped = state
            .payload_dropped(
                "main-3",
                json!({"tabs": [1, 2]}),
                (30.0, 12.0),
                Some("slot:2".into()),
            )
            .expect("a drag is running");
        assert_eq!(dropped.window, "main-3");
        assert_eq!(dropped.payload, json!({"tabs": [1, 2]}));
        assert_eq!((dropped.x, dropped.y), (30.0, 12.0));
        assert_eq!(dropped.region.as_deref(), Some("slot:2"));
        let ended = state.finish(Finish::Finished).unwrap();
        assert_eq!(ended.outcome, ToplevelOutcome::DroppedOnWindow);
        assert_eq!(ended.target.as_deref(), Some("main-3"));
        assert_eq!(ended.region.as_deref(), Some("slot:2"));
        assert_eq!(ended.window, "main-2");
        assert_eq!(ended.source, "main-1");
        assert_eq!(ended.payload, json!({"tabs": [1, 2]}));
    }

    #[test]
    fn a_drop_back_on_the_source_is_a_drop_on_a_window() {
        let state = begun();
        assert!(state
            .payload_dropped("main-1", json!(null), (0.0, 0.0), None)
            .is_some());
        let ended = state.finish(Finish::Finished).unwrap();
        assert_eq!(ended.outcome, ToplevelOutcome::DroppedOnWindow);
        assert_eq!(ended.target.as_deref(), Some("main-1"));
    }

    #[test]
    fn a_drop_on_the_dragged_window_itself_counts_for_nothing() {
        let state = begun();
        assert!(state
            .payload_dropped("main-2", json!(null), (0.0, 0.0), None)
            .is_none());
        let ended = state.finish(Finish::Finished).unwrap();
        assert_eq!(ended.outcome, ToplevelOutcome::DroppedElsewhere);
        assert_eq!(ended.target, None);
    }

    #[test]
    fn a_payload_is_not_routed_when_no_drag_runs() {
        assert!(State::default()
            .payload_dropped("main-3", json!(1), (0.0, 0.0), None)
            .is_none());
    }

    #[test]
    fn a_release_over_nothing_leaves_the_window_where_it_is() {
        let state = begun();
        let ended = state
            .finish(Finish::Cancelled { after_drop: true })
            .unwrap();
        assert_eq!(ended.outcome, ToplevelOutcome::DroppedElsewhere);
    }

    #[test]
    fn escape_is_a_cancel() {
        let state = begun();
        let ended = state
            .finish(Finish::Cancelled { after_drop: false })
            .unwrap();
        assert_eq!(ended.outcome, ToplevelOutcome::Cancelled);
        assert_eq!(ended.target, None);
    }

    #[test]
    fn a_failure_carries_its_reason() {
        let state = begun();
        let ended = state.finish(Finish::Failed("no button".into())).unwrap();
        assert_eq!(ended.outcome, ToplevelOutcome::Failed);
        assert_eq!(ended.reason.as_deref(), Some("no button"));
    }

    #[test]
    fn a_drag_ends_once() {
        let state = begun();
        state.payload_dropped("main-3", json!(null), (0.0, 0.0), None);
        assert!(state.finish(Finish::Finished).is_some());
        // `cancelled` can arrive right after `dnd_finished`.
        assert!(state
            .finish(Finish::Cancelled { after_drop: false })
            .is_none());
        assert!(!state.is_active());
    }

    #[test]
    fn each_result_is_numbered() {
        let state = begun();
        let first = state.finish(Finish::Finished).unwrap();
        state.begin("main-1", "main-2", &json!(null)).unwrap();
        let second = state.finish(Finish::Finished).unwrap();
        assert!(second.seq > first.seq);
    }

    #[test]
    fn a_drag_that_never_started_still_tells_the_window_it_named() {
        let state = State::default();
        let ended = state.fail_unstarted("main-1", "main-2", &json!({"a": 1}), "no button");
        assert_eq!(ended.outcome, ToplevelOutcome::Failed);
        assert_eq!(ended.window, "main-2");
        assert_eq!(ended.reason.as_deref(), Some("no button"));
        assert_eq!(recipients(&ended), ["main-1", "main-2"]);
        assert_eq!(state.take_result("main-2").unwrap().seq, ended.seq);
    }

    #[test]
    fn the_result_is_kept_for_a_page_that_loaded_late_and_read_once() {
        let state = begun();
        state.finish(Finish::Cancelled { after_drop: false });
        let result = state.take_result("main-2").expect("kept");
        assert_eq!(result.outcome, ToplevelOutcome::Cancelled);
        assert!(state.take_result("main-2").is_none());
        assert!(state.take_result("main-1").is_none());
    }

    #[test]
    fn a_new_drag_of_a_window_drops_its_old_result() {
        let state = begun();
        state.finish(Finish::Cancelled { after_drop: false });
        state.begin("main-1", "main-2", &json!(null)).unwrap();
        assert!(state.take_result("main-2").is_none());
    }

    #[test]
    fn the_result_goes_to_the_source_and_the_dragged_window() {
        let state = begun();
        let ended = state.finish(Finish::Finished).unwrap();
        assert_eq!(recipients(&ended), ["main-1", "main-2"]);
        let same = ToplevelDragEnded {
            window: "main-1".into(),
            ..ended
        };
        assert_eq!(recipients(&same), ["main-1"]);
    }

    #[test]
    fn a_destroyed_window_ends_a_drag_it_is_part_of() {
        let state = begun();
        assert!(state.involves("main-1"));
        assert!(state.involves("main-2"));
        assert!(!state.involves("main-3"));
        state.finish(Finish::Cancelled { after_drop: false });
        assert!(!state.involves("main-1"));
    }

    #[test]
    fn forgetting_a_window_drops_its_unread_result() {
        let state = begun();
        state.finish(Finish::Finished);
        state.forget("main-2");
        assert!(state.take_result("main-2").is_none());
    }

    #[test]
    fn only_a_window_other_than_the_dragged_one_is_hovered() {
        let state = begun();
        assert_eq!(state.hover_payload("main-3"), Some(json!({"tabs": [1, 2]})));
        assert!(state.hover_payload("main-1").is_some());
        assert_eq!(state.hover_payload("main-2"), None);
        assert_eq!(State::default().hover_payload("main-3"), None);
    }

    fn ms(base: Instant, n: u64) -> Instant {
        base + Duration::from_millis(n)
    }

    #[test]
    fn the_first_motion_in_a_window_is_sent_at_once() {
        let base = Instant::now();
        let mut gate = HoverGate::default();
        let motion = gate.motion("main-1", 10.0, 5.0, base);
        assert_eq!(
            motion,
            Motion {
                leave: None,
                hover: true,
                trail: false
            }
        );
    }

    #[test]
    fn motion_is_limited_to_about_twenty_a_second() {
        let base = Instant::now();
        let mut gate = HoverGate::default();
        assert!(gate.motion("a", 1.0, 1.0, base).hover);
        assert!(!gate.motion("a", 2.0, 1.0, ms(base, 10)).hover);
        assert!(!gate.motion("a", 3.0, 1.0, ms(base, 49)).hover);
        assert!(gate.motion("a", 4.0, 1.0, ms(base, 50)).hover);
        assert!(!gate.motion("a", 5.0, 1.0, ms(base, 60)).hover);
        assert!(gate.motion("a", 6.0, 1.0, ms(base, 100)).hover);
    }

    #[test]
    fn a_throttled_position_is_sent_after_the_interval_when_the_pointer_stops() {
        let base = Instant::now();
        let mut gate = HoverGate::default();
        assert!(gate.motion("a", 1.0, 1.0, base).hover);
        // Held back: one flush is owed, not one per skipped position.
        let held = gate.motion("a", 2.0, 1.0, ms(base, 20));
        assert!(!held.hover && held.trail);
        assert!(!gate.motion("a", 3.0, 1.0, ms(base, 30)).trail);
        // The pointer stopped at 3: that is what the flush sends, once.
        assert_eq!(gate.flush("a", ms(base, 70)), Some((3.0, 1.0)));
        assert_eq!(gate.flush("a", ms(base, 120)), None);
    }

    #[test]
    fn a_flush_for_a_window_the_pointer_left_sends_nothing() {
        let base = Instant::now();
        let mut gate = HoverGate::default();
        gate.motion("a", 1.0, 1.0, base);
        assert!(gate.motion("a", 2.0, 1.0, ms(base, 20)).trail);
        gate.leave("a");
        assert_eq!(gate.flush("a", ms(base, 70)), None);
    }

    #[test]
    fn a_pointer_that_has_not_moved_is_not_sent_again() {
        let base = Instant::now();
        let mut gate = HoverGate::default();
        assert!(gate.motion("a", 1.0, 1.0, base).hover);
        assert!(!gate.motion("a", 1.0, 1.0, ms(base, 500)).hover);
        assert!(gate.motion("a", 1.0, 2.0, ms(base, 501)).hover);
    }

    #[test]
    fn moving_to_another_window_leaves_the_first_and_is_sent_at_once() {
        let base = Instant::now();
        let mut gate = HoverGate::default();
        gate.motion("a", 1.0, 1.0, base);
        let motion = gate.motion("b", 9.0, 9.0, ms(base, 1));
        assert_eq!(motion.leave.as_deref(), Some("a"));
        assert!(motion.hover);
        // `a` was left already, so it is not left again.
        assert!(!gate.leave("a"));
        assert!(gate.leave("b"));
    }

    #[test]
    fn a_leave_is_owed_only_to_a_window_that_was_hovered_and_only_once() {
        let base = Instant::now();
        let mut gate = HoverGate::default();
        assert!(!gate.leave("a"));
        gate.motion("a", 1.0, 1.0, base);
        assert!(gate.leave("a"));
        assert!(!gate.leave("a"));
        // Coming back is sent at once again.
        assert!(gate.motion("a", 1.0, 1.0, ms(base, 1)).hover);
    }

    #[test]
    fn the_end_of_a_drag_leaves_the_hovered_window() {
        let base = Instant::now();
        let mut gate = HoverGate::default();
        gate.motion("a", 1.0, 1.0, base);
        assert_eq!(gate.end().as_deref(), Some("a"));
        assert_eq!(gate.end(), None);
    }
}
