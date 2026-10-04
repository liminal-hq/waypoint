// The operations state held in Tauri state: the queue and the undo journal behind one lock, the
// shared clipboard, the progress channels, and the worker pool that plans and runs the jobs.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// One `Mutex<Core>` holds every piece of state, so a command, a worker and the journal's debounce
// thread each see one consistent picture and the events of a change are sent in revision order, to
// every window, before the lock is released. A worker never holds the lock while it touches the
// file system: it takes what it needs, unlocks, works, and locks again to report. A job that waits
// for the user (conflicts, an error) or is paused parks its worker on `Shared::cv` and holds its
// slot, as `OpsStore` expects (A46).

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

use tauri::ipc::Channel;
use tauri::{AppHandle, Emitter, Runtime};
use waypoint_ops::{
    plan, preview_batch, BatchPreview, Bucket, Clock, Decision, ExecEnv, IdSource, JobId, JobKind,
    JobPriority, JobRequest, JobSnapshot, JobState, Journal, JournalDeps, JournalDocument,
    JournalEntrySummary, JournalId, JournalStorage, Loaded, OpsError, OpsEvent, OpsSettings,
    OpsSnapshot, OpsStore, Pacer, PlanCtx, PlanWarning, Prepared, QueueError, RateCell,
    RecoveryReport, Resolution, SaveRequest, Schedule, ScheduledRecord, SelectionResolver,
    SettingsReader, SimpleCopy, Sources, StorageError, SystemPacer, Throttle,
};
use waypoint_protocol::Location;
use waypoint_vfs::{CancelToken, ListingHandle, SelectionSpec};

use crate::deps::{ChangeHook, OpsDeps, SettingsStorage};
use crate::models::{
    Clipboard, ClipboardMode, ClipboardSource, Error, JobProgress, PlanNote, PlanPreview,
};
use crate::worker;
use crate::{CLIPBOARD_EVENT, EVENT, RECOVERED_EVENT};

/// The most workers the settings can ask for.
pub const MAX_CONCURRENCY: u32 = 16;

/// The most days the Trash sweep can be set to wait (a hundred years: far more than anyone wants,
/// and low enough that no day count overflows).
pub const MAX_TRASH_EXPIRY_DAYS: u32 = 36_500;

/// The fastest limit the settings and a job can ask for: a terabyte a second, which is no limit
/// anyone means and keeps the arithmetic far from overflowing.
pub const MAX_SPEED_LIMIT: u64 = 1_000_000_000_000;

/// The most journal entries the settings can ask to keep.
pub const MAX_UNDO_DEPTH: u32 = 500;

/// The ranges an extraction's limits may be set within (D155): a limit below the lower end would
/// refuse ordinary archives, and one above the upper end would be no limit at all.
pub const ARCHIVE_ENTRIES_RANGE: std::ops::RangeInclusive<u64> = 1_000..=100_000_000;
pub const ARCHIVE_BYTES_RANGE: std::ops::RangeInclusive<u64> = GIB..=1024 * GIB;
pub const ARCHIVE_RATIO_RANGE: std::ops::RangeInclusive<u32> = 10..=100_000;
pub const ARCHIVE_RATIO_FLOOR_RANGE: std::ops::RangeInclusive<u64> = MIB..=1024 * GIB;
const MIB: u64 = 1024 * 1024;
const GIB: u64 = 1024 * MIB;

pub(crate) fn locked<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    // A poisoned lock only means a thread panicked; the state is still consistent.
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

/// Numbers for the names of partial files that differ from run to run: they start from the clock,
/// so one a crashed run left behind cannot be mistaken for the next run's.
struct StartedIds(AtomicU64);

impl IdSource for StartedIds {
    fn next(&self) -> u64 {
        self.0.fetch_add(1, Ordering::Relaxed)
    }
}

/// The settings the queue and the journal read on every use, so a change reaches the next job.
pub(crate) struct SettingsCell(Mutex<OpsSettings>);

impl SettingsCell {
    pub(crate) fn get(&self) -> OpsSettings {
        *locked(&self.0)
    }

    fn set(&self, settings: OpsSettings) {
        *locked(&self.0) = settings;
    }
}

impl SettingsReader for SettingsCell {
    fn ops_settings(&self) -> OpsSettings {
        self.get()
    }
}

/// The journal's storage with the writes put in order. The journal saves while the core lock is
/// held, which is right for the write-ahead record (it must be on disk before the job's first
/// write) but not for a flush on a window closing, which can wait on a slow disk. A flush takes a
/// copy of the document and a ticket under the lock, and writes with the lock released; every write
/// goes through here, which keeps them one at a time, drops one whose ticket is older than what was
/// written (so the last state wins), and skips a document that is already what the file holds.
struct OrderedStorage {
    inner: Arc<dyn JournalStorage>,
    /// Counts tickets; a ticket is taken under the core lock, so a larger one is a later state.
    tickets: AtomicU64,
    /// The newest ticket written, and the document it wrote.
    written: Mutex<(u64, Option<JournalDocument>)>,
}

impl OrderedStorage {
    fn new(inner: Arc<dyn JournalStorage>) -> Self {
        Self {
            inner,
            tickets: AtomicU64::new(0),
            written: Mutex::new((0, None)),
        }
    }

    /// The ticket for a document copied just now, with the core lock held.
    fn ticket(&self) -> u64 {
        self.tickets.fetch_add(1, Ordering::AcqRel) + 1
    }

    fn write(&self, ticket: u64, document: &JournalDocument) -> Result<(), StorageError> {
        let mut written = locked(&self.written);
        if ticket < written.0 || written.1.as_ref() == Some(document) {
            return Ok(());
        }
        self.inner.save(document)?;
        *written = (ticket, Some(document.clone()));
        Ok(())
    }
}

