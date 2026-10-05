// The worker pool's loop: plans a job (cancellable, with progress), runs it through the executor,
// and bridges the executor's questions to the user by parking the worker until the answer comes.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// A job's life on a worker:
//
//   plan      Planning, on a worker with the lock released; progress as it walks; a cancel stops it
//   queue     the plan waits in `Ctl::prepared` until `next_runnable` names the job
//   start     Queued -> Running; clashes the planner found wait as `Waiting { Conflicts }`
//   begin     the write-ahead record of the journal is stored before the first write
//   run       the executor, with this module's `Sink`; progress, a pause, and any question park
//             the worker on the condition variable with the job's slot held
//   finish    fingerprints are read with the lock released, then one locked step commits the journal
//             entry and ends the job, and the events go out in order

use std::sync::Arc;
use std::time::Duration;

use tauri::{Emitter, Runtime};
use waypoint_ops::{
    fingerprint_steps, plan_with_progress, prepare_redo, prepare_undo, Conflict, Counts, Decision,
    ExecFailure, ExecReport, ExecSink, Executor, JobId, JobKind, JobState, OpsError, PendingRecord,
    PlanCtx, PlanProgress, Prepared, Progress, Recorded, Resolution, RunOptions, WaitReason,
};
use waypoint_protocol::Location;

use crate::models::JobJournal;
use crate::ops::{Core, Ops, Shared, Stage, Task};

/// What a planning worker takes from the journal before it unlocks.
#[allow(clippy::large_enum_variant)]
enum Pre {
    Plain,
    Undo(waypoint_ops::JournalId, Vec<waypoint_ops::InverseStep>),
    Redo(waypoint_ops::JournalId, waypoint_ops::JobRequest),
}

pub(crate) fn worker_main<R: Runtime>(shared: Arc<Shared<R>>) {
    loop {
        let task = {
            let mut core = shared.lock();
            loop {
                if core.shutdown {
                    core.workers = core.workers.saturating_sub(1);
                    shared.cv.notify_all();
                    return;
                }
                if let Some(task) = shared.claim_task(&mut core) {
                    core.busy += 1;
                    // Every worker busy while work may wait: the pool grows, up to its bound.
                    shared.ensure_workers(&mut core);
                    break task;
                }
                // A queued job held by a schedule is the one thing a worker waits on the clock for;
                // the wait is capped, so a clock that was set is read again within a minute.
                core = match core.store.next_wake_in_ms() {
                    Some(ms) => {
                        let wait = Duration::from_millis(ms.clamp(1, 60_000) as u64 + 5);
                        shared
                            .cv
                            .wait_timeout(core, wait)
                            .unwrap_or_else(|e| e.into_inner())
                            .0
                    }
                    None => shared.cv.wait(core).unwrap_or_else(|e| e.into_inner()),
                };
            }
        };
        match task {
            Task::Plan(id) => plan_job(&shared, id),
            Task::Run(id) => run_job(&shared, id),
        }
        let mut core = shared.lock();
        core.busy = core.busy.saturating_sub(1);
        core.prune();
        // A finished task frees a slot or a worker for the queue.
        shared.cv.notify_all();
    }
}

// ---- planning ----

