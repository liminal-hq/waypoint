// The helper's serve loop: reads requests off a byte stream, checks them, runs them against a
// provider and writes the answers.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::HashMap;
use std::fmt;
use std::io::{Read, Write};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use waypoint_path::VfsPath;
use waypoint_protocol::{Location, VfsError};
use waypoint_vfs::{
    from_io, CancelToken, Provider, ReadStream, ScannedEntry, Watch, WatchSink, WriteStream,
};

use crate::frame::{read_frame, Frame, FrameError, MAX_CHUNK};
use crate::os_name::WireOs;
use crate::outbox::{Outbox, SendError, WatchQueue};
use crate::policy;
use crate::sync::locked;
use crate::wire::{
    permissions_reply, times_from_wire, write_options, EventBody, Message, Op, Reply, Request,
    WireChange, WireEntry, WireWatchEvent,
};

/// How a serve loop is bounded. The defaults suit the helper; tests shrink them.
#[derive(Debug, Clone)]
pub struct ServeConfig {
    /// How long the helper stays up with no request in flight, no open handle and no watch.
    pub idle: Duration,
    /// Open file handles at once.
    pub max_handles: usize,
    /// Watches at once.
    pub max_watches: usize,
    /// Requests in flight at once; one more is refused with an answer.
    pub max_in_flight: usize,
    /// Threads that run requests, so a slow listing does not hold up the rest.
    pub workers: usize,
    /// Events a watch holds for a reader that has fallen behind before they become one `Rescan`.
    pub watch_queue: usize,
}

impl Default for ServeConfig {
    fn default() -> Self {
        Self {
            idle: Duration::from_secs(60),
            max_handles: 64,
            max_watches: 16,
            max_in_flight: 64,
            workers: 4,
            watch_queue: 256,
        }
    }
}

/// What the peer did to end the connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fault {
    /// A frame over the size limit.
    Oversize,
    /// The stream ended inside a frame.
    Truncated,
    /// A frame that is not a frame.
    BadFrame,
    /// A message that does not parse, or is not a request.
    Malformed,
    /// A message at a time it is not allowed: a data frame nobody asked for.
    Unexpected,
    /// A request id that is still in use.
    DuplicateId,
}

/// Why `serve` returned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServeEnd {
    /// The client closed its side.
    EndOfInput,
    /// Nothing was in flight, open or watched for `ServeConfig::idle`.
    Idle,
    /// The client broke the protocol; the connection is closed.
    ProtocolError(Fault),
    /// The stream out failed.
    WriteFailed,
    /// A thread could not be started.
    StartFailed,
}

impl fmt::Display for ServeEnd {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ServeEnd::EndOfInput => f.write_str("the client closed the stream"),
            ServeEnd::Idle => f.write_str("idle"),
            ServeEnd::ProtocolError(fault) => write!(f, "protocol error: {fault:?}"),
            ServeEnd::WriteFailed => f.write_str("the stream out failed"),
            ServeEnd::StartFailed => f.write_str("a thread could not be started"),
        }
    }
}

/// What the reader thread hands the loop.
enum FromReader {
    Frame(Frame),
    Ended(Result<(), FrameError>),
}

type Job = Box<dyn FnOnce() + Send>;

/// A fixed set of threads running jobs from one queue.
struct Pool {
    jobs: Option<mpsc::Sender<Job>>,
    threads: Vec<JoinHandle<()>>,
}

impl Pool {
    fn start(workers: usize) -> Option<Pool> {
        let (jobs, queue) = mpsc::channel::<Job>();
        let queue = Arc::new(Mutex::new(queue));
        let mut threads = Vec::new();
        for _ in 0..workers.max(1) {
            let queue = queue.clone();
            let spawned = std::thread::Builder::new()
                .name("elevated-worker".to_owned())
                .spawn(move || loop {
                    let job = locked(&queue).recv();
                    match job {
                        Ok(job) => job(),
                        Err(_) => break,
                    }
                });
            match spawned {
                Ok(handle) => threads.push(handle),
                Err(_) => break,
            }
        }
        if threads.is_empty() {
            return None;
        }
        Some(Pool {
            jobs: Some(jobs),
            threads,
        })
    }