impl JournalStorage for OrderedStorage {
    fn load(&self) -> Result<Loaded, StorageError> {
        self.inner.load()
    }

    /// The journal's own saves, made under the core lock: the newest state there is.
    fn save(&self, document: &JournalDocument) -> Result<(), StorageError> {
        self.write(self.ticket(), document)
    }

    fn set_aside(&self) -> Option<String> {
        self.inner.set_aside()
    }
}

type FlushHook = OnceLock<Box<dyn Fn() + Send + Sync>>;

/// Asks for the journal to be written a moment after a change, once per burst. It runs inside
/// journal calls that hold the core lock, so it only notes the request and starts a timer thread
/// that takes the lock later.
struct Debouncer {
    pending: Arc<AtomicBool>,
    delay: Duration,
    flush: Arc<FlushHook>,
}

impl SaveRequest for Debouncer {
    fn save_requested(&self) {
        if self.pending.swap(true, Ordering::AcqRel) {
            return;
        }
        let (pending, flush, delay) = (self.pending.clone(), self.flush.clone(), self.delay);
        let spawned = std::thread::Builder::new()
            .name("waypoint-ops-save".into())
            .spawn(move || {
                std::thread::sleep(delay);
                // Cleared first: a change made while the write runs asks again.
                pending.store(false, Ordering::Release);
                if let Some(flush) = flush.get() {
                    flush();
                }
            });
        if spawned.is_err() {
            self.pending.store(false, Ordering::Release);
        }
    }
}

/// Which step of its life a job's worker bookkeeping is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Stage {
    /// Added, and no worker has taken it to plan yet.
    NeedsPlan,
    /// A worker is planning it.
    Planning,
    /// Planned and queued, with the plan waiting in `Ctl::prepared`.
    Planned,
    /// A worker runs it (it may be parked).
    Running,
}

/// A job's side of the bookkeeping the queue does not hold.
pub(crate) struct Ctl {
    pub stage: Stage,
    pub prepared: Option<Prepared>,
    /// The answers the user gave to the conflict the worker waits on.
    pub answers: Vec<Resolution>,
    /// The answer to the error the worker waits on.
    pub decision: Option<Decision>,
    /// The job's own speed limit, which the running copy reads at every piece.
    pub rate: Arc<RateCell>,
}

impl Ctl {
    fn new(limit: Option<u64>) -> Self {
        Self {
            rate: Arc::new(RateCell::new(limit)),
            stage: Stage::NeedsPlan,
            prepared: None,
            answers: Vec::new(),
            decision: None,
        }
    }
}

struct Subscriber {
    window: String,
    /// What `subscribe_progress` returned, so a stop that arrives late can tell it is out of date.
    token: u64,
    channel: Channel<JobProgress>,
}

/// What a worker does next.
pub(crate) enum Task {
    Plan(JobId),
    Run(JobId),
}

pub(crate) struct Core {
    pub store: OpsStore,
    pub journal: Journal,
    pub clipboard: Clipboard,
    pub ctl: HashMap<JobId, Ctl>,
    subscribers: Vec<Subscriber>,
    next_token: u64,
    /// The journal entry each job made, for the jobs the queue still lists.
    pub entries: HashMap<JobId, JournalId>,
    pub recovery: Option<RecoveryReport>,
    /// Worker threads started, and how many are doing a task.
    pub workers: usize,
    pub busy: usize,
    pub shutdown: bool,
}

impl Core {
    /// Drops the bookkeeping of jobs that are gone or finished, which no worker holds.
    pub(crate) fn prune(&mut self) {
        let store = &self.store;
        self.ctl.retain(|id, ctl| {
            matches!(ctl.stage, Stage::Planning | Stage::Running)
                || store.job(*id).is_some_and(|j| !j.state.is_finished())
        });
        self.entries.retain(|id, _| store.job(*id).is_some());
        self.sync_scheduled();
    }

    /// Records in the journal the jobs that wait for a schedule, so they are queued again after a
    /// restart; a job that has started, ended or been dismissed drops out of it.
    pub(crate) fn sync_scheduled(&mut self) {
        // On the way out every job is cancelled, and the scheduled ones must stay in the file.
        if self.shutdown {
            return;
        }
        let held = self
            .store
            .scheduled_requests()
            .into_iter()
            .map(|(job, request)| ScheduledRecord { job, request })
            .collect();
        self.journal.set_scheduled(held);
    }
}

/// Everything the workers and the commands share.
pub(crate) struct Shared<R: Runtime> {
    pub app: AppHandle<R>,
    pub env: ExecEnv,
    pub resolver: Arc<dyn SelectionResolver>,
    pub clock: Arc<dyn Clock>,
    pub settings: Arc<SettingsCell>,
    /// The speed limit of every copy together, and its bucket, which every running copy shares.
    pub global_rate: Arc<RateCell>,
    pub global_bucket: Arc<Bucket>,
    pacer: Arc<SystemPacer>,
    settings_store: Arc<dyn SettingsStorage>,
    storage: Arc<OrderedStorage>,
    on_change: Option<ChangeHook>,
    exit_wait: Duration,
    pub core: Mutex<Core>,
    pub cv: Condvar,
}

/// The operations state: one queue, one journal and one clipboard for every window.
pub struct Ops<R: Runtime> {
    pub(crate) shared: Arc<Shared<R>>,
}

impl<R: Runtime> Clone for Ops<R> {
    fn clone(&self) -> Self {
        Self {
            shared: Arc::clone(&self.shared),
        }
    }
}

