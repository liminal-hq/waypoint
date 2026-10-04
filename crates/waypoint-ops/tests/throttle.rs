// The speed limit of a copy (D157): held within a few percent on a large synthetic copy over the
// in-memory provider (whose I/O is as fast as memory), changed while the copy runs, shared between
// jobs, and never changing what is copied or how it verifies. Time is virtual: the pacer's sleeps
// move a clock and take none.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod common;
mod xfer;

use std::sync::Arc;
use std::time::Duration;

use waypoint_ops::testing::pacer::VirtualPacer;
use xfer::*;

const MIB: u64 = 1024 * 1024;

fn memory_harness() -> (Harness<MemoryProvider>, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let root = FilePath::from_path(dir.path()).unwrap();
    let base = VfsPath::File(root.clone());
    (
        Harness::new(MemoryProvider::new(root, CaseRule::Sensitive), base),
        dir,
    )
}

struct Setup {
    pacer: Arc<VirtualPacer>,
    global: Arc<RateCell>,
    job: Arc<RateCell>,
}

impl Setup {
    fn new(global: Option<u64>, job: Option<u64>) -> Self {
        Self {
            pacer: Arc::new(VirtualPacer::default()),
            global: Arc::new(RateCell::new(global)),
            job: Arc::new(RateCell::new(job)),
        }
    }

    fn throttle(&self) -> Throttle {
        let bucket = |rate: &Arc<RateCell>| Arc::new(Bucket::new(rate.clone(), self.pacer.now()));
        Throttle::new(
            self.pacer.clone(),
            vec![bucket(&self.global), bucket(&self.job)],
        )
    }
}

/// Counts what the executor reports.
struct Watch<F: FnMut(&Progress)> {
    on_progress: F,
}

impl<F: FnMut(&Progress)> ExecSink for Watch<F> {
    fn progress(&mut self, progress: &Progress, _: &Counts) {
        (self.on_progress)(progress);
    }
}

fn copy<F: FnMut(&Progress)>(
    h: &Harness<MemoryProvider>,
    options: RunOptions,
    cancel: &CancelToken,
    on_progress: F,
) -> Result<ExecReport, Box<ExecFailure>> {
    let planned = h
        .plan(&req(h, JobKind::Copy, &["src/big.bin"], "dst", None))
        .unwrap();
    Executor::new(h.env.clone()).run_with(
        JobId(1),
        &planned,
        cancel,
        &mut Watch { on_progress },
        options,
    )
}

fn big(h: &Harness<MemoryProvider>, mib: u64) -> Vec<u8> {
    build(h, &tree(&[("src/", ""), ("dst/", "")]));
    let bytes = pattern((mib * MIB) as usize, 5);
    put_bytes(h, "src/big.bin", &bytes);
    bytes
}

fn options(setup: &Setup) -> RunOptions {
    RunOptions {
        throttle: Some(setup.throttle()),
        ..RunOptions::default()
    }
}

#[test]
fn a_large_copy_is_held_to_the_limit_within_ten_percent() {
    let (h, _dir) = memory_harness();
    let bytes = big(&h, 128);
    let setup = Setup::new(None, Some(16 * MIB));
    copy(&h, options(&setup), &CancelToken::new(), |_| {}).unwrap();
    let seconds = setup.pacer.now().as_secs_f64();
    let speed = bytes.len() as f64 / seconds;
    let limit = (16 * MIB) as f64;
    assert!((speed - limit).abs() < limit * 0.1, "{speed} vs {limit}");
    // The limit paced the copy and changed nothing in it.
    assert_eq!(read_bytes(&h, "dst/big.bin"), bytes);
    assert!(leftovers(&work_tree(&h)).is_empty());
}

#[test]
fn the_global_limit_holds_when_it_is_the_tighter_one() {
    let (h, _dir) = memory_harness();
    let bytes = big(&h, 64);
    let setup = Setup::new(Some(8 * MIB), Some(64 * MIB));
    copy(&h, options(&setup), &CancelToken::new(), |_| {}).unwrap();
    let speed = bytes.len() as f64 / setup.pacer.now().as_secs_f64();
    let limit = (8 * MIB) as f64;
    assert!((speed - limit).abs() < limit * 0.1, "{speed} vs {limit}");
}

#[test]
fn an_unlimited_copy_never_waits() {
    let (h, _dir) = memory_harness();
    let bytes = big(&h, 16);
    let setup = Setup::new(None, None);
    copy(&h, options(&setup), &CancelToken::new(), |_| {}).unwrap();
    assert_eq!(setup.pacer.now(), Duration::ZERO);
    assert_eq!(read_bytes(&h, "dst/big.bin"), bytes);
}

