// The client: a `Provider` for the `admin:` scheme that sends each call to a privileged helper.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::ffi::{OsStr, OsString};
use std::io::{Read, Write};
use std::sync::mpsc::RecvTimeoutError;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use waypoint_path::{ConnectionKey, VfsPath, ELEVATED_SCHEME};
use waypoint_protocol::{ConnectionState, Location, VfsError};
use waypoint_vfs::{
    CancelToken, Capabilities, ConnectAnswer, EntryDetails, FileTimes, FolderSizeRun,
    FolderSizeTotals, Permissions, Provider, ReadStream, ScannedEntry, VolumeId, VolumeSpace,
    Watch, WatchSink, WriteStream,
};

use crate::connection::{Connection, Incoming};
use crate::paths::{from_helper, rewrite_error, to_helper};
use crate::streams::{RemoteRead, RemoteWrite};
use crate::sync::locked;
use crate::wire::{entries_from_wire, times_to_wire, EventBody, Op, Reply, WireEntry};

/// How often a call waiting for its answer looks at its cancel token.
const CANCEL_POLL: Duration = Duration::from_millis(20);

/// How long a helper that has started may take to say hello before it is given up on. The prompt
/// that starts it is the launcher's, and is over by then; a helper that is running and silent is
/// not one to wait for.
const HELLO_TIMEOUT: Duration = Duration::from_secs(15);

/// A duplex byte stream to a helper: what it writes goes to the helper's input, and what the
/// helper writes comes back on `reader`. Dropping the writer is how the client tells the helper
/// that it is finished.
pub struct Transport {
    pub reader: Box<dyn Read + Send>,
    pub writer: Box<dyn Write + Send>,
}

/// Starts a helper and returns the stream to it. How is the launcher's business (the system's
/// prompt, a pipe, a thread in a test); the client only ever calls it from an explicit `connect`.
///
/// `cancel` is the connecting call's own token. A launcher that can wait on a person (the
/// system's prompt) watches it and gives the wait up with `VfsError::Cancelled` once it is set.
pub trait Launcher: Send + Sync {
    fn launch(&self, cancel: &CancelToken) -> Result<Transport, VfsError>;
}

/// The place the connection as a whole is at, for an error that has no path of its own.
fn root() -> Location {
    Location::new("/", format!("{ELEVATED_SCHEME}:///"))
}

fn disconnected(location: &Location) -> VfsError {
    VfsError::Disconnected {
        location: location.clone(),
    }
}

enum State {
    Idle,
    Connecting,
    Connected(Arc<Connection>),
    Failed(VfsError),
}

/// What one request comes back with. It lives for one call, so its size does not matter.
#[allow(clippy::large_enum_variant)]
pub(crate) enum Answer {
    Reply(Reply),
    Data(Vec<u8>),
}

/// What a request sends: an operation, or a chunk of a file to write.
pub(crate) enum Outgoing {
    Op(Op),
    Write { handle: u64, bytes: Vec<u8> },
}

pub(crate) struct Inner {
    launcher: Box<dyn Launcher>,
    state: Mutex<State>,
    /// Held while a launch is under way, so two `connect` calls never start two helpers.
    connecting: Mutex<()>,
    caps: Mutex<Option<Capabilities>>,
    hello_timeout: Duration,
}

impl Inner {
    /// The connection to use. Nothing is started here: a call made while not connected is refused.
    fn live(&self, location: &Location) -> Result<Arc<Connection>, VfsError> {
        match &*locked(&self.state) {
            State::Connected(conn) if !conn.is_dead() => Ok(conn.clone()),
            _ => Err(disconnected(location)),
        }
    }

    /// Notes that `conn` has ended. If it was the live connection the state becomes `Failed`; nothing
    /// reconnects by itself, because a reconnection is a prompt.
    fn ended(&self, conn: &Arc<Connection>) {
        let mut state = locked(&self.state);
        if matches!(&*state, State::Connected(current) if Arc::ptr_eq(current, conn)) {
            *state = State::Failed(disconnected(&root()));
        }
    }