impl<R: Runtime> Ops<R> {
    /// Builds the state: loads the settings, opens the journal and recovers from what the last run
    /// left (cleaning up partial files, never resuming), and keeps the report for
    /// `take_recovery_report` and the `waypoint-ops://recovered` event.
    pub fn new(app: AppHandle<R>, deps: OpsDeps) -> Self {
        let loaded = match deps.settings.load() {
            Ok(settings) => settings.unwrap_or_default(),
            Err(why) => {
                log::warn!("could not load the operations settings, using the defaults: {why}");
                OpsSettings::default()
            }
        };
        let settings = Arc::new(SettingsCell(Mutex::new(clamped(loaded))));
        let flush_hook = Arc::new(FlushHook::new());
        let debouncer = Arc::new(Debouncer {
            pending: Arc::new(AtomicBool::new(false)),
            delay: deps.save_delay,
            flush: flush_hook.clone(),
        });
        let storage = Arc::new(OrderedStorage::new(deps.journal_storage.clone()));
        let (journal, report) = Journal::open(
            JournalDeps {
                settings: settings.clone(),
                clock: deps.clock.clone(),
                storage: storage.clone(),
                saver: debouncer.clone(),
            },
            &deps.providers,
        );
        let store = OpsStore::new(settings.clone(), deps.clock.clone());
        let ids = Arc::new(StartedIds(AtomicU64::new(
            deps.clock.now_ms().max(1) as u64 * 1000,
        )));
        let env = ExecEnv {
            providers: deps.providers.clone(),
            trash: deps.trash.clone(),
            protected: deps.protected.clone(),
            ids,
            copier: Arc::new(SimpleCopy),
        };
        let recovery = report.needs_notice().then_some(report);
        let pacer = Arc::new(SystemPacer::new());
        let global_rate = Arc::new(RateCell::new(settings.get().speed_limit_bps));
        let global_bucket = Arc::new(Bucket::new(global_rate.clone(), pacer.now()));
        let shared = Arc::new(Shared {
            app,
            global_rate,
            global_bucket,
            pacer,
            env,
            resolver: deps.resolver,
            clock: deps.clock,
            settings,
            settings_store: deps.settings,
            storage,
            on_change: deps.on_change,
            exit_wait: deps.exit_wait,
            core: Mutex::new(Core {
                store,
                journal,
                clipboard: Clipboard::default(),
                ctl: HashMap::new(),
                subscribers: Vec::new(),
                next_token: 0,
                entries: HashMap::new(),
                recovery: recovery.clone(),
                workers: 0,
                busy: 0,
                shutdown: false,
            }),
            cv: Condvar::new(),
        });
        // Jobs the last run left waiting for their time are queued again, and the journal records
        // them anew (a time that has passed means they start now).
        {
            let mut core = shared.lock();
            let held = core.journal.take_scheduled();
            for record in held {
                shared.enqueue(&mut core, record.request);
            }
        }
        let weak = Arc::downgrade(&shared);
        let _ = flush_hook.set(Box::new(move || {
            if let Some(shared) = weak.upgrade() {
                shared.flush_journal();
            }
        }));
        if let Some(report) = &recovery {
            if let Err(e) = shared.app.emit(RECOVERED_EVENT, report) {
                log::warn!("could not announce the recovery report: {e}");
            }
        }
        Ops { shared }
    }

    /// The whole queue and the undo history at one revision.
    pub fn snapshot(&self) -> OpsSnapshot {
        self.shared.snapshot()
    }

    /// Writes the journal now if it has unsaved changes. The app calls it when a window closes, and
    /// the plugin does when the app exits.
    pub fn flush_journal(&self) {
        self.shared.flush_journal();
    }

    /// Cancels every job that has not finished, waits up to `wait` for the workers to unwind, and
    /// writes the journal. The plugin calls it as the app exits, with `OpsDeps::exit_wait`.
    pub fn shutdown(&self, wait: Duration) {
        self.shared.shutdown(wait);
    }

    /// The configured wait for `shutdown` on exit.
    pub fn exit_wait(&self) -> Duration {
        self.shared.exit_wait
    }

    /// The shared clipboard.
    pub fn clipboard(&self) -> Clipboard {
        locked(&self.shared.core).clipboard.clone()
    }

    /// The settings in force.
    pub fn settings(&self) -> OpsSettings {
        self.shared.settings.get()
    }

    /// A window closed: it hears no more progress. Jobs are not touched, and the journal is written
    /// (with the lock released, so a slow disk holds up nothing else).
    pub fn window_closed(&self, label: &str) {
        locked(&self.shared.core)
            .subscribers
            .retain(|s| s.window != label);
        self.shared.flush_journal();
    }
}

fn clamped(mut settings: OpsSettings) -> OpsSettings {
    settings.concurrency = settings.concurrency.clamp(1, MAX_CONCURRENCY);
    settings.undo_depth = settings.undo_depth.min(MAX_UNDO_DEPTH);
    // A stored zero means no limit, as it does in the cell that reads it.
    settings.speed_limit_bps = settings
        .speed_limit_bps
        .filter(|limit| (1..=MAX_SPEED_LIMIT).contains(limit));
    // A stored zero or absurd count means no sweep, never "empty everything at every start".
    settings.trash_expiry_days = settings
        .trash_expiry_days
        .filter(|days| (1..=MAX_TRASH_EXPIRY_DAYS).contains(days));
    // A stored limit out of range falls back to the nearest end of it.
    settings.archive_max_entries = clamp_to(settings.archive_max_entries, &ARCHIVE_ENTRIES_RANGE);
    settings.archive_max_bytes = clamp_to(settings.archive_max_bytes, &ARCHIVE_BYTES_RANGE);
    settings.archive_max_ratio = clamp_to(settings.archive_max_ratio, &ARCHIVE_RATIO_RANGE);
    settings.archive_ratio_floor_bytes = clamp_to(
        settings.archive_ratio_floor_bytes,
        &ARCHIVE_RATIO_FLOOR_RANGE,
    );
    settings
}

