// A complete engine over one provider, for tests: a sandbox, fault injection, a fake Trash, a store
// and an executor, with a driver that runs a request the way the plugin's worker will.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};

use waypoint_path::VfsPath;
use waypoint_protocol::Location;
use waypoint_vfs::{CancelToken, ListingHandle, Provider, SelectionSpec};

use crate::exec::{ExecEnv, ExecFailure, ExecReport, ExecSink, Executor, SimpleCopy};
use crate::model::{
    Counts, Decision, JobId, JobKind, JobOptions, JobRequest, JobState, OpsError, OpsEvent,
    OpsSettings, OpsSnapshot, Progress, Sources,
};
use crate::plan::{plan, Plan, PlanCtx};
use crate::queue::OpsStore;
use crate::testing::faulty::FaultyProvider;
use crate::testing::sandbox::SandboxProvider;
use crate::testing::trash::FakeTrash;
use crate::traits::{Clock, CounterIds, Protected, Providers, SelectionResolver, StaticSettings};

/// A clock a test moves by hand.
#[derive(Debug, Default)]
pub struct ManualClock(AtomicI64);

impl ManualClock {
    pub fn new(start_ms: i64) -> Self {
        Self(AtomicI64::new(start_ms))
    }

    pub fn advance(&self, ms: i64) {
        self.0.fetch_add(ms, Ordering::Relaxed);
    }
}

impl Clock for ManualClock {
    fn now_ms(&self) -> i64 {
        self.0.load(Ordering::Relaxed)
    }
}

/// Resolves every selection to the same locations.
#[derive(Debug, Default)]
pub struct FixedResolver(pub Mutex<Vec<Location>>);

impl SelectionResolver for FixedResolver {
    fn resolve(
        &self,
        _handle: ListingHandle,
        _spec: &SelectionSpec,
        _window: &str,
    ) -> Result<Vec<Location>, OpsError> {
        Ok(self.0.lock().unwrap().clone())
    }
}

/// What a driven job came to.
#[derive(Debug)]
pub struct RunResult {
    pub id: JobId,
    pub state: JobState,
    /// The report of a job that finished.
    pub report: Option<ExecReport>,
    /// What stopped a job that did not.
    pub failure: Option<Box<ExecFailure>>,
    /// The plan, when planning succeeded.
    pub plan: Option<Plan>,
}

struct StoreSink<'a> {
    store: &'a mut OpsStore,
    id: JobId,
    events: &'a mut Vec<OpsEvent>,
}

impl ExecSink for StoreSink<'_> {
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

/// The engine over one wrapped provider. The provider is `Faulty(Sandbox(P))`, so every call is
/// counted and can fail, and none can leave `base`.
pub struct Harness<P: Provider + 'static> {
    /// The sandbox root. Holds `work` and the Trash.
    pub base: VfsPath,
    /// Where tests make their trees.
    pub work: VfsPath,
    pub provider: Arc<FaultyProvider<SandboxProvider<P>>>,
    pub trash: Arc<FakeTrash>,
    pub clock: Arc<ManualClock>,
    pub resolver: Arc<FixedResolver>,
    pub protected: Protected,
    pub env: ExecEnv,
    pub store: OpsStore,
    /// Every event the store has made, in order.
    pub events: Vec<OpsEvent>,
}

