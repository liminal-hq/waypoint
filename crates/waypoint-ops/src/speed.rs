// Transfer speed and the time left, as an exponentially weighted moving average over the byte
// counts a job reports. Pure: it is fed timestamps (from the injected `Clock`) and byte counts and
// holds no clock of its own.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

/// How often a new sample is taken at the most, in milliseconds. Reports that come faster only
/// accumulate bytes, so a burst of tiny chunks does not make the speed jitter.
const MIN_SAMPLE_MS: i64 = 250;

/// The time constant of the average, in milliseconds: a change in speed is mostly reflected after
/// about this long.
const TAU_MS: f64 = 3_000.0;

/// Estimates how fast a job moves bytes and when it will be done.
#[derive(Debug, Clone, PartialEq)]
pub struct SpeedEstimator {
    last_ms: Option<i64>,
    last_bytes: u64,
    /// Bytes per second; `None` before the first sample.
    average: Option<f64>,
}

impl Default for SpeedEstimator {
    fn default() -> Self {
        Self::new()
    }
}

impl SpeedEstimator {
    pub const fn new() -> Self {
        Self {
            last_ms: None,
            last_bytes: 0,
            average: None,
        }
    }

    /// Records that `bytes_done` bytes were done at `now_ms` and returns the speed in bytes per
    /// second (zero until a first sample exists). A count that went down (a copy was rolled back)
    /// starts a new baseline and leaves the average alone; a clock that went backwards is ignored.
    pub fn update(&mut self, now_ms: i64, bytes_done: u64) -> u64 {
        match self.last_ms {
            None => {
                self.last_ms = Some(now_ms);
                self.last_bytes = bytes_done;
            }
            Some(last) => {
                let elapsed = now_ms - last;
                if bytes_done < self.last_bytes {
                    self.last_ms = Some(now_ms);
                    self.last_bytes = bytes_done;
                } else if elapsed >= MIN_SAMPLE_MS {
                    let instant = (bytes_done - self.last_bytes) as f64 * 1000.0 / elapsed as f64;
                    let weight = 1.0 - (-(elapsed as f64) / TAU_MS).exp();
                    self.average = Some(match self.average {
                        None => instant,
                        Some(average) => average + weight * (instant - average),
                    });
                    self.last_ms = Some(now_ms);
                    self.last_bytes = bytes_done;
                }
            }
        }
        self.speed()
    }

    /// The current speed in bytes per second.
    pub fn speed(&self) -> u64 {
        self.average.map_or(0, |a| a.max(0.0).round() as u64)
    }

    /// Milliseconds until `bytes_total` is reached at the current speed; `None` while the speed is
    /// unknown or zero, and zero when there is nothing left.
    pub fn eta_ms(&self, bytes_done: u64, bytes_total: u64) -> Option<u64> {
        let remaining = bytes_total.saturating_sub(bytes_done);
        if remaining == 0 {
            return Some(0);
        }
        let speed = self.average.filter(|a| *a >= 1.0)?;
        Some((remaining as f64 * 1000.0 / speed).ceil() as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn there_is_no_speed_before_a_second_sample() {
        let mut e = SpeedEstimator::new();
        assert_eq!(e.update(0, 0), 0);
        assert_eq!(e.update(100, 1_000), 0, "too soon for a sample");
        assert_eq!(e.eta_ms(0, 100), None);
    }

    #[test]
    fn a_steady_rate_is_reported_exactly() {
        let mut e = SpeedEstimator::new();
        e.update(0, 0);
        for step in 1..=20 {
            let speed = e.update(step * 500, step as u64 * 5_000_000);
            assert_eq!(speed, 10_000_000);
        }
        // 100 MB done of 200 MB at 10 MB/s: ten seconds.
        assert_eq!(e.eta_ms(100_000_000, 200_000_000), Some(10_000));
    }

    #[test]
    fn the_average_follows_a_change_and_never_overshoots() {
        let mut e = SpeedEstimator::new();
        e.update(0, 0);
        let mut bytes = 0u64;
        let mut now = 0i64;
        for _ in 0..10 {
            now += 500;
            bytes += 5_000_000;
            e.update(now, bytes);
        }
        assert_eq!(e.speed(), 10_000_000);
        // The rate falls to a tenth: the speed falls toward it, monotonically, never below it.
        let mut last = e.speed();
        for _ in 0..60 {
            now += 500;
            bytes += 500_000;
            let speed = e.update(now, bytes);
            assert!(speed <= last, "{speed} > {last}");
            assert!(speed >= 1_000_000, "{speed}");
            last = speed;
        }
        assert!(last < 1_100_000, "{last}");
    }

    #[test]
    fn a_stall_drives_the_speed_down_and_the_eta_up() {
        let mut e = SpeedEstimator::new();
        e.update(0, 0);
        e.update(1_000, 10_000_000);
        let before = e.eta_ms(10_000_000, 110_000_000).unwrap();
        e.update(2_000, 10_000_000);
        e.update(3_000, 10_000_000);
        let after = e.eta_ms(10_000_000, 110_000_000).unwrap();
        assert!(after > before, "{after} <= {before}");
    }

    #[test]
    fn nothing_left_means_no_wait_whatever_the_speed() {
        let e = SpeedEstimator::new();
        assert_eq!(e.eta_ms(10, 10), Some(0));
        assert_eq!(e.eta_ms(11, 10), Some(0));
    }

    #[test]
    fn a_count_that_goes_down_or_a_clock_that_goes_back_does_not_poison_the_average() {
        let mut e = SpeedEstimator::new();
        e.update(0, 0);
        e.update(1_000, 8_000_000);
        let speed = e.speed();
        // A rolled-back copy.
        assert_eq!(e.update(1_500, 1_000_000), speed);
        // The clock steps back: no sample, no change.
        assert_eq!(e.update(1_000, 2_000_000), speed);
        // The baseline moved, so the next sample measures from it.
        let next = e.update(2_500, 9_000_000);
        assert!(next > 0);
    }

    #[test]
    fn the_estimator_is_deterministic() {
        let run = || {
            let mut e = SpeedEstimator::new();
            let mut out = Vec::new();
            for step in 0..40i64 {
                out.push(e.update(step * 333, (step as u64) * 1_234_567 % 50_000_000));
            }
            out
        };
        assert_eq!(run(), run());
    }
}