    fn submit(&self, job: Job) {
        if let Some(jobs) = &self.jobs {
            let _ = jobs.send(job);
        }
    }

    /// Lets the queued jobs finish and waits for the threads.
    fn finish(mut self) {
        self.jobs = None;
        for thread in self.threads.drain(..) {
            let _ = thread.join();
        }
    }
}

enum HandleKind {
    Read(ReadStream),
    Write(Box<dyn WriteStream>),
}

/// An open file. The slot is emptied when the handle finishes, so a late request finds it stale.
struct Handle {
    kind: Option<HandleKind>,
    location: Location,
}

/// A running watch; dropping it stops the provider's watcher and then the emitter.
struct ActiveWatch {
    watch: Option<Box<dyn Watch>>,
    queue: Arc<WatchQueue>,
}

impl Drop for ActiveWatch {
    fn drop(&mut self) {
        self.watch.take();
        self.queue.close();
    }
}

/// What a request answers with. It lives for one request, so its size does not matter.
#[allow(clippy::large_enum_variant)]
enum Done {
    Reply(Reply),
    Data(Vec<u8>),
}

struct Shared {
    provider: Arc<dyn Provider>,
    out: Outbox,
    config: ServeConfig,
    /// The requests being worked on, each with the token a `Cancel` trips.
    in_flight: Mutex<HashMap<u64, CancelToken>>,
    handles: Mutex<HashMap<u64, Arc<Mutex<Handle>>>>,
    watches: Mutex<HashMap<u64, ActiveWatch>>,
    next_handle: AtomicU64,
}

fn io_error(message: &str) -> VfsError {
    VfsError::Io {
        message: message.to_owned(),
        location: None,
    }
}

fn from_send(error: SendError) -> VfsError {
    match error {
        SendError::TooLarge => io_error("a message is too large"),
        SendError::Broken => io_error("the stream out failed"),
    }
}

/// Lets a progress report through at most this often, so a fast listing is not one event per entry.
struct Throttle {
    last: Option<Instant>,
}

impl Throttle {
    const EVERY: Duration = Duration::from_millis(50);

    fn new() -> Self {
        Self { last: None }
    }

    fn ready(&mut self) -> bool {
        let now = Instant::now();
        if self.last.is_some_and(|last| now - last < Self::EVERY) {
            return false;
        }
        self.last = Some(now);
        true
    }
}

/// Serves requests read from `reader`, answering on `writer`, with `provider` doing the work. It
/// returns when the client closes the stream, when it breaks the protocol, or after
/// `config.idle` with nothing in flight, open or watched, so the helper keeps no state beyond the
/// connection and does not outlive its use.
///
/// The reader runs on a thread of its own because a blocking read cannot be given a timeout; when
/// the loop ends for another reason than the end of the input, that thread is left to end with its
/// stream.
pub fn serve<R, W>(
    reader: R,
    writer: W,
    provider: Arc<dyn Provider>,
    config: ServeConfig,
) -> ServeEnd
where
    R: Read + Send + 'static,
    W: Write + Send + 'static,
{
    let idle = config.idle;
    let workers = config.workers;
    let shared = Arc::new(Shared {
        provider,
        out: Outbox::new(Box::new(writer)),
        config,
        in_flight: Mutex::new(HashMap::new()),
        handles: Mutex::new(HashMap::new()),
        watches: Mutex::new(HashMap::new()),
        next_handle: AtomicU64::new(1),
    });
    let Some(pool) = Pool::start(workers) else {
        return ServeEnd::StartFailed;
    };
    let (frames, incoming) = mpsc::sync_channel::<FromReader>(16);
    if start_reader(reader, frames).is_err() {
        pool.finish();
        return ServeEnd::StartFailed;
    }
    let end = shared.run(&incoming, &pool, idle);
    shared.stop(pool);
    end
}

