// The harness with a journal on it: runs jobs the way the plugin's worker will with the journal in
// play (write-ahead record, executor, fingerprints, commit), runs undo and redo as queue jobs, and
// restarts the journal from its storage to test recovery.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::ops::{Deref, DerefMut};
use std::sync::Arc;

use waypoint_path::VfsPath;
use waypoint_protocol::Location;
use waypoint_vfs::{CancelToken, Provider};

use crate::exec::{ExecFailure, ExecReport, ExecSink, Executor, RunOptions, CHUNK_BYTES};
use crate::journal::{
    fingerprint_steps, prepare, Journal, JournalDeps, JournalId, PendingRecord, Prepared, Recorded,
    RecoveryReport, UndoFailure, UndoReport,
};
use crate::model::{
    Counts, Decision, JobId, JobKind, JobRequest, JobState, OpsError, OpsEvent, OpsSettings,
    Progress,
};
use crate::plan::Plan;
use crate::testing::harness::Harness;
use crate::testing::journal_storage::{CountingSaver, MemoryJournalStorage};
use crate::traits::{Clock, SettingsReader, StaticSettings};

/// What a journalled run came to.
#[derive(Debug)]
pub struct JournalRun {
    pub id: JobId,
    pub state: JobState,
    /// The entry the run made (an ordinary job) or finished (a redo that did the whole of it).
    pub entry: Option<JournalId>,
    pub report: Option<ExecReport>,
    pub failure: Option<Box<ExecFailure>>,
    pub undo: Option<Result<UndoReport, Box<UndoFailure>>>,
    pub plan: Option<Plan>,
    /// The process "died" during the run (`FaultyProvider::crash_at`): the journal was left as it
    /// was, with the write-ahead record in it.
    pub crashed: bool,
    /// How many provider calls had been made when the executor finished, before the fingerprints
    /// were read: a fault scripted at a call up to this one lands in the job itself.
    pub exec_calls: usize,
}

struct Sink<'a> {
    store: &'a mut crate::queue::OpsStore,
    id: JobId,
    events: &'a mut Vec<OpsEvent>,
}

impl ExecSink for Sink<'_> {
    fn progress(&mut self, progress: &Progress, counts: &Counts) {
        let events = self
            .store
            .report(self.id, progress.clone(), *counts, false)
            .expect("the job is known");
        self.events.extend(events);
    }

    fn on_error(&mut self, _item: &Location, _error: &OpsError) -> Option<Decision> {
        None
    }
}

/// A `Harness` plus a journal over in-memory storage.
pub struct JournalHarness<P: Provider + 'static> {
    pub harness: Harness<P>,
    pub journal: Journal,
    pub storage: Arc<MemoryJournalStorage>,
    pub saver: Arc<CountingSaver>,
    pub settings: Arc<dyn SettingsReader>,
    /// Every journal event, in order.
    pub journal_events: Vec<OpsEvent>,
    /// The size of one copy chunk, which tests make small to cross many boundaries.
    pub chunk_bytes: usize,
}

impl<P: Provider + 'static> Deref for JournalHarness<P> {
    type Target = Harness<P>;
    fn deref(&self) -> &Harness<P> {
        &self.harness
    }
}

impl<P: Provider + 'static> DerefMut for JournalHarness<P> {
    fn deref_mut(&mut self) -> &mut Harness<P> {
        &mut self.harness
    }
}