fn clamp_to<T: Ord + Copy>(value: T, range: &std::ops::RangeInclusive<T>) -> T {
    value.clamp(*range.start(), *range.end())
}

impl<R: Runtime> Shared<R> {
    pub(crate) fn lock(&self) -> MutexGuard<'_, Core> {
        locked(&self.core)
    }

    pub(crate) fn snapshot(&self) -> OpsSnapshot {
        let core = self.lock();
        let mut snapshot = core.store.snapshot();
        snapshot.journal = core.journal.snapshot();
        snapshot
    }

    /// Sends the events of a change to every window, in the order made, and to the hook. Called
    /// with the lock held, so events reach a window in revision order.
    pub(crate) fn publish(&self, events: Vec<OpsEvent>) {
        if events.is_empty() {
            return;
        }
        for event in &events {
            if let Err(e) = self.app.emit(EVENT, event) {
                log::warn!("could not send an operations event: {e}");
            }
        }
        if let Some(hook) = &self.on_change {
            hook(&events);
        }
    }

    /// Sends progress ticks to the windows that subscribed, and drops a subscriber whose channel
    /// is closed. Progress is not broadcast.
    pub(crate) fn send_progress(&self, core: &mut Core, events: Vec<OpsEvent>) {
        if core.subscribers.is_empty() {
            return;
        }
        for event in events {
            let OpsEvent::JobChanged { job, revision } = event else {
                continue;
            };
            let tick = JobProgress {
                job: job.id,
                revision,
                progress: job.progress,
                counts: job.counts,
            };
            core.subscribers
                .retain(|s| s.channel.send(tick.clone()).is_ok());
        }
    }

    /// Writes the journal if it has unsaved changes. The copy is taken under the lock and written
    /// after it is released.
    pub(crate) fn flush_journal(&self) {
        let copy = {
            let core = self.lock();
            core.journal
                .is_dirty()
                .then(|| (self.storage.ticket(), core.journal.document()))
        };
        self.write_journal(copy);
    }

    fn write_journal(&self, copy: Option<(u64, JournalDocument)>) {
        if let Some((ticket, document)) = copy {
            if let Err(e) = self.storage.write(ticket, &document) {
                log::warn!("could not save the undo journal: {e}");
            }
        }
    }

    pub(crate) fn shutdown(&self, wait: Duration) {
        let mut core = self.lock();
        let ids: Vec<JobId> = core
            .store
            .snapshot()
            .jobs
            .iter()
            .filter(|j| !j.state.is_finished())
            .map(|j| j.id)
            .collect();
        for id in ids {
            if let Ok(events) = core.store.cancel(id) {
                self.publish(events);
            }
        }
        core.shutdown = true;
        self.cv.notify_all();
        let start = Instant::now();
        // Workers that are mid-job unwind (removing their partial files) and commit what they did.
        while core.busy > 0 && start.elapsed() < wait {
            let (next, _) = self
                .cv
                .wait_timeout(core, Duration::from_millis(20))
                .unwrap_or_else(|e| e.into_inner());
            core = next;
        }
        if core.busy > 0 {
            log::warn!(
                "{} operation(s) were still unwinding when the app exited; recovery will clean up",
                core.busy
            );
        }
        let copy = core
            .journal
            .is_dirty()
            .then(|| (self.storage.ticket(), core.journal.document()));
        drop(core);
        self.write_journal(copy);
    }

    /// Starts a worker when every one is busy and the pool is under its bound: one more than the
    /// concurrency setting, so a job can be planned while that many are parked on a question.
    pub(crate) fn ensure_workers(self: &Arc<Self>, core: &mut Core) {
        let bound = self.settings.get().concurrency as usize + 1;
        while core.workers < bound && core.workers <= core.busy {
            let shared = Arc::clone(self);
            let spawned = std::thread::Builder::new()
                .name("waypoint-ops-worker".into())
                .spawn(move || worker::worker_main(shared));
            match spawned {
                Ok(_) => core.workers += 1,
                Err(e) => {
                    log::error!("could not start an operations worker: {e}");
                    break;
                }
            }
        }
    }

    /// Adds a job and wakes a worker to plan it.
    pub(crate) fn enqueue(self: &Arc<Self>, core: &mut Core, request: JobRequest) -> JobId {
        let limit = request.options.speed_limit;
        let (id, events) = core.store.add(request);
        core.ctl.insert(id, Ctl::new(limit));
        core.sync_scheduled();
        self.publish(events);
        self.ensure_workers(core);
        self.cv.notify_all();
        id
    }

    /// The next task for a free worker, claimed: a job to plan first, then the first queued job a
    /// slot is free for.
    pub(crate) fn claim_task(&self, core: &mut Core) -> Option<Task> {
        let to_plan = core
            .ctl
            .iter()
            .filter(|(_, c)| c.stage == Stage::NeedsPlan)
            .map(|(id, _)| *id)
            .min();
        if let Some(id) = to_plan {
            if let Some(ctl) = core.ctl.get_mut(&id) {
                ctl.stage = Stage::Planning;
            }
            return Some(Task::Plan(id));
        }
        let id = core.store.next_runnable()?;
        match core.ctl.get(&id) {
            Some(ctl) if ctl.stage == Stage::Planned => {}
            _ => return None,
        }
        match core.store.start(id) {
            Ok(events) => {
                if let Some(ctl) = core.ctl.get_mut(&id) {
                    ctl.stage = Stage::Running;
                }
                core.sync_scheduled();
                self.publish(events);
                Some(Task::Run(id))
            }
            Err(e) => {
                log::warn!("could not start job {}: {e}", id.0);
                None
            }
        }
    }

    /// Waits on the condition variable while `still` holds for the job's state, the app is not
    /// shutting down and the job exists.
    pub(crate) fn park<'a>(
        &self,
        mut core: MutexGuard<'a, Core>,
        id: JobId,
        still: impl Fn(&JobState) -> bool,
    ) -> MutexGuard<'a, Core> {
        loop {
            if core.shutdown {
                return core;
            }
            match core.store.job(id) {
                Some(job) if still(&job.state) => {
                    core = self.cv.wait(core).unwrap_or_else(|e| e.into_inner());
                }
                _ => return core,
            }
        }
    }

    pub(crate) fn job(&self, core: &Core, id: JobId) -> Result<JobSnapshot, Error> {
        core.store
            .job(id)
            .cloned()
            .ok_or(Error::Queue(QueueError::UnknownJob(id)))
    }

    /// What a batch rename of the request's sources would do: a row for each entry, with the
    /// clashes among the results. Reads only. The time "today" means is fixed here (from the
    /// injected clock) and returned, so a job submitted from the preview writes the same names.
    pub(crate) fn preview_batch(&self, request: &JobRequest) -> Result<BatchPreview, Error> {
        let mut request = request.clone();
        self.freeze_now(&mut request);
        let cancel = CancelToken::new();
        let ctx = PlanCtx {
            providers: &self.env.providers,
            resolver: self.resolver.as_ref(),
            trash: self.env.trash.as_ref(),
            protected: &self.env.protected,
            cancel: &cancel,
            archive_limits: self.settings.get().archive_limits(),
        };
        Ok(preview_batch(&request, &ctx)?)
    }

    /// Gives a batch rename the time "today" means, when the request has not fixed one.
    fn freeze_now(&self, request: &mut JobRequest) {
        if let Some(spec) = request.rename.as_mut() {
            spec.now_ms.get_or_insert_with(|| self.clock.now_ms());
        }
    }

    /// A dry-run plan of a request, for the drag's default action and the conflict dialog. Writes
    /// nothing and does not touch the queue.
    pub(crate) fn preview(&self, request: &JobRequest) -> Result<PlanPreview, Error> {
        let cancel = CancelToken::new();
        let ctx = PlanCtx {
            providers: &self.env.providers,
            resolver: self.resolver.as_ref(),
            trash: self.env.trash.as_ref(),
            protected: &self.env.protected,
            cancel: &cancel,
            archive_limits: self.settings.get().archive_limits(),
        };
        let planned = plan(request, &ctx)?;
        let totals = planned.totals();
        Ok(PlanPreview {
            kind: planned.kind,
            sources: totals.sources,
            items: planned.total_items,
            bytes: planned.total_bytes,
            same_volume: planned.same_volume,
            conflicts: planned.conflicts.clone(),
            notes: planned
                .warnings
                .iter()
                .map(|w| match w {
                    PlanWarning::DuplicateSource { location } => PlanNote::DuplicateSource {
                        location: location.clone(),
                    },
                    PlanWarning::AlreadyThere { location } => PlanNote::AlreadyThere {
                        location: location.clone(),
                    },
                    PlanWarning::LeftOut { location, why } => PlanNote::LeftOut {
                        location: location.clone(),
                        why: (*why).into(),
                    },
                    PlanWarning::EmptyArchive { location } => PlanNote::EmptyArchive {
                        location: location.clone(),
                    },
                })
                .collect(),
        })
    }

    pub(crate) fn emit_clipboard(&self, clipboard: &Clipboard) {
        if let Err(e) = self.app.emit(CLIPBOARD_EVENT, clipboard) {
            log::warn!("could not send the clipboard: {e}");
        }
    }
}

