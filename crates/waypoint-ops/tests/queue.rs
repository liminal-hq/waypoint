// The queue: its state machine, concurrency, ordering, and invariants under random event
// sequences.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod common;

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use common::*;
use waypoint_ops::testing::harness::ManualClock;
use waypoint_protocol::Location;

struct Settings(Mutex<OpsSettings>);

impl SettingsReader for Settings {
    fn ops_settings(&self) -> OpsSettings {
        *self.0.lock().unwrap()
    }
}

fn store(concurrency: u32) -> (OpsStore, Arc<Settings>, Arc<ManualClock>) {
    let settings = Arc::new(Settings(Mutex::new(OpsSettings {
        concurrency,
        ..OpsSettings::default()
    })));
    let clock = Arc::new(ManualClock::new(1_000));
    (
        OpsStore::new(settings.clone(), clock.clone()),
        settings,
        clock,
    )
}

fn request(name: &str, destination: Option<&str>) -> JobRequest {
    let at = |p: &str| Location::new(p, format!("file://{p}"));
    JobRequest {
        kind: JobKind::Copy,
        sources: Sources::Locations {
            locations: vec![at(&format!("/src/{name}"))],
        },
        destination: destination.map(at),
        name: None,
        options: JobOptions::default(),
        origin_window: "main-1".to_owned(),
        rename: None,
    }
}

fn queued(store: &mut OpsStore, name: &str) -> JobId {
    let (id, _) = store.add(request(name, Some("/dst")));
    store.plan_done(id, PlanTotals::default()).unwrap();
    id
}

fn state_of(store: &OpsStore, id: JobId) -> &'static str {
    store.job(id).unwrap().state.name()
}

#[test]
fn a_job_is_added_planned_queued_run_and_finished() {
    let (mut s, _, clock) = store(2);
    let (id, events) = s.add(request("a", Some("/dst")));
    assert_eq!(id, JobId(1));
    assert!(matches!(
        events.as_slice(),
        [OpsEvent::JobAdded { revision: 1, .. }]
    ));
    assert_eq!(state_of(&s, id), "planning");
    assert_eq!(s.job(id).unwrap().created_ms, 1_000);
    assert_eq!(s.next_runnable(), None);
    clock.advance(10);
    let totals = PlanTotals {
        sources: SourcesSummary {
            count: Some(1),
            first: Some("a".to_owned()),
        },
        items: 4,
        bytes: 99,
        ..PlanTotals::default()
    };
    s.plan_done(id, totals).unwrap();
    let job = s.job(id).unwrap();
    assert_eq!(
        (job.progress.items_total, job.progress.bytes_total),
        (4, 99)
    );
    assert_eq!(job.title, "Copy \u{201c}a\u{201d}");
    assert_eq!(s.next_runnable(), Some(id));
    clock.advance(10);
    s.start(id).unwrap();
    assert_eq!(s.job(id).unwrap().started_ms, Some(1_020));
    clock.advance(10);
    s.done(id).unwrap();
    let job = s.job(id).unwrap();
    assert_eq!(job.state, JobState::Done);
    assert_eq!(job.finished_ms, Some(1_030));
    assert_eq!(s.revision(), 4);
    assert!(s.violations().is_empty());
}

#[test]
fn a_planning_failure_ends_the_job_with_the_error() {
    let (mut s, _, _) = store(2);
    let (id, _) = s.add(request("a", Some("/dst")));
    s.plan_failed(id, OpsError::SameFolder).unwrap();
    assert_eq!(
        s.job(id).unwrap().state,
        JobState::Failed {
            error: OpsError::SameFolder,
            item: None,
            done: 0
        }
    );
}

