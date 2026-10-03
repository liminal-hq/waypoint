// Reports an outbound drag's position to the application's own windows under it, for platforms whose own drag events for them are held back until the drag ends
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::{
    collections::HashSet,
    time::{Duration, Instant},
};

use crate::models::Modifiers;

/// How long after an outbound drag ends the platform's own events for it may still arrive. A drop that has not arrived by then is not coming, so the window is told the files left.
pub const SETTLE: Duration = Duration::from_millis(1500);

/// Where the drag's cursor is, as one sample.
#[derive(Debug, Clone, PartialEq)]
pub struct Sample {
    /// The window under the cursor.
    pub window: String,
    /// The cursor in that window's physical client pixels.
    pub position: (f64, f64),
    pub modifiers: Modifiers,
}

/// What to tell a window.
#[derive(Debug, Clone, PartialEq)]
pub enum Out {
    Enter(Sample),
    Over(Sample),
    Leave(String),
}

/// The kinds of event the platform's own drag handler delivers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Real {
    Enter,
    Over,
    Leave,
    Drop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Idle,
    Running,
    Settling(Instant),
}

/// Turns cursor samples taken while an outbound drag runs into `enter`, `over` and `leave` for the windows under it, and swallows the platform's own, late copies of those events so a page never sees a drag begin or end twice. A `drop` is always delivered: the platform's is the only one there is.
#[derive(Debug)]
pub struct Hover {
    phase: Phase,
    /// The window the cursor is over now.
    current: Option<String>,
    /// The last sample sent, to send `over` only when something changed.
    last: Option<Sample>,
    /// The windows that were sent an `enter` during this drag and have not yet had its end from the platform.
    synthesised: HashSet<String>,
}

impl Default for Hover {
    fn default() -> Self {
        Hover {
            phase: Phase::Idle,
            current: None,
            last: None,
            synthesised: HashSet::new(),
        }
    }
}

impl Hover {
    /// An outbound drag starts. Anything left from the last one is dropped.
    pub fn begin(&mut self) {
        *self = Hover {
            phase: Phase::Running,
            ..Hover::default()
        };
    }

    /// The cursor, sampled while the drag runs; `None` when it is over no window of this application.
    pub fn sample(&mut self, at: Option<Sample>) -> Vec<Out> {
        if self.phase != Phase::Running {
            return Vec::new();
        }
        let mut out = Vec::new();
        match at {
            None => self.leave(&mut out),
            Some(sample) => {
                if self.current.as_deref() != Some(sample.window.as_str()) {
                    self.leave(&mut out);
                    self.current = Some(sample.window.clone());
                    self.synthesised.insert(sample.window.clone());
                    self.last = Some(sample.clone());
                    out.push(Out::Enter(sample));
                } else if self.last.as_ref() != Some(&sample) {
                    self.last = Some(sample.clone());
                    out.push(Out::Over(sample));
                }
            }
        }
        out
    }

    fn leave(&mut self, out: &mut Vec<Out>) {
        self.last = None;
        if let Some(window) = self.current.take() {
            out.push(Out::Leave(window));
        }
    }

    /// The drag ends. Abandoned, the window under the cursor is told the files left at once. Released, the platform delivers a drop to it (or a leave if it takes none), so it is left as it is until that arrives or `expire` gives up on it.
    pub fn end(&mut self, cancelled: bool, now: Instant) -> Vec<Out> {
        if self.phase != Phase::Running {
            return Vec::new();
        }
        let mut out = Vec::new();
        if cancelled {
            self.leave(&mut out);
        }
        self.phase = Phase::Settling(now);
        out
    }

    /// Whether the platform's event `kind` for `window` is to be delivered. The ones for a window already told about the drag are not, except its drop, which also finishes the window's part in the drag.
    pub fn deliver(&mut self, kind: Real, window: &str, now: Instant) -> bool {
        if let Phase::Settling(since) = self.phase {
            if now.saturating_duration_since(since) >= SETTLE {
                self.reset();
            }
        }
        if !self.synthesised.contains(window) {
            return true;
        }
        if kind == Real::Drop {
            self.synthesised.remove(window);
            if self.current.as_deref() == Some(window) {
                self.current = None;
                self.last = None;
            }
            if self.synthesised.is_empty() && self.current.is_none() {
                self.reset();
            }
            return true;
        }
        false
    }

    /// Gives up on the platform's own events once `SETTLE` has passed since the drag ended: a window still holding the files is told they left.
    pub fn expire(&mut self, now: Instant) -> Vec<Out> {
        let Phase::Settling(since) = self.phase else {
            return Vec::new();
        };
        if now.saturating_duration_since(since) < SETTLE {
            return Vec::new();
        }
        let mut out = Vec::new();
        self.leave(&mut out);
        self.reset();
        out
    }

    fn reset(&mut self) {
        *self = Hover::default();
    }

