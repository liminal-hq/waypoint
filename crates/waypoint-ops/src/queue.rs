// The queue: every job under one revision and one writer, with the state machine enforced.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// An `OpsStore` is plain data with one owner: every method takes `&mut self` and returns the
// events the change made, each carrying the new global revision, so the plugin wraps it in one
// mutex and sends the events on. It does no work itself and starts no threads; the plugin's workers
// ask `next_runnable`, run the job through the executor, and report back through `report`, `done`,
// `fail` and `cancelled`.
//
// The legal transitions (anything else is `QueueError::Illegal` and changes nothing):
//
//   planning   -> queued | failed | cancelled
//   queued     -> running | cancelled
//   running    -> paused | waiting | offline | done | failed | cancelling
//   paused     -> running | cancelling
//   waiting    -> running | cancelling
//   offline    -> running | cancelling
//   cancelling -> cancelled | done | failed
//
// `done`, `failed` and `cancelled` are final; `dismiss` removes the job from the list and `retry`
// makes a new job from the same request. A job holds one of the worker slots from `running` until
// it ends, including while it is paused, waiting, offline or cancelling, because a worker is parked on it.

use std::sync::Arc;

use thiserror::Error;
use waypoint_path::TrashPath;
use waypoint_protocol::Location;
use waypoint_vfs::CancelToken;

use crate::exec::Resolutions;
use crate::model::{
    Counts, DroppedDetail, JobId, JobKind, JobPriority, JobRequest, JobSnapshot, JobState,
    OpsError, OpsEvent, OpsSnapshot, PartialNote, PlanTotals, Progress, Resolution, Sources,
    SourcesSummary, Verification, WaitReason,
};
use crate::schedule::Schedule;
use crate::traits::{Clock, SettingsReader};

/// Why the queue refused a change. The store is unchanged.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum QueueError {
    #[error("there is no job {}", .0 .0)]
    UnknownJob(JobId),
    #[error("job {} is {from} and cannot {action}", .id.0)]
    Illegal {
        id: JobId,
        from: &'static str,
        action: &'static str,
    },
    #[error("every worker slot is taken")]
    NoSlot,
}

/// Whether a job may move from one state to another. The table is in the module comment.
pub fn is_legal(from: &JobState, to: &JobState) -> bool {
    use JobState::*;
    matches!(
        (from, to),
        (Planning, Queued | Failed { .. } | Cancelled)
            | (Queued, Running | Cancelled)
            | (
                Running,
                Paused | Waiting { .. } | Offline { .. } | Done | Failed { .. } | Cancelling
            )
            | (Paused, Running | Cancelling)
            | (Offline { .. }, Running | Cancelling)
            | (Waiting { .. }, Running | Cancelling)
            | (Cancelling, Cancelled | Done | Failed { .. })
    )
}

/// Decides which progress reports are worth an event: a report passes when at least a tenth of a
/// second has gone by and the job has moved by at least a percent since the last one that passed,
/// and always when the job completes. Pure, so it is tested without a clock.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProgressGate {
    min_interval_ms: i64,
    min_step: f64,
    last: Option<(i64, f64)>,
}

impl Default for ProgressGate {
    fn default() -> Self {
        Self::new()
    }
}

impl ProgressGate {
    /// The house limits: 100 ms and 1 %.
    pub const fn new() -> Self {
        Self::with_limits(100, 0.01)
    }

    pub const fn with_limits(min_interval_ms: i64, min_step: f64) -> Self {
        Self {
            min_interval_ms,
            min_step,
            last: None,
        }
    }

    /// Whether to let this report through. `force` passes it regardless (a change of what the job
    /// is working on that must not be lost).
    pub fn admit(&mut self, now_ms: i64, fraction: f64, force: bool) -> bool {
        let pass = match self.last {
            None => true,
            Some(_) if force => true,
            Some((at, last)) => {
                (fraction >= 1.0 && last < 1.0)
                    || (now_ms - at >= self.min_interval_ms
                        && (fraction - last).abs() >= self.min_step)
            }
        };
        if pass {
            self.last = Some((now_ms, fraction));
        }
        pass
    }
}

struct Job {
    snapshot: JobSnapshot,
    request: JobRequest,
    cancel: CancelToken,
    gate: ProgressGate,
    /// Folders the job reads from or writes into.
    touches: Vec<Location>,
    /// Entries the job removes or moves, which cover everything below them.
    trees: Vec<Location>,
    /// The answers the user has given to its conflicts (A48), kept so a run that is retried or
    /// resumed does not ask twice.
    resolutions: Resolutions,
}