// ---- what the commands do, on the shared state ----

impl<R: Runtime> Ops<R> {
    /// Puts a request on the queue. `origin_window` is the caller's label, never what the page
    /// sent: a selection handle belongs to the window that opened its listing.
    pub fn submit(&self, window: &str, mut request: JobRequest) -> Result<JobId, Error> {
        if matches!(request.kind, JobKind::Undo { .. } | JobKind::Redo { .. }) {
            return Err(Error::Ops(OpsError::Unsupported {
                what: "submitting an undo or redo; use the undo and redo commands".to_owned(),
            }));
        }
        Self::validate_options(&request)?;
        request.origin_window = window.to_owned();
        // A selection is resolved once, here, and the job keeps the locations: a retry, a redo and
        // the journal then work on the list the user confirmed, not on a listing that has changed
        // since (an `AllExcept` would pick up new files, and a permanent delete would remove them).
        if let Sources::Selection { handle, spec } = &request.sources {
            let locations = self.shared.resolver.resolve(*handle, spec, window)?;
            request.sources = Sources::Locations { locations };
        }
        self.shared.freeze_now(&mut request);
        let mut core = self.shared.lock();
        Ok(self.shared.enqueue(&mut core, request))
    }

    fn validate_options(request: &JobRequest) -> Result<(), Error> {
        if request
            .options
            .speed_limit
            .is_some_and(|limit| limit == 0 || limit > MAX_SPEED_LIMIT)
        {
            return Err(Error::Invalid(format!(
                "the speed limit must be between 1 and {MAX_SPEED_LIMIT} bytes a second, or off"
            )));
        }
        if let Some(schedule) = request.options.schedule {
            schedule.validate().map_err(Error::Invalid)?;
        }
        Ok(())
    }

