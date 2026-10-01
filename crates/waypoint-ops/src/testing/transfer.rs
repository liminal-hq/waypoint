// A driver that runs a copy or a move the way the plugin's worker will, answering conflicts and
// errors from a script: plan, wait for answers to the clashes the planner found, run, and park the
// job in `Waiting` whenever the executor asks.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use waypoint_protocol::Location;
use waypoint_vfs::{CancelToken, Provider};

use crate::exec::{ExecSink, Executor, Resolutions, RunOptions, CHUNK_BYTES};
use crate::model::{
    Conflict, Counts, Decision, JobRequest, JobState, OpsError, OpsEvent, OpsSettings, Progress,
    Resolution, WaitReason,
};
use crate::plan::plan;
use crate::queue::OpsStore;
use crate::testing::harness::{Harness, RunResult};
use crate::traits::Clock;

/// Answers conflicts: given the ones the job waits on, returns the answers to give.
pub type ConflictAnswer = Box<dyn FnMut(&[Conflict]) -> Vec<Resolution>>;

/// Answers an error: given the item and the error, returns the decision, if any.
pub type ErrorAnswer = Box<dyn FnMut(&Location, &OpsError) -> Option<Decision>>;

/// What answers the questions a running transfer asks.
pub struct Answers {
    /// Called with the conflicts the job waits on; returns the answers to give (an empty list
    /// leaves the job waiting).
    pub conflicts: ConflictAnswer,
    /// Called with the item and the error; `None` answers nothing, which fails the job there.
    pub errors: ErrorAnswer,
}

impl Default for Answers {
    fn default() -> Self {
        Self {
            conflicts: Box::new(|_| Vec::new()),
            errors: Box::new(|_, _| None),
        }
    }
}

impl Answers {
    /// Answers every conflict with `policy` for all, and every error with `decision`.
    pub fn always(policy: Option<crate::ConflictPolicy>, decision: Option<Decision>) -> Self {
        Self {
            conflicts: Box::new(move |_| {
                policy
                    .map(|policy| {
                        vec![Resolution {
                            source: None,
                            policy,
                        }]
                    })
                    .unwrap_or_default()
            }),
            errors: Box::new(move |_, _| decision),
        }
    }
}

/// How the driver runs a job beyond the request.
#[derive(Clone)]
pub struct TransferOptions {
    pub settings: OpsSettings,
    pub chunk_bytes: usize,
}

impl Default for TransferOptions {
    fn default() -> Self {
        Self {
            settings: OpsSettings::default(),
            chunk_bytes: CHUNK_BYTES,
        }
    }
}

struct DriveSink<'a> {
    store: &'a mut OpsStore,
    id: crate::JobId,
    events: &'a mut Vec<OpsEvent>,
    answers: &'a mut Answers,
}

impl ExecSink for DriveSink<'_> {
    fn progress(&mut self, progress: &Progress, counts: &Counts) {
        let events = self
            .store
            .report(self.id, progress.clone(), *counts, false)
            .expect("the job is known");
        self.events.extend(events);
    }

    fn on_error(&mut self, item: &Location, error: &OpsError) -> Option<Decision> {
        let events = self
            .store
            .wait(
                self.id,
                WaitReason::Error {
                    error: error.clone(),
                    item: item.clone(),
                },
            )
            .expect("a running job waits");
        self.events.extend(events);
        let decision = (self.answers.errors)(item, error);
        let events = self.store.answered(self.id).expect("a waiting job runs");
        self.events.extend(events);
        decision
    }

    fn on_conflict(&mut self, conflict: &Conflict) -> Option<Resolution> {
        let events = self
            .store
            .wait(
                self.id,
                WaitReason::Conflicts {
                    conflicts: vec![conflict.clone()],
                },
            )
            .expect("a running job waits");
        self.events.extend(events);
        let given = (self.answers.conflicts)(std::slice::from_ref(conflict));
        let events = self
            .store
            .resolve(self.id, &given)
            .expect("a waiting job takes answers");
        self.events.extend(events);
        if matches!(
            self.store.job(self.id).map(|j| &j.state),
            Some(JobState::Waiting { .. })
        ) {
            let events = self.store.answered(self.id).expect("a waiting job runs");
            self.events.extend(events);
        }
        given
            .into_iter()
            .find(|r| r.source.is_none() || r.source.as_ref() == Some(&conflict.source))
    }
}