    /// The helper said something it should not have: the connection is closed.
    pub(crate) fn fault(&self, conn: &Arc<Connection>, location: &Location) -> VfsError {
        conn.shutdown();
        self.ended(conn);
        disconnected(location)
    }

    /// Sends a request and waits for its one answer, handing the events that arrive meanwhile to
    /// `on_event` (which returns `false` for one it does not expect). With a `cancel` token, a
    /// cancel is passed on to the helper as soon as it is seen.
    pub(crate) fn exchange(
        &self,
        conn: &Arc<Connection>,
        location: &Location,
        outgoing: Outgoing,
        cancel: Option<&CancelToken>,
        on_event: &mut dyn FnMut(EventBody) -> bool,
    ) -> Result<Answer, VfsError> {
        self.exchange_as(conn, conn.next_id(), location, outgoing, cancel, on_event)
    }

    fn exchange_as(
        &self,
        conn: &Arc<Connection>,
        id: u64,
        location: &Location,
        outgoing: Outgoing,
        cancel: Option<&CancelToken>,
        on_event: &mut dyn FnMut(EventBody) -> bool,
    ) -> Result<Answer, VfsError> {
        let Some(answers) = conn.register(id) else {
            return Err(disconnected(location));
        };
        let sent = match outgoing {
            Outgoing::Op(op) => conn.send_request(id, op),
            Outgoing::Write { handle, bytes } => conn.send_write(id, handle, bytes),
        };
        if !sent {
            conn.forget(id);
            self.ended(conn);
            return Err(disconnected(location));
        }
        let mut cancel_sent = false;
        loop {
            let incoming = match cancel {
                None => answers.recv().unwrap_or(Incoming::Closed),
                Some(token) => {
                    if token.is_cancelled() && !cancel_sent {
                        cancel_sent = true;
                        conn.send_and_forget(Op::Cancel { target: id });
                    }
                    match answers.recv_timeout(CANCEL_POLL) {
                        Ok(incoming) => incoming,
                        Err(RecvTimeoutError::Timeout) => continue,
                        Err(RecvTimeoutError::Disconnected) => Incoming::Closed,
                    }
                }
            };
            match incoming {
                Incoming::Event(body) => {
                    if !on_event(body) {
                        return Err(self.fault(conn, location));
                    }
                }
                Incoming::Data(bytes) => return Ok(Answer::Data(bytes)),
                Incoming::Reply(Reply::Error { error }) => return Err(rewrite_error(error)),
                Incoming::Reply(reply) => return Ok(Answer::Reply(reply)),
                Incoming::Closed => {
                    self.ended(conn);
                    return Err(disconnected(location));
                }
            }
        }
    }

    /// One request that answers with a reply and sends no events.
    fn ask(&self, location: &Location, op: Op) -> Result<(Arc<Connection>, Reply), VfsError> {
        let conn = self.live(location)?;
        match self.exchange(&conn, location, Outgoing::Op(op), None, &mut |_| false)? {
            Answer::Reply(reply) => Ok((conn, reply)),
            Answer::Data(_) => Err(self.fault(&conn, location)),
        }
    }

    fn unit(&self, location: &Location, op: Op) -> Result<(), VfsError> {
        match self.ask(location, op)? {
            (_, Reply::Unit) => Ok(()),
            (conn, _) => Err(self.fault(&conn, location)),
        }
    }

    fn entry(
        &self,
        conn: &Arc<Connection>,
        location: &Location,
        entry: WireEntry,
    ) -> Result<ScannedEntry, VfsError> {
        ScannedEntry::try_from(entry).map_err(|_| self.fault(conn, location))
    }
}

