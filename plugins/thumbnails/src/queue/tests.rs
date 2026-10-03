// Tests the queue's order, deduplication, cancellation and reprioritisation
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

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

fn sink() -> Sink {
    Arc::new(|_| {})
}

fn drain(queue: &mut Queue) -> Vec<String> {
    let mut keys = Vec::new();
    while let Some(work) = queue.take_next() {
        keys.push(work.request.key.clone());
        queue.finish(work.id);
    }
    keys
}

#[test]
fn the_newest_request_runs_first_and_each_request_runs_in_order() {
    let mut q = Queue::new();
    q.enqueue(reqs(&["a1", "a2", "a3"]), sink());
    q.enqueue(reqs(&["b1", "b2"]), sink());
    assert_eq!(drain(&mut q), ["b1", "b2", "a1", "a2", "a3"]);
}

#[test]
fn a_key_is_one_job_and_a_repeat_moves_it_to_the_front() {
    let mut q = Queue::new();
    let t1 = q.enqueue(reqs(&["a", "b", "c"]), sink());
    let t2 = q.enqueue(reqs(&["c", "x"]), sink());
    assert_ne!(t1, t2);
    assert_eq!(q.pending_len(), 4);
    // `c` was asked for again by the newer request, so it runs in that request's place.
    assert_eq!(drain(&mut q), ["c", "x", "a", "b"]);
}

#[test]
fn a_repeated_key_in_one_request_is_one_job() {
    let mut q = Queue::new();
    q.enqueue(reqs(&["a", "a", "b"]), sink());
    assert_eq!(q.pending_len(), 2);
}

#[test]
fn a_shared_job_reports_to_every_request_that_wanted_it() {
    let mut q = Queue::new();
    let t1 = q.enqueue(reqs(&["a"]), sink());
    let t2 = q.enqueue(reqs(&["a"]), sink());
    let work = q.take_next().unwrap();
    // A third request while it runs joins the running job instead of starting another.
    let t3 = q.enqueue(reqs(&["a"]), sink());
    assert_eq!(q.pending_len(), 0);
    let tickets: Vec<Ticket> = q.finish(work.id).iter().map(|s| s.ticket).collect();
    assert_eq!(tickets, [t1, t2, t3]);
}

#[test]
fn a_changed_file_is_a_new_job_even_while_the_old_one_runs() {
    let mut q = Queue::new();
    q.enqueue(reqs(&["a"]), sink());
    let work = q.take_next().unwrap();
    let mut changed = req("a");
    changed.mtime_ms = 5;
    q.enqueue(vec![changed], sink());
    assert_eq!(q.pending_len(), 1);
    q.finish(work.id);
}

#[test]
fn cancel_removes_pending_work_nobody_else_wants() {
    let mut q = Queue::new();
    let t1 = q.enqueue(reqs(&["a", "b"]), sink());
    let t2 = q.enqueue(reqs(&["b", "c"]), sink());
    assert!(q.cancel(t2));
    // `b` is still wanted by the first request (and keeps the front place the second gave it); `c` is gone.
    assert_eq!(drain(&mut q), ["b", "a"]);
    assert!(!q.cancel(Ticket(99)));
    let _ = t1;
}

#[test]
fn cancelling_the_only_request_of_a_running_job_flags_it() {
    let mut q = Queue::new();
    let t1 = q.enqueue(reqs(&["a"]), sink());
    let t2 = q.enqueue(reqs(&["b"]), sink());
    let a = q.take_next().unwrap();
    let b = q.take_next().unwrap();
    q.cancel(t1);
    q.cancel(t2);
    // Each running job lost its only request.
    assert!(a.cancel.load(Ordering::Relaxed) && b.cancel.load(Ordering::Relaxed));
    assert!(q.finish(a.id).is_empty());
    assert!(q.finish(b.id).is_empty());
}

#[test]
fn a_running_job_that_another_request_shares_is_not_flagged() {
    let mut q = Queue::new();
    let t1 = q.enqueue(reqs(&["a"]), sink());
    let t2 = q.enqueue(reqs(&["a"]), sink());
    let work = q.take_next().unwrap();
    q.cancel(t1);
    assert!(!work.cancel.load(Ordering::Relaxed));
    q.cancel(t2);
    assert!(work.cancel.load(Ordering::Relaxed));
}

#[test]
fn prioritise_moves_keys_to_the_front_in_the_order_given() {
    let mut q = Queue::new();
    let t = q.enqueue(reqs(&["a", "b", "c", "d", "e"]), sink());
    q.prioritise(
        t,
        &["d".to_string(), "c".to_string(), "missing".to_string()],
    );
    assert_eq!(drain(&mut q), ["d", "c", "a", "b", "e"]);
}

#[test]
fn prioritise_only_touches_the_requests_own_jobs() {
    let mut q = Queue::new();
    let t1 = q.enqueue(reqs(&["a", "b"]), sink());
    let _t2 = q.enqueue(reqs(&["x", "y"]), sink());
    q.prioritise(t1, &["y".to_string(), "b".to_string()]);
    assert_eq!(drain(&mut q), ["b", "x", "y", "a"]);
}

#[test]
fn clear_drops_pending_work_and_flags_running_work() {
    let mut q = Queue::new();
    q.enqueue(reqs(&["a", "b"]), sink());
    let work = q.take_next().unwrap();
    q.clear();
    assert_eq!(q.pending_len(), 0);
    assert!(work.cancel.load(Ordering::Relaxed));
    assert_eq!(q.running_len(), 1);
}