fn start_reader<R: Read + Send + 'static>(
    mut reader: R,
    frames: SyncSender<FromReader>,
) -> std::io::Result<JoinHandle<()>> {
    std::thread::Builder::new()
        .name("elevated-reader".to_owned())
        .spawn(move || loop {
            let message = match read_frame(&mut reader) {
                Ok(Some(frame)) => FromReader::Frame(frame),
                Ok(None) => FromReader::Ended(Ok(())),
                Err(error) => FromReader::Ended(Err(error)),
            };
            let last = matches!(message, FromReader::Ended(_));
            if frames.send(message).is_err() || last {
                break;
            }
        })
}

impl Shared {
    fn run(
        self: &Arc<Self>,
        incoming: &Receiver<FromReader>,
        pool: &Pool,
        idle: Duration,
    ) -> ServeEnd {
        let tick = (idle / 4).clamp(Duration::from_millis(10), Duration::from_millis(250));
        let mut idle_since = Instant::now();
        // A `Write` request waits for the data frame that follows it: (request id, handle).
        let mut awaiting: Option<(u64, u64)> = None;
        loop {
            match incoming.recv_timeout(tick) {
                Ok(FromReader::Frame(frame)) => {
                    if let Err(end) = self.on_frame(frame, &mut awaiting, pool) {
                        return end;
                    }
                }
                Ok(FromReader::Ended(Ok(()))) | Err(RecvTimeoutError::Disconnected) => {
                    return ServeEnd::EndOfInput;
                }
                Ok(FromReader::Ended(Err(error))) => return end_of(&error),
                Err(RecvTimeoutError::Timeout) => {}
            }
            if self.out.is_broken() {
                return ServeEnd::WriteFailed;
            }
            if awaiting.is_some() || self.busy() {
                idle_since = Instant::now();
            } else if idle_since.elapsed() >= idle {
                return ServeEnd::Idle;
            }
        }
    }

    fn busy(&self) -> bool {
        !locked(&self.in_flight).is_empty()
            || !locked(&self.handles).is_empty()
            || !locked(&self.watches).is_empty()
    }

    /// Ends everything that belongs to the connection: work in flight is cancelled and awaited,
    /// open files are closed and watches stop.
    fn stop(&self, pool: Pool) {
        for token in locked(&self.in_flight).values() {
            token.cancel();
        }
        locked(&self.watches).clear();
        locked(&self.handles).clear();
        pool.finish();
        locked(&self.handles).clear();
    }

    fn on_frame(
        self: &Arc<Self>,
        frame: Frame,
        awaiting: &mut Option<(u64, u64)>,
        pool: &Pool,
    ) -> Result<(), ServeEnd> {
        let fault = |fault| ServeEnd::ProtocolError(fault);
        match frame {
            Frame::Control(body) => {
                if awaiting.is_some() {
                    return Err(fault(Fault::Unexpected));
                }
                let request = match Message::from_body(&body) {
                    Ok(Message::Request(request)) => request,
                    Ok(_) => return Err(fault(Fault::Unexpected)),
                    Err(_) => return Err(fault(Fault::Malformed)),
                };
                self.on_request(request, awaiting, pool)
            }
            Frame::Data { id, bytes } => match awaiting.take() {
                Some((request, handle)) if request == id => {
                    self.start(id, Op::Write { handle }, Some(bytes), pool)
                }
                _ => Err(fault(Fault::Unexpected)),
            },
        }
    }