#[test]
fn at_most_the_concurrency_runs_and_the_setting_is_read_each_time() {
    let (mut s, settings, _) = store(2);
    let ids: Vec<JobId> = (0..4).map(|i| queued(&mut s, &format!("j{i}"))).collect();
    for id in &ids[..2] {
        assert_eq!(s.next_runnable(), Some(*id));
        s.start(*id).unwrap();
    }
    assert_eq!(s.next_runnable(), None);
    assert_eq!(s.start(ids[2]), Err(QueueError::NoSlot));
    assert_eq!(state_of(&s, ids[2]), "queued");
    s.done(ids[0]).unwrap();
    assert_eq!(s.next_runnable(), Some(ids[2]));
    // Raising the limit frees a slot at once; lowering it never stops a running job.
    settings.0.lock().unwrap().concurrency = 3;
    s.start(ids[2]).unwrap();
    assert_eq!(s.next_runnable(), Some(ids[3]));
    settings.0.lock().unwrap().concurrency = 1;
    assert_eq!(s.slots_in_use(), 2);
    assert_eq!(s.next_runnable(), None);
    // A limit of zero still lets one job run.
    settings.0.lock().unwrap().concurrency = 0;
    s.done(ids[1]).unwrap();
    s.done(ids[2]).unwrap();
    assert_eq!(s.next_runnable(), Some(ids[3]));
}

#[test]
fn paused_waiting_and_cancelling_jobs_keep_their_slot() {
    let (mut s, _, _) = store(1);
    let a = queued(&mut s, "a");
    let b = queued(&mut s, "b");
    s.start(a).unwrap();
    s.pause(a).unwrap();
    assert_eq!(s.next_runnable(), None);
    s.resume(a).unwrap();
    s.wait(a, WaitReason::Conflicts { conflicts: vec![] })
        .unwrap();
    assert_eq!(s.next_runnable(), None);
    s.answered(a).unwrap();
    s.cancel(a).unwrap();
    assert_eq!(state_of(&s, a), "cancelling");
    assert_eq!(s.next_runnable(), None);
    s.cancelled(a).unwrap();
    assert_eq!(s.next_runnable(), Some(b));
}

#[test]
fn cancel_acts_by_state() {
    let (mut s, _, _) = store(2);
    // Planning and queued jobs are cancelled at once, and their token is flipped.
    let (planning, _) = s.add(request("p", None));
    let token = s.cancel_token(planning).unwrap();
    s.cancel(planning).unwrap();
    assert_eq!(state_of(&s, planning), "cancelled");
    assert!(token.is_cancelled());
    // A late planning result for a cancelled job changes nothing.
    assert!(s
        .plan_done(planning, PlanTotals::default())
        .unwrap()
        .is_empty());
    assert!(s
        .plan_failed(planning, OpsError::Cancelled)
        .unwrap()
        .is_empty());
    let q = queued(&mut s, "q");
    s.cancel(q).unwrap();
    assert_eq!(state_of(&s, q), "cancelled");
    // A running job unwinds through `cancelling`; cancelling twice is harmless.
    let r = queued(&mut s, "r");
    s.start(r).unwrap();
    let revision = s.revision();
    s.cancel(r).unwrap();
    assert_eq!(state_of(&s, r), "cancelling");
    assert!(s.cancel_token(r).unwrap().is_cancelled());
    assert!(s.cancel(r).unwrap().is_empty());
    assert_eq!(s.revision(), revision + 1);
    // Cancelling after a worker finished anyway is allowed to end as done.
    s.done(r).unwrap();
    // A finished job cannot be cancelled.
    assert!(matches!(s.cancel(r), Err(QueueError::Illegal { .. })));
}

#[test]
fn illegal_calls_are_refused_and_change_nothing() {
    let (mut s, _, _) = store(2);
    let a = queued(&mut s, "a");
    let before = s.snapshot();
    assert!(matches!(s.pause(a), Err(QueueError::Illegal { .. })));
    assert!(matches!(s.resume(a), Err(QueueError::Illegal { .. })));
    assert!(matches!(s.done(a), Err(QueueError::Illegal { .. })));
    assert!(matches!(s.answered(a), Err(QueueError::Illegal { .. })));
    assert!(matches!(s.dismiss(a), Err(QueueError::Illegal { .. })));
    assert!(matches!(s.retry(a), Err(QueueError::Illegal { .. })));
    assert_eq!(s.start(JobId(99)), Err(QueueError::UnknownJob(JobId(99))));
    assert_eq!(s.snapshot(), before);
}

