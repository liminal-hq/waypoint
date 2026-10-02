// Tests the engine with a fake processor: ordering with one worker, sharing, cancelling and a crash
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::sync::mpsc;
use std::time::Duration;

use super::*;
use crate::models::ThumbSize;

fn req(key: &str) -> ThumbRequest {
    ThumbRequest {
        key: key.to_string(),
        path: format!("/data/{key}"),
        size: ThumbSize::Normal,
        mtime_ms: 0,
    }
}

fn reqs(keys: &[&str]) -> Vec<ThumbRequest> {
    keys.iter().map(|k| req(k)).collect()
}

/// Blocks the first job it gets until released, so a test can queue behind it.
struct Gate {
    started: Mutex<mpsc::Sender<String>>,
    release: Mutex<mpsc::Receiver<()>>,
    seen: Mutex<Vec<String>>,
}

impl Processor for Gate {
    fn process(&self, request: &ThumbRequest, cancel: &AtomicBool) -> Outcome {
        self.seen.lock().unwrap().push(request.key.clone());
        let _ = self.started.lock().unwrap().send(request.key.clone());
        if request.key == "gate" {
            // Wait to be released or cancelled.
            let release = self.release.lock().unwrap();
            while release.recv_timeout(Duration::from_millis(5)).is_err() {
                if cancel.load(Ordering::Relaxed) {
                    return Outcome::Cancelled;
                }
            }
        }
        if request.key == "boom" {
            panic!("a generator bug");
        }
        Outcome::Ready {
            url: format!("u/{}", request.key),
        }
    }
}

fn collector() -> (Sink, mpsc::Receiver<ThumbEvent>) {
    let (tx, rx) = mpsc::channel();
    let tx = Mutex::new(tx);
    (
        Arc::new(move |event| {
            let _ = tx.lock().unwrap().send(event);
        }),
        rx,
    )
}

fn gate() -> (Arc<Gate>, mpsc::Receiver<String>, mpsc::Sender<()>) {
    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let gate = Arc::new(Gate {
        started: Mutex::new(started_tx),
        release: Mutex::new(release_rx),
        seen: Mutex::new(Vec::new()),
    });
    (gate, started_rx, release_tx)
}

const WAIT: Duration = Duration::from_secs(5);

#[test]
fn one_worker_serves_the_newest_request_first_and_prioritise_reorders() {
    let (gate, started, release) = gate();
    let engine = Engine::new(1, gate.clone());
    let (sink, events) = collector();
    engine.request(reqs(&["gate"]), Arc::clone(&sink));
    assert_eq!(started.recv_timeout(WAIT).unwrap(), "gate");
    engine.request(reqs(&["a", "b", "c"]), Arc::clone(&sink));
    let t = engine.request(reqs(&["d", "e", "f"]), Arc::clone(&sink));
    engine.prioritise(t, &["f".to_string()]);
    release.send(()).unwrap();
    let mut keys = Vec::new();
    for _ in 0..7 {
        keys.push(events.recv_timeout(WAIT).unwrap().key().to_string());
    }
    assert_eq!(keys, ["gate", "f", "d", "e", "a", "b", "c"]);
}

#[test]
fn a_cancelled_request_gets_nothing_and_its_pending_work_never_runs() {
    let (gate, started, release) = gate();
    let engine = Engine::new(1, gate.clone());
    let (sink, events) = collector();
    engine.request(reqs(&["gate"]), Arc::clone(&sink));
    assert_eq!(started.recv_timeout(WAIT).unwrap(), "gate");
    let (other, other_events) = collector();
    let doomed = engine.request(reqs(&["x", "y"]), other);
    engine.request(reqs(&["z"]), Arc::clone(&sink));
    assert!(engine.cancel(doomed));
    release.send(()).unwrap();
    assert_eq!(events.recv_timeout(WAIT).unwrap().key(), "gate");
    assert_eq!(events.recv_timeout(WAIT).unwrap().key(), "z");
    assert!(other_events
        .recv_timeout(Duration::from_millis(100))
        .is_err());
    assert_eq!(*gate.seen.lock().unwrap(), ["gate", "z"]);
}

#[test]
fn cancelling_the_running_job_tells_the_processor_to_stop() {
    let (gate, started, _release) = gate();
    let engine = Engine::new(1, gate.clone());
    let (sink, events) = collector();
    let t = engine.request(reqs(&["gate"]), sink);
    assert_eq!(started.recv_timeout(WAIT).unwrap(), "gate");
    engine.cancel(t);
    // The processor returns `Cancelled`, so the worker is free again and no event is sent.
    let (sink2, events2) = collector();
    engine.request(reqs(&["next"]), sink2);
    assert_eq!(events2.recv_timeout(WAIT).unwrap().key(), "next");
    assert!(events.try_recv().is_err());
}

#[test]
fn two_requests_for_one_key_share_one_run() {
    let (gate, started, release) = gate();
    let engine = Engine::new(1, gate.clone());
    let (sink1, events1) = collector();
    let (sink2, events2) = collector();
    engine.request(reqs(&["gate"]), sink1);
    assert_eq!(started.recv_timeout(WAIT).unwrap(), "gate");
    engine.request(reqs(&["gate"]), sink2);
    release.send(()).unwrap();
    assert_eq!(events1.recv_timeout(WAIT).unwrap().key(), "gate");
    assert_eq!(events2.recv_timeout(WAIT).unwrap().key(), "gate");
    assert_eq!(gate.seen.lock().unwrap().len(), 1);
}

#[test]
fn a_crashing_generator_fails_the_item_and_the_worker_lives_on() {
    let (gate, _started, _release) = gate();
    let engine = Engine::new(1, gate);
    let (sink, events) = collector();
    engine.request(reqs(&["boom"]), Arc::clone(&sink));
    assert!(matches!(
        events.recv_timeout(WAIT).unwrap(),
        ThumbEvent::Failed { .. }
    ));
    engine.request(reqs(&["fine"]), sink);
    assert_eq!(
        events.recv_timeout(WAIT).unwrap(),
        ThumbEvent::Ready {
            key: "fine".into(),
            url: "u/fine".into()
        }
    );
    assert_eq!(engine.load(), (0, 0));
}

#[test]
fn several_workers_run_jobs_at_once() {
    let (gate, started, release) = gate();
    let engine = Engine::new(3, gate);
    let (sink, events) = collector();
    engine.request(reqs(&["gate"]), Arc::clone(&sink));
    assert_eq!(started.recv_timeout(WAIT).unwrap(), "gate");
    // The gate holds one worker; the others still serve.
    engine.request(reqs(&["a", "b"]), sink);
    let mut keys = vec![
        events.recv_timeout(WAIT).unwrap().key().to_string(),
        events.recv_timeout(WAIT).unwrap().key().to_string(),
    ];
    keys.sort();
    assert_eq!(keys, ["a", "b"]);
    release.send(()).unwrap();
}