impl<P: Provider + 'static> JournalHarness<P> {
    pub fn new(inner: P, base: VfsPath) -> Self {
        Self::with_settings(inner, base, OpsSettings::default())
    }

    pub fn with_settings(inner: P, base: VfsPath, settings: OpsSettings) -> Self {
        let harness = Harness::with_settings(inner, base, settings);
        let storage = Arc::new(MemoryJournalStorage::new());
        let saver = Arc::new(CountingSaver::default());
        let settings: Arc<dyn SettingsReader> = Arc::new(StaticSettings(settings));
        let journal = Journal::new(JournalDeps {
            settings: settings.clone(),
            clock: harness.clock.clone() as Arc<dyn Clock>,
            storage: storage.clone(),
            saver: saver.clone(),
        });
        Self {
            harness,
            journal,
            storage,
            saver,
            settings,
            journal_events: Vec::new(),
            chunk_bytes: CHUNK_BYTES,
        }
    }

    fn deps(&self) -> JournalDeps {
        JournalDeps {
            settings: self.settings.clone(),
            clock: self.harness.clock.clone() as Arc<dyn Clock>,
            storage: self.storage.clone(),
            saver: self.saver.clone(),
        }
    }

    /// Stops and starts again: the provider's scripts are forgotten, the storage is seen by a new
    /// run, and a new journal opens over it with recovery. Whatever the old journal had not saved
    /// is lost, as in a crash.
    pub fn restart(&mut self) -> RecoveryReport {
        self.harness.provider.reset();
        self.storage = Arc::new(self.storage.reopen());
        let (journal, report) = Journal::open(self.deps(), &self.harness.env.providers);
        self.journal = journal;
        report
    }

    /// Runs a request with the journal.
    pub fn run_journalled(&mut self, request: JobRequest) -> JournalRun {
        self.run_journalled_hooked(request, &mut |_, _| {})
    }

    /// Runs the request of the undo of `id`, or of its redo.
    pub fn undo(&mut self, id: JournalId) -> JournalRun {
        let request = self
            .journal
            .undo_request(id, "main-1")
            .expect("the entry can be undone");
        self.run_journalled(request)
    }

    pub fn undo_last(&mut self) -> JournalRun {
        let request = self
            .journal
            .undo_last_request("main-1")
            .expect("there is something to undo");
        self.run_journalled(request)
    }

    pub fn redo(&mut self, id: JournalId) -> JournalRun {
        let request = self
            .journal
            .redo_request(id, "main-1")
            .expect("the entry can be redone");
        self.run_journalled(request)
    }

    pub fn redo_last(&mut self) -> JournalRun {
        let request = self
            .journal
            .redo_last_request("main-1")
            .expect("there is something to redo");
        self.run_journalled(request)
    }

    /// Like `run_journalled`, with a hook called once the job is added and before it is planned
    /// (to script faults counted from the planner's first call, or a crash).
    pub fn run_journalled_hooked(
        &mut self,
        request: JobRequest,
        hook: &mut dyn FnMut(&Harness<P>, &CancelToken),
    ) -> JournalRun {
        let h = &mut self.harness;
        let (id, events) = h.store.add(request.clone());
        h.events.extend(events);
        let token = h.store.cancel_token(id).expect("the job was added");
        hook(h, &token);

        let prepared = {
            let ctx = h.plan_ctx(&token);
            prepare(&request, &ctx, &self.journal, &mut |_| {})
        };
        let prepared = match prepared {
            Ok(prepared) => prepared,
            Err(error) => {
                if error == OpsError::Cancelled {
                    let events = h.store.cancel(id).expect("a planning job cancels");
                    h.events.extend(events);
                }
                let events = h.store.plan_failed(id, error).expect("planning fails");
                h.events.extend(events);
                return JournalRun {
                    id,
                    state: h.store.job(id).unwrap().state.clone(),
                    entry: None,
                    report: None,
                    failure: None,
                    undo: None,
                    plan: None,
                    crashed: h.provider.is_crashed(),
                    exec_calls: 0,
                };
            }
        };
        let plan = match &prepared {
            Prepared::Plain(plan) => plan.clone(),
            Prepared::Undo(undo) => undo.plan.clone(),
            Prepared::Redo(redo) => redo.plan.clone(),
        };
        let events = h
            .store
            .plan_done(id, plan.totals())
            .expect("planning finishes");
        h.events.extend(events);
        let events = h.store.start(id).expect("a slot is free");
        h.events.extend(events);

        let now = h.clock.now_ms();
        let record = match &prepared {
            Prepared::Plain(plan) => PendingRecord::for_plan(id, now, &request, plan),
            Prepared::Redo(redo) => {
                let mut record = PendingRecord::for_plan(id, now, &redo.forward, &redo.plan);
                record.kind = request.kind;
                record
            }
            Prepared::Undo(undo) => {
                let entry = self.journal.entry(undo.entry).expect("the entry is there");
                PendingRecord::for_undo(id, now, entry, &undo.steps)
            }
        };
        self.journal.begin(record).expect("the record is stored");

        let executor = Executor::new(h.env.clone());
        let mut run = JournalRun {
            id,
            state: JobState::Running,
            entry: None,
            report: None,
            failure: None,
            undo: None,
            plan: Some(plan.clone()),
            crashed: false,
            exec_calls: 0,
        };
        let mut journal_events: Vec<OpsEvent> = Vec::new();
        match &prepared {
            Prepared::Undo(undo) => {
                let outcome = {
                    let mut sink = Sink {
                        store: &mut h.store,
                        id,
                        events: &mut h.events,
                    };
                    executor.run_undo(id, &undo.steps, &token, &mut sink)
                };
                run.crashed = h.provider.is_crashed();
                let applied = match &outcome {
                    Ok(done) => done.applied,
                    Err(failure) => failure.applied,
                };
                if !run.crashed {
                    journal_events.extend(self.journal.finish_undo(id, undo.entry, applied));
                }
                let events = match &outcome {
                    Ok(_) => h.store.done(id),
                    Err(failure) if failure.error == OpsError::Cancelled => {
                        let mut events = h.store.cancel(id).expect("a running job cancels");
                        events.extend(h.store.cancelled(id).expect("a cancelling job ends"));
                        Ok(events)
                    }
                    Err(failure) => h.store.fail(
                        id,
                        failure.error.clone(),
                        failure.item.clone(),
                        failure.applied as u64,
                    ),
                };
                h.events.extend(events.expect("the job ends"));
                run.undo = Some(outcome);
            }
            Prepared::Plain(_) | Prepared::Redo(_) => {
                let options = {
                    let job = h.store.job(id).expect("the job is listed");
                    let mut resolutions =
                        h.store.resolutions(id).expect("the job is known").clone();
                    let mut job_options = job.options;
                    if let Prepared::Redo(redo) = &prepared {
                        // A redo runs what the job did, with the policy and verification it had.
                        if let (None, Some(policy)) =
                            (resolutions.all(), redo.forward.options.conflict)
                        {
                            resolutions.set_all(policy);
                        }
                        job_options = redo.forward.options;
                    }
                    let mut options = RunOptions::for_job(
                        &job_options,
                        &self.settings.ops_settings(),
                        resolutions,
                    );
                    options.chunk_bytes = self.chunk_bytes;
                    options.clock = h.clock.clone() as Arc<dyn Clock>;
                    options
                };
                let outcome = {
                    let mut sink = Sink {
                        store: &mut h.store,
                        id,
                        events: &mut h.events,
                    };
                    executor.run_with(id, &plan, &token, &mut sink, options)
                };
                run.crashed = h.provider.is_crashed();
                run.exec_calls = h.provider.calls();
                let (mut report, failed) = match outcome {
                    Ok(report) => (report, None),
                    Err(failure) => (failure.report.clone(), Some(failure)),
                };
                if !run.crashed {
                    fingerprint_steps(&h.env.providers, &mut report.inverse);
                    run.crashed = h.provider.is_crashed();
                }
                if !run.crashed {
                    match &prepared {
                        Prepared::Redo(redo) if failed.is_none() => {
                            journal_events.extend(self.journal.finish_redo(
                                id,
                                redo.entry,
                                report.inverse.clone(),
                            ));
                            run.entry = Some(redo.entry);
                        }
                        Prepared::Redo(redo) => {
                            let recorded = Recorded::from_run(&redo.forward, Some(&plan), &report);
                            match recorded {
                                Some(recorded) => {
                                    let (entry, events) =
                                        self.journal.finish_redo_partly(id, recorded);
                                    run.entry = entry;
                                    journal_events.extend(events);
                                }
                                None => self.journal.abort(id),
                            }
                        }
                        _ => match Recorded::from_run(&request, Some(&plan), &report) {
                            Some(recorded) => {
                                let (entry, events) = self.journal.commit(id, recorded);
                                run.entry = entry;
                                journal_events.extend(events);
                            }
                            None => self.journal.abort(id),
                        },
                    }
                }
                let events = match failed {
                    None => h.store.done(id),
                    Some(failure) if failure.error == OpsError::Cancelled => {
                        let mut events = h.store.cancel(id).expect("a running job cancels");
                        events.extend(h.store.cancelled(id).expect("a cancelling job ends"));
                        run.failure = Some(failure);
                        Ok(events)
                    }
                    Some(failure) => {
                        let events = h.store.fail(
                            id,
                            failure.error.clone(),
                            failure.item.clone(),
                            failure.done,
                        );
                        run.failure = Some(failure);
                        events
                    }
                };
                h.events.extend(events.expect("the job ends"));
                if run.failure.is_none() {
                    run.report = Some(report);
                }
            }
        }
        if run.entry.is_some() && !matches!(request.kind, JobKind::Redo { .. }) {
            let events = h.store.mark_undoable(id, true).expect("the job is listed");
            h.events.extend(events);
        }
        self.journal_events.extend(journal_events);
        run.state = self.harness.store.job(id).unwrap().state.clone();
        run
    }
}