#[test]
fn raising_the_limit_on_a_running_copy_speeds_up_the_rest() {
    let (h, _dir) = memory_harness();
    big(&h, 64);
    let setup = Setup::new(None, Some(4 * MIB));
    let job = setup.job.clone();
    let pacer = setup.pacer.clone();
    let mut at_half = None;
    copy(&h, options(&setup), &CancelToken::new(), |p| {
        if at_half.is_none() && p.bytes_done >= 16 * MIB {
            at_half = Some(pacer.now());
            job.set(Some(32 * MIB));
        }
    })
    .unwrap();
    let first = at_half.unwrap().as_secs_f64();
    let rest = setup.pacer.now().as_secs_f64() - first;
    // 16 MiB at 4 MiB/s, then 48 MiB at 32 MiB/s.
    assert!((first - 4.0).abs() < 0.4, "{first}");
    assert!((rest - 1.5).abs() < 0.3, "{rest}");
}

#[test]
fn lowering_the_global_limit_slows_a_running_copy() {
    let (h, _dir) = memory_harness();
    big(&h, 64);
    let setup = Setup::new(Some(32 * MIB), None);
    let global = setup.global.clone();
    let pacer = setup.pacer.clone();
    let mut at = None;
    copy(&h, options(&setup), &CancelToken::new(), |p| {
        if at.is_none() && p.bytes_done >= 32 * MIB {
            at = Some(pacer.now());
            global.set(Some(8 * MIB));
        }
    })
    .unwrap();
    let rest = setup.pacer.now().as_secs_f64() - at.unwrap().as_secs_f64();
    // The other 32 MiB at 8 MiB/s.
    assert!((rest - 4.0).abs() < 0.4, "{rest}");
}

#[test]
fn removing_the_limit_mid_copy_lets_it_run_free() {
    let (h, _dir) = memory_harness();
    big(&h, 64);
    let setup = Setup::new(None, Some(MIB));
    let job = setup.job.clone();
    copy(&h, options(&setup), &CancelToken::new(), |p| {
        if p.bytes_done >= 2 * MIB {
            job.set(None);
        }
    })
    .unwrap();
    // About two seconds for the first 2 MiB and nothing for the rest.
    assert!(
        setup.pacer.now() < Duration::from_secs(3),
        "{:?}",
        setup.pacer.now()
    );
}

#[test]
fn a_limit_does_not_change_what_verification_records() {
    let (h, _dir) = memory_harness();
    let bytes = big(&h, 8);
    let verified = |limit: Option<u64>| {
        let _ = h.provider.remove_file(&h.path("dst/big.bin"));
        let setup = Setup::new(None, limit);
        let mut o = options(&setup);
        o.verify = Some(VerifyAlgorithm::Blake3);
        let report = copy(&h, o, &CancelToken::new(), |_| {}).unwrap();
        assert_eq!(read_bytes(&h, "dst/big.bin"), bytes);
        report.transfer.verified
    };
    let free = verified(None);
    let slow = verified(Some(2 * MIB));
    assert!(free.is_some());
    assert_eq!(free, slow);
}

#[test]
fn cancelling_a_throttled_copy_stops_it_and_leaves_nothing() {
    let (h, _dir) = memory_harness();
    big(&h, 16);
    let setup = Setup::new(None, Some(MIB));
    let cancel = CancelToken::new();
    let stop = cancel.clone();
    setup.pacer.on_sleep(Box::new(move |now| {
        if now > Duration::from_secs(2) {
            stop.cancel();
        }
    }));
    let failure = copy(&h, options(&setup), &cancel, |_| {}).unwrap_err();
    assert_eq!(failure.error, OpsError::Cancelled);
    assert!(setup.pacer.now() < Duration::from_secs(4));
    assert!(!work_tree(&h).contains_key("dst/big.bin"));
    assert!(leftovers(&work_tree(&h)).is_empty());
}

#[test]
fn a_real_clock_copy_is_paced_too() {
    let (h, _dir) = memory_harness();
    let bytes = big(&h, 6);
    let rate = Arc::new(RateCell::new(Some(24 * MIB)));
    let throttle = Throttle::new(
        Arc::new(SystemPacer::new()),
        vec![Arc::new(Bucket::new(rate, Duration::ZERO))],
    );
    let started = std::time::Instant::now();
    let options = RunOptions {
        throttle: Some(throttle),
        ..RunOptions::default()
    };
    copy(&h, options, &CancelToken::new(), |_| {}).unwrap();
    // 6 MiB at 24 MiB/s is a quarter of a second; a slow machine may take longer, never less.
    let took = started.elapsed().as_secs_f64();
    assert!(took >= 0.22, "{took}");
    assert_eq!(read_bytes(&h, "dst/big.bin"), bytes);
}