fn plan_job<R: Runtime>(shared: &Arc<Shared<R>>, id: JobId) {
    let (request, token, pre) = {
        let mut core = shared.lock();
        let planning = core
            .store
            .job(id)
            .is_some_and(|j| j.state == JobState::Planning);
        let (Some(request), Some(token)) =
            (core.store.request(id).cloned(), core.store.cancel_token(id))
        else {
            core.ctl.remove(&id);
            return;
        };
        if !planning {
            // Cancelled before a worker came to it.
            core.ctl.remove(&id);
            return;
        }
        let pre = match request.kind {
            JobKind::Undo { of } => match core.journal.undo_steps(of) {
                Ok(steps) => Ok(Pre::Undo(of, steps)),
                Err(e) => Err(e),
            },
            JobKind::Redo { of } => match core.journal.redo_forward(of) {
                Ok(forward) => Ok(Pre::Redo(of, forward)),
                Err(e) => Err(e),
            },
            _ => Ok(Pre::Plain),
        };
        match pre {
            Ok(pre) => (request, token, pre),
            Err(error) => {
                fail_planning(shared, &mut core, id, error);
                return;
            }
        }
    };

    let ctx = PlanCtx {
        providers: &shared.env.providers,
        resolver: shared.resolver.as_ref(),
        trash: shared.env.trash.as_ref(),
        protected: &shared.env.protected,
        cancel: &token,
        archive_limits: shared.settings.get().archive_limits(),
    };
    let mut progress = |p: &PlanProgress| {
        let mut core = shared.lock();
        // Planning counts what it has found so far: the totals, not what is done.
        let tick = Progress {
            items_total: p.items,
            bytes_total: p.bytes,
            current: p.current.clone(),
            ..Progress::default()
        };
        if let Ok(events) = core.store.report(id, tick, Counts::default(), false) {
            shared.send_progress(&mut core, events);
        }
    };
    let result = match pre {
        Pre::Plain => plan_with_progress(&request, &ctx, &mut progress).map(Prepared::Plain),
        Pre::Undo(of, steps) => prepare_undo(of, steps, &ctx).map(Prepared::Undo),
        Pre::Redo(of, forward) => {
            prepare_redo(of, forward, &ctx, &mut progress).map(Prepared::Redo)
        }
    };

    let mut core = shared.lock();
    match result {
        Ok(prepared) => {
            let plan = match &prepared {
                Prepared::Plain(plan) => plan,
                Prepared::Undo(undo) => &undo.plan,
                Prepared::Redo(redo) => &redo.plan,
            };
            match core.store.plan_done(id, plan.totals()) {
                Ok(events) => {
                    shared.publish(events);
                    if let Some(ctl) = core.ctl.get_mut(&id) {
                        ctl.prepared = Some(prepared);
                        ctl.stage = Stage::Planned;
                    }
                }
                Err(e) => {
                    log::warn!("could not queue job {}: {e}", id.0);
                    core.ctl.remove(&id);
                }
            }
        }
        Err(error) => fail_planning(shared, &mut core, id, error),
    }
}

fn fail_planning<R: Runtime>(shared: &Shared<R>, core: &mut Core, id: JobId, error: OpsError) {
    // A job cancelled while it was planned stays cancelled and says nothing more.
    if let Ok(events) = core.store.plan_failed(id, error) {
        shared.publish(events);
    }
    core.ctl.remove(&id);
}

// ---- running ----

struct Sink<'a, R: Runtime> {
    shared: &'a Arc<Shared<R>>,
    id: JobId,
}

impl<R: Runtime> Sink<'_, R> {
    fn cancelled(core: &Core, id: JobId) -> bool {
        core.shutdown || core.store.cancel_token(id).is_none_or(|t| t.is_cancelled())
    }
}