    /// Sets or clears when a job that has not started may start; `None` starts it as soon as a slot
    /// is free ("Run now"). A schedule is kept in the journal, so the job survives a restart.
    pub fn set_job_schedule(&self, id: JobId, schedule: Option<Schedule>) -> Result<(), Error> {
        if let Some(schedule) = schedule {
            schedule.validate().map_err(Error::Invalid)?;
        }
        let mut core = self.shared.lock();
        let events = core.store.set_schedule(id, schedule)?;
        core.sync_scheduled();
        self.shared.publish(events);
        // A job that may start now is for a worker that is waiting; one that must wait sets its timer.
        self.shared.cv.notify_all();
        Ok(())
    }

    /// Pause all: every running job pauses where it is and nothing queued starts.
    pub fn pause_all(&self) {
        let mut core = self.shared.lock();
        let events = core.store.pause_all();
        self.shared.publish(events);
        self.shared.cv.notify_all();
    }

    /// Resume all: queued jobs may start again and every paused job runs on.
    pub fn resume_all(&self) {
        let mut core = self.shared.lock();
        let events = core.store.resume_all();
        self.shared.publish(events);
        self.shared.cv.notify_all();
    }

    /// What a batch rename would do, without queueing it.
    pub fn preview_batch_rename(
        &self,
        window: &str,
        mut request: JobRequest,
    ) -> Result<BatchPreview, Error> {
        request.origin_window = window.to_owned();
        self.shared.preview_batch(&request)
    }

    /// What a request would do, without queueing it.
    pub fn plan(&self, window: &str, mut request: JobRequest) -> Result<PlanPreview, Error> {
        request.origin_window = window.to_owned();
        self.shared.preview(&request)
    }

    /// Compares the two files of one clash a job waits on. The job's cancel token stops the reads,
    /// and nothing holds the queue's lock while they run.
    pub fn conflict_preview(
        &self,
        job: JobId,
        item: &Location,
    ) -> Result<waypoint_ops::ConflictPreview, Error> {
        let (conflict, cancel) = {
            let core = self.shared.lock();
            let snapshot = self.shared.job(&core, job)?;
            let JobState::Waiting {
                reason: waypoint_ops::WaitReason::Conflicts { conflicts },
            } = snapshot.state
            else {
                return Err(Error::Invalid(format!(
                    "job {} is not waiting on conflicts",
                    job.0
                )));
            };
            let conflict = conflicts
                .into_iter()
                .find(|c| &c.source == item)
                .ok_or_else(|| {
                    Error::Invalid(format!("{} is not a clash of job {}", item.display, job.0))
                })?;
            let cancel = core
                .store
                .cancel_token(job)
                .ok_or(Error::Queue(QueueError::UnknownJob(job)))?;
            (conflict, cancel)
        };
        Ok(waypoint_ops::conflict_preview(
            &self.shared.env.providers,
            &conflict,
            &cancel,
        )?)
    }

    pub fn pause(&self, id: JobId) -> Result<(), Error> {
        let mut core = self.shared.lock();
        let events = core.store.pause(id)?;
        self.shared.publish(events);
        self.shared.cv.notify_all();
        Ok(())
    }

    pub fn resume(&self, id: JobId) -> Result<(), Error> {
        let mut core = self.shared.lock();
        let events = core.store.resume(id)?;
        self.shared.publish(events);
        self.shared.cv.notify_all();
        Ok(())
    }

    pub fn cancel(&self, id: JobId) -> Result<(), Error> {
        let mut core = self.shared.lock();
        let events = core.store.cancel(id)?;
        self.shared.publish(events);
        core.prune();
        self.shared.cv.notify_all();
        Ok(())
    }

    pub fn retry(&self, id: JobId) -> Result<JobId, Error> {
        let mut core = self.shared.lock();
        let (new, events) = core.store.retry(id)?;
        let limit = core.store.job(new).and_then(|j| j.options.speed_limit);
        core.ctl.insert(new, Ctl::new(limit));
        self.shared.publish(events);
        self.shared.ensure_workers(&mut core);
        self.shared.cv.notify_all();
        Ok(new)
    }

    pub fn dismiss(&self, id: JobId) -> Result<(), Error> {
        let mut core = self.shared.lock();
        let events = core.store.dismiss(id)?;
        self.shared.publish(events);
        core.prune();
        Ok(())
    }

    pub fn dismiss_finished(&self) {
        let mut core = self.shared.lock();
        let events = core.store.dismiss_finished();
        self.shared.publish(events);
        core.prune();
    }

    /// Changes a job's own speed limit (bytes a second, `None` for none) and priority while it
    /// waits or runs. A running copy obeys the new limit within a fraction of a second, and a new
    /// priority decides which queued job a freed slot goes to.
    pub fn set_job_limits(
        &self,
        id: JobId,
        speed_limit: Option<u64>,
        priority: Option<JobPriority>,
    ) -> Result<(), Error> {
        if speed_limit.is_some_and(|limit| limit == 0 || limit > MAX_SPEED_LIMIT) {
            return Err(Error::Invalid(format!(
                "the speed limit must be between 1 and {MAX_SPEED_LIMIT} bytes a second, or off"
            )));
        }
        let mut core = self.shared.lock();
        let events = core.store.set_limits(id, speed_limit, priority)?;
        if let Some(ctl) = core.ctl.get(&id) {
            ctl.rate.set(speed_limit);
        }
        self.shared.publish(events);
        // A job that outranks the others may now be the one a waiting worker should take.
        self.shared.cv.notify_all();
        Ok(())
    }