    /// Whether a drag is running (for the glue to know it should be sampling).
    pub fn running(&self) -> bool {
        self.phase == Phase::Running
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(window: &str, x: f64, y: f64) -> Option<Sample> {
        Some(Sample {
            window: window.into(),
            position: (x, y),
            modifiers: Modifiers::default(),
        })
    }

    fn running() -> Hover {
        let mut hover = Hover::default();
        hover.begin();
        hover
    }

    #[test]
    fn the_first_sample_over_a_window_enters_and_the_next_moves() {
        let mut hover = running();
        assert_eq!(
            hover.sample(at("shelf", 10.0, 20.0)),
            vec![Out::Enter(at("shelf", 10.0, 20.0).unwrap())]
        );
        assert_eq!(
            hover.sample(at("shelf", 12.0, 20.0)),
            vec![Out::Over(at("shelf", 12.0, 20.0).unwrap())]
        );
    }

    #[test]
    fn a_sample_that_changed_nothing_says_nothing() {
        let mut hover = running();
        hover.sample(at("shelf", 10.0, 20.0));
        assert!(hover.sample(at("shelf", 10.0, 20.0)).is_empty());
        let mut with_ctrl = at("shelf", 10.0, 20.0).unwrap();
        with_ctrl.modifiers.ctrl = true;
        assert_eq!(
            hover.sample(Some(with_ctrl.clone())),
            vec![Out::Over(with_ctrl)]
        );
    }

    #[test]
    fn moving_to_another_window_leaves_one_and_enters_the_other() {
        let mut hover = running();
        hover.sample(at("main-1", 5.0, 5.0));
        assert_eq!(
            hover.sample(at("main-2", 7.0, 9.0)),
            vec![
                Out::Leave("main-1".into()),
                Out::Enter(at("main-2", 7.0, 9.0).unwrap())
            ]
        );
    }

    #[test]
    fn leaving_every_window_leaves_once_and_coming_back_enters_again() {
        let mut hover = running();
        hover.sample(at("shelf", 1.0, 1.0));
        assert_eq!(hover.sample(None), vec![Out::Leave("shelf".into())]);
        assert!(hover.sample(None).is_empty());
        assert_eq!(
            hover.sample(at("shelf", 2.0, 2.0)),
            vec![Out::Enter(at("shelf", 2.0, 2.0).unwrap())]
        );
    }

    #[test]
    fn nothing_is_sent_when_no_drag_is_running() {
        let mut hover = Hover::default();
        assert!(hover.sample(at("shelf", 1.0, 1.0)).is_empty());
        assert!(hover.end(true, Instant::now()).is_empty());
    }

    #[test]
    fn cancelling_leaves_the_window_at_once() {
        let mut hover = running();
        hover.sample(at("shelf", 1.0, 1.0));
        assert_eq!(
            hover.end(true, Instant::now()),
            vec![Out::Leave("shelf".into())]
        );
        assert!(!hover.running());
    }

    #[test]
    fn releasing_keeps_the_window_entered_for_the_drop_to_come() {
        let mut hover = running();
        hover.sample(at("shelf", 1.0, 1.0));
        let now = Instant::now();
        assert!(hover.end(false, now).is_empty());
        // The platform's late copies are swallowed, its drop is delivered.
        assert!(!hover.deliver(Real::Enter, "shelf", now));
        assert!(!hover.deliver(Real::Over, "shelf", now));
        assert!(!hover.deliver(Real::Leave, "shelf", now));
        assert!(hover.deliver(Real::Drop, "shelf", now));
        // The window has been dealt with, so nothing is left to expire.
        assert!(hover.expire(now + SETTLE).is_empty());
    }

    #[test]
    fn windows_the_cursor_never_reached_are_not_filtered() {
        let mut hover = running();
        hover.sample(at("shelf", 1.0, 1.0));
        let now = Instant::now();
        hover.end(false, now);
        assert!(hover.deliver(Real::Enter, "main-2", now));
        assert!(hover.deliver(Real::Over, "main-2", now));
        assert!(hover.deliver(Real::Leave, "main-2", now));
    }

    #[test]
    fn a_window_left_during_the_drag_still_has_its_late_events_swallowed() {
        let mut hover = running();
        hover.sample(at("main-1", 1.0, 1.0));
        hover.sample(at("main-2", 1.0, 1.0));
        let now = Instant::now();
        hover.end(false, now);
        for kind in [Real::Enter, Real::Over, Real::Leave] {
            assert!(!hover.deliver(kind, "main-1", now));
        }
        assert!(hover.deliver(Real::Drop, "main-2", now));
    }

    #[test]
    fn a_release_over_a_window_that_takes_no_drop_is_given_up_on_after_a_while() {
        let mut hover = running();
        hover.sample(at("ghost", 1.0, 1.0));
        let now = Instant::now();
        hover.end(false, now);
        assert!(hover.expire(now + SETTLE / 2).is_empty());
        assert_eq!(hover.expire(now + SETTLE), vec![Out::Leave("ghost".into())]);
        // Afterwards the window's events are the platform's again.
        assert!(hover.deliver(Real::Enter, "ghost", now + SETTLE));
        assert!(hover.expire(now + SETTLE * 2).is_empty());
    }

    #[test]
    fn events_after_the_wait_are_never_swallowed() {
        let mut hover = running();
        hover.sample(at("shelf", 1.0, 1.0));
        hover.sample(None);
        let now = Instant::now();
        hover.end(false, now);
        assert!(hover.deliver(Real::Enter, "shelf", now + SETTLE));
    }

    #[test]
    fn a_new_drag_forgets_the_last() {
        let mut hover = running();
        hover.sample(at("shelf", 1.0, 1.0));
        hover.end(false, Instant::now());
        hover.begin();
        assert!(hover.deliver(Real::Enter, "shelf", Instant::now()));
        assert_eq!(
            hover.sample(at("shelf", 1.0, 1.0)),
            vec![Out::Enter(at("shelf", 1.0, 1.0).unwrap())]
        );
    }
}