/// The title a job shows until the frontend words its own.
fn title_for(kind: JobKind, sources: &SourcesSummary, name: Option<&str>) -> String {
    let verb = match kind {
        JobKind::CreateFolder => "Create folder",
        JobKind::CreateFile => "Create file",
        JobKind::Rename => "Rename",
        JobKind::Duplicate => "Duplicate",
        JobKind::Trash => "Move to Trash",
        JobKind::Restore => "Restore",
        JobKind::Delete => "Delete",
        JobKind::EmptyTrash {
            older_than_days: None,
        } => "Empty Trash",
        JobKind::EmptyTrash { .. } => "Empty old items from the Trash",
        JobKind::Copy => "Copy",
        JobKind::Move => "Move",
        JobKind::Link => "Link",
        JobKind::BatchRename => "Rename",
        JobKind::Extract => "Extract",
        JobKind::Compress => "Compress",
        JobKind::Undo { .. } => "Undo",
        JobKind::Redo { .. } => "Redo",
    };
    let subject = match (sources.count, sources.first.as_deref(), name) {
        (Some(1), Some(first), _) => format!(" \u{201c}{first}\u{201d}"),
        (Some(n), _, _) if n > 1 => format!(" {n} items"),
        (_, _, Some(name)) if matches!(kind, JobKind::CreateFolder | JobKind::CreateFile) => {
            format!(" \u{201c}{name}\u{201d}")
        }
        _ => String::new(),
    };
    format!("{verb}{subject}")
}

/// Every job, in queue order.
pub struct OpsStore {
    revision: u64,
    next_id: u64,
    jobs: Vec<Job>,
    /// Pause all is in force.
    paused: bool,
    settings: Arc<dyn SettingsReader>,
    clock: Arc<dyn Clock>,
}