    /// The limits a copy that runs now obeys: the queue's and the job's own.
    pub(crate) fn throttle_for(shared: &Shared<R>, ctl: &Ctl) -> Throttle {
        let job = Arc::new(Bucket::new(ctl.rate.clone(), shared.pacer.now()));
        Throttle::new(
            shared.pacer.clone(),
            vec![shared.global_bucket.clone(), job],
        )
    }

    pub fn reorder(&self, id: JobId, to: usize) -> Result<(), Error> {
        let mut core = self.shared.lock();
        let events = core.store.reorder(id, to)?;
        self.shared.publish(events);
        Ok(())
    }

    /// Answers the conflicts a job waits on: each decision settles one source's clash, and
    /// `apply_to_all` is the policy for every other clash the job meets.
    pub fn resolve(
        &self,
        id: JobId,
        mut decisions: Vec<Resolution>,
        apply_to_all: Option<waypoint_ops::ConflictPolicy>,
    ) -> Result<(), Error> {
        if let Some(policy) = apply_to_all {
            decisions.push(Resolution {
                source: None,
                policy,
            });
        }
        let mut core = self.shared.lock();
        let events = core.store.resolve(id, &decisions)?;
        if let Some(ctl) = core.ctl.get_mut(&id) {
            ctl.answers.extend(decisions);
        }
        self.shared.publish(events);
        self.shared.cv.notify_all();
        Ok(())
    }

    /// Answers the error a job waits on.
    pub fn resolve_error(&self, id: JobId, decision: Decision) -> Result<(), Error> {
        let mut core = self.shared.lock();
        let job = self.shared.job(&core, id)?;
        if !matches!(
            job.state,
            JobState::Waiting {
                reason: waypoint_ops::WaitReason::Error { .. }
            }
        ) {
            return Err(Error::Queue(QueueError::Illegal {
                id,
                from: job.state.name(),
                action: "take an answer to an error",
            }));
        }
        let events = core.store.answered(id)?;
        if let Some(ctl) = core.ctl.get_mut(&id) {
            ctl.decision = Some(decision);
        }
        self.shared.publish(events);
        self.shared.cv.notify_all();
        Ok(())
    }

    fn already_underway(core: &Core, entry: JournalId) -> bool {
        core.store.snapshot().jobs.iter().any(|j| {
            !j.state.is_finished()
                && matches!(j.kind,
                    JobKind::Undo { of } | JobKind::Redo { of } if of == entry)
        })
    }

    /// Undoes `entry`, or the newest applied entry when `None`, as a job on the queue.
    pub fn undo(&self, window: &str, entry: Option<JournalId>) -> Result<JobId, Error> {
        let mut core = self.shared.lock();
        let request = match entry {
            Some(id) => core.journal.undo_request(id, window)?,
            None => core.journal.undo_last_request(window)?,
        };
        if let JobKind::Undo { of } = request.kind {
            if Self::already_underway(&core, of) {
                return Err(Error::Ops(OpsError::UndoUnavailable {
                    reason: "that is already being undone or redone".to_owned(),
                }));
            }
        }
        Ok(self.shared.enqueue(&mut core, request))
    }

    /// Redoes `entry`, or the entry undone most recently when `None`.
    pub fn redo(&self, window: &str, entry: Option<JournalId>) -> Result<JobId, Error> {
        let mut core = self.shared.lock();
        let request = match entry {
            Some(id) => core.journal.redo_request(id, window)?,
            None => core.journal.redo_last_request(window)?,
        };
        if let JobKind::Redo { of } = request.kind {
            if Self::already_underway(&core, of) {
                return Err(Error::Ops(OpsError::UndoUnavailable {
                    reason: "that is already being undone or redone".to_owned(),
                }));
            }
        }
        Ok(self.shared.enqueue(&mut core, request))
    }

    /// The undo history, newest first.
    pub fn journal_summaries(&self) -> Vec<JournalEntrySummary> {
        self.shared.lock().journal.summaries()
    }

    /// Subscribes the window to progress ticks on `channel`, replacing an earlier subscription of
    /// the same window. Returns the subscription's token.
    pub fn subscribe_progress(&self, window: &str, channel: Channel<JobProgress>) -> u64 {
        let mut core = self.shared.lock();
        core.subscribers.retain(|s| s.window != window);
        core.next_token += 1;
        let token = core.next_token;
        core.subscribers.push(Subscriber {
            window: window.to_owned(),
            token,
            channel,
        });
        token
    }

    /// Stops the window's progress. With a `token`, only that subscription: a stop from one that
    /// was replaced since (a page that mounted twice) leaves the newer one alone.
    pub fn unsubscribe_progress(&self, window: &str, token: Option<u64>) {
        self.shared
            .lock()
            .subscribers
            .retain(|s| s.window != window || token.is_some_and(|t| t != s.token));
    }

    /// The journal entry the job made, once it has recorded one.
    pub fn journal_entry_of(&self, job: JobId) -> Option<JournalId> {
        self.shared.lock().entries.get(&job).copied()
    }

    /// Replaces the shared clipboard and tells every window. An empty list clears it.
    pub fn set_clipboard(&self, mode: ClipboardMode, items: Vec<Location>) -> Clipboard {
        self.set_clipboard_from(mode, items, ClipboardSource::App)
    }

