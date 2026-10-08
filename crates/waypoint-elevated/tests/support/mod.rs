// Shared fixtures for the tests of the elevated provider: a provider behind a loopback helper, a
// provider that waits to be cancelled, a gated stream and a raw client that speaks frames.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![allow(dead_code)]

use std::io::{self, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use waypoint_elevated::testing::loopback::LoopbackLauncher;
use waypoint_elevated::testing::pipe::{duplex, PipeReader, PipeWriter};
use waypoint_elevated::wire::{Message, Op, Reply, Request};
use waypoint_elevated::{
    read_frame, serve, write_frame, ElevatedProvider, Frame, Launcher, ServeConfig, ServeEnd,
    Transport,
};
use waypoint_path::{ConnectionKey, FilePath, VfsPath};
use waypoint_protocol::VfsError;
use waypoint_vfs::{
    CancelToken, Capabilities, EntryKind, FolderSizeRun, FolderSizeTotals, LocalProvider, Provider,
    ScannedEntry, Watch, WatchOptions, WatchSink,
};

pub const PATIENCE: Duration = Duration::from_secs(10);

pub fn admin(dir: &Path) -> VfsPath {
    VfsPath::File(FilePath::from_path(dir).unwrap())
        .elevated()
        .unwrap()
}

pub fn file(dir: &Path) -> VfsPath {
    VfsPath::File(FilePath::from_path(dir).unwrap())
}

/// A client wired to a serve loop on a thread, not yet connected.
pub struct Rig {
    pub client: ElevatedProvider,
    pub launcher: LoopbackLauncher,
}

impl Rig {
    pub fn new(provider: Arc<dyn Provider>, config: ServeConfig) -> Self {
        let launcher = LoopbackLauncher::new(provider, config);
        let client = ElevatedProvider::new(Box::new(launcher.clone()));
        Rig { client, launcher }
    }

    pub fn local() -> Self {
        Self::new(Arc::new(LocalProvider::new()), ServeConfig::default())
    }

    pub fn connected(provider: Arc<dyn Provider>, config: ServeConfig) -> Self {
        let rig = Self::new(provider, config);
        rig.connect();
        rig
    }

    pub fn connect(&self) {
        self.client
            .connect(&ConnectionKey::elevated(), None, &CancelToken::new())
            .unwrap();
    }
}

pub fn fast_watching() -> LocalProvider {
    LocalProvider::with_watch_options(WatchOptions {
        debounce: Duration::from_millis(20),
        max_wait: Duration::from_millis(200),
        rename_grace: Duration::from_millis(20),
        poll_interval: Duration::from_millis(50),
        ..WatchOptions::default()
    })
}

pub fn wait_until(what: &str, mut condition: impl FnMut() -> bool) {
    let started = Instant::now();
    while !condition() {
        assert!(started.elapsed() < PATIENCE, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

pub fn kind<T>(result: &Result<T, VfsError>) -> String {
    waypoint_vfs::conformance::kind(result)
}

/// A local provider whose `list` and `folder_size` wait until they are cancelled (or released),
/// and whose watches hand their sink to the test.
pub struct TestProvider {
    pub local: LocalProvider,
    pub release: Arc<AtomicBool>,
    pub sink: Arc<Mutex<Option<WatchSink>>>,
    pub started: Arc<AtomicUsize>,
}

impl TestProvider {
    pub fn new() -> Self {
        Self {
            local: LocalProvider::new(),
            release: Arc::new(AtomicBool::new(false)),
            sink: Arc::default(),
            started: Arc::default(),
        }
    }

    fn wait(&self, cancel: &CancelToken) -> Result<(), VfsError> {
        self.started.fetch_add(1, Ordering::SeqCst);
        while !self.release.load(Ordering::SeqCst) {
            if cancel.is_cancelled() {
                return Err(VfsError::Cancelled);
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        Ok(())
    }
}

struct Noop;

impl Watch for Noop {}

impl Provider for TestProvider {
    fn scheme(&self) -> &'static str {
        "file"
    }

    fn capabilities(&self) -> Capabilities {
        self.local.capabilities()
    }

    fn stat(&self, path: &VfsPath) -> Result<ScannedEntry, VfsError> {
        self.local.stat(path)
    }

    fn list(
        &self,
        path: &VfsPath,
        cancel: &CancelToken,
        budget: usize,
        progress: &mut dyn FnMut(u32),
    ) -> Result<Vec<ScannedEntry>, VfsError> {
        self.wait(cancel)?;
        self.local.list(path, cancel, budget, progress)
    }

    fn resolve_link(
        &self,
        folder: &VfsPath,
        entry: &ScannedEntry,
    ) -> Result<ScannedEntry, VfsError> {
        self.local.resolve_link(folder, entry)
    }

    fn folder_size(
        &self,
        path: &VfsPath,
        cancel: &CancelToken,
        report: &mut dyn FnMut(&FolderSizeTotals),
    ) -> Result<FolderSizeRun, VfsError> {
        match self.wait(cancel) {
            Err(VfsError::Cancelled) => Ok(FolderSizeRun {
                totals: FolderSizeTotals::default(),
                cancelled: true,
            }),
            Err(other) => Err(other),
            Ok(()) => self.local.folder_size(path, cancel, report),
        }
    }

    fn watch(&self, _path: &VfsPath, sink: WatchSink) -> Result<Box<dyn Watch>, VfsError> {
        *self.sink.lock().unwrap() = Some(sink);
        Ok(Box::new(Noop))
    }

    fn open_read(&self, path: &VfsPath) -> Result<waypoint_vfs::ReadStream, VfsError> {
        self.local.open_read(path)
    }
}

pub fn entry_names(entries: &[ScannedEntry]) -> Vec<String> {
    let mut names: Vec<String> = entries
        .iter()
        .map(|entry| entry.name.to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

pub fn is_folder(entry: &ScannedEntry) -> bool {
    entry.kind == EntryKind::Directory
}

/// A stream whose writes wait while the gate is shut, so a test can hold a helper's output.
pub struct Gate {
    open: Mutex<bool>,
    changed: Condvar,
    pub blocked: AtomicUsize,
}

impl Gate {
    pub fn new() -> Arc<Self> {
        Arc::new(Gate {
            open: Mutex::new(true),
            changed: Condvar::new(),
            blocked: AtomicUsize::new(0),
        })
    }

    pub fn shut(&self) {
        *self.open.lock().unwrap() = false;
    }

    pub fn open(&self) {
        *self.open.lock().unwrap() = true;
        self.changed.notify_all();
    }
}

pub struct GatedWriter<W> {
    pub inner: W,
    pub gate: Arc<Gate>,
}

impl<W: Write> Write for GatedWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut open = self.gate.open.lock().unwrap();
        if !*open {
            self.gate.blocked.fetch_add(1, Ordering::SeqCst);
            while !*open {
                open = self.gate.changed.wait(open).unwrap();
            }
            self.gate.blocked.fetch_sub(1, Ordering::SeqCst);
        }
        drop(open);
        self.inner.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

/// Launches a serve loop whose output goes through a gate.
pub struct GatedLauncher {
    pub provider: Arc<dyn Provider>,
    pub config: ServeConfig,
    pub gate: Arc<Gate>,
}

impl Launcher for GatedLauncher {
    fn launch(&self) -> Result<Transport, VfsError> {
        let streams = duplex();
        let writer = GatedWriter {
            inner: streams.server_writer,
            gate: self.gate.clone(),
        };
        let (provider, config) = (self.provider.clone(), self.config.clone());
        let reader = streams.server_reader;
        std::thread::spawn(move || serve(reader, writer, provider, config));
        Ok(Transport {
            reader: Box::new(streams.client_reader),
            writer: Box::new(streams.client_writer),
        })
    }
}

/// Speaks the protocol to a serve loop directly, so a test can say what a client never would.
pub struct Raw {
    pub reader: PipeReader,
    pub writer: PipeWriter,
    pub end: Option<JoinHandle<ServeEnd>>,
}

impl Raw {
    pub fn new(provider: Arc<dyn Provider>, config: ServeConfig) -> Self {
        let streams = duplex();
        let (reader, writer) = (streams.server_reader, streams.server_writer);
        let end = std::thread::spawn(move || serve(reader, writer, provider, config));
        Raw {
            reader: streams.client_reader,
            writer: streams.client_writer,
            end: Some(end),
        }
    }

    pub fn local(config: ServeConfig) -> Self {
        Self::new(Arc::new(LocalProvider::new()), config)
    }

    pub fn send(&mut self, id: u64, op: Op) {
        let frame = Message::Request(Request { id, op }).to_frame().unwrap();
        write_frame(&mut self.writer, &frame).unwrap();
    }

    pub fn send_bytes(&mut self, bytes: &[u8]) {
        self.writer.write_all(bytes).unwrap();
    }

    pub fn frame(&mut self) -> Option<Frame> {
        read_frame(&mut self.reader).unwrap()
    }

    /// The next response, skipping events; `None` at the end of the stream.
    pub fn reply(&mut self) -> Option<(u64, Reply)> {
        loop {
            match self.frame()? {
                Frame::Control(body) => match Message::from_body(&body).unwrap() {
                    Message::Response(response) => return Some((response.id, response.reply)),
                    Message::Event(_) => {}
                    Message::Request(_) => panic!("the helper sent a request"),
                },
                Frame::Data { .. } => panic!("unexpected data"),
            }
        }
    }

    pub fn ask(&mut self, id: u64, op: Op) -> Reply {
        self.send(id, op);
        let (answered, reply) = self.reply().expect("an answer");
        assert_eq!(answered, id);
        reply
    }

    /// Closes the input and waits for the serve loop to return.
    pub fn finish(mut self) -> ServeEnd {
        let end = self.end.take().unwrap();
        drop(self.writer);
        end.join().unwrap()
    }

    /// Waits for the serve loop to return without closing the input.
    pub fn wait_end(&mut self) -> ServeEnd {
        let end = self.end.take().unwrap();
        let started = Instant::now();
        while !end.is_finished() {
            assert!(started.elapsed() < PATIENCE, "the serve loop did not end");
            std::thread::sleep(Duration::from_millis(5));
        }
        end.join().unwrap()
    }
}

pub fn error_kind(reply: &Reply) -> String {
    match reply {
        Reply::Error { error } => kind(&Err::<(), _>(error.clone())),
        other => format!("not an error: {other:?}"),
    }
}