    fn on_request(
        self: &Arc<Self>,
        request: Request,
        awaiting: &mut Option<(u64, u64)>,
        pool: &Pool,
    ) -> Result<(), ServeEnd> {
        let Request { id, op } = request;
        log::trace!("elevated: request {id} {}", op.kind());
        match op {
            // These are quick and must not wait behind slow work, so they run in the loop.
            Op::Cancel { target } => {
                if let Some(token) = locked(&self.in_flight).get(&target) {
                    token.cancel();
                }
                self.answer(id, Done::Reply(Reply::Unit))
            }
            Op::Unwatch { watch } => {
                let removed = locked(&self.watches).remove(&watch);
                drop(removed);
                self.answer(id, Done::Reply(Reply::Unit))
            }
            Op::CloseHandle { handle } => {
                let removed = locked(&self.handles).remove(&handle);
                drop(removed);
                self.answer(id, Done::Reply(Reply::Unit))
            }
            Op::Write { handle } => {
                if locked(&self.in_flight).contains_key(&id) {
                    return Err(ServeEnd::ProtocolError(Fault::DuplicateId));
                }
                *awaiting = Some((id, handle));
                Ok(())
            }
            other => self.start(id, other, None, pool),
        }
    }

    /// Admits a request to the pool, or answers that there is no room.
    fn start(
        self: &Arc<Self>,
        id: u64,
        op: Op,
        data: Option<Vec<u8>>,
        pool: &Pool,
    ) -> Result<(), ServeEnd> {
        let token = CancelToken::new();
        {
            let mut in_flight = locked(&self.in_flight);
            if in_flight.contains_key(&id) {
                return Err(ServeEnd::ProtocolError(Fault::DuplicateId));
            }
            if let Err(error) =
                policy::admit(in_flight.len(), self.config.max_in_flight, "requests")
            {
                drop(in_flight);
                return self.answer(id, Done::Reply(Reply::Error { error }));
            }
            in_flight.insert(id, token.clone());
        }
        let shared = self.clone();
        pool.submit(Box::new(move || {
            let outcome = catch_unwind(AssertUnwindSafe(|| shared.execute(id, op, data, &token)));
            let done = match outcome {
                Ok(Ok(done)) => done,
                Ok(Err(error)) => Done::Reply(Reply::Error { error }),
                Err(_) => Done::Reply(Reply::Error {
                    error: io_error("the helper failed on this request"),
                }),
            };
            let _ = shared.answer(id, done);
            locked(&shared.in_flight).remove(&id);
        }));
        Ok(())
    }

    fn answer(&self, id: u64, done: Done) -> Result<(), ServeEnd> {
        let sent = match done {
            Done::Reply(reply) => self.out.reply(id, reply),
            Done::Data(bytes) => self.out.data(id, bytes),
        };
        sent.map_err(|_| ServeEnd::WriteFailed)
    }

    fn event(&self, id: u64, body: EventBody) {
        let _ = self.out.event(id, body);
    }

    /// Sends `items` as events of at most 256 each, halving a batch that would not fit a frame.
    fn send_batched<T>(
        &self,
        id: u64,
        items: &[T],
        make: &dyn Fn(&[T]) -> EventBody,
    ) -> Result<(), VfsError> {
        for chunk in items.chunks(256) {
            self.send_split(id, chunk, make)?;
        }
        Ok(())
    }

    fn send_split<T>(
        &self,
        id: u64,
        chunk: &[T],
        make: &dyn Fn(&[T]) -> EventBody,
    ) -> Result<(), VfsError> {
        match self.out.event(id, make(chunk)) {
            Ok(()) => Ok(()),
            Err(SendError::TooLarge) if chunk.len() > 1 => {
                let (first, second) = chunk.split_at(chunk.len() / 2);
                self.send_split(id, first, make)?;
                self.send_split(id, second, make)
            }
            Err(error) => Err(from_send(error)),
        }
    }

    fn send_entries(&self, id: u64, entries: &[ScannedEntry]) -> Result<(), VfsError> {
        let wire: Vec<WireEntry> = entries.iter().map(WireEntry::from).collect();
        self.send_batched(id, &wire, &|chunk| EventBody::Entries {
            entries: chunk.to_vec(),
        })
    }

