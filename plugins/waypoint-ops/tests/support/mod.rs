// Fixtures for the plugin's mock-runtime tests: a provider that can hold a job at a chosen write or
// fail one, the app with two windows and every listener attached, and polling helpers. Everything
// happens in a temporary directory behind a sandbox, and the Trash is a fake over that directory.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![allow(dead_code)]

use std::ffi::{OsStr, OsString};
use std::io::Write;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use tauri::test::{mock_builder, mock_context, noop_assets, MockRuntime};
use tauri::{Listener, Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_waypoint_ops::{
    commands, init, Clipboard, MemorySettings, Ops, OpsDeps, CLIPBOARD_EVENT, EVENT,
    RECOVERED_EVENT,
};
use waypoint_ops::testing::harness::FixedResolver;
use waypoint_ops::testing::journal_storage::MemoryJournalStorage;
use waypoint_ops::testing::sandbox::SandboxProvider;
use waypoint_ops::testing::trash::FakeTrash;
use waypoint_ops::{
    Clock, ConflictPolicy, CounterIds, JobId, JobKind, JobOptions, JobRequest, JobSnapshot,
    JobState, JournalStorage, OpsEvent, OpsSnapshot, Protected, Providers, RecoveryReport, Sources,
    SystemClock,
};
use waypoint_path::{FilePath, VfsPath};
use waypoint_protocol::{Location, VfsError};
use waypoint_vfs::{
    CancelToken, Capabilities, FileTimes, LocalProvider, Permissions, Provider, ReadStream,
    ScannedEntry, VolumeId, VolumeSpace, Watch, WatchSink, WriteOptions, WriteStream,
};

/// Where a gate holds the job.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Block {
    Nowhere,
    /// At the nth `create_write` call (1 is the first).
    At(usize),
    /// At every one.
    All,
}

struct GateState {
    calls: usize,
    block: Block,
    open: bool,
    entered: usize,
    fail: Vec<(usize, VfsError)>,
}

/// Holds a copy at a chosen `create_write` until the test opens the gate, so a job is parked in a
/// known place, and can make a chosen one fail.
pub struct Gate {
    state: Mutex<GateState>,
    cv: Condvar,
}