/// The provider of the `admin:` scheme: the local file system as the administrator sees it,
/// reached through a helper that runs with more privilege.
///
/// It is connected only by an explicit `connect`, which is where the launcher starts the helper
/// (and so where the system's prompt appears). A call made without a connection fails with
/// `Disconnected` and starts nothing. If the stream ends, every call in flight fails with
/// `Disconnected`, every watch gets `Lost`, and the connection stays down until the next `connect`.
pub struct ElevatedProvider {
    inner: Arc<Inner>,
}

impl ElevatedProvider {
    pub fn new(launcher: Box<dyn Launcher>) -> Self {
        Self {
            inner: Arc::new(Inner {
                launcher,
                state: Mutex::new(State::Idle),
                connecting: Mutex::new(()),
                caps: Mutex::new(None),
                hello_timeout: HELLO_TIMEOUT,
            }),
        }
    }

    /// How long a started helper may take to say hello. Set before the provider is shared.
    pub fn with_hello_timeout(mut self, timeout: Duration) -> Self {
        if let Some(inner) = Arc::get_mut(&mut self.inner) {
            inner.hello_timeout = timeout;
        }
        self
    }

    fn target(&self, path: &VfsPath) -> Result<(crate::os_name::WireOs, Location), VfsError> {
        Ok((to_helper(path)?, path.to_location()))
    }

    /// What the helper's provider can do, as seen from here: never remote and always watched, and
    /// the helper's own `write`, rename, permissions and the rest.
    fn adjust(mut caps: Capabilities) -> Capabilities {
        caps.remote = false;
        caps.watch = true;
        caps.server_copy = true;
        caps.atomic_write = false;
        caps
    }

    fn default_capabilities() -> Capabilities {
        Self::adjust(Capabilities::local())
    }

    fn open(&self, path: &VfsPath, op: Op) -> Result<(Arc<Connection>, u64, Location), VfsError> {
        let location = path.to_location();
        match self.inner.ask(&location, op)? {
            (conn, Reply::Handle { handle }) => Ok((conn, handle, location)),
            (conn, _) => Err(self.inner.fault(&conn, &location)),
        }
    }

    fn run_connect(&self, cancel: &CancelToken) -> Result<(), VfsError> {
        let inner = &self.inner;
        let _launching = locked(&inner.connecting);
        if matches!(&*locked(&inner.state), State::Connected(conn) if !conn.is_dead()) {
            return Ok(());
        }
        if cancel.is_cancelled() {
            return Err(VfsError::Cancelled);
        }
        *locked(&inner.state) = State::Connecting;
        let fail = |error: VfsError| {
            *locked(&inner.state) = State::Failed(error.clone());
            Err(error)
        };
        let transport = match inner.launcher.launch(cancel) {
            Ok(transport) => transport,
            Err(error) => return fail(error),
        };
        if cancel.is_cancelled() {
            drop(transport);
            *locked(&inner.state) = State::Idle;
            return Err(VfsError::Cancelled);
        }
        let Transport { reader, writer } = transport;
        let conn = Arc::new(Connection::new(writer));
        let reading = {
            let (conn, inner) = (conn.clone(), inner.clone());
            std::thread::Builder::new()
                .name("elevated-client".to_owned())
                .spawn(move || {
                    conn.read_until_end(reader);
                    inner.ended(&conn);
                })
        };
        if reading.is_err() {
            conn.shutdown();
            return fail(VfsError::Io {
                message: "a thread could not be started".to_owned(),
                location: None,
            });
        }
        let location = root();
        // A helper that starts and then never answers must not hang `connect`: the watchdog closes
        // the connection after `hello_timeout`, which ends the wait below as `Disconnected`.
        let (answered, waiting) = std::sync::mpsc::channel::<()>();
        let watchdog = {
            let (conn, timeout) = (conn.clone(), inner.hello_timeout);
            std::thread::Builder::new()
                .name("elevated-hello".to_owned())
                .spawn(move || {
                    if let Err(RecvTimeoutError::Timeout) = waiting.recv_timeout(timeout) {
                        conn.shutdown();
                    }
                })
        };
        if watchdog.is_err() {
            conn.shutdown();
            return fail(VfsError::Io {
                message: "a thread could not be started".to_owned(),
                location: None,
            });
        }
        let hello = inner.exchange(&conn, &location, Outgoing::Op(Op::Hello), None, &mut |_| {
            false
        });
        drop(answered);
        match hello {
            Ok(Answer::Reply(Reply::Caps { caps })) => {
                *locked(&inner.caps) = Some(Self::adjust(caps.into()));
                *locked(&inner.state) = State::Connected(conn);
                Ok(())
            }
            Ok(_) => {
                conn.shutdown();
                fail(disconnected(&location))
            }
            Err(error) => {
                conn.shutdown();
                fail(error)
            }
        }
    }
}