    fn open_handle(&self, kind: HandleKind, location: Location) -> Result<Reply, VfsError> {
        let mut handles = locked(&self.handles);
        policy::admit(handles.len(), self.config.max_handles, "open files")?;
        let handle = self.next_handle.fetch_add(1, Ordering::Relaxed);
        handles.insert(
            handle,
            Arc::new(Mutex::new(Handle {
                kind: Some(kind),
                location,
            })),
        );
        Ok(Reply::Handle { handle })
    }

    fn handle(&self, handle: u64) -> Result<Arc<Mutex<Handle>>, VfsError> {
        locked(&self.handles)
            .get(&handle)
            .cloned()
            .ok_or(VfsError::StaleHandle)
    }

    fn execute(
        self: &Arc<Self>,
        id: u64,
        op: Op,
        data: Option<Vec<u8>>,
        cancel: &CancelToken,
    ) -> Result<Done, VfsError> {
        let provider = &*self.provider;
        let reply = match op {
            Op::Hello => Reply::Caps {
                caps: provider.capabilities().into(),
            },
            Op::Stat { path } => Reply::Entry {
                entry: (&provider.stat(&policy::path(&path)?)?).into(),
            },
            Op::List {
                path,
                inline_link_budget,
            } => {
                let path = policy::path(&path)?;
                let mut throttle = Throttle::new();
                let entries =
                    provider.list(&path, cancel, inline_link_budget as usize, &mut |count| {
                        if throttle.ready() {
                            self.event(id, EventBody::ListProgress { count });
                        }
                    })?;
                self.send_entries(id, &entries)?;
                Reply::Unit
            }
            Op::ListBatches {
                path,
                inline_link_budget,
            } => {
                let path = policy::path(&path)?;
                let mut failed = None;
                provider.list_batches(
                    &path,
                    cancel,
                    inline_link_budget as usize,
                    &mut |batch| {
                        if failed.is_none() {
                            if let Err(error) = self.send_entries(id, &batch) {
                                cancel.cancel();
                                failed = Some(error);
                            }
                        }
                    },
                )?;
                if let Some(error) = failed {
                    return Err(error);
                }
                Reply::Unit
            }
            Op::ResolveLink { folder, entry } => {
                let folder = policy::path(&folder)?;
                let entry = ScannedEntry::try_from(entry).map_err(|_| VfsError::InvalidName {
                    name: String::new(),
                    reason: "not a single file name".to_owned(),
                })?;
                Reply::Entry {
                    entry: (&provider.resolve_link(&folder, &entry)?).into(),
                }
            }
            Op::Watch { path } => self.start_watch(id, &policy::path(&path)?)?,
            Op::CreateDir { path } => {
                provider.create_dir(&policy::path(&path)?)?;
                Reply::Unit
            }
            Op::CreateFile { path } => {
                provider.create_file(&policy::path(&path)?)?;
                Reply::Unit
            }
            Op::Rename {
                from,
                to,
                overwrite,
            } => {
                provider.rename(&policy::path(&from)?, &policy::path(&to)?, overwrite)?;
                Reply::Unit
            }
            Op::RemoveFile { path } => {
                provider.remove_file(&policy::path(&path)?)?;
                Reply::Unit
            }
            Op::RemoveDir { path } => {
                provider.remove_dir(&policy::path(&path)?)?;
                Reply::Unit
            }
            Op::OpenRead { path } => {
                let path = policy::path(&path)?;
                let stream = provider.open_read(&path)?;
                self.open_handle(HandleKind::Read(stream), path.to_location())?
            }
            Op::OpenReadAt { path, start } => {
                let path = policy::path(&path)?;
                let stream = provider.open_read_at(&path, start)?;
                self.open_handle(HandleKind::Read(stream), path.to_location())?
            }
            Op::Read { handle, len } => return self.read(handle, len),
            Op::Details { path } => Reply::Details {
                details: provider.details(&policy::path(&path)?)?,
            },
            Op::FolderSize { path } => {
                let path = policy::path(&path)?;
                let mut throttle = Throttle::new();
                let run = provider.folder_size(&path, cancel, &mut |totals| {
                    if throttle.ready() {
                        self.event(id, EventBody::FolderSize { totals: *totals });
                    }
                })?;
                Reply::FolderSize {
                    totals: run.totals,
                    cancelled: run.cancelled,
                }
            }
            Op::CreateWrite {
                path,
                exclusive,
                mode,
            } => {
                let path = policy::path(&path)?;
                let stream = provider.create_write(&path, write_options(exclusive, mode))?;
                self.open_handle(HandleKind::Write(stream), path.to_location())?
            }
            Op::ResumeWrite { path, offset } => {
                let path = policy::path(&path)?;
                let stream = provider.resume_write(&path, offset)?;
                self.open_handle(HandleKind::Write(stream), path.to_location())?
            }
            Op::Write { handle } => return self.write(handle, data.unwrap_or_default()),
            Op::FinishWrite { handle, sync } => return self.finish_write(handle, sync),
            Op::SetTimes {
                path,
                accessed,
                modified,
            } => {
                let path = policy::path(&path)?;
                let times = times_from_wire(accessed, modified)
                    .map_err(|_| io_error("a time out of range"))?;
                provider.set_times(&path, times)?;
                Reply::Unit
            }
            Op::Permissions { path } => {
                permissions_reply(provider.permissions(&policy::path(&path)?)?)
            }
            Op::SetPermissions {
                path,
                mode,
                readonly,
            } => {
                provider.set_permissions(
                    &policy::path(&path)?,
                    waypoint_vfs::Permissions { mode, readonly },
                )?;
                Reply::Unit
            }
            Op::Symlink { link, target } => {
                let target = target
                    .to_os_string()
                    .map_err(|_| io_error("a link target of another platform"))?;
                provider.symlink(&policy::path(&link)?, &target)?;
                Reply::Unit
            }
            Op::ReadLink { path } => Reply::Name {
                name: WireOs::from_os(&provider.read_link(&policy::path(&path)?)?),
            },
            Op::Canonicalize { path } => match provider.canonicalize(&policy::path(&path)?)? {
                VfsPath::File(resolved) => Reply::Path {
                    path: WireOs::from_os(resolved.as_path().as_os_str()),
                },
                _ => {
                    return Err(VfsError::Unsupported {
                        what: "resolving symlinks here".to_owned(),
                    })
                }
            },
            Op::VolumeId { path } => Reply::Volume {
                id: provider.volume_id(&policy::path(&path)?),
            },
            Op::FreeSpace { path } => Reply::Space {
                space: provider.free_space(&policy::path(&path)?),
            },
            Op::CopyFileWithin { src, dst } => {
                let (src, dst) = (policy::path(&src)?, policy::path(&dst)?);
                let mut throttle = Throttle::new();
                let copied = provider.copy_file_within(
                    &src,
                    &dst,
                    &mut |bytes| {
                        if throttle.ready() {
                            self.event(id, EventBody::CopyProgress { bytes });
                        }
                    },
                    cancel,
                );
                match copied {
                    None => Reply::Copied { bytes: None },
                    Some(Ok(bytes)) => Reply::Copied { bytes: Some(bytes) },
                    Some(Err(error)) => return Err(error),
                }
            }
            // Handled by the loop and never started.
            Op::Unwatch { .. } | Op::Cancel { .. } | Op::CloseHandle { .. } => {
                return Err(io_error("not a pooled request"));
            }
        };
        Ok(Done::Reply(reply))
    }