#[test]
fn retry_makes_a_new_job_and_ids_are_never_reused() {
    let (mut s, _, _) = store(2);
    let a = queued(&mut s, "a");
    s.start(a).unwrap();
    s.fail(
        a,
        OpsError::Io {
            message: "boom".into(),
        },
        None,
        3,
    )
    .unwrap();
    let (again, events) = s.retry(a).unwrap();
    assert_eq!(again, JobId(2));
    assert!(matches!(events[0], OpsEvent::JobAdded { .. }));
    assert_eq!(s.request(again), s.request(a));
    s.dismiss(a).unwrap();
    s.dismiss(again).unwrap_err();
    s.cancel(again).unwrap();
    s.dismiss(again).unwrap();
    let (next, _) = s.add(request("z", None));
    assert_eq!(next, JobId(3));
}

#[test]
fn dismiss_removes_finished_jobs_only() {
    let (mut s, _, _) = store(2);
    let a = queued(&mut s, "a");
    let b = queued(&mut s, "b");
    let c = queued(&mut s, "c");
    s.start(a).unwrap();
    s.done(a).unwrap();
    s.cancel(b).unwrap();
    let events = s.dismiss_finished();
    assert_eq!(events.len(), 2);
    let ids: Vec<JobId> = s.snapshot().jobs.iter().map(|j| j.id).collect();
    assert_eq!(ids, [c]);
}

#[test]
fn queued_jobs_can_be_reordered_among_themselves() {
    let (mut s, _, _) = store(1);
    let r = queued(&mut s, "running");
    s.start(r).unwrap();
    let a = queued(&mut s, "a");
    let b = queued(&mut s, "b");
    let c = queued(&mut s, "c");
    let order = |s: &OpsStore| -> Vec<JobId> { s.snapshot().jobs.iter().map(|j| j.id).collect() };
    assert_eq!(order(&s), [r, a, b, c]);
    let events = s.reorder(c, 0).unwrap();
    assert!(matches!(&events[0], OpsEvent::QueueReordered { order, .. } if *order == [r, c, a, b]));
    assert_eq!(order(&s), [r, c, a, b]);
    assert_eq!(s.next_runnable(), None);
    s.reorder(c, 99).unwrap();
    assert_eq!(order(&s), [r, a, b, c]);
    assert!(s.reorder(c, 2).unwrap().is_empty());
    assert!(matches!(s.reorder(r, 0), Err(QueueError::Illegal { .. })));
    s.done(r).unwrap();
    assert_eq!(s.next_runnable(), Some(a));
}

#[test]
fn progress_is_gated_and_late_reports_are_dropped() {
    let (mut s, _, clock) = store(2);
    let a = queued(&mut s, "a");
    s.start(a).unwrap();
    let at = |done: u64| Progress {
        items_done: done,
        items_total: 1000,
        ..Progress::default()
    };
    let none = Counts::default();
    assert_eq!(
        s.report(a, at(0), none, false).unwrap().len(),
        1,
        "the totals are new"
    );
    assert!(
        s.report(a, at(0), none, false).unwrap().is_empty(),
        "unchanged"
    );
    clock.advance(200);
    assert_eq!(s.report(a, at(100), none, false).unwrap().len(), 1);
    clock.advance(10);
    assert!(s.report(a, at(300), none, false).unwrap().is_empty());
    clock.advance(200);
    assert_eq!(s.report(a, at(300), none, false).unwrap().len(), 1);
    assert_eq!(s.job(a).unwrap().progress.items_done, 300);
    s.pause(a).unwrap();
    clock.advance(500);
    assert!(
        s.report(a, at(900), none, false).unwrap().is_empty(),
        "paused"
    );
    assert_eq!(s.job(a).unwrap().progress.items_done, 300);
}

#[test]
fn jobs_targeting_a_location_are_the_unfinished_ones_that_touch_it() {
    let (mut s, _, _) = store(2);
    let at = |p: &str| Location::new(p, format!("file://{p}"));
    let (a, _) = s.add(request("a", Some("/dst")));
    s.plan_done(
        a,
        PlanTotals {
            touches: vec![at("/src")],
            trees: vec![at("/src/a")],
            ..PlanTotals::default()
        },
    )
    .unwrap();
    // Destination and the folders named by the plan.
    assert_eq!(s.jobs_targeting(&at("/dst")), [a]);
    assert_eq!(s.jobs_targeting(&at("/src")), [a]);
    // Inside an entry the job removes or moves.
    assert_eq!(s.jobs_targeting(&at("/src/a/deeper")), [a]);
    assert!(s.jobs_targeting(&at("/src/ab")).is_empty());
    assert!(s.jobs_targeting(&at("/elsewhere")).is_empty());
    s.start(a).unwrap();
    s.done(a).unwrap();
    assert!(s.jobs_targeting(&at("/dst")).is_empty());
}