/// Runs a copy or move request start to end. The job waits for `answers.conflicts` to settle the
/// clashes the planner found; one it leaves unanswered parks the job in `Waiting` and the result
/// is that state, with the plan and nothing run. `hook` runs once the job is added and before it is
/// planned, given the job's cancel token.
pub fn run_transfer<P: Provider + 'static>(
    h: &mut Harness<P>,
    request: JobRequest,
    answers: &mut Answers,
    options: &TransferOptions,
    hook: &mut dyn FnMut(&Harness<P>, &CancelToken),
) -> RunResult {
    let (id, events) = h.store.add(request.clone());
    h.events.extend(events);
    let token = h.store.cancel_token(id).expect("the job was added");
    hook(h, &token);
    let planned = {
        let ctx = h.plan_ctx(&token);
        plan(&request, &ctx)
    };
    let finish = |h: &Harness<P>, report, failure, plan| RunResult {
        id,
        state: h.store.job(id).expect("the job is listed").state.clone(),
        report,
        failure,
        plan,
    };
    let planned = match planned {
        Ok(planned) => planned,
        Err(error) => {
            if error == OpsError::Cancelled {
                let events = h.store.cancel(id).expect("a planning job cancels");
                h.events.extend(events);
            }
            let events = h.store.plan_failed(id, error).expect("planning fails");
            h.events.extend(events);
            return finish(h, None, None, None);
        }
    };
    let events = h
        .store
        .plan_done(id, planned.totals())
        .expect("planning finishes");
    h.events.extend(events);
    assert_eq!(h.store.next_runnable(), Some(id), "the job is next");
    let events = h.store.start(id).expect("a slot is free");
    h.events.extend(events);

    // The clashes the planner found wait for the user.
    let mut pending = h
        .store
        .resolutions(id)
        .expect("the job is known")
        .unresolved(&planned.conflicts);
    if !pending.is_empty() {
        let events = h
            .store
            .wait(
                id,
                WaitReason::Conflicts {
                    conflicts: pending.clone(),
                },
            )
            .expect("a running job waits");
        h.events.extend(events);
        while matches!(
            h.store.job(id).map(|j| &j.state),
            Some(JobState::Waiting { .. })
        ) {
            let given = (answers.conflicts)(&pending);
            if given.is_empty() {
                // Nobody answered: the job stays parked.
                return finish(h, None, None, Some(planned));
            }
            let events = h.store.resolve(id, &given).expect("the job takes answers");
            h.events.extend(events);
            if let Some(JobState::Waiting {
                reason: WaitReason::Conflicts { conflicts },
            }) = h.store.job(id).map(|j| j.state.clone())
            {
                pending = conflicts;
            }
        }
    }

    let run_options = {
        let job = h.store.job(id).expect("the job is listed");
        let mut run_options = RunOptions::for_job(
            &job.options,
            &options.settings,
            h.store.resolutions(id).expect("the job is known").clone(),
        );
        run_options.verify = request
            .options
            .verify
            .unwrap_or(options.settings.verify_after_copy)
            .then_some(options.settings.verify_algorithm);
        run_options.chunk_bytes = options.chunk_bytes;
        run_options.clock = h.clock.clone() as std::sync::Arc<dyn Clock>;
        run_options
    };
    let executor = Executor::new(h.env.clone());
    let outcome = {
        let mut sink = DriveSink {
            store: &mut h.store,
            id,
            events: &mut h.events,
            answers,
        };
        executor.run_with(id, &planned, &token, &mut sink, run_options)
    };
    let verified = match &outcome {
        Ok(report) => report.transfer.verified.clone(),
        Err(failure) => failure.report.transfer.verified.clone(),
    };
    let events = h
        .store
        .set_verified(id, verified)
        .expect("the job is known");
    h.events.extend(events);
    match outcome {
        Ok(report) => {
            let events = h.store.done(id).expect("a running job finishes");
            h.events.extend(events);
            finish(h, Some(report), None, Some(planned))
        }
        Err(failure) if failure.error == OpsError::Cancelled => {
            let events = h.store.cancel(id).expect("a running job cancels");
            h.events.extend(events);
            let events = h.store.cancelled(id).expect("a cancelling job ends");
            h.events.extend(events);
            finish(h, None, Some(failure), Some(planned))
        }
        Err(failure) => {
            let events = h
                .store
                .fail(
                    id,
                    failure.error.clone(),
                    failure.item.clone(),
                    failure.done,
                )
                .expect("a running job fails");
            h.events.extend(events);
            finish(h, None, Some(failure), Some(planned))
        }
    }
}

/// The resolutions a job holds, for tests that look inside.
pub fn resolutions_of<P: Provider + 'static>(
    h: &Harness<P>,
    id: crate::JobId,
) -> Option<Resolutions> {
    h.store.resolutions(id).cloned()
}