    fn read(&self, handle: u64, len: u32) -> Result<Done, VfsError> {
        let slot = self.handle(handle)?;
        let mut slot = locked(&slot);
        let location = slot.location.clone();
        let Some(HandleKind::Read(stream)) = slot.kind.as_mut() else {
            return Err(VfsError::StaleHandle);
        };
        let mut buf = vec![0u8; (len as usize).min(MAX_CHUNK)];
        let mut filled = 0;
        while filled < buf.len() {
            match stream.read(&mut buf[filled..]) {
                Ok(0) => break,
                Ok(n) => filled += n,
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                Err(error) => return Err(from_io(&error, &location)),
            }
        }
        buf.truncate(filled);
        Ok(Done::Data(buf))
    }

    fn write(&self, handle: u64, bytes: Vec<u8>) -> Result<Done, VfsError> {
        let slot = self.handle(handle)?;
        let mut slot = locked(&slot);
        let location = slot.location.clone();
        let Some(HandleKind::Write(stream)) = slot.kind.as_mut() else {
            return Err(VfsError::StaleHandle);
        };
        stream
            .write_all(&bytes)
            .map_err(|error| from_io(&error, &location))?;
        Ok(Done::Reply(Reply::Unit))
    }

    fn finish_write(&self, handle: u64, sync: bool) -> Result<Done, VfsError> {
        let Some(slot) = locked(&self.handles).remove(&handle) else {
            return Err(VfsError::StaleHandle);
        };
        let kind = locked(&slot).kind.take();
        match kind {
            Some(HandleKind::Write(stream)) => {
                stream.finish(sync)?;
                Ok(Done::Reply(Reply::Unit))
            }
            _ => Err(VfsError::StaleHandle),
        }
    }

