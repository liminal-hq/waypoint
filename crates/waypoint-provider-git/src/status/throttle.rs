// When a burst of file changes becomes one status run.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::time::{Duration, Instant};

/// How events are gathered and how often a status may run. Pure, so the rules are tested without a
/// clock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Throttle {
    /// A run starts once no event has come for this long: a build or a checkout that touches
    /// thousands of files is one run, not thousands.
    pub debounce: Duration,
    /// But never wait longer than this after the first event, so steady changes still show up.
    pub max_wait: Duration,
    /// The least gap between the end of a run and the start of the next.
    pub min_interval: Duration,
    /// A run that took `d` also keeps the next one at least `slowdown * d` away, so a repository
    /// where status takes seconds is not recomputed back to back while files keep changing.
    pub slowdown: u32,
}

impl Default for Throttle {
    fn default() -> Self {
        Self {
            // Status "within a second of a change": 100 ms to gather, then the run itself.
            debounce: Duration::from_millis(100),
            max_wait: Duration::from_millis(500),
            min_interval: Duration::from_millis(150),
            slowdown: 2,
        }
    }
}

impl Throttle {
    /// When the run for events first seen at `first` and last seen at `last` may start, given when
    /// the previous run ended and how long it took.
    pub fn start_at(
        &self,
        first: Instant,
        last: Instant,
        previous: Option<(Instant, Duration)>,
    ) -> Instant {
        let gathered = (last + self.debounce).min(first + self.max_wait);
        match previous {
            Some((ended, took)) => {
                gathered.max(ended + self.min_interval.max(took * self.slowdown))
            }
            None => gathered,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn a_burst_waits_for_quiet() {
        let t = Throttle::default();
        let now = Instant::now();
        assert_eq!(t.start_at(now, now, None), now + ms(100));
        assert_eq!(t.start_at(now, now + ms(60), None), now + ms(160));
    }

    #[test]
    fn steady_changes_cannot_postpone_a_run_for_ever() {
        let t = Throttle::default();
        let now = Instant::now();
        assert_eq!(t.start_at(now, now + ms(5_000), None), now + ms(500));
    }

    #[test]
    fn a_run_is_kept_away_from_the_last_one_in_proportion_to_its_cost() {
        let t = Throttle::default();
        let now = Instant::now();
        // The last run ended just now and took 1 s: the next waits 2 s.
        assert_eq!(
            t.start_at(now, now, Some((now, ms(1_000)))),
            now + ms(2_000)
        );
        // A quick run only keeps the minimum gap.
        assert_eq!(t.start_at(now, now, Some((now, ms(10)))), now + ms(150));
        // Time already passed counts.
        let later = now + ms(10_000);
        assert_eq!(
            t.start_at(later, later, Some((now, ms(1_000)))),
            later + ms(100)
        );
    }
}