impl<R: Runtime> ExecSink for Sink<'_, R> {
    fn progress(&mut self, progress: &Progress, counts: &Counts) {
        let core = self.shared.lock();
        let mut core = self
            .shared
            .park(core, self.id, |s| matches!(s, JobState::Paused));
        if let Ok(events) = core.store.report(self.id, progress.clone(), *counts, false) {
            self.shared.send_progress(&mut core, events);
        }
    }

    fn between_items(&mut self) {
        let core = self.shared.lock();
        drop(
            self.shared
                .park(core, self.id, |s| matches!(s, JobState::Paused)),
        );
    }

    fn on_error(&mut self, item: &Location, error: &OpsError) -> Option<Decision> {
        let core = self.shared.lock();
        let mut core = self
            .shared
            .park(core, self.id, |s| matches!(s, JobState::Paused));
        if Self::cancelled(&core, self.id) {
            return Some(Decision::Cancel);
        }
        let events = match core.store.wait(
            self.id,
            WaitReason::Error {
                error: error.clone(),
                item: item.clone(),
            },
        ) {
            Ok(events) => events,
            Err(e) => {
                log::warn!("job {} could not ask about an error: {e}", self.id.0);
                return None;
            }
        };
        if let Some(ctl) = core.ctl.get_mut(&self.id) {
            ctl.decision = None;
        }
        self.shared.publish(events);
        let mut core = self
            .shared
            .park(core, self.id, |s| matches!(s, JobState::Waiting { .. }));
        if Self::cancelled(&core, self.id) {
            return Some(Decision::Cancel);
        }
        core.ctl.get_mut(&self.id).and_then(|c| c.decision.take())
    }

    fn on_conflict(&mut self, conflict: &Conflict) -> Option<Resolution> {
        let core = self.shared.lock();
        let mut core = self
            .shared
            .park(core, self.id, |s| matches!(s, JobState::Paused));
        let skip = || {
            // Cancelling: settle it quietly so the executor reaches its next cancel check.
            Some(Resolution {
                source: Some(conflict.source.clone()),
                policy: waypoint_ops::ConflictPolicy::Skip,
            })
        };
        if Self::cancelled(&core, self.id) {
            return skip();
        }
        let events = match core.store.wait(
            self.id,
            WaitReason::Conflicts {
                conflicts: vec![conflict.clone()],
            },
        ) {
            Ok(events) => events,
            Err(e) => {
                log::warn!("job {} could not ask about a conflict: {e}", self.id.0);
                return None;
            }
        };
        if let Some(ctl) = core.ctl.get_mut(&self.id) {
            ctl.answers.clear();
        }
        self.shared.publish(events);
        let mut core = self
            .shared
            .park(core, self.id, |s| matches!(s, JobState::Waiting { .. }));
        if Self::cancelled(&core, self.id) {
            return skip();
        }
        let answers = core
            .ctl
            .get_mut(&self.id)
            .map(|c| std::mem::take(&mut c.answers))
            .unwrap_or_default();
        answers
            .into_iter()
            .find(|r| r.source.is_none() || r.source.as_ref() == Some(&conflict.source))
    }
}

/// What an executor run came to. It lives only from the end of the run to the commit.
#[allow(clippy::large_enum_variant)]
enum Outcome {
    Plain(Result<ExecReport, Box<ExecFailure>>),
    Undo(Result<waypoint_ops::UndoReport, Box<waypoint_ops::UndoFailure>>),
}