impl Gate {
    fn new() -> Self {
        Self {
            state: Mutex::new(GateState {
                calls: 0,
                block: Block::Nowhere,
                open: true,
                entered: 0,
                fail: Vec::new(),
            }),
            cv: Condvar::new(),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, GateState> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Holds the job at its nth write from now.
    pub fn block_at(&self, n: usize) {
        let mut state = self.lock();
        state.calls = 0;
        state.entered = 0;
        state.block = Block::At(n);
        state.open = false;
    }

    /// Holds every job at every write.
    pub fn block_all(&self) {
        let mut state = self.lock();
        state.entered = 0;
        state.block = Block::All;
        state.open = false;
    }

    pub fn open(&self) {
        let mut state = self.lock();
        state.open = true;
        state.block = Block::Nowhere;
        self.cv.notify_all();
    }

    /// Fails the nth write from now with `error`.
    pub fn fail_at(&self, n: usize, error: VfsError) {
        let mut state = self.lock();
        state.calls = 0;
        state.fail.push((n, error));
    }

    /// Waits until `count` workers are held at the gate.
    pub fn wait_held(&self, count: usize) {
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut state = self.lock();
        while state.entered < count {
            let left = deadline.saturating_duration_since(Instant::now());
            assert!(!left.is_zero(), "no worker reached the gate");
            state = self
                .cv
                .wait_timeout(state, left)
                .unwrap_or_else(|e| e.into_inner())
                .0;
        }
    }

    fn pass(&self) -> Result<(), VfsError> {
        let mut state = self.lock();
        state.calls += 1;
        let n = state.calls;
        if let Some(at) = state.fail.iter().position(|(when, _)| *when == n) {
            return Err(state.fail.remove(at).1);
        }
        let hold = match state.block {
            Block::Nowhere => false,
            Block::At(at) => at == n,
            Block::All => true,
        };
        if hold && !state.open {
            state.entered += 1;
            self.cv.notify_all();
            while !state.open {
                state = self.cv.wait(state).unwrap_or_else(|e| e.into_inner());
            }
        }
        Ok(())
    }
}

/// A provider that forwards everything and holds or fails `create_write` as the gate says. It
/// leaves out the same-provider fast path, so a copy goes through `create_write`.
pub struct GateProvider<P> {
    inner: P,
    pub gate: Arc<Gate>,
}

impl<P: Provider> Provider for GateProvider<P> {
    fn scheme(&self) -> &'static str {
        self.inner.scheme()
    }
    fn capabilities(&self) -> Capabilities {
        self.inner.capabilities()
    }
    fn stat(&self, path: &VfsPath) -> Result<ScannedEntry, VfsError> {
        self.inner.stat(path)
    }
    fn list(
        &self,
        path: &VfsPath,
        cancel: &CancelToken,
        budget: usize,
        progress: &mut dyn FnMut(u32),
    ) -> Result<Vec<ScannedEntry>, VfsError> {
        self.inner.list(path, cancel, budget, progress)
    }
    fn resolve_link(
        &self,
        folder: &VfsPath,
        entry: &ScannedEntry,
    ) -> Result<ScannedEntry, VfsError> {
        self.inner.resolve_link(folder, entry)
    }
    fn watch(&self, path: &VfsPath, sink: WatchSink) -> Result<Box<dyn Watch>, VfsError> {
        self.inner.watch(path, sink)
    }
    fn create_dir(&self, path: &VfsPath) -> Result<(), VfsError> {
        self.inner.create_dir(path)
    }
    fn create_file(&self, path: &VfsPath) -> Result<(), VfsError> {
        self.inner.create_file(path)
    }
    fn rename(&self, from: &VfsPath, to: &VfsPath, overwrite: bool) -> Result<(), VfsError> {
        self.inner.rename(from, to, overwrite)
    }
    fn remove_file(&self, path: &VfsPath) -> Result<(), VfsError> {
        self.inner.remove_file(path)
    }
    fn remove_dir(&self, path: &VfsPath) -> Result<(), VfsError> {
        self.inner.remove_dir(path)
    }
    fn open_read(&self, path: &VfsPath) -> Result<ReadStream, VfsError> {
        self.inner.open_read(path)
    }
    fn create_write(
        &self,
        path: &VfsPath,
        options: WriteOptions,
    ) -> Result<Box<dyn WriteStream>, VfsError> {
        self.gate.pass()?;
        self.inner.create_write(path, options)
    }
    fn set_times(&self, path: &VfsPath, times: FileTimes) -> Result<(), VfsError> {
        self.inner.set_times(path, times)
    }
    fn permissions(&self, path: &VfsPath) -> Result<Permissions, VfsError> {
        self.inner.permissions(path)
    }
    fn set_permissions(&self, path: &VfsPath, permissions: Permissions) -> Result<(), VfsError> {
        self.inner.set_permissions(path, permissions)
    }
    fn symlink(&self, link: &VfsPath, target: &OsStr) -> Result<(), VfsError> {
        self.inner.symlink(link, target)
    }
    fn read_link(&self, path: &VfsPath) -> Result<OsString, VfsError> {
        self.inner.read_link(path)
    }
    fn volume_id(&self, path: &VfsPath) -> Option<VolumeId> {
        self.inner.volume_id(path)
    }
    fn free_space(&self, path: &VfsPath) -> Option<VolumeSpace> {
        self.inner.free_space(path)
    }
}

pub type Fs = GateProvider<SandboxProvider<LocalProvider>>;

pub struct Env {
    pub dir: tempfile::TempDir,
    pub base: VfsPath,
    pub work: VfsPath,
    pub app: tauri::App<MockRuntime>,
    pub gate: Arc<Gate>,
    pub fs: Arc<Fs>,
    pub journal: Arc<MemoryJournalStorage>,
    pub settings: Arc<MemorySettings>,
    pub trash: Arc<FakeTrash>,
    pub resolver: Arc<FixedResolver>,
    /// Every event, with the label of the window that received it.
    pub events: Arc<Mutex<Vec<(String, OpsEvent)>>>,
    pub clipboards: Arc<Mutex<Vec<(String, Clipboard)>>>,
    pub recovered: Arc<Mutex<Vec<RecoveryReport>>>,
}

/// What runs on the work folder before the plugin starts.
pub type Prepare = Box<dyn FnOnce(&VfsPath, &Fs)>;

/// How a test shapes the app before it is built.
pub struct Setup {
    pub journal: Arc<MemoryJournalStorage>,
    pub settings: Arc<MemorySettings>,
    pub save_delay: Duration,
    /// Stands in for `journal` as what the plugin writes to (a slow disk, say); `journal` still
    /// reads back what reached it.
    pub storage: Option<Arc<dyn JournalStorage>>,
    /// Runs on the work folder before the plugin starts (a stale partial file, say).
    pub prepare: Prepare,
}

impl Default for Setup {
    fn default() -> Self {
        Self {
            journal: Arc::new(MemoryJournalStorage::new()),
            settings: Arc::new(MemorySettings::default()),
            save_delay: Duration::from_millis(30),
            storage: None,
            prepare: Box::new(|_, _| {}),
        }
    }
}

pub fn env() -> Env {
    env_with(Setup::default())
}

pub fn env_with(setup: Setup) -> Env {
    let dir = tempfile::tempdir().unwrap();
    let base = VfsPath::File(FilePath::from_path(dir.path()).unwrap());
    let sandbox = SandboxProvider::new(LocalProvider::new(), &base);
    let gate = Arc::new(Gate::new());
    let fs = Arc::new(GateProvider {
        inner: sandbox,
        gate: gate.clone(),
    });
    let work = base.join("work").unwrap();
    fs.create_dir(&work).unwrap();
    let trash = Arc::new(FakeTrash::new(
        fs.clone(),
        base.join("trash").unwrap(),
        Arc::new(CounterIds::default()),
        Arc::new(SystemClock),
    ));
    (setup.prepare)(&work, &fs);

    let resolver = Arc::new(FixedResolver::default());
    let mut deps = OpsDeps::new(
        Providers::single(fs.clone()),
        trash.clone(),
        resolver.clone(),
        setup
            .storage
            .clone()
            .unwrap_or_else(|| setup.journal.clone()),
        setup.settings.clone(),
        Arc::new(SystemClock),
        Protected::new(vec![base.clone()]),
    );
    deps.save_delay = setup.save_delay;
    deps.exit_wait = Duration::from_secs(5);

    let recovered: Arc<Mutex<Vec<RecoveryReport>>> = Arc::default();
    let heard = recovered.clone();
    // Listens before the operations plugin starts, since it announces recovery as it sets up.
    let listener = tauri::plugin::Builder::<MockRuntime>::new("recovery-listener")
        .setup(move |app, _| {
            app.listen(RECOVERED_EVENT, move |event| {
                if let Ok(report) = serde_json::from_str(event.payload()) {
                    heard.lock().unwrap().push(report);
                }
            });
            Ok(())
        })
        .build();
    let app = mock_builder()
        .plugin(listener)
        .plugin(init(deps))
        .build(mock_context(noop_assets()))
        .expect("the mock app builds");

    let events: Arc<Mutex<Vec<(String, OpsEvent)>>> = Arc::default();
    let clipboards: Arc<Mutex<Vec<(String, Clipboard)>>> = Arc::default();
    for label in ["main-1", "main-2", "settings"] {
        let window = WebviewWindowBuilder::new(&app, label, WebviewUrl::App("index.html".into()))
            .build()
            .expect("the mock window opens");
        let (log, name) = (events.clone(), label.to_owned());
        window.listen(EVENT, move |event| {
            let parsed: OpsEvent = serde_json::from_str(event.payload()).expect("an ops event");
            log.lock().unwrap().push((name.clone(), parsed));
        });
        let (log, name) = (clipboards.clone(), label.to_owned());
        window.listen(CLIPBOARD_EVENT, move |event| {
            let parsed: Clipboard = serde_json::from_str(event.payload()).expect("a clipboard");
            log.lock().unwrap().push((name.clone(), parsed));
        });
    }
    Env {
        dir,
        base,
        work,
        app,
        gate,
        fs,
        journal: setup.journal,
        settings: setup.settings,
        trash,
        resolver,
        events,
        clipboards,
        recovered,
    }
}

impl Env {
    pub fn ops(&self) -> Ops<MockRuntime> {
        self.app.state::<Ops<MockRuntime>>().inner().clone()
    }

