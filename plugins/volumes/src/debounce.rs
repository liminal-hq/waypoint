// Decides when a burst of change notifications has settled, over a clock the caller supplies
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

/// A trailing-edge debounce with a ceiling. A burst of events fires once, `window_ms` after the last of them, but never later than `max_wait_ms` after the first, so a steady trickle (a busy bus) cannot hold the list back for ever. Times are milliseconds on any monotonic clock.
#[derive(Debug, Clone)]
pub struct Debounce {
    window_ms: u64,
    max_wait_ms: u64,
    first: Option<u64>,
    last: u64,
}

impl Debounce {
    pub fn new(window_ms: u64, max_wait_ms: u64) -> Self {
        Debounce {
            window_ms,
            max_wait_ms: max_wait_ms.max(window_ms),
            first: None,
            last: 0,
        }
    }

    /// Records an event at `now`.
    pub fn event(&mut self, now: u64) {
        self.first.get_or_insert(now);
        self.last = now;
    }

    /// How long until the burst fires, or `None` when no event is waiting. Zero means it is due.
    pub fn remaining(&self, now: u64) -> Option<u64> {
        let first = self.first?;
        let due = (self.last + self.window_ms).min(first + self.max_wait_ms);
        Some(due.saturating_sub(now))
    }

    /// True when a burst is waiting and has settled (or has waited long enough).
    pub fn due(&self, now: u64) -> bool {
        self.remaining(now) == Some(0)
    }

    /// Starts over, after the burst has been handled.
    pub fn reset(&mut self) {
        self.first = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_waiting_is_never_due() {
        let debounce = Debounce::new(100, 1000);
        assert_eq!(debounce.remaining(5), None);
        assert!(!debounce.due(5_000));
    }

    #[test]
    fn a_single_event_is_due_after_the_window() {
        let mut debounce = Debounce::new(100, 1000);
        debounce.event(1000);
        assert_eq!(debounce.remaining(1000), Some(100));
        assert!(!debounce.due(1099));
        assert!(debounce.due(1100));
    }

    #[test]
    fn each_event_in_a_burst_pushes_the_deadline_out() {
        let mut debounce = Debounce::new(100, 1000);
        debounce.event(0);
        debounce.event(60);
        debounce.event(150);
        assert!(!debounce.due(249));
        assert!(debounce.due(250));
    }

    #[test]
    fn a_steady_trickle_fires_at_the_ceiling() {
        let mut debounce = Debounce::new(100, 500);
        for now in (0..=600).step_by(50) {
            debounce.event(now);
        }
        // Events kept coming every 50 ms, but the first was 500 ms ago at t = 500.
        assert!(debounce.due(600));
        assert_eq!(debounce.remaining(500), Some(0));
        assert_eq!(debounce.remaining(450), Some(50));
    }

    #[test]
    fn resetting_starts_a_new_burst() {
        let mut debounce = Debounce::new(100, 500);
        debounce.event(0);
        debounce.reset();
        assert_eq!(debounce.remaining(10_000), None);
        debounce.event(10_000);
        assert_eq!(debounce.remaining(10_000), Some(100));
    }

    #[test]
    fn the_ceiling_is_never_below_the_window() {
        let mut debounce = Debounce::new(100, 10);
        debounce.event(0);
        assert_eq!(debounce.remaining(0), Some(100));
    }
}