fn run_job<R: Runtime>(shared: &Arc<Shared<R>>, id: JobId) {
    let mut core = shared.lock();
    let prepared = core.ctl.get_mut(&id).and_then(|c| c.prepared.take());
    let (Some(prepared), Some(request), Some(token)) = (
        prepared,
        core.store.request(id).cloned(),
        core.store.cancel_token(id),
    ) else {
        end_unrun(shared, &mut core, id, OpsError::Cancelled);
        return;
    };

    // The clashes the planner found wait for the user before anything is written.
    let conflicts = match &prepared {
        Prepared::Plain(plan) => plan.conflicts.clone(),
        Prepared::Redo(redo) => redo.plan.conflicts.clone(),
        Prepared::Undo(_) => Vec::new(),
    };
    let pending = core
        .store
        .resolutions(id)
        .map(|r| r.unresolved(&conflicts))
        .unwrap_or_default();
    if !pending.is_empty() {
        if let Ok(events) = core
            .store
            .wait(id, WaitReason::Conflicts { conflicts: pending })
        {
            shared.publish(events);
            core = shared.park(core, id, |s| matches!(s, JobState::Waiting { .. }));
        }
    }
    if token.is_cancelled() || core.shutdown {
        end_unrun(shared, &mut core, id, OpsError::Cancelled);
        return;
    }

    // What the executor runs with, and the record that makes the job recoverable.
    let settings = shared.settings.get();
    let mut resolutions = core.store.resolutions(id).cloned().unwrap_or_default();
    let mut job_options = core.store.job(id).map(|j| j.options).unwrap_or_default();
    if let Prepared::Redo(redo) = &prepared {
        // A redo runs what the job did, with the policy and verification it had.
        if let (None, Some(policy)) = (resolutions.all(), redo.forward.options.conflict) {
            resolutions.set_all(policy);
        }
        job_options = redo.forward.options;
    }
    let mut options = RunOptions::for_job(&job_options, &settings, resolutions);
    options.clock = shared.clock.clone();
    options.throttle = core
        .ctl
        .get(&id)
        .map(|ctl| Ops::<R>::throttle_for(shared, ctl));
    let now = shared.clock.now_ms();
    let record = match &prepared {
        Prepared::Plain(plan) => PendingRecord::for_plan(id, now, &request, plan),
        Prepared::Redo(redo) => {
            let mut record = PendingRecord::for_plan(id, now, &redo.forward, &redo.plan);
            record.kind = request.kind;
            record
        }
        Prepared::Undo(undo) => match core.journal.entry(undo.entry) {
            Some(entry) => PendingRecord::for_undo(id, now, entry, &undo.steps),
            None => {
                end_unrun(
                    shared,
                    &mut core,
                    id,
                    OpsError::UndoUnavailable {
                        reason: "that is no longer in the undo history".to_owned(),
                    },
                );
                return;
            }
        },
    };
    if let Err(e) = core.journal.begin(record) {
        // The job is not recoverable if it is interrupted; it still runs, as the user asked.
        log::warn!(
            "could not store the write-ahead record of job {}: {e}",
            id.0
        );
    }
    drop(core);

    let executor = Executor::new(shared.env.clone());
    let mut sink = Sink { shared, id };
    let outcome = match &prepared {
        Prepared::Undo(undo) => {
            Outcome::Undo(executor.run_undo(id, &undo.steps, &token, &mut sink))
        }
        Prepared::Plain(plan) => {
            Outcome::Plain(executor.run_with(id, plan, &token, &mut sink, options))
        }
        Prepared::Redo(redo) => {
            Outcome::Plain(executor.run_with(id, &redo.plan, &token, &mut sink, options))
        }
    };
    finish(shared, id, &request, prepared, outcome);
}

/// Ends a job that was started and never ran (cancelled while it waited, or its entry vanished).
fn end_unrun<R: Runtime>(shared: &Shared<R>, core: &mut Core, id: JobId, error: OpsError) {
    let events = if error == OpsError::Cancelled {
        let mut events = core.store.cancel(id).unwrap_or_default();
        events.extend(core.store.cancelled(id).unwrap_or_default());
        events
    } else {
        core.store.fail(id, error, None, 0).unwrap_or_default()
    };
    shared.publish(events);
    core.journal.abort(id);
    core.ctl.remove(&id);
}