impl Drop for ElevatedProvider {
    fn drop(&mut self) {
        self.disconnect(&ConnectionKey::elevated());
    }
}

/// Keeps a watch alive; dropping it tells the helper and stops the events.
struct RemoteWatch {
    conn: Arc<Connection>,
    id: u64,
}

impl Watch for RemoteWatch {}

impl Drop for RemoteWatch {
    fn drop(&mut self) {
        if self.conn.remove_watch(self.id) {
            self.conn.send_and_forget(Op::Unwatch { watch: self.id });
        }
    }
}

impl Provider for ElevatedProvider {
    fn scheme(&self) -> &'static str {
        ELEVATED_SCHEME
    }

    fn capabilities(&self) -> Capabilities {
        locked(&self.inner.caps).unwrap_or_else(Self::default_capabilities)
    }

    fn read_only(&self) -> bool {
        false
    }

    fn stat(&self, path: &VfsPath) -> Result<ScannedEntry, VfsError> {
        let (target, location) = self.target(path)?;
        match self.inner.ask(&location, Op::Stat { path: target })? {
            (conn, Reply::Entry { entry }) => self.inner.entry(&conn, &location, entry),
            (conn, _) => Err(self.inner.fault(&conn, &location)),
        }
    }

    fn list(
        &self,
        path: &VfsPath,
        cancel: &CancelToken,
        inline_link_budget: usize,
        progress: &mut dyn FnMut(u32),
    ) -> Result<Vec<ScannedEntry>, VfsError> {
        let (target, location) = self.target(path)?;
        if cancel.is_cancelled() {
            return Err(VfsError::Cancelled);
        }
        let conn = self.inner.live(&location)?;
        let mut entries = Vec::new();
        let op = Op::List {
            path: target,
            inline_link_budget: budget(inline_link_budget),
        };
        let answer = self.inner.exchange(
            &conn,
            &location,
            Outgoing::Op(op),
            Some(cancel),
            &mut |event| match event {
                EventBody::ListProgress { count } => {
                    progress(count);
                    true
                }
                EventBody::Entries { entries: batch } => match entries_from_wire(batch) {
                    Ok(batch) => {
                        entries.extend(batch);
                        true
                    }
                    Err(_) => false,
                },
                _ => false,
            },
        )?;
        match answer {
            Answer::Reply(Reply::Unit) => Ok(entries),
            _ => Err(self.inner.fault(&conn, &location)),
        }
    }

    fn resolve_link(
        &self,
        folder: &VfsPath,
        entry: &ScannedEntry,
    ) -> Result<ScannedEntry, VfsError> {
        let (target, location) = self.target(folder)?;
        let op = Op::ResolveLink {
            folder: target,
            entry: entry.into(),
        };
        match self.inner.ask(&location, op)? {
            (conn, Reply::Entry { entry }) => self.inner.entry(&conn, &location, entry),
            (conn, _) => Err(self.inner.fault(&conn, &location)),
        }
    }

    fn watch(&self, path: &VfsPath, sink: WatchSink) -> Result<Box<dyn Watch>, VfsError> {
        let (target, location) = self.target(path)?;
        let VfsPath::Elevated(elevated) = path else {
            return Err(VfsError::InvalidLocation {
                input: path.to_uri(),
            });
        };
        let conn = self.inner.live(&location)?;
        // The watch is known before the request goes out, so no event can arrive ahead of it.
        let id = conn.next_id();
        conn.add_watch(id, sink, elevated.file().clone(), location.clone());
        let started = self.inner.exchange_as(
            &conn,
            id,
            &location,
            Outgoing::Op(Op::Watch { path: target }),
            None,
            &mut |_| false,
        );
        match started {
            Ok(Answer::Reply(Reply::Unit)) => Ok(Box::new(RemoteWatch { conn, id })),
            Ok(_) => {
                conn.remove_watch(id);
                Err(self.inner.fault(&conn, &location))
            }
            Err(error) => {
                conn.remove_watch(id);
                Err(error)
            }
        }
    }

    fn list_batches(
        &self,
        path: &VfsPath,
        cancel: &CancelToken,
        inline_link_budget: usize,
        sink: &mut dyn FnMut(Vec<ScannedEntry>),
    ) -> Result<(), VfsError> {
        let (target, location) = self.target(path)?;
        if cancel.is_cancelled() {
            return Err(VfsError::Cancelled);
        }
        let conn = self.inner.live(&location)?;
        let op = Op::ListBatches {
            path: target,
            inline_link_budget: budget(inline_link_budget),
        };
        let answer = self.inner.exchange(
            &conn,
            &location,
            Outgoing::Op(op),
            Some(cancel),
            &mut |event| match event {
                EventBody::Entries { entries } => match entries_from_wire(entries) {
                    Ok(batch) => {
                        sink(batch);
                        true
                    }
                    Err(_) => false,
                },
                _ => false,
            },
        )?;
        match answer {
            Answer::Reply(Reply::Unit) => Ok(()),
            _ => Err(self.inner.fault(&conn, &location)),
        }
    }

    fn connection_key(&self, path: &VfsPath) -> Option<ConnectionKey> {
        matches!(path, VfsPath::Elevated(_)).then(ConnectionKey::elevated)
    }

    fn connection_state(&self, key: &ConnectionKey) -> ConnectionState {
        if *key != ConnectionKey::elevated() {
            return ConnectionState::Idle;
        }
        match &*locked(&self.inner.state) {
            State::Idle => ConnectionState::Idle,
            State::Connecting => ConnectionState::Connecting,
            State::Connected(conn) if conn.is_dead() => ConnectionState::Failed {
                error: disconnected(&root()),
            },
            State::Connected(_) => ConnectionState::Connected,
            State::Failed(error) => ConnectionState::Failed {
                error: error.clone(),
            },
        }
    }

    /// Starts the helper. This is the one place anything is launched: the system's prompt appears
    /// here and nowhere else. Connecting while connected does nothing.
    fn connect(
        &self,
        key: &ConnectionKey,
        _answer: Option<ConnectAnswer>,
        cancel: &CancelToken,
    ) -> Result<(), VfsError> {
        if *key != ConnectionKey::elevated() {
            return Err(VfsError::Unsupported {
                what: "connecting to another login here".to_owned(),
            });
        }
        self.run_connect(cancel)
    }

    /// Closes the stream, which ends the helper, and fails whatever was in flight with
    /// `Disconnected`.
    fn disconnect(&self, key: &ConnectionKey) {
        if *key != ConnectionKey::elevated() {
            return;
        }
        let previous = std::mem::replace(&mut *locked(&self.inner.state), State::Idle);
        if let State::Connected(conn) = previous {
            conn.shutdown();
        }
    }

    fn create_dir(&self, path: &VfsPath) -> Result<(), VfsError> {
        let (target, location) = self.target(path)?;
        self.inner.unit(&location, Op::CreateDir { path: target })
    }

    fn create_file(&self, path: &VfsPath) -> Result<(), VfsError> {
        let (target, location) = self.target(path)?;
        self.inner.unit(&location, Op::CreateFile { path: target })
    }

    fn rename(&self, from: &VfsPath, to: &VfsPath, overwrite: bool) -> Result<(), VfsError> {
        let (source, location) = self.target(from)?;
        let (destination, _) = self.target(to)?;
        self.inner.unit(
            &location,
            Op::Rename {
                from: source,
                to: destination,
                overwrite,
            },
        )
    }

    fn remove_file(&self, path: &VfsPath) -> Result<(), VfsError> {
        let (target, location) = self.target(path)?;
        self.inner.unit(&location, Op::RemoveFile { path: target })
    }

    fn remove_dir(&self, path: &VfsPath) -> Result<(), VfsError> {
        let (target, location) = self.target(path)?;
        self.inner.unit(&location, Op::RemoveDir { path: target })
    }

    fn open_read(&self, path: &VfsPath) -> Result<ReadStream, VfsError> {
        let (target, _) = self.target(path)?;
        let (conn, handle, location) = self.open(path, Op::OpenRead { path: target })?;
        Ok(Box::new(RemoteRead::new(
            self.inner.clone(),
            conn,
            handle,
            location,
        )))
    }

    fn open_read_at(&self, path: &VfsPath, start: u64) -> Result<ReadStream, VfsError> {
        let (target, _) = self.target(path)?;
        let op = Op::OpenReadAt {
            path: target,
            start,
        };
        let (conn, handle, location) = self.open(path, op)?;
        Ok(Box::new(RemoteRead::new(
            self.inner.clone(),
            conn,
            handle,
            location,
        )))
    }

    fn details(&self, path: &VfsPath) -> Result<EntryDetails, VfsError> {
        let (target, location) = self.target(path)?;
        match self.inner.ask(&location, Op::Details { path: target })? {
            (_, Reply::Details { details }) => Ok(details),
            (conn, _) => Err(self.inner.fault(&conn, &location)),
        }
    }

    fn folder_size(
        &self,
        path: &VfsPath,
        cancel: &CancelToken,
        report: &mut dyn FnMut(&FolderSizeTotals),
    ) -> Result<FolderSizeRun, VfsError> {
        let (target, location) = self.target(path)?;
        let conn = self.inner.live(&location)?;
        let answer = self.inner.exchange(
            &conn,
            &location,
            Outgoing::Op(Op::FolderSize { path: target }),
            Some(cancel),
            &mut |event| match event {
                EventBody::FolderSize { totals } => {
                    report(&totals);
                    true
                }
                _ => false,
            },
        )?;
        match answer {
            Answer::Reply(Reply::FolderSize { totals, cancelled }) => {
                Ok(FolderSizeRun { totals, cancelled })
            }
            _ => Err(self.inner.fault(&conn, &location)),
        }
    }

    fn create_write(
        &self,
        path: &VfsPath,
        options: waypoint_vfs::WriteOptions,
    ) -> Result<Box<dyn WriteStream>, VfsError> {
        let (target, _) = self.target(path)?;
        let op = Op::CreateWrite {
            path: target,
            exclusive: options.exclusive,
            mode: options.mode,
        };
        let (conn, handle, location) = self.open(path, op)?;
        Ok(Box::new(RemoteWrite::new(
            self.inner.clone(),
            conn,
            handle,
            location,
        )))
    }

    fn resume_write(&self, path: &VfsPath, offset: u64) -> Result<Box<dyn WriteStream>, VfsError> {
        let (target, _) = self.target(path)?;
        let op = Op::ResumeWrite {
            path: target,
            offset,
        };
        let (conn, handle, location) = self.open(path, op)?;
        Ok(Box::new(RemoteWrite::new(
            self.inner.clone(),
            conn,
            handle,
            location,
        )))
    }

    fn set_times(&self, path: &VfsPath, times: FileTimes) -> Result<(), VfsError> {
        let (target, location) = self.target(path)?;
        let (accessed, modified) = times_to_wire(times);
        self.inner.unit(
            &location,
            Op::SetTimes {
                path: target,
                accessed,
                modified,
            },
        )
    }

    fn permissions(&self, path: &VfsPath) -> Result<Permissions, VfsError> {
        let (target, location) = self.target(path)?;
        match self
            .inner
            .ask(&location, Op::Permissions { path: target })?
        {
            (_, Reply::Permissions { mode, readonly }) => Ok(Permissions { mode, readonly }),
            (conn, _) => Err(self.inner.fault(&conn, &location)),
        }
    }

    fn set_permissions(&self, path: &VfsPath, permissions: Permissions) -> Result<(), VfsError> {
        let (target, location) = self.target(path)?;
        self.inner.unit(
            &location,
            Op::SetPermissions {
                path: target,
                mode: permissions.mode,
                readonly: permissions.readonly,
            },
        )
    }

    fn symlink(&self, link: &VfsPath, target: &OsStr) -> Result<(), VfsError> {
        let (at, location) = self.target(link)?;
        self.inner.unit(
            &location,
            Op::Symlink {
                link: at,
                target: crate::os_name::WireOs::from_os(target),
            },
        )
    }

    fn read_link(&self, path: &VfsPath) -> Result<OsString, VfsError> {
        let (target, location) = self.target(path)?;
        match self.inner.ask(&location, Op::ReadLink { path: target })? {
            (conn, Reply::Name { name }) => name
                .to_os_string()
                .map_err(|_| self.inner.fault(&conn, &location)),
            (conn, _) => Err(self.inner.fault(&conn, &location)),
        }
    }

    fn canonicalize(&self, path: &VfsPath) -> Result<VfsPath, VfsError> {
        let (target, location) = self.target(path)?;
        match self
            .inner
            .ask(&location, Op::Canonicalize { path: target })?
        {
            (conn, Reply::Path { path }) => {
                from_helper(&path).ok_or_else(|| self.inner.fault(&conn, &location))
            }
            (conn, _) => Err(self.inner.fault(&conn, &location)),
        }
    }

    fn volume_id(&self, path: &VfsPath) -> Option<VolumeId> {
        let (target, location) = self.target(path).ok()?;
        match self.inner.ask(&location, Op::VolumeId { path: target }) {
            Ok((_, Reply::Volume { id })) => id,
            _ => None,
        }
    }

    fn free_space(&self, path: &VfsPath) -> Option<VolumeSpace> {
        let (target, location) = self.target(path).ok()?;
        match self.inner.ask(&location, Op::FreeSpace { path: target }) {
            Ok((_, Reply::Space { space })) => space,
            _ => None,
        }
    }

    fn copy_file_within(
        &self,
        src: &VfsPath,
        dst: &VfsPath,
        progress: &mut dyn FnMut(u64),
        cancel: &CancelToken,
    ) -> Option<Result<u64, VfsError>> {
        let mut copy = || -> Result<Option<u64>, VfsError> {
            let (from, location) = self.target(src)?;
            let (to, _) = self.target(dst)?;
            let conn = self.inner.live(&location)?;
            let answer = self.inner.exchange(
                &conn,
                &location,
                Outgoing::Op(Op::CopyFileWithin { src: from, dst: to }),
                Some(cancel),
                &mut |event| match event {
                    EventBody::CopyProgress { bytes } => {
                        progress(bytes);
                        true
                    }
                    _ => false,
                },
            )?;
            match answer {
                Answer::Reply(Reply::Copied { bytes }) => Ok(bytes),
                _ => Err(self.inner.fault(&conn, &location)),
            }
        };
        copy().transpose()
    }
}

fn budget(inline_link_budget: usize) -> u32 {
    inline_link_budget.min(u32::MAX as usize) as u32
}