impl OpsStore {
    pub fn new(settings: Arc<dyn SettingsReader>, clock: Arc<dyn Clock>) -> Self {
        Self {
            revision: 0,
            next_id: 1,
            jobs: Vec::new(),
            paused: false,
            settings,
            clock,
        }
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn snapshot(&self) -> OpsSnapshot {
        OpsSnapshot {
            revision: self.revision,
            jobs: self.jobs.iter().map(|j| j.snapshot.clone()).collect(),
            paused: self.paused,
            journal: Default::default(),
        }
    }

    pub fn job(&self, id: JobId) -> Option<&JobSnapshot> {
        self.find(id).map(|j| &j.snapshot)
    }

    /// The request a job was made from.
    pub fn request(&self, id: JobId) -> Option<&JobRequest> {
        self.find(id).map(|j| &j.request)
    }

    /// The token the job's executor and planner watch.
    pub fn cancel_token(&self, id: JobId) -> Option<CancelToken> {
        self.find(id).map(|j| j.cancel.clone())
    }

    fn find(&self, id: JobId) -> Option<&Job> {
        self.jobs.iter().find(|j| j.snapshot.id == id)
    }

    fn index(&self, id: JobId) -> Result<usize, QueueError> {
        self.jobs
            .iter()
            .position(|j| j.snapshot.id == id)
            .ok_or(QueueError::UnknownJob(id))
    }

    /// How many jobs hold a worker slot.
    pub fn slots_in_use(&self) -> usize {
        self.jobs
            .iter()
            .filter(|j| j.snapshot.state.holds_slot())
            .count()
    }

    /// How many jobs may run at once: the setting, and at least one.
    pub fn concurrency(&self) -> usize {
        self.settings.ops_settings().concurrency.max(1) as usize
    }

    fn bump(&mut self) -> u64 {
        self.revision += 1;
        self.revision
    }

    fn changed(&mut self, index: usize) -> OpsEvent {
        let revision = self.bump();
        OpsEvent::JobChanged {
            job: self.jobs[index].snapshot.clone(),
            revision,
        }
    }

    /// Records whether the journal can undo the job, once the journal has committed its entry.
    pub fn mark_undoable(
        &mut self,
        id: JobId,
        undoable: bool,
    ) -> Result<Vec<OpsEvent>, QueueError> {
        let index = self.index(id)?;
        if self.jobs[index].snapshot.undoable == undoable {
            return Ok(Vec::new());
        }
        self.jobs[index].snapshot.undoable = undoable;
        Ok(vec![self.changed(index)])
    }

    /// Adds a job at the end of the queue, in `Planning`.
    pub fn add(&mut self, request: JobRequest) -> (JobId, Vec<OpsEvent>) {
        let id = JobId(self.next_id);
        self.next_id += 1;
        let sources = SourcesSummary {
            count: match &request.sources {
                Sources::Locations { locations } => Some(locations.len() as u64),
                Sources::Selection { .. } => None,
            },
            first: match &request.sources {
                Sources::Locations { locations } => locations.first().map(source_name),
                Sources::Selection { .. } => None,
            },
        };
        let snapshot = JobSnapshot {
            id,
            kind: request.kind,
            state: JobState::Planning,
            title: title_for(request.kind, &sources, request.name.as_deref()),
            sources,
            destination: request.destination.clone(),
            options: request.options,
            origin_window: request.origin_window.clone(),
            counts: Counts::default(),
            progress: Progress::default(),
            created_ms: self.clock.now_ms(),
            started_ms: None,
            finished_ms: None,
            undoable: false,
            verified: None,
            ends: None,
            dropped: None,
            partial: None,
        };
        let mut touches = Vec::new();
        touches.extend(request.destination.clone());
        let request_policy = request.options.conflict;
        let job = Job {
            snapshot: snapshot.clone(),
            request,
            cancel: CancelToken::new(),
            gate: ProgressGate::new(),
            touches,
            trees: Vec::new(),
            resolutions: Resolutions::new(request_policy),
        };
        self.jobs.push(job);
        let revision = self.bump();
        (
            id,
            vec![OpsEvent::JobAdded {
                job: snapshot,
                revision,
            }],
        )
    }

    /// Moves a job to `to` if the table allows it, stamping the times.
    fn go(
        &mut self,
        id: JobId,
        to: JobState,
        action: &'static str,
    ) -> Result<Vec<OpsEvent>, QueueError> {
        let index = self.index(id)?;
        let from = &self.jobs[index].snapshot.state;
        if !is_legal(from, &to) {
            return Err(QueueError::Illegal {
                id,
                from: from.name(),
                action,
            });
        }
        let now = self.clock.now_ms();
        let snapshot = &mut self.jobs[index].snapshot;
        if matches!(to, JobState::Running) && snapshot.started_ms.is_none() {
            snapshot.started_ms = Some(now);
        }
        if to.is_finished() {
            snapshot.finished_ms = Some(now);
        }
        snapshot.state = to;
        Ok(vec![self.changed(index)])
    }

    /// Planning finished: records what the planner found and queues the job. A job cancelled
    /// while it was being planned stays cancelled and nothing happens.
    pub fn plan_done(
        &mut self,
        id: JobId,
        totals: PlanTotals,
    ) -> Result<Vec<OpsEvent>, QueueError> {
        let index = self.index(id)?;
        let state = &self.jobs[index].snapshot.state;
        if *state == JobState::Cancelled {
            return Ok(Vec::new());
        }
        if !is_legal(state, &JobState::Queued) {
            return Err(QueueError::Illegal {
                id,
                from: state.name(),
                action: "finish planning",
            });
        }
        let job = &mut self.jobs[index];
        job.snapshot.progress.items_total = totals.items;
        job.snapshot.progress.bytes_total = totals.bytes;
        job.snapshot.title = title_for(
            job.request.kind,
            &totals.sources,
            job.request.name.as_deref(),
        );
        job.snapshot.sources = totals.sources;
        job.snapshot.ends = (!totals.ends.is_local()).then_some(totals.ends);
        job.touches.extend(totals.touches);
        job.trees = totals.trees;
        self.go(id, JobState::Queued, "finish planning")
    }

    /// Planning failed: the job ends. A job already cancelled stays cancelled.
    pub fn plan_failed(&mut self, id: JobId, error: OpsError) -> Result<Vec<OpsEvent>, QueueError> {
        let index = self.index(id)?;
        if self.jobs[index].snapshot.state == JobState::Cancelled {
            return Ok(Vec::new());
        }
        self.go(
            id,
            JobState::Failed {
                error,
                item: None,
                done: 0,
            },
            "fail",
        )
    }

    /// The next job a free worker should run, if a slot is free: the queued job of the highest
    /// priority, the first in queue order among equals. Nothing starts while Pause all is in force,
    /// and a job whose schedule is not open yet (D157) is passed over.
    pub fn next_runnable(&self) -> Option<JobId> {
        if self.paused || self.slots_in_use() >= self.concurrency() {
            return None;
        }
        let now = self.clock.now_ms();
        self.jobs
            .iter()
            .filter(|j| j.snapshot.state == JobState::Queued)
            .filter(|j| j.snapshot.options.schedule.is_none_or(|s| s.is_open(now)))
            // `max_by_key` keeps the last of equals, so the order is reversed to keep the first.
            .rev()
            .max_by_key(|j| j.snapshot.options.priority())
            .map(|j| j.snapshot.id)
    }

    /// When the soonest queued job held by its schedule opens, in milliseconds from now, for the
    /// plugin's timer; `None` when no queued job waits for a time.
    pub fn next_wake_in_ms(&self) -> Option<i64> {
        let now = self.clock.now_ms();
        self.jobs
            .iter()
            .filter(|j| j.snapshot.state == JobState::Queued)
            .filter_map(|j| j.snapshot.options.schedule)
            .map(|s| s.opens_in_ms(now))
            .filter(|ms| *ms > 0)
            .min()
    }

    /// Sets or clears when a job that has not started may start. `None` is "Run now".
    pub fn set_schedule(
        &mut self,
        id: JobId,
        schedule: Option<Schedule>,
    ) -> Result<Vec<OpsEvent>, QueueError> {
        let index = self.index(id)?;
        let snapshot = &mut self.jobs[index].snapshot;
        if !matches!(snapshot.state, JobState::Planning | JobState::Queued) {
            return Err(QueueError::Illegal {
                id,
                from: snapshot.state.name(),
                action: "schedule",
            });
        }
        if snapshot.options.schedule == schedule {
            return Ok(Vec::new());
        }
        snapshot.options.schedule = schedule;
        self.jobs[index].request.options.schedule = schedule;
        Ok(vec![self.changed(index)])
    }

    /// The jobs that have not started and are held by a schedule, as the requests that would make
    /// them again: what the journal keeps so they survive a restart.
    pub fn scheduled_requests(&self) -> Vec<(JobId, JobRequest)> {
        self.jobs
            .iter()
            .filter(|j| matches!(j.snapshot.state, JobState::Planning | JobState::Queued))
            .filter(|j| j.snapshot.options.schedule.is_some())
            .map(|j| {
                let mut request = j.request.clone();
                request.options = j.snapshot.options;
                (j.snapshot.id, request)
            })
            .collect()
    }

    /// Pause all: nothing queued starts, and every running job pauses where it is (a job that is
    /// waiting for an answer keeps waiting). Idempotent.
    pub fn pause_all(&mut self) -> Vec<OpsEvent> {
        let mut events = Vec::new();
        if !self.paused {
            self.paused = true;
            let revision = self.bump();
            events.push(OpsEvent::QueuePaused {
                paused: true,
                revision,
            });
        }
        let running: Vec<JobId> = self
            .jobs
            .iter()
            .filter(|j| j.snapshot.state == JobState::Running)
            .map(|j| j.snapshot.id)
            .collect();
        for id in running {
            events.extend(self.pause(id).unwrap_or_default());
        }
        events
    }

    /// Resume all: queued jobs may start again and every paused job runs on. Idempotent.
    pub fn resume_all(&mut self) -> Vec<OpsEvent> {
        let mut events = Vec::new();
        if self.paused {
            self.paused = false;
            let revision = self.bump();
            events.push(OpsEvent::QueuePaused {
                paused: false,
                revision,
            });
        }
        let paused: Vec<JobId> = self
            .jobs
            .iter()
            .filter(|j| j.snapshot.state == JobState::Paused)
            .map(|j| j.snapshot.id)
            .collect();
        for id in paused {
            events.extend(self.resume(id).unwrap_or_default());
        }
        events
    }

    /// Whether Pause all is in force.
    pub fn is_paused(&self) -> bool {
        self.paused
    }

    /// Changes a job's speed limit and priority while it waits or runs (D157); a finished job keeps
    /// what it had.
    pub fn set_limits(
        &mut self,
        id: JobId,
        speed_limit: Option<u64>,
        priority: Option<JobPriority>,
    ) -> Result<Vec<OpsEvent>, QueueError> {
        let index = self.index(id)?;
        let snapshot = &mut self.jobs[index].snapshot;
        if snapshot.state.is_finished() {
            return Err(QueueError::Illegal {
                id,
                from: snapshot.state.name(),
                action: "change the limits of",
            });
        }
        let options = &mut snapshot.options;
        if options.speed_limit == speed_limit && options.priority == priority {
            return Ok(Vec::new());
        }
        options.speed_limit = speed_limit;
        options.priority = priority;
        Ok(vec![self.changed(index)])
    }

    /// Starts a queued job on a free slot.
    pub fn start(&mut self, id: JobId) -> Result<Vec<OpsEvent>, QueueError> {
        let index = self.index(id)?;
        if self.jobs[index].snapshot.state == JobState::Queued
            && self.slots_in_use() >= self.concurrency()
        {
            return Err(QueueError::NoSlot);
        }
        self.go(id, JobState::Running, "start")
    }

    pub fn pause(&mut self, id: JobId) -> Result<Vec<OpsEvent>, QueueError> {
        self.go(id, JobState::Paused, "pause")
    }

    pub fn resume(&mut self, id: JobId) -> Result<Vec<OpsEvent>, QueueError> {
        let index = self.index(id)?;
        if self.jobs[index].snapshot.state != JobState::Paused {
            let from = self.jobs[index].snapshot.state.name();
            return Err(QueueError::Illegal {
                id,
                from,
                action: "resume",
            });
        }
        self.go(id, JobState::Running, "resume")
    }

    /// A running job stops for the user (conflicts, or an error on one item).
    pub fn wait(&mut self, id: JobId, reason: WaitReason) -> Result<Vec<OpsEvent>, QueueError> {
        self.go(id, JobState::Waiting { reason }, "wait")
    }

    /// The user answered a job that waits on conflicts (A48). Each answer is kept on the job: one
    /// with a source settles that source's clash, one without is the policy for every later clash
    /// and is stored in the job's options. The conflicts still unanswered stay in the waiting
    /// state (and the job keeps waiting); once none are left the job runs again. An answer for a
    /// source that is not waiting is kept too, since a clash found later may be that source's.
    pub fn resolve(
        &mut self,
        id: JobId,
        answers: &[Resolution],
    ) -> Result<Vec<OpsEvent>, QueueError> {
        let index = self.index(id)?;
        let JobState::Waiting {
            reason: WaitReason::Conflicts { conflicts },
        } = self.jobs[index].snapshot.state.clone()
        else {
            let from = self.jobs[index].snapshot.state.name();
            return Err(QueueError::Illegal {
                id,
                from,
                action: "take conflict answers",
            });
        };
        let job = &mut self.jobs[index];
        for answer in answers {
            job.resolutions.apply(answer);
            if answer.source.is_none() {
                job.snapshot.options.conflict = Some(answer.policy);
            }
        }
        let remaining = job.resolutions.unresolved(&conflicts);
        if remaining.is_empty() {
            return self.go(id, JobState::Running, "take an answer");
        }
        if remaining.len() == conflicts.len() {
            return Ok(Vec::new());
        }
        job.snapshot.state = JobState::Waiting {
            reason: WaitReason::Conflicts {
                conflicts: remaining,
            },
        };
        Ok(vec![self.changed(index)])
    }

    /// The answers the user has given a job, for its executor.
    pub fn resolutions(&self, id: JobId) -> Option<&Resolutions> {
        self.find(id).map(|j| &j.resolutions)
    }

    /// Records what verification has checked so far. The worker calls it from the executor's report
    /// just before it ends the job.
    pub fn set_verified(
        &mut self,
        id: JobId,
        verified: Option<Verification>,
    ) -> Result<Vec<OpsEvent>, QueueError> {
        let index = self.index(id)?;
        if self.jobs[index].snapshot.verified == verified {
            return Ok(Vec::new());
        }
        self.jobs[index].snapshot.verified = verified;
        Ok(vec![self.changed(index)])
    }

    /// Records what the copies could not keep (A84). The worker calls it from the executor's report
    /// just before it ends the job.
    pub fn set_dropped(
        &mut self,
        id: JobId,
        dropped: Vec<DroppedDetail>,
    ) -> Result<Vec<OpsEvent>, QueueError> {
        let index = self.index(id)?;
        let dropped = (!dropped.is_empty()).then_some(dropped);
        if self.jobs[index].snapshot.dropped == dropped {
            return Ok(Vec::new());
        }
        self.jobs[index].snapshot.dropped = dropped;
        Ok(vec![self.changed(index)])
    }

    /// A server stopped answering: the running job waits for it and tries again by itself (D165).
    pub fn offline(
        &mut self,
        id: JobId,
        error: OpsError,
        item: Location,
        attempt: u32,
        retry_at_ms: i64,
    ) -> Result<Vec<OpsEvent>, QueueError> {
        self.go(
            id,
            JobState::Offline {
                error,
                item,
                attempt,
                retry_at_ms,
            },
            "wait for the connection",
        )
    }

    /// The offline wait is over: the job runs again (and tries the item once more).
    pub fn online(&mut self, id: JobId) -> Result<Vec<OpsEvent>, QueueError> {
        self.go(id, JobState::Running, "try again")
    }

    /// Records what a lost connection did with the file a job was writing (D165), or clears it.
    pub fn set_partial(
        &mut self,
        id: JobId,
        partial: Option<PartialNote>,
    ) -> Result<Vec<OpsEvent>, QueueError> {
        let index = self.index(id)?;
        if self.jobs[index].snapshot.partial == partial {
            return Ok(Vec::new());
        }
        self.jobs[index].snapshot.partial = partial;
        Ok(vec![self.changed(index)])
    }

    /// The user answered: a waiting job runs again.
    pub fn answered(&mut self, id: JobId) -> Result<Vec<OpsEvent>, QueueError> {
        let index = self.index(id)?;
        if !matches!(self.jobs[index].snapshot.state, JobState::Waiting { .. }) {
            let from = self.jobs[index].snapshot.state.name();
            return Err(QueueError::Illegal {
                id,
                from,
                action: "take an answer",
            });
        }
        self.go(id, JobState::Running, "take an answer")
    }

    /// Records progress. A report from a job that is not planning or running (a late one, after a
    /// pause or a cancel) is dropped, as is one the gate holds back.
    pub fn report(
        &mut self,
        id: JobId,
        progress: Progress,
        counts: Counts,
        force: bool,
    ) -> Result<Vec<OpsEvent>, QueueError> {
        let index = self.index(id)?;
        let now = self.clock.now_ms();
        let job = &mut self.jobs[index];
        if !matches!(job.snapshot.state, JobState::Planning | JobState::Running) {
            return Ok(Vec::new());
        }
        if job.snapshot.progress == progress && job.snapshot.counts == counts {
            return Ok(Vec::new());
        }
        if !job.gate.admit(now, progress.fraction(), force) {
            return Ok(Vec::new());
        }
        job.snapshot.progress = progress;
        job.snapshot.counts = counts;
        Ok(vec![self.changed(index)])
    }

    /// The job finished all its work.
    pub fn done(&mut self, id: JobId) -> Result<Vec<OpsEvent>, QueueError> {
        self.go(id, JobState::Done, "finish")
    }

    /// The job stopped on an error after completing `done` items.
    pub fn fail(
        &mut self,
        id: JobId,
        error: OpsError,
        item: Option<Location>,
        done: u64,
    ) -> Result<Vec<OpsEvent>, QueueError> {
        self.go(id, JobState::Failed { error, item, done }, "fail")
    }

    /// The job finished unwinding after a cancel.
    pub fn cancelled(&mut self, id: JobId) -> Result<Vec<OpsEvent>, QueueError> {
        self.go(id, JobState::Cancelled, "finish cancelling")
    }

    /// Asks a job to stop. One that is only planning or queued is cancelled at once; one that has
    /// started becomes `Cancelling` until its worker has unwound. Cancelling a job that is already
    /// cancelling changes nothing.
    pub fn cancel(&mut self, id: JobId) -> Result<Vec<OpsEvent>, QueueError> {
        let index = self.index(id)?;
        let state = self.jobs[index].snapshot.state.clone();
        match state {
            JobState::Cancelling => Ok(Vec::new()),
            JobState::Planning | JobState::Queued => {
                self.jobs[index].cancel.cancel();
                self.go(id, JobState::Cancelled, "cancel")
            }
            _ => {
                let events = self.go(id, JobState::Cancelling, "cancel")?;
                self.jobs[index].cancel.cancel();
                Ok(events)
            }
        }
    }

    /// Makes a new job from a failed or cancelled one's request. The old job stays until it is
    /// dismissed; ids are never reused.
    pub fn retry(&mut self, id: JobId) -> Result<(JobId, Vec<OpsEvent>), QueueError> {
        let index = self.index(id)?;
        let state = &self.jobs[index].snapshot.state;
        if !matches!(state, JobState::Failed { .. } | JobState::Cancelled) {
            return Err(QueueError::Illegal {
                id,
                from: state.name(),
                action: "retry",
            });
        }
        let request = self.jobs[index].request.clone();
        Ok(self.add(request))
    }

    /// Removes a finished job from the list.
    pub fn dismiss(&mut self, id: JobId) -> Result<Vec<OpsEvent>, QueueError> {
        let index = self.index(id)?;
        let state = &self.jobs[index].snapshot.state;
        if !state.is_finished() {
            return Err(QueueError::Illegal {
                id,
                from: state.name(),
                action: "dismiss",
            });
        }
        self.jobs.remove(index);
        let revision = self.bump();
        Ok(vec![OpsEvent::JobRemoved { id, revision }])
    }

    /// Removes every finished job.
    pub fn dismiss_finished(&mut self) -> Vec<OpsEvent> {
        let finished: Vec<JobId> = self
            .jobs
            .iter()
            .filter(|j| j.snapshot.state.is_finished())
            .map(|j| j.snapshot.id)
            .collect();
        finished
            .into_iter()
            .filter_map(|id| self.dismiss(id).ok())
            .flatten()
            .collect()
    }

    /// Moves a queued job to `to` among the queued jobs (0 is the next to run, past the end is the
    /// last). The other jobs keep their places.
    pub fn reorder(&mut self, id: JobId, to: usize) -> Result<Vec<OpsEvent>, QueueError> {
        let index = self.index(id)?;
        let state = &self.jobs[index].snapshot.state;
        if *state != JobState::Queued {
            return Err(QueueError::Illegal {
                id,
                from: state.name(),
                action: "reorder",
            });
        }
        let slots: Vec<usize> = (0..self.jobs.len())
            .filter(|&i| self.jobs[i].snapshot.state == JobState::Queued)
            .collect();
        let from = slots
            .iter()
            .position(|&i| i == index)
            .expect("the job is queued");
        let to = to.min(slots.len() - 1);
        if from == to {
            return Ok(Vec::new());
        }
        let mut queued: Vec<Job> = Vec::new();
        for &i in slots.iter().rev() {
            queued.push(self.jobs.remove(i));
        }
        queued.reverse();
        let moved = queued.remove(from);
        queued.insert(to, moved);
        for (slot, job) in slots.iter().zip(queued) {
            self.jobs.insert(*slot, job);
        }
        let revision = self.bump();
        Ok(vec![OpsEvent::QueueReordered {
            order: self.jobs.iter().map(|j| j.snapshot.id).collect(),
            revision,
        }])
    }

    /// The jobs that have not finished and read from or write into `location`, or remove or move
    /// something it lies inside: what the close guard (D29) warns about before closing a tab or a
    /// window showing it.
    pub fn jobs_targeting(&self, location: &Location) -> Vec<JobId> {
        self.jobs
            .iter()
            .filter(|j| !j.snapshot.state.is_finished())
            .filter(|j| {
                j.touches.iter().any(|t| t.uri == location.uri)
                    || j.trees.iter().any(|t| inside(&location.uri, &t.uri))
            })
            .map(|j| j.snapshot.id)
            .collect()
    }

    /// Everything wrong with the store, as readable lines; empty when every invariant holds.
    pub fn violations(&self) -> Vec<String> {
        let mut out = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for job in &self.jobs {
            let s = &job.snapshot;
            if !seen.insert(s.id) {
                out.push(format!("job {} appears twice", s.id.0));
            }
            if s.id.0 >= self.next_id {
                out.push(format!("job {} is at or above next_id", s.id.0));
            }
            if s.state.is_finished() != s.finished_ms.is_some() {
                out.push(format!("job {} has the wrong finished time", s.id.0));
            }
            let started = matches!(
                s.state,
                JobState::Running
                    | JobState::Paused
                    | JobState::Waiting { .. }
                    | JobState::Cancelling
            );
            if started && s.started_ms.is_none() {
                out.push(format!("job {} runs without a start time", s.id.0));
            }
            if matches!(s.state, JobState::Planning | JobState::Queued) && s.started_ms.is_some() {
                out.push(format!("job {} has a start time before it started", s.id.0));
            }
        }
        out
    }
}

/// Whether the URI `child` is `tree` or lies below it.
fn inside(child: &str, tree: &str) -> bool {
    child == tree
        || child
            .strip_prefix(tree)
            .is_some_and(|rest| tree.ends_with('/') || rest.starts_with('/'))
}

/// What a job's first source is called in a title: the last segment of its path, or for an item in
/// the Trash the name it was trashed under (its id is `{trash folder}|{name}`, which no person reads).
fn source_name(location: &Location) -> String {
    if let Ok(TrashPath::Item(id)) = TrashPath::from_uri(&location.uri) {
        if let Some((_, name)) = id.rsplit_once('|') {
            return name.to_owned();
        }
    }
    last_segment(&location.display)
}

fn last_segment(display: &str) -> String {
    display
        .trim_end_matches(['/', '\\'])
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(display)
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_trashed_source_is_named_by_the_name_it_was_trashed_under() {
        let item = |id: &str| {
            let path = TrashPath::Item(id.to_owned());
            Location {
                display: path.display(),
                uri: path.to_uri(),
            }
        };
        assert_eq!(
            source_name(&item("/home/a/.local/share/Trash|report (2).txt")),
            "report (2).txt"
        );
        let plain = Location {
            display: "/home/a/notes.txt".to_owned(),
            uri: "file:///home/a/notes.txt".to_owned(),
        };
        assert_eq!(source_name(&plain), "notes.txt");
    }

    #[test]
    fn the_gate_passes_the_first_report_and_then_waits_for_time_and_progress() {
        let mut gate = ProgressGate::new();
        assert!(gate.admit(0, 0.0, false));
        // Too soon, however far it moved.
        assert!(!gate.admit(50, 0.5, false));
        // Long enough, but under a percent.
        assert!(!gate.admit(500, 0.005, false));
        // Both.
        assert!(gate.admit(500, 0.02, false));
        // The limits count from the report that passed, not the one that was held back.
        assert!(!gate.admit(550, 0.5, false));
        assert!(gate.admit(600, 0.5, false));
    }

    #[test]
    fn completion_and_force_always_pass() {
        let mut gate = ProgressGate::new();
        assert!(gate.admit(0, 0.0, false));
        assert!(gate.admit(1, 1.0, false));
        assert!(!gate.admit(2, 1.0, false));
        assert!(gate.admit(3, 0.2, true));
    }

    #[test]
    fn the_gate_bounds_the_event_rate() {
        // One report a millisecond for ten seconds, over a job that moves a hundredth of a
        // percent each time: at most ten events a second get through.
        let mut gate = ProgressGate::new();
        let passed = (0..10_000)
            .filter(|&t| gate.admit(t, t as f64 / 10_000.0, false))
            .count();
        assert!(passed <= 101, "{passed}");
        assert!(passed >= 50, "{passed}");
    }

    #[test]
    fn the_table_lists_every_legal_move_and_no_other() {
        use JobState::*;
        let failed = || Failed {
            error: OpsError::Cancelled,
            item: None,
            done: 0,
        };
        let waiting = || Waiting {
            reason: WaitReason::Conflicts { conflicts: vec![] },
        };
        let states = [
            Planning,
            Queued,
            Running,
            Paused,
            waiting(),
            Cancelling,
            Cancelled,
            Done,
            failed(),
        ];
        let legal: [(&str, &[&str]); 9] = [
            ("planning", &["queued", "failed", "cancelled"]),
            ("queued", &["running", "cancelled"]),
            (
                "running",
                &["paused", "waiting", "done", "failed", "cancelling"],
            ),
            ("paused", &["running", "cancelling"]),
            ("waiting", &["running", "cancelling"]),
            ("cancelling", &["cancelled", "done", "failed"]),
            ("cancelled", &[]),
            ("done", &[]),
            ("failed", &[]),
        ];
        for from in &states {
            for to in &states {
                let expected = legal
                    .iter()
                    .find(|(name, _)| *name == from.name())
                    .is_some_and(|(_, targets)| targets.contains(&to.name()));
                assert_eq!(
                    is_legal(from, to),
                    expected,
                    "{} -> {}",
                    from.name(),
                    to.name()
                );
            }
        }
    }
}