    pub fn window(&self, label: &str) -> tauri::WebviewWindow<MockRuntime> {
        self.app
            .get_webview_window(label)
            .expect("the window exists")
    }

    pub fn path(&self, relative: &str) -> VfsPath {
        relative
            .split('/')
            .filter(|s| !s.is_empty())
            .fold(self.work.clone(), |p, name| p.join(name).unwrap())
    }

    pub fn loc(&self, relative: &str) -> Location {
        self.path(relative).to_location()
    }

    pub fn write(&self, relative: &str, bytes: &[u8]) {
        let path = self.path(relative);
        if let Some(parent) = path.parent() {
            let _ = self.fs.create_dir(&parent);
        }
        let mut stream = self
            .fs
            .create_write(&path, WriteOptions::exclusive())
            .unwrap();
        stream.write_all(bytes).unwrap();
        stream.finish(false).unwrap();
        // The set-up writes must not use up the gate's count.
        self.gate_reset();
    }

    pub fn dir(&self, relative: &str) {
        self.fs.create_dir(&self.path(relative)).unwrap();
    }

    fn gate_reset(&self) {
        let mut state = self.gate.lock();
        state.calls = 0;
    }

    pub fn exists(&self, relative: &str) -> bool {
        self.fs.stat(&self.path(relative)).is_ok()
    }