fn finish<R: Runtime>(
    shared: &Arc<Shared<R>>,
    id: JobId,
    request: &waypoint_ops::JobRequest,
    prepared: Prepared,
    outcome: Outcome,
) {
    // Fingerprints read the result, which can take a while for a big tree: not under the lock.
    let outcome = match outcome {
        Outcome::Plain(Ok(mut report)) => {
            fingerprint_steps(&shared.env.providers, &mut report.inverse);
            Outcome::Plain(Ok(report))
        }
        Outcome::Plain(Err(mut failure)) => {
            fingerprint_steps(&shared.env.providers, &mut failure.report.inverse);
            Outcome::Plain(Err(failure))
        }
        undo => undo,
    };

    let mut core = shared.lock();
    // A job that was paused as it ended runs once more so it can end.
    if core
        .store
        .job(id)
        .is_some_and(|j| j.state == JobState::Paused)
    {
        if let Ok(events) = core.store.resume(id) {
            shared.publish(events);
        }
    }
    let mut store_events = Vec::new();
    let mut journal_events = Vec::new();
    let mut undoable = false;
    let mut made: Option<waypoint_ops::JournalId> = None;
    match (&prepared, outcome) {
        (Prepared::Undo(undo), Outcome::Undo(result)) => {
            let applied = match &result {
                Ok(done) => done.applied,
                Err(failure) => failure.applied,
            };
            journal_events.extend(core.journal.finish_undo(id, undo.entry, applied));
            match result {
                Ok(_) => store_events.extend(core.store.done(id).unwrap_or_default()),
                Err(failure) => end_failed(
                    &mut core,
                    id,
                    failure.error.clone(),
                    failure.item.clone(),
                    failure.applied as u64,
                    &mut store_events,
                ),
            }
        }
        (_, Outcome::Plain(result)) => {
            let plan = match &prepared {
                Prepared::Plain(plan) => plan,
                Prepared::Redo(redo) => &redo.plan,
                Prepared::Undo(undo) => &undo.plan,
            };
            let (report, failure) = match result {
                Ok(report) => (report, None),
                Err(failure) => (failure.report.clone(), Some(failure)),
            };
            match &prepared {
                Prepared::Redo(redo) if failure.is_none() => {
                    journal_events.extend(core.journal.finish_redo(
                        id,
                        redo.entry,
                        report.inverse.clone(),
                    ));
                    // The entry is the one the redo made applied again; no new entry.
                    made = Some(redo.entry);
                }
                Prepared::Redo(redo) => {
                    match Recorded::from_run(&redo.forward, Some(plan), &report) {
                        Some(recorded) => {
                            let (entry, events) = core.journal.finish_redo_partly(id, recorded);
                            undoable = entry.is_some();
                            made = entry;
                            journal_events.extend(events);
                        }
                        None => core.journal.abort(id),
                    }
                }
                _ => match Recorded::from_run(request, Some(plan), &report) {
                    Some(recorded) => {
                        let (entry, events) = core.journal.commit(id, recorded);
                        undoable = entry.is_some();
                        made = entry;
                        journal_events.extend(events);
                    }
                    None => core.journal.abort(id),
                },
            }
            store_events.extend(
                core.store
                    .set_verified(id, report.transfer.verified.clone())
                    .unwrap_or_default(),
            );
            match failure {
                None => store_events.extend(core.store.done(id).unwrap_or_default()),
                Some(failure) => end_failed(
                    &mut core,
                    id,
                    failure.error.clone(),
                    failure.item.clone(),
                    failure.done,
                    &mut store_events,
                ),
            }
        }
        (_, Outcome::Undo(_)) => {
            core.journal.abort(id);
        }
    }
    if undoable {
        store_events.extend(core.store.mark_undoable(id, true).unwrap_or_default());
    }
    shared.publish(store_events);
    shared.publish(journal_events);
    if let Some(entry) = made {
        core.entries.insert(id, entry);
        let told = JobJournal { job: id, entry };
        if let Err(e) = shared.app.emit(crate::JOB_JOURNAL_EVENT, told) {
            log::warn!("could not announce the entry of job {}: {e}", id.0);
        }
    }
    core.ctl.remove(&id);
}

/// Ends a job that stopped: a cancel (asked for, or answered with Cancel) is `Cancelled`, anything
/// else `Failed`.
fn end_failed(
    core: &mut Core,
    id: JobId,
    error: OpsError,
    item: Option<Location>,
    done: u64,
    events: &mut Vec<waypoint_ops::OpsEvent>,
) {
    if error == OpsError::Cancelled {
        events.extend(core.store.cancel(id).unwrap_or_default());
        events.extend(core.store.cancelled(id).unwrap_or_default());
    } else {
        events.extend(core.store.fail(id, error, item, done).unwrap_or_default());
    }
}