    /// Replaces the shared clipboard with entries from `source` and tells every window.
    pub fn set_clipboard_from(
        &self,
        mode: ClipboardMode,
        items: Vec<Location>,
        source: ClipboardSource,
    ) -> Clipboard {
        let mut core = self.shared.lock();
        let clipboard = Clipboard {
            mode,
            items,
            source,
            revision: core.clipboard.revision + 1,
        };
        core.clipboard = clipboard.clone();
        self.shared.emit_clipboard(&clipboard);
        clipboard
    }

    /// Puts what `spec` selects in the listing `handle` (which `window` opened) on the clipboard.
    /// The locations are resolved here, by the app's resolver, so the page never builds a path and
    /// a selection of "everything except these" stays a handle and a range. A selection of nothing
    /// is refused rather than clearing the clipboard.
    pub fn set_clipboard_from_selection(
        &self,
        window: &str,
        handle: ListingHandle,
        spec: &SelectionSpec,
        mode: ClipboardMode,
    ) -> Result<Clipboard, Error> {
        let items = self.shared.resolver.resolve(handle, spec, window)?;
        if items.is_empty() {
            return Err(Error::Ops(OpsError::Unsupported {
                what: "copying an empty selection".to_owned(),
            }));
        }
        Ok(self.set_clipboard(mode, items))
    }

    /// The locations `spec` selects in the listing `handle` (which `window` opened), resolved by
    /// the app's resolver so the page never builds a path. The page hands them to the system as an
    /// outbound drag. A selection of nothing is refused, as it is for the clipboard.
    pub fn resolve_selection(
        &self,
        window: &str,
        handle: ListingHandle,
        spec: &SelectionSpec,
    ) -> Result<Vec<Location>, Error> {
        let items = self.shared.resolver.resolve(handle, spec, window)?;
        if items.is_empty() {
            return Err(Error::Ops(OpsError::Unsupported {
                what: "dragging an empty selection".to_owned(),
            }));
        }
        Ok(items)
    }

    /// The unfinished jobs that read from, write into, or remove something that holds `location`.
    pub fn jobs_targeting(&self, location: &Location) -> Vec<JobId> {
        self.shared.lock().store.jobs_targeting(location)
    }

    /// Checks `settings` against the ranges a change must be in, touching nothing.
    pub fn validate_settings(settings: &OpsSettings) -> Result<(), Error> {
        if settings.concurrency == 0 || settings.concurrency > MAX_CONCURRENCY {
            return Err(Error::Invalid(format!(
                "the number of jobs at once must be between 1 and {MAX_CONCURRENCY}"
            )));
        }
        if settings
            .speed_limit_bps
            .is_some_and(|limit| limit == 0 || limit > MAX_SPEED_LIMIT)
        {
            return Err(Error::Invalid(format!(
                "the speed limit must be between 1 and {MAX_SPEED_LIMIT} bytes a second, or off"
            )));
        }
        if settings.undo_depth > MAX_UNDO_DEPTH {
            return Err(Error::Invalid(format!(
                "the undo history can keep at most {MAX_UNDO_DEPTH} entries"
            )));
        }
        if settings
            .trash_expiry_days
            .is_some_and(|days| days == 0 || days > MAX_TRASH_EXPIRY_DAYS)
        {
            return Err(Error::Invalid(format!(
                "the Trash sweep must wait between 1 and {MAX_TRASH_EXPIRY_DAYS} days, or be off"
            )));
        }
        if !ARCHIVE_ENTRIES_RANGE.contains(&settings.archive_max_entries) {
            return Err(Error::Invalid(format!(
                "the most entries an archive may hold must be between {} and {}",
                ARCHIVE_ENTRIES_RANGE.start(),
                ARCHIVE_ENTRIES_RANGE.end()
            )));
        }
        if !ARCHIVE_BYTES_RANGE.contains(&settings.archive_max_bytes) {
            return Err(Error::Invalid(
                "the most an archive may expand to must be between 1 GiB and 1 TiB".to_owned(),
            ));
        }
        if !ARCHIVE_RATIO_RANGE.contains(&settings.archive_max_ratio) {
            return Err(Error::Invalid(format!(
                "how many times its size an archive may expand to must be between {} and {}",
                ARCHIVE_RATIO_RANGE.start(),
                ARCHIVE_RATIO_RANGE.end()
            )));
        }
        if !ARCHIVE_RATIO_FLOOR_RANGE.contains(&settings.archive_ratio_floor_bytes) {
            return Err(Error::Invalid(
                "the size above which the expansion ratio is checked must be between 1 MiB and 1 TiB"
                    .to_owned(),
            ));
        }
        Ok(())
    }

    /// Changes the settings: saved first, then in force for the next job. A change that cannot be
    /// saved changes nothing.
    pub fn set_settings(&self, settings: OpsSettings) -> Result<OpsSettings, Error> {
        Self::validate_settings(&settings)?;
        self.shared
            .settings_store
            .save(&settings)
            .map_err(Error::Storage)?;
        self.shared.settings.set(settings);
        // A running copy reads the limit at its next piece.
        self.shared.global_rate.set(settings.speed_limit_bps);
        let mut core = self.shared.lock();
        self.shared.ensure_workers(&mut core);
        // A raised concurrency lets queued jobs start.
        self.shared.cv.notify_all();
        Ok(settings)
    }

    /// The recovery report of the start-up, once; `None` after that and when there was nothing to
    /// tell.
    pub fn take_recovery_report(&self) -> Option<RecoveryReport> {
        self.shared.lock().recovery.take()
    }

    /// Whether the request names sources by selection (which the planner resolves).
    pub fn is_selection(request: &JobRequest) -> bool {
        matches!(request.sources, Sources::Selection { .. })
    }
}
