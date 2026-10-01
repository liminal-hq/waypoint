// The follow loop's decisions as a pure state machine, and the guard that stops two drags starting at once
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

use crate::models::Point;

/// How often the ghost is moved.
pub const TICK: Duration = Duration::from_millis(16);
/// How long a drag may run before the plugin ends it.
pub const TIMEOUT: Duration = Duration::from_secs(30);
/// How long the cursor value may stay identical while a button is held before it is reported stale.
pub const STALE_AFTER: Duration = Duration::from_millis(1000);

/// Lets one drag run at a time.
#[derive(Debug, Default)]
pub struct StartGuard(AtomicBool);

impl StartGuard {
    /// True if the caller now owns the drag; false if one is already running.
    pub fn try_start(&self) -> bool {
        self.0
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    pub fn finish(&self) {
        self.0.store(false, Ordering::Release);
    }

    pub fn is_active(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

/// What the loop should do after one tick.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Action {
    /// Move the ghost to this cursor position.
    Move(Point),
    /// Leave the ghost where it is.
    Idle,
    /// The cursor stopped changing while a button was held: stop moving the ghost and report a stale cursor.
    Stale,
    /// The cursor changed again after a stale report: report it fresh and move the ghost.
    Recovered(Point),
    /// The drag ran too long: end it.
    Timeout,
}

/// Decides, tick by tick, whether the ghost follows the cursor.
///
/// The cursor source is untrustworthy: on Wayland it is a constant `(0, 0)`, and under XWayland it freezes at the last value once the pointer leaves the app's windows unless a button is held. A button held with a value that never changes is therefore treated as stale, whatever the platform said. The clock, the cursor and the button state are passed in so the machine needs no thread to test.
#[derive(Debug, Clone)]
pub struct FollowMachine {
    timeout: Duration,
    stale_after: Duration,
    last: Option<Point>,
    unchanged_since: Duration,
    stale: bool,
}

impl FollowMachine {
    pub fn new(timeout: Duration, stale_after: Duration) -> Self {
        Self {
            timeout,
            stale_after,
            last: None,
            unchanged_since: Duration::ZERO,
            stale: false,
        }
    }

    pub fn is_stale(&self) -> bool {
        self.stale
    }

    /// The last cursor the machine saw.
    pub fn last_cursor(&self) -> Option<Point> {
        self.last
    }

    /// `now` is the time since the drag began; `cursor` is `None` when the system gave no value.
    pub fn tick(&mut self, now: Duration, cursor: Option<Point>, button_held: bool) -> Action {
        if now >= self.timeout {
            return Action::Timeout;
        }
        let Some(cursor) = cursor else {
            return Action::Idle;
        };
        if self.last != Some(cursor) {
            let recovered = self.stale;
            self.last = Some(cursor);
            self.unchanged_since = now;
            self.stale = false;
            return if recovered {
                Action::Recovered(cursor)
            } else {
                Action::Move(cursor)
            };
        }
        if button_held
            && !self.stale
            && now.saturating_sub(self.unchanged_since) >= self.stale_after
        {
            self.stale = true;
            return Action::Stale;
        }
        Action::Idle
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(x: f64, y: f64) -> Point {
        Point { x, y }
    }

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    fn machine() -> FollowMachine {
        FollowMachine::new(Duration::from_secs(30), ms(1000))
    }

    #[test]
    fn the_guard_admits_one_drag_at_a_time() {
        let guard = StartGuard::default();
        assert!(!guard.is_active());
        assert!(guard.try_start());
        assert!(!guard.try_start());
        assert!(guard.is_active());
        guard.finish();
        assert!(guard.try_start());
    }

    #[test]
    fn the_guard_admits_exactly_one_of_many_racing_threads() {
        let guard = std::sync::Arc::new(StartGuard::default());
        let winners: usize = (0..16)
            .map(|_| {
                let guard = guard.clone();
                std::thread::spawn(move || guard.try_start())
            })
            .collect::<Vec<_>>()
            .into_iter()
            .map(|handle| usize::from(handle.join().unwrap()))
            .sum();
        assert_eq!(winners, 1);
    }

    #[test]
    fn a_changing_cursor_moves_the_ghost() {
        let mut m = machine();
        assert_eq!(
            m.tick(ms(0), Some(at(1.0, 1.0)), true),
            Action::Move(at(1.0, 1.0))
        );
        assert_eq!(
            m.tick(ms(16), Some(at(2.0, 1.0)), true),
            Action::Move(at(2.0, 1.0))
        );
        assert_eq!(m.last_cursor(), Some(at(2.0, 1.0)));
    }

    #[test]
    fn an_unchanged_cursor_is_idle_until_it_is_stale() {
        let mut m = machine();
        m.tick(ms(0), Some(at(5.0, 5.0)), true);
        assert_eq!(m.tick(ms(500), Some(at(5.0, 5.0)), true), Action::Idle);
        assert_eq!(m.tick(ms(999), Some(at(5.0, 5.0)), true), Action::Idle);
        assert_eq!(m.tick(ms(1000), Some(at(5.0, 5.0)), true), Action::Stale);
        assert!(m.is_stale());
        // Reported once, then quiet.
        assert_eq!(m.tick(ms(1016), Some(at(5.0, 5.0)), true), Action::Idle);
    }

    #[test]
    fn a_wayland_style_constant_origin_goes_stale() {
        let mut m = machine();
        let mut stale_at = None;
        for tick in 0..100u64 {
            if m.tick(ms(tick * 16), Some(at(0.0, 0.0)), true) == Action::Stale {
                stale_at = Some(tick * 16);
                break;
            }
        }
        assert_eq!(stale_at, Some(1008));
    }

    #[test]
    fn staleness_is_measured_from_the_last_change() {
        let mut m = machine();
        m.tick(ms(0), Some(at(1.0, 1.0)), true);
        m.tick(ms(900), Some(at(2.0, 2.0)), true);
        assert_eq!(m.tick(ms(1800), Some(at(2.0, 2.0)), true), Action::Idle);
        assert_eq!(m.tick(ms(1900), Some(at(2.0, 2.0)), true), Action::Stale);
    }

    #[test]
    fn a_change_after_a_stale_report_recovers() {
        let mut m = machine();
        m.tick(ms(0), Some(at(5.0, 5.0)), true);
        assert_eq!(m.tick(ms(1000), Some(at(5.0, 5.0)), true), Action::Stale);
        assert_eq!(
            m.tick(ms(1016), Some(at(6.0, 5.0)), true),
            Action::Recovered(at(6.0, 5.0))
        );
        assert!(!m.is_stale());
        assert_eq!(
            m.tick(ms(1032), Some(at(7.0, 5.0)), true),
            Action::Move(at(7.0, 5.0))
        );
    }

    #[test]
    fn no_button_means_an_unchanged_cursor_is_not_stale() {
        let mut m = machine();
        m.tick(ms(0), Some(at(5.0, 5.0)), false);
        assert_eq!(m.tick(ms(5000), Some(at(5.0, 5.0)), false), Action::Idle);
        assert!(!m.is_stale());
    }

    #[test]
    fn a_missing_cursor_is_idle() {
        let mut m = machine();
        assert_eq!(m.tick(ms(0), None, true), Action::Idle);
        assert_eq!(m.tick(ms(5000), None, true), Action::Idle);
        assert_eq!(m.last_cursor(), None);
    }

    #[test]
    fn the_drag_times_out() {
        let mut m = machine();
        assert_eq!(
            m.tick(ms(29_984), Some(at(1.0, 1.0)), true),
            Action::Move(at(1.0, 1.0))
        );
        assert_eq!(
            m.tick(ms(30_000), Some(at(2.0, 2.0)), true),
            Action::Timeout
        );
        assert_eq!(m.tick(ms(30_016), None, false), Action::Timeout);
    }

    #[test]
    fn the_defaults_are_a_sixteenth_of_a_second_and_thirty_seconds() {
        assert_eq!(TICK, ms(16));
        assert_eq!(TIMEOUT, Duration::from_secs(30));
    }
}
