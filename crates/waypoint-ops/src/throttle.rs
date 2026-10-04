// The speed limit of a copy or move (D157): a token bucket for each limit in force (the whole queue's
// and the job's own), charged in the copy loop before each piece is written. It paces the loop and
// nothing else: the bytes, their order, the hashing and the read-back are the same with or without
// a limit. Time comes from an injected `Pacer`, so the tests run on a virtual clock.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// A bucket holds `tokens` (bytes it may still pass without waiting) and refills at the limit's rate
// up to a small burst. Charging subtracts the piece, and a bucket that has gone into debt says how
// long to wait for the debt to be repaid. The wait is taken in slices that re-read the rate, so a
// limit that is raised, lowered or removed takes effect within one slice, on a job already running.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use waypoint_vfs::CancelToken;

/// How long a wait is slept before the limits are read again: the delay before a changed limit
/// reaches a running job.
pub const WAIT_SLICE: Duration = Duration::from_millis(50);

/// How much a bucket may save while idle, as a fraction of a second of its rate.
const BURST_SECONDS: f64 = 0.1;

/// The smallest piece a limited copy moves at a time, in bytes.
const MIN_PIECE: usize = 16 * 1024;

/// How many pieces a second of a limited copy is cut into.
const PIECES_PER_SECOND: u64 = 20;

/// The clock and the sleep the throttle uses.
pub trait Pacer: Send + Sync {
    /// Time since an arbitrary fixed start; never goes back.
    fn now(&self) -> Duration;
    fn sleep(&self, duration: Duration);
}

/// The real clock and `thread::sleep`.
pub struct SystemPacer {
    start: Instant,
}

impl SystemPacer {
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
        }
    }
}

impl Default for SystemPacer {
    fn default() -> Self {
        Self::new()
    }
}

impl Pacer for SystemPacer {
    fn now(&self) -> Duration {
        self.start.elapsed()
    }

    fn sleep(&self, duration: Duration) {
        std::thread::sleep(duration);
    }
}

/// A limit in bytes per second that can be changed while a copy reads it. Zero means no limit.
#[derive(Debug, Default)]
pub struct RateCell(AtomicU64);

impl RateCell {
    pub fn new(limit: Option<u64>) -> Self {
        Self(AtomicU64::new(limit.unwrap_or(0)))
    }

    pub fn get(&self) -> Option<u64> {
        match self.0.load(Ordering::Relaxed) {
            0 => None,
            rate => Some(rate),
        }
    }

    /// Sets the limit; `None` and zero both mean unlimited.
    pub fn set(&self, limit: Option<u64>) {
        self.0.store(limit.unwrap_or(0), Ordering::Relaxed);
    }
}

#[derive(Debug)]
struct Level {
    /// Bytes that may pass at once; negative when the bucket owes time.
    tokens: f64,
    last: Duration,
}

/// One limit's bucket. Jobs that share a limit share its bucket, so the limit holds for their sum.
#[derive(Debug)]
pub struct Bucket {
    rate: Arc<RateCell>,
    level: Mutex<Level>,
}

impl Bucket {
    pub fn new(rate: Arc<RateCell>, now: Duration) -> Self {
        Self {
            rate,
            level: Mutex::new(Level {
                tokens: 0.0,
                last: now,
            }),
        }
    }

    pub fn rate(&self) -> Option<u64> {
        self.rate.get()
    }

    fn refill(&self, level: &mut Level, now: Duration) {
        let elapsed = now.saturating_sub(level.last).as_secs_f64();
        level.last = level.last.max(now);
        level.tokens = match self.rate.get() {
            // Nothing is saved while there is no limit, so a limit set later starts from empty.
            None => 0.0,
            Some(rate) => {
                let rate = rate as f64;
                (level.tokens + elapsed * rate).min(rate * BURST_SECONDS)
            }
        };
    }

    fn charge(&self, now: Duration, bytes: usize) {
        let mut level = self.level.lock().unwrap_or_else(|e| e.into_inner());
        self.refill(&mut level, now);
        if self.rate.get().is_some() {
            level.tokens -= bytes as f64;
        }
    }