    /// Starts `provider.watch` on a folder. The sink only fills a bounded queue; an emitter thread
    /// owns the writing, so the watcher is never held up by the stream.
    fn start_watch(self: &Arc<Self>, id: u64, path: &VfsPath) -> Result<Reply, VfsError> {
        policy::admit(
            locked(&self.watches).len(),
            self.config.max_watches,
            "watches",
        )?;
        let queue = Arc::new(WatchQueue::new(self.config.watch_queue));
        let sink: WatchSink = {
            let queue = queue.clone();
            Arc::new(move |event| queue.push(event))
        };
        let watch = self.provider.watch(path, sink)?;
        let active = ActiveWatch {
            watch: Some(watch),
            queue: queue.clone(),
        };
        let shared = self.clone();
        let emitter = std::thread::Builder::new()
            .name("elevated-watch".to_owned())
            .spawn(move || shared.emit(id, &queue));
        if emitter.is_err() {
            return Err(io_error("a thread could not be started"));
        }
        let mut watches = locked(&self.watches);
        // A refusal drops `active`: the watcher stops and the emitter ends.
        policy::admit(watches.len(), self.config.max_watches, "watches")?;
        if watches.contains_key(&id) {
            return Err(io_error("a watch with this id exists"));
        }
        watches.insert(id, active);
        Ok(Reply::Unit)
    }

    /// Writes a watch's events, in order, until its queue is closed or the stream is gone.
    fn emit(&self, id: u64, queue: &WatchQueue) {
        while let Some(event) = queue.pop() {
            if self.out.is_broken() {
                return;
            }
            let sent = match WireWatchEvent::from(&event) {
                WireWatchEvent::Changes { changes } => {
                    self.send_batched(id, &changes, &|chunk: &[WireChange]| EventBody::Watch {
                        event: WireWatchEvent::Changes {
                            changes: chunk.to_vec(),
                        },
                    })
                }
                other => self
                    .out
                    .event(id, EventBody::Watch { event: other })
                    .map_err(from_send),
            };
            if sent.is_err() && self.out.is_broken() {
                return;
            }
        }
    }
}

fn end_of(error: &FrameError) -> ServeEnd {
    match error {
        FrameError::Io(_) => ServeEnd::EndOfInput,
        FrameError::TooLarge => ServeEnd::ProtocolError(Fault::Oversize),
        FrameError::Truncated => ServeEnd::ProtocolError(Fault::Truncated),
        FrameError::Empty
        | FrameError::UnknownKind(_)
        | FrameError::BadData
        | FrameError::Encode => ServeEnd::ProtocolError(Fault::BadFrame),
    }
}