#[test]
fn replaying_the_events_reproduces_the_snapshot() {
    let (mut s, _, _) = store(1);
    let mut mirror = OpsSnapshot::default();
    let feed = |events: Vec<OpsEvent>, mirror: &mut OpsSnapshot| {
        for e in &events {
            mirror.apply(e);
        }
    };
    let (a, ev) = s.add(request("a", Some("/d")));
    feed(ev, &mut mirror);
    feed(s.plan_done(a, PlanTotals::default()).unwrap(), &mut mirror);
    let (b, ev) = s.add(request("b", Some("/d")));
    feed(ev, &mut mirror);
    feed(s.plan_done(b, PlanTotals::default()).unwrap(), &mut mirror);
    feed(s.start(a).unwrap(), &mut mirror);
    feed(s.reorder(b, 0).unwrap(), &mut mirror);
    feed(s.cancel(a).unwrap(), &mut mirror);
    feed(s.cancelled(a).unwrap(), &mut mirror);
    feed(s.dismiss(a).unwrap(), &mut mirror);
    assert_eq!(mirror, s.snapshot());
    // A stale event is ignored.
    let stale = OpsEvent::JobRemoved { id: b, revision: 1 };
    mirror.apply(&stale);
    assert_eq!(mirror, s.snapshot());
}

/// Random calls against the store, valid and not: whatever happens, the invariants hold.
#[test]
fn random_call_sequences_keep_the_invariants() {
    let mut reached: HashSet<&'static str> = HashSet::new();
    for seed in 0..60u64 {
        let mut rng = Rng::seeded(seed);
        let concurrency = 1 + rng.below(3) as u32;
        let (mut s, _, clock) = store(concurrency);
        let mut mirror = OpsSnapshot::default();
        let mut seen: HashSet<JobId> = HashSet::new();
        let mut high = 0u64;
        for step in 0..250 {
            clock.advance(rng.below(150) as i64);
            let before = s.snapshot();
            let known: Vec<JobId> = before.jobs.iter().map(|j| j.id).collect();
            let pick = |rng: &mut Rng| -> JobId {
                if known.is_empty() || rng.chance(15) {
                    JobId(1 + rng.below(40) as u64)
                } else {
                    *rng.pick(&known)
                }
            };
            let id = pick(&mut rng);
            let result: Result<Vec<OpsEvent>, QueueError> = match rng.below(17) {
                0 | 1 => {
                    let (new, events) = s.add(request(&format!("n{step}"), Some("/dst")));
                    assert!(seen.insert(new), "seed {seed}: id {new:?} reused");
                    assert!(new.0 > high, "seed {seed}: ids rise");
                    high = new.0;
                    Ok(events)
                }
                2 | 3 => s.plan_done(id, PlanTotals::default()),
                4 => s.plan_failed(id, OpsError::SameFolder),
                5 | 6 => s
                    .next_runnable()
                    .map_or(Ok(Vec::new()), |next| s.start(next)),
                7 => s.start(id),
                8 => s.pause(id),
                9 => s.resume(id),
                10 => s.wait(id, WaitReason::Conflicts { conflicts: vec![] }),
                11 => s.answered(id),
                12 => s.done(id),
                13 => s.cancel(id),
                14 => match rng.below(3) {
                    0 => s.cancelled(id),
                    1 => s.fail(id, OpsError::Cancelled, None, 1),
                    _ => s.dismiss(id),
                },
                15 => s.retry(id).map(|(_, events)| events),
                _ => s.reorder(id, rng.below(5)),
            };
            if let Ok(events) = &result {
                for e in events {
                    if let OpsEvent::JobAdded { job, .. } = e {
                        if job.id.0 > high {
                            assert!(seen.insert(job.id), "seed {seed}: id reused");
                            high = job.id.0;
                        }
                    }
                }
            }
            let events = result.clone().unwrap_or_default();
            // Revisions rise by exactly one per event, and an error changes nothing.
            let revisions: Vec<u64> = events.iter().map(OpsEvent::revision).collect();
            let expected: Vec<u64> =
                (before.revision + 1..=before.revision + events.len() as u64).collect();
            assert_eq!(revisions, expected, "seed {seed} step {step}");
            assert_eq!(s.revision(), before.revision + events.len() as u64);
            if result.is_err() {
                assert_eq!(
                    s.snapshot(),
                    before,
                    "seed {seed}: an error changed the store"
                );
            }
            for e in &events {
                mirror.apply(e);
            }
            assert_eq!(
                mirror,
                s.snapshot(),
                "seed {seed} step {step}: replay differs"
            );
            assert!(
                s.violations().is_empty(),
                "seed {seed}: {:?}",
                s.violations()
            );
            assert!(
                s.slots_in_use() <= concurrency as usize,
                "seed {seed} step {step}: {} slots over {concurrency}",
                s.slots_in_use()
            );
            // Every state a job reaches follows the table.
            for job in &s.snapshot().jobs {
                reached.insert(job.state.name());
                if let Some(old) = before.jobs.iter().find(|j| j.id == job.id) {
                    if old.state != job.state {
                        assert!(
                            is_legal(&old.state, &job.state),
                            "seed {seed}: {} -> {}",
                            old.state.name(),
                            job.state.name()
                        );
                    }
                }
            }
        }
    }
    // The sequences get everywhere: no state is untested for want of being reached.
    for state in [
        "planning",
        "queued",
        "running",
        "paused",
        "waiting",
        "cancelling",
        "cancelled",
        "done",
        "failed",
    ] {
        assert!(
            reached.contains(state),
            "{state} was never reached: {reached:?}"
        );
    }
}