    /// How long until the bucket is out of debt at the rate in force now.
    fn debt(&self, now: Duration) -> Duration {
        let mut level = self.level.lock().unwrap_or_else(|e| e.into_inner());
        self.refill(&mut level, now);
        match self.rate.get() {
            Some(rate) if level.tokens < 0.0 => {
                Duration::from_secs_f64(-level.tokens / rate as f64)
            }
            _ => Duration::ZERO,
        }
    }
}

/// The limits one running copy obeys.
#[derive(Clone)]
pub struct Throttle {
    pacer: Arc<dyn Pacer>,
    buckets: Vec<Arc<Bucket>>,
}

impl Throttle {
    /// A throttle over `buckets` (the queue's and the job's), each of which may be unlimited.
    pub fn new(pacer: Arc<dyn Pacer>, buckets: Vec<Arc<Bucket>>) -> Self {
        Self { pacer, buckets }
    }

    /// The tightest limit in force, in bytes per second.
    pub fn limit(&self) -> Option<u64> {
        self.buckets.iter().filter_map(|b| b.rate()).min()
    }

    /// Whether any limit is in force now.
    pub fn is_limited(&self) -> bool {
        self.limit().is_some()
    }

    /// The size of the next piece to move, at most `max`: the whole of it when nothing limits the
    /// copy, and otherwise a fraction of a second at the tightest limit, so the pace is even and a
    /// change of limit does not wait on a big chunk.
    pub fn piece(&self, max: usize) -> usize {
        match self.limit() {
            None => max,
            Some(rate) => {
                let piece = (rate / PIECES_PER_SECOND).max(MIN_PIECE as u64);
                max.min(usize::try_from(piece).unwrap_or(usize::MAX)).max(1)
            }
        }
    }