    pub fn read(&self, relative: &str) -> Vec<u8> {
        use std::io::Read;
        let mut out = Vec::new();
        self.fs
            .open_read(&self.path(relative))
            .unwrap()
            .read_to_end(&mut out)
            .unwrap();
        out
    }

    /// The names in the work folder, sorted.
    pub fn names(&self, relative: &str) -> Vec<String> {
        let mut names: Vec<String> = self
            .fs
            .list(&self.path(relative), &CancelToken::new(), 0, &mut |_| {})
            .unwrap()
            .into_iter()
            .map(|e| e.name.to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

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
            origin_window: "someone-else".to_owned(),
            rename: None,
        }
    }

    pub fn copy(&self, sources: &[&str], destination: &str) -> JobRequest {
        self.request(JobKind::Copy, sources, Some(destination), None)
    }

    pub fn copy_with(
        &self,
        sources: &[&str],
        destination: &str,
        policy: ConflictPolicy,
    ) -> JobRequest {
        let mut request = self.copy(sources, destination);
        request.options.conflict = Some(policy);
        request
    }

    pub fn submit(&self, window: &str, request: JobRequest) -> JobId {
        tauri::async_runtime::block_on(commands::submit(
            self.window(window),
            self.app.state::<Ops<MockRuntime>>(),
            request,
        ))
        .expect("the job is accepted")
    }

    pub fn snapshot(&self) -> OpsSnapshot {
        self.ops().snapshot()
    }

    pub fn job(&self, id: JobId) -> JobSnapshot {
        self.snapshot()
            .jobs
            .into_iter()
            .find(|j| j.id == id)
            .unwrap_or_else(|| panic!("job {} is not in the queue", id.0))
    }

    pub fn state(&self, id: JobId) -> JobState {
        self.job(id).state
    }

    /// Waits for the job's state to satisfy `want`, and returns it.
    pub fn wait_state(&self, id: JobId, what: &str, want: impl Fn(&JobState) -> bool) -> JobState {
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            let state = self.state(id);
            if want(&state) {
                return state;
            }
            assert!(
                Instant::now() < deadline,
                "job {} never became {what}; it is {state:?}",
                id.0
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    pub fn wait_done(&self, id: JobId) {
        let state = self.wait_state(id, "done", |s| s.is_finished());
        assert_eq!(state, JobState::Done, "job {}", id.0);
    }

    pub fn wait_for(&self, what: &str, cond: impl Fn(&Env) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(15);
        while !cond(self) {
            assert!(Instant::now() < deadline, "never saw: {what}");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// The events one window received, in the order it received them.
    pub fn events_of(&self, window: &str) -> Vec<OpsEvent> {
        self.events
            .lock()
            .unwrap()
            .iter()
            .filter(|(label, _)| label == window)
            .map(|(_, e)| e.clone())
            .collect()
    }
}

/// A file of `megabytes` MiB of a repeating pattern, which takes more than one chunk to copy.
pub fn big(megabytes: usize) -> Vec<u8> {
    (0..megabytes * 1024 * 1024)
        .map(|i| (i % 251) as u8)
        .collect()
}

pub fn clock_ms() -> i64 {
    SystemClock.now_ms()
}