impl<P: Provider + 'static> Harness<P> {
    /// `inner` serves paths below `base`, which must exist. The harness makes `base/work` and the
    /// Trash folder `base/trash`.
    pub fn new(inner: P, base: VfsPath) -> Self {
        Self::with_settings(inner, base, OpsSettings::default())
    }

    pub fn with_settings(inner: P, base: VfsPath, settings: OpsSettings) -> Self {
        let sandbox = SandboxProvider::new(inner, &base);
        let provider = Arc::new(FaultyProvider::new(sandbox));
        let work = base.join("work").unwrap();
        provider.create_dir(&work).expect("the work folder is made");
        let clock = Arc::new(ManualClock::new(1_700_000_000_000));
        let ids = Arc::new(CounterIds::default());
        let trash = Arc::new(FakeTrash::new(
            provider.clone(),
            base.join("trash").unwrap(),
            ids.clone(),
            clock.clone(),
        ));
        let protected = Protected::new(vec![base.clone()]);
        let env = ExecEnv {
            providers: Providers::single(provider.clone()),
            trash: trash.clone(),
            protected: protected.clone(),
            ids,
            copier: Arc::new(SimpleCopy),
        };
        let store = OpsStore::new(Arc::new(StaticSettings(settings)), clock.clone());
        provider.reset();
        Self {
            base,
            work,
            provider,
            trash,
            clock,
            resolver: Arc::new(FixedResolver::default()),
            protected,
            env,
            store,
            events: Vec::new(),
        }
    }

    /// The path of `relative` (segments joined with `/`) below the work folder.
    pub fn path(&self, relative: &str) -> VfsPath {
        relative
            .split('/')
            .filter(|s| !s.is_empty())
            .fold(self.work.clone(), |p, name| p.join(name).unwrap())
    }

    pub fn loc(&self, relative: &str) -> Location {
        self.path(relative).to_location()
    }

    /// A request over explicit locations below the work folder.
    pub fn request(
        &self,
        kind: JobKind,
        sources: &[&str],
        destination: Option<&str>,
        name: Option<&str>,
    ) -> JobRequest {
        JobRequest {
            kind,
            sources: Sources::Locations {
                locations: sources.iter().map(|s| self.loc(s)).collect(),
            },
            destination: destination.map(|d| self.loc(d)),
            name: name.map(str::to_owned),
            options: JobOptions::default(),
            origin_window: "main-1".to_owned(),
            rename: None,
            archive: None,
        }
    }

    pub fn plan_ctx<'a>(&'a self, cancel: &'a CancelToken) -> PlanCtx<'a> {
        PlanCtx {
            providers: &self.env.providers,
            resolver: self.resolver.as_ref(),
            trash: self.trash.as_ref(),
            protected: &self.protected,
            cancel,
        }
    }

    /// Plans a request without running it.
    pub fn plan(&self, request: &JobRequest) -> Result<Plan, OpsError> {
        let cancel = CancelToken::new();
        plan(request, &self.plan_ctx(&cancel))
    }

    /// Runs a request through the store and the executor, start to end.
    pub fn run(&mut self, request: JobRequest) -> RunResult {
        self.run_hooked(request, &mut |_, _| {})
    }

    /// Like `run`, with a hook called once the job is added and before it is planned, given the
    /// job's cancel token (to script a cancel through the faulty provider, counted from the first
    /// call of the planner).
    pub fn run_hooked(
        &mut self,
        request: JobRequest,
        hook: &mut dyn FnMut(&Self, &CancelToken),
    ) -> RunResult {
        let (id, events) = self.store.add(request.clone());
        self.events.extend(events);
        let token = self.store.cancel_token(id).expect("the job was added");
        hook(self, &token);
        let planned = {
            let ctx = self.plan_ctx(&token);
            plan(&request, &ctx)
        };
        let planned = match planned {
            Ok(planned) => planned,
            Err(error) => {
                if error == OpsError::Cancelled {
                    let events = self.store.cancel(id).expect("a planning job cancels");
                    self.events.extend(events);
                }
                let events = self.store.plan_failed(id, error).expect("planning fails");
                self.events.extend(events);
                return self.result(id, None, None, None);
            }
        };
        let events = self
            .store
            .plan_done(id, planned.totals())
            .expect("planning finishes");
        self.events.extend(events);
        assert_eq!(self.store.next_runnable(), Some(id), "the job is next");
        let events = self.store.start(id).expect("a slot is free");
        self.events.extend(events);
        let executor = Executor::new(self.env.clone());
        let outcome = {
            let mut sink = StoreSink {
                store: &mut self.store,
                id,
                events: &mut self.events,
            };
            executor.run(id, &planned, &token, &mut sink)
        };
        match outcome {
            Ok(report) => {
                let events = self.store.done(id).expect("a running job finishes");
                self.events.extend(events);
                self.result(id, Some(report), None, Some(planned))
            }
            Err(failure) if failure.error == OpsError::Cancelled => {
                let events = self.store.cancel(id).expect("a running job cancels");
                self.events.extend(events);
                let events = self.store.cancelled(id).expect("a cancelling job ends");
                self.events.extend(events);
                self.result(id, None, Some(failure), Some(planned))
            }
            Err(failure) => {
                let events = self
                    .store
                    .fail(
                        id,
                        failure.error.clone(),
                        failure.item.clone(),
                        failure.done,
                    )
                    .expect("a running job fails");
                self.events.extend(events);
                self.result(id, None, Some(failure), Some(planned))
            }
        }
    }

    fn result(
        &self,
        id: JobId,
        report: Option<ExecReport>,
        failure: Option<Box<ExecFailure>>,
        plan: Option<Plan>,
    ) -> RunResult {
        RunResult {
            id,
            state: self.store.job(id).expect("the job is listed").state.clone(),
            report,
            failure,
            plan,
        }
    }

    /// The events so far replayed from nothing.
    pub fn replayed(&self) -> OpsSnapshot {
        let mut snapshot = OpsSnapshot::default();
        for event in &self.events {
            snapshot.apply(event);
        }
        snapshot
    }
}