    /// Takes `bytes` out of every bucket and waits until none of them owes time, giving up with
    /// `Cancelled` if the job is cancelled meanwhile.
    pub fn pace(
        &self,
        bytes: usize,
        cancel: &CancelToken,
    ) -> Result<(), waypoint_protocol::VfsError> {
        let now = self.pacer.now();
        for bucket in &self.buckets {
            bucket.charge(now, bytes);
        }
        loop {
            let wait = self
                .buckets
                .iter()
                .map(|b| b.debt(self.pacer.now()))
                .max()
                .unwrap_or(Duration::ZERO);
            if wait.is_zero() {
                return Ok(());
            }
            if cancel.is_cancelled() {
                return Err(waypoint_protocol::VfsError::Cancelled);
            }
            self.pacer.sleep(wait.min(WAIT_SLICE));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::pacer::VirtualPacer;

    fn throttle(pacer: &Arc<VirtualPacer>, limits: &[&Arc<RateCell>]) -> Throttle {
        let buckets = limits
            .iter()
            .map(|r| Arc::new(Bucket::new((*r).clone(), pacer.now())))
            .collect();
        Throttle::new(pacer.clone(), buckets)
    }

    #[test]
    fn no_limit_never_waits_and_keeps_the_whole_chunk() {
        let pacer = Arc::new(VirtualPacer::default());
        let rate = Arc::new(RateCell::new(None));
        let t = throttle(&pacer, &[&rate]);
        t.pace(1 << 30, &CancelToken::new()).unwrap();
        assert_eq!(pacer.now(), Duration::ZERO);
        assert_eq!(t.piece(8 << 20), 8 << 20);
        assert!(!t.is_limited());
    }

    #[test]
    fn a_limit_holds_the_rate_within_a_few_percent() {
        let pacer = Arc::new(VirtualPacer::default());
        let rate = Arc::new(RateCell::new(Some(10_000_000)));
        let t = throttle(&pacer, &[&rate]);
        let cancel = CancelToken::new();
        let mut moved = 0usize;
        while moved < 500_000_000 {
            let piece = t.piece(8 << 20);
            t.pace(piece, &cancel).unwrap();
            moved += piece;
        }
        let seconds = pacer.now().as_secs_f64();
        let speed = moved as f64 / seconds;
        assert!((speed - 1e7).abs() < 1e7 * 0.02, "{speed}");
    }

    #[test]
    fn the_tightest_of_two_limits_wins() {
        let pacer = Arc::new(VirtualPacer::default());
        let global = Arc::new(RateCell::new(Some(50_000_000)));
        let job = Arc::new(RateCell::new(Some(5_000_000)));
        let t = throttle(&pacer, &[&global, &job]);
        assert_eq!(t.limit(), Some(5_000_000));
        let cancel = CancelToken::new();
        let mut moved = 0usize;
        while moved < 50_000_000 {
            let piece = t.piece(8 << 20);
            t.pace(piece, &cancel).unwrap();
            moved += piece;
        }
        let speed = moved as f64 / pacer.now().as_secs_f64();
        assert!((speed - 5e6).abs() < 5e6 * 0.02, "{speed}");
    }

    #[test]
    fn two_jobs_share_one_global_bucket() {
        let pacer = Arc::new(VirtualPacer::default());
        let global = Arc::new(Bucket::new(
            Arc::new(RateCell::new(Some(10_000_000))),
            pacer.now(),
        ));
        let own =
            |pacer: &Arc<VirtualPacer>| Bucket::new(Arc::new(RateCell::new(None)), pacer.now());
        let a = Throttle::new(pacer.clone(), vec![global.clone(), Arc::new(own(&pacer))]);
        let b = Throttle::new(pacer.clone(), vec![global, Arc::new(own(&pacer))]);
        let cancel = CancelToken::new();
        let mut moved = 0usize;
        while moved < 100_000_000 {
            for t in [&a, &b] {
                let piece = t.piece(1 << 20);
                t.pace(piece, &cancel).unwrap();
                moved += piece;
            }
        }
        let speed = moved as f64 / pacer.now().as_secs_f64();
        assert!((speed - 1e7).abs() < 1e7 * 0.03, "{speed}");
    }

    #[test]
    fn a_limit_changed_while_waiting_takes_effect_at_once() {
        let pacer = Arc::new(VirtualPacer::default());
        let rate = Arc::new(RateCell::new(Some(1_000_000)));
        let t = throttle(&pacer, &[&rate]);
        let cancel = CancelToken::new();
        // A wait of ten seconds at 1 MB/s; the limit is lifted after a quarter of a second.
        let lift = rate.clone();
        pacer.on_sleep(Box::new(move |now| {
            if now >= Duration::from_millis(250) {
                lift.set(None);
            }
        }));
        t.pace(10_000_000, &cancel).unwrap();
        assert!(
            pacer.now() < Duration::from_millis(400),
            "{:?}",
            pacer.now()
        );
    }

    #[test]
    fn raising_a_limit_speeds_up_what_is_left() {
        let pacer = Arc::new(VirtualPacer::default());
        let rate = Arc::new(RateCell::new(Some(1_000_000)));
        let t = throttle(&pacer, &[&rate]);
        let cancel = CancelToken::new();
        let mut moved = 0usize;
        while moved < 4_000_000 {
            let piece = t.piece(1 << 20);
            t.pace(piece, &cancel).unwrap();
            moved += piece;
        }
        let first = pacer.now();
        rate.set(Some(4_000_000));
        let before = moved;
        while moved < before + 4_000_000 {
            let piece = t.piece(1 << 20);
            t.pace(piece, &cancel).unwrap();
            moved += piece;
        }
        let second = pacer.now() - first;
        let ratio = first.as_secs_f64() / second.as_secs_f64();
        assert!((3.6..4.4).contains(&ratio), "{ratio}");
    }

    #[test]
    fn a_cancel_ends_the_wait() {
        let pacer = Arc::new(VirtualPacer::default());
        let rate = Arc::new(RateCell::new(Some(1_000)));
        let t = throttle(&pacer, &[&rate]);
        let cancel = CancelToken::new();
        let stop = cancel.clone();
        pacer.on_sleep(Box::new(move |_| stop.cancel()));
        let result = t.pace(1_000_000, &cancel);
        assert!(matches!(
            result,
            Err(waypoint_protocol::VfsError::Cancelled)
        ));
    }

    #[test]
    fn pieces_are_a_twentieth_of_a_second_and_never_empty() {
        let pacer = Arc::new(VirtualPacer::default());
        let rate = Arc::new(RateCell::new(Some(20_000_000)));
        let t = throttle(&pacer, &[&rate]);
        assert_eq!(t.piece(8 << 20), 1_000_000);
        rate.set(Some(100));
        assert_eq!(t.piece(8 << 20), MIN_PIECE);
        assert_eq!(t.piece(4), 4);
    }
}