fn with_priority(store: &mut OpsStore, name: &str, priority: JobPriority) -> JobId {
    let id = queued(store, name);
    store.set_limits(id, None, Some(priority)).unwrap();
    id
}

#[test]
fn a_higher_priority_starts_first_and_equals_keep_the_queue_order() {
    let (mut s, _, _) = store(1);
    let low = with_priority(&mut s, "low", JobPriority::Low);
    let first = queued(&mut s, "first");
    let high = with_priority(&mut s, "high", JobPriority::High);
    let second = queued(&mut s, "second");
    let also_high = with_priority(&mut s, "also-high", JobPriority::High);
    let mut order = Vec::new();
    while let Some(id) = s.next_runnable() {
        s.start(id).unwrap();
        s.done(id).unwrap();
        order.push(id);
    }
    assert_eq!(order, vec![high, also_high, first, second, low]);
}

#[test]
fn changing_a_priority_changes_who_is_next_and_a_running_job_is_never_stopped() {
    let (mut s, _, _) = store(1);
    let a = queued(&mut s, "a");
    let b = queued(&mut s, "b");
    let running = s.next_runnable().unwrap();
    assert_eq!(running, a);
    s.start(a).unwrap();
    s.set_limits(b, None, Some(JobPriority::High)).unwrap();
    // Outranking the running job does not take its slot.
    assert_eq!(s.next_runnable(), None);
    assert_eq!(state_of(&s, a), "running");
    s.done(a).unwrap();
    assert_eq!(s.next_runnable(), Some(b));
    // Back to normal: the job is as a job that never had one.
    s.set_limits(b, None, None).unwrap();
    assert_eq!(s.job(b).unwrap().options, JobOptions::default());
}

#[test]
fn limits_change_on_a_running_job_with_an_event_and_not_on_a_finished_one() {
    let (mut s, _, _) = store(1);
    let id = queued(&mut s, "a");
    s.start(id).unwrap();
    let events = s.set_limits(id, Some(2_000_000), None).unwrap();
    assert!(matches!(
        events.as_slice(),
        [OpsEvent::JobChanged { job, .. }] if job.options.speed_limit == Some(2_000_000)
    ));
    // The same values again change nothing and say nothing.
    assert!(s.set_limits(id, Some(2_000_000), None).unwrap().is_empty());
    s.done(id).unwrap();
    assert!(matches!(
        s.set_limits(id, None, None),
        Err(QueueError::Illegal { .. })
    ));
    assert_eq!(
        s.set_limits(JobId(99), None, None),
        Err(QueueError::UnknownJob(JobId(99)))
    );
}
