// An in-memory server for tests: a remote provider over `MemoryProvider` that simulates latency,
// timeouts, going offline, logins, host keys and folders that cannot be watched.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `FakeRemoteProvider` serves one remote scheme (`sftp://…` by default) from a `MemoryProvider`,
//! so the contract of `remote-locations.md` can be exercised without a server: lazy connections
//! and their states, `AuthRequired` answered through `connect` or a `CredentialSource`,
//! `HostKeyUnknown` answered by trusting the key, `Timeout` when the latency passes the timeout,
//! `Unreachable` while offline, listings in batches, resumed writes, and no watching unless a
//! polling interval is set. Every path is mapped to the memory tree by its names, whatever the
//! host, and errors are reported at the remote location that was asked about.

use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::io::{Read, Write};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::Duration;

use waypoint_path::{CaseRule, ConnectionKey, FilePath, RemotePath, RemoteScheme, VfsPath};
use waypoint_protocol::{
    AuthPrompt, ConnectionState, HostKey, Location, UnreachableReason, VfsError,
};

use crate::memory::MemoryProvider;
use crate::provider::{Capabilities, Provider, ScannedEntry, Watch, WatchSink};
use crate::remote::{ConnectAnswer, Credential, CredentialSource, NoCredentials};
use crate::write::{FileTimes, Permissions, ReadStream, VolumeId, WriteOptions, WriteStream};
use crate::{CancelToken, PollWatch, VolumeSpace};

/// A failure the fake server shows to every call until it is cleared.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoteFault {
    /// Nothing answers: `Unreachable` with this reason.
    Unreachable(UnreachableReason),
    /// The server stops answering: `Timeout`.
    Timeout,
    /// The service asks to slow down: `RateLimited`.
    RateLimited,
}

#[derive(Default)]
struct Session {
    state: Option<ConnectionState>,
    authenticated: bool,
}

struct State {
    latency: Duration,
    timeout: Duration,
    fault: Option<RemoteFault>,
    /// The login the server wants, when it wants one.
    password: Option<(Option<String>, String)>,
    host_key: Option<HostKey>,
    host_key_trusted: bool,
    sessions: HashMap<ConnectionKey, Session>,
    connects: usize,
    batch: usize,
    poll: Option<Duration>,
}

/// An in-memory server. Cloning shares it.
#[derive(Clone)]
pub struct FakeRemoteProvider {
    scheme: RemoteScheme,
    base: FilePath,
    memory: MemoryProvider,
    state: Arc<Mutex<State>>,
    credentials: Arc<dyn CredentialSource>,
}

fn base_path() -> FilePath {
    let root = if cfg!(windows) { r"C:\fake" } else { "/fake" };
    FilePath::parse(root).expect("the fake server's root is absolute")
}

/// Rewrites every location a `VfsError` carries.
fn relocate(error: VfsError, map: &dyn Fn(&Location) -> Location) -> VfsError {
    use VfsError as E;
    match error {
        E::NotFound { location } => E::NotFound {
            location: map(&location),
        },
        E::PermissionDenied { location } => E::PermissionDenied {
            location: map(&location),
        },
        E::NotADirectory { location } => E::NotADirectory {
            location: map(&location),
        },
        E::AlreadyExists { location } => E::AlreadyExists {
            location: map(&location),
        },
        E::NotEmpty { location } => E::NotEmpty {
            location: map(&location),
        },
        E::IsADirectory { location } => E::IsADirectory {
            location: map(&location),
        },
        E::CrossesDevices { from, to } => E::CrossesDevices {
            from: map(&from),
            to: map(&to),
        },
        E::StorageFull { location } => E::StorageFull {
            location: map(&location),
        },
        E::ReadOnly { location } => E::ReadOnly {
            location: map(&location),
        },
        E::InUse { location } => E::InUse {
            location: map(&location),
        },
        E::NotText { location } => E::NotText {
            location: map(&location),
        },
        E::Io { message, location } => E::Io {
            message,
            location: location.as_ref().map(map),
        },
        other => other,
    }
}

impl FakeRemoteProvider {
    /// An empty, open server for `scheme`, whose names compare by `rule`.
    pub fn new(scheme: RemoteScheme, rule: CaseRule) -> Self {
        let base = base_path();
        Self {
            scheme,
            memory: MemoryProvider::new(base.clone(), rule),
            base,
            state: Arc::new(Mutex::new(State {
                latency: Duration::ZERO,
                timeout: Duration::from_secs(30),
                fault: None,
                password: None,
                host_key: None,
                host_key_trusted: false,
                sessions: HashMap::new(),
                connects: 0,
                batch: 2,
                poll: None,
            })),
            credentials: Arc::new(NoCredentials),
        }
    }

    /// An SFTP server with case-sensitive names.
    pub fn sftp() -> Self {
        Self::new(RemoteScheme::Sftp, CaseRule::Sensitive)
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    // Configuration, for tests.

    /// Where credentials come from before a login fails with `AuthRequired`.
    pub fn with_credentials(mut self, source: Arc<dyn CredentialSource>) -> Self {
        self.credentials = source;
        self
    }

    /// Every call waits this long first.
    pub fn set_latency(&self, latency: Duration) {
        self.lock().latency = latency;
    }

    /// A call whose latency reaches this fails with `Timeout` after waiting it.
    pub fn set_timeout(&self, timeout: Duration) {
        self.lock().timeout = timeout;
    }

    /// Fails every call with `fault` until it is set to `None`. Going offline also drops every
    /// session, as a real network loss does.
    pub fn set_fault(&self, fault: Option<RemoteFault>) {
        let mut state = self.lock();
        if fault.is_some() {
            for session in state.sessions.values_mut() {
                *session = Session::default();
            }
        }
        state.fault = fault;
    }

    /// The server wants this login (`user` `None` takes any user).
    pub fn require_password(&self, user: Option<&str>, password: &str) {
        self.lock().password = Some((user.map(str::to_owned), password.to_owned()));
    }

    /// The server offers this host key, which is not trusted until a `connect` trusts it.
    pub fn require_host_key(&self, key: HostKey) {
        let mut state = self.lock();
        state.host_key = Some(key);
        state.host_key_trusted = false;
    }

    /// How many entries each batch of `list_batches` holds.
    pub fn set_batch(&self, batch: usize) {
        self.lock().batch = batch.max(1);
    }

    /// Opts in to watching by polling every `interval` (a connection's "refresh every N seconds").
    pub fn set_poll(&self, interval: Option<Duration>) {
        self.lock().poll = interval;
    }

    /// The memory tree behind the server, for seeding files and injecting failures.
    pub fn memory(&self) -> &MemoryProvider {
        &self.memory
    }

    /// How many sessions have been opened.
    pub fn connects(&self) -> usize {
        self.lock().connects
    }

    /// The root of the server at `authority` (`me@fake.test`).
    pub fn root(&self, authority: &str) -> VfsPath {
        VfsPath::from_uri(&format!("{}://{authority}/", self.scheme))
            .expect("a well-formed authority")
    }

    /// Puts a file on the server, with its folders.
    pub fn put_file(&self, path: &VfsPath, content: &[u8]) {
        let (local, _) = self.local(path).expect("a path on this server");
        self.memory.put_file(&VfsPath::File(local), content);
    }

    /// Puts a folder on the server, with its parents.
    pub fn put_dir(&self, path: &VfsPath) {
        let (local, _) = self.local(path).expect("a path on this server");
        let local = VfsPath::File(local);
        // Putting a folder that exists would empty it.
        if self.memory.stat(&local).is_err() {
            self.memory.put_dir(&local);
        }
    }

    // Mapping between the server and the memory tree.

    fn remote<'a>(&self, path: &'a VfsPath) -> Result<&'a RemotePath, VfsError> {
        match path {
            VfsPath::Remote(remote) if remote.scheme() == self.scheme => Ok(remote),
            _ => Err(VfsError::InvalidLocation {
                input: path.to_uri(),
            }),
        }
    }

    fn local(&self, path: &VfsPath) -> Result<(FilePath, RemotePath), VfsError> {
        let remote = self.remote(path)?;
        let mut local = self.base.clone();
        for name in remote.segments() {
            let name = String::from_utf8_lossy(name).into_owned();
            local = local
                .join(OsStr::new(&name))
                .map_err(|_| VfsError::InvalidName {
                    name,
                    reason: "the fake server cannot hold this name".to_owned(),
                })?;
        }
        let mut root = remote.clone();
        while let Some(parent) = root.parent() {
            root = parent;
        }
        Ok((local, root))
    }

    fn to_remote(&self, root: &RemotePath, location: &Location) -> Location {
        let Ok(file) = FilePath::from_location(location) else {
            return location.clone();
        };
        let Ok(below) = file.as_path().strip_prefix(self.base.as_path()) else {
            return location.clone();
        };
        let mut remote = root.clone();
        for part in below.components() {
            match remote.join(part.as_os_str()) {
                Ok(next) => remote = next,
                Err(_) => return location.clone(),
            }
        }
        VfsPath::Remote(remote).to_location()
    }

    // Connections.

    fn key_of(&self, path: &VfsPath) -> Result<ConnectionKey, VfsError> {
        Ok(self.remote(path)?.connection_key())
    }

    /// Waits the latency, then fails with the current fault, if any.
    fn wire(&self, location: &Location) -> Result<(), VfsError> {
        let (latency, timeout, fault) = {
            let state = self.lock();
            (state.latency, state.timeout, state.fault.clone())
        };
        if latency >= timeout {
            thread::sleep(timeout);
            return Err(VfsError::Timeout {
                location: location.clone(),
            });
        }
        if !latency.is_zero() {
            thread::sleep(latency);
        }
        let location = location.clone();
        match fault {
            None => Ok(()),
            Some(RemoteFault::Unreachable(reason)) => {
                Err(VfsError::Unreachable { location, reason })
            }
            Some(RemoteFault::Timeout) => Err(VfsError::Timeout { location }),
            Some(RemoteFault::RateLimited) => Err(VfsError::RateLimited {
                location,
                retry_after_ms: Some(1000),
            }),
        }
    }

    fn password_prompt(&self) -> Option<(AuthPrompt, Option<String>, String)> {
        let state = self.lock();
        let (user, password) = state.password.clone()?;
        Some((AuthPrompt::Password { user: user.clone() }, user, password))
    }

    /// Opens the session for `key` if it is not open, with `credential` or what the source has.
    fn open(
        &self,
        key: &ConnectionKey,
        location: &Location,
        credential: Option<Credential>,
    ) -> Result<(), VfsError> {
        if self
            .lock()
            .sessions
            .get(key)
            .is_some_and(|s| s.authenticated)
        {
            return Ok(());
        }
        let fail = |state: &mut State, error: VfsError| {
            state.sessions.entry(key.clone()).or_default().state = Some(ConnectionState::Failed {
                error: error.clone(),
            });
            Err(error)
        };
        {
            let mut state = self.lock();
            if let Some(host_key) = state.host_key.clone() {
                if !state.host_key_trusted {
                    let error = VfsError::HostKeyUnknown {
                        location: location.clone(),
                        key: Box::new(host_key),
                    };
                    return fail(&mut state, error);
                }
            }
        }
        if let Some((prompt, user, password)) = self.password_prompt() {
            let offered = credential.or_else(|| self.credentials.credential(key, &prompt));
            match offered {
                None => {
                    let error = VfsError::AuthRequired {
                        location: location.clone(),
                        prompt: Box::new(prompt),
                    };
                    return fail(&mut self.lock(), error);
                }
                Some(Credential::Password {
                    user: offered_user,
                    password: offered,
                }) if offered.expose_str() == Some(password.as_str())
                    && (user.is_none() || offered_user.is_none() || offered_user == user) => {}
                Some(_) => {
                    self.credentials.rejected(key, &prompt);
                    let error = VfsError::AuthFailed {
                        location: location.clone(),
                    };
                    return fail(&mut self.lock(), error);
                }
            }
        }
        let mut state = self.lock();
        state.connects += 1;
        let session = state.sessions.entry(key.clone()).or_default();
        session.authenticated = true;
        session.state = Some(ConnectionState::Connected);
        Ok(())
    }

    /// The start of every call: the wire, then the session, then the path in the memory tree.
    fn enter(&self, path: &VfsPath) -> Result<(FilePath, RemotePath), VfsError> {
        let location = path.to_location();
        let key = self.key_of(path)?;
        self.wire(&location)?;
        self.open(&key, &location, None)?;
        self.local(path)
    }

    fn run<T>(
        &self,
        path: &VfsPath,
        call: impl FnOnce(&VfsPath) -> Result<T, VfsError>,
    ) -> Result<T, VfsError> {
        let (local, root) = self.enter(path)?;
        call(&VfsPath::File(local))
            .map_err(|error| relocate(error, &|location| self.to_remote(&root, location)))
    }
}

/// A write stream that discards data past an offset by copying the kept prefix first.
struct Resumed(Box<dyn WriteStream>);

impl Write for Resumed {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.0.flush()
    }
}

impl WriteStream for Resumed {
    fn finish(self: Box<Self>, sync: bool) -> Result<(), VfsError> {
        self.0.finish(sync)
    }
}

impl Provider for FakeRemoteProvider {
    fn scheme(&self) -> &'static str {
        self.scheme.as_str()
    }

    fn capabilities(&self) -> Capabilities {
        let memory = self.memory.capabilities();
        let mut capabilities = Capabilities::new(memory.case_rule);
        capabilities.watch = self.lock().poll.is_some();
        capabilities.remote = true;
        capabilities.write = true;
        capabilities.rename = memory.rename;
        capabilities.server_copy = memory.server_copy;
        capabilities.resume_write = true;
        capabilities.permissions = memory.permissions;
        capabilities.symlinks = memory.symlinks;
        capabilities.set_times = memory.set_times;
        capabilities.max_name_len = memory.max_name_len;
        capabilities
    }

    fn read_only(&self) -> bool {
        self.memory.read_only()
    }

    fn stat(&self, path: &VfsPath) -> Result<ScannedEntry, VfsError> {
        self.run(path, |local| self.memory.stat(local))
    }

    fn list(
        &self,
        path: &VfsPath,
        cancel: &CancelToken,
        inline_link_budget: usize,
        progress: &mut dyn FnMut(u32),
    ) -> Result<Vec<ScannedEntry>, VfsError> {
        if cancel.is_cancelled() {
            return Err(VfsError::Cancelled);
        }
        self.run(path, |local| {
            self.memory
                .list(local, cancel, inline_link_budget, progress)
        })
    }

    fn list_batches(
        &self,
        path: &VfsPath,
        cancel: &CancelToken,
        inline_link_budget: usize,
        sink: &mut dyn FnMut(Vec<ScannedEntry>),
    ) -> Result<(), VfsError> {
        let entries = self.list(path, cancel, inline_link_budget, &mut |_| {})?;
        let (batch, latency) = {
            let state = self.lock();
            (state.batch, state.latency)
        };
        for chunk in entries.chunks(batch) {
            if cancel.is_cancelled() {
                return Err(VfsError::Cancelled);
            }
            sink(chunk.to_vec());
            if !latency.is_zero() {
                thread::sleep(latency);
            }
        }
        Ok(())
    }

    fn resolve_link(
        &self,
        folder: &VfsPath,
        entry: &ScannedEntry,
    ) -> Result<ScannedEntry, VfsError> {
        self.run(folder, |local| self.memory.resolve_link(local, entry))
    }

    fn watch(&self, path: &VfsPath, sink: WatchSink) -> Result<Box<dyn Watch>, VfsError> {
        let Some(interval) = self.lock().poll else {
            return Err(VfsError::Unsupported {
                what: "watching a folder on this server".to_owned(),
            });
        };
        self.enter(path)?;
        let watch = PollWatch::start(Arc::new(self.clone()), path.clone(), interval, sink)?;
        Ok(Box::new(watch))
    }

    fn connection_key(&self, path: &VfsPath) -> Option<ConnectionKey> {
        self.key_of(path).ok()
    }

    fn connection_state(&self, key: &ConnectionKey) -> ConnectionState {
        self.lock()
            .sessions
            .get(key)
            .and_then(|session| session.state.clone())
            .unwrap_or(ConnectionState::Idle)
    }

    fn connect(
        &self,
        key: &ConnectionKey,
        answer: Option<ConnectAnswer>,
        cancel: &CancelToken,
    ) -> Result<(), VfsError> {
        let root =
            VfsPath::from_uri(&format!("{key}/")).map_err(|_| VfsError::InvalidLocation {
                input: key.to_string(),
            })?;
        let location = root.to_location();
        self.lock().sessions.entry(key.clone()).or_default().state =
            Some(ConnectionState::Connecting);
        self.wire(&location)?;
        if cancel.is_cancelled() {
            self.lock().sessions.remove(key);
            return Err(VfsError::Cancelled);
        }
        let credential = match answer {
            Some(ConnectAnswer::TrustHostKey { fingerprint, .. }) => {
                let mut state = self.lock();
                if state
                    .host_key
                    .as_ref()
                    .is_some_and(|key| key.fingerprint == fingerprint)
                {
                    state.host_key_trusted = true;
                }
                None
            }
            Some(ConnectAnswer::Credential(credential)) => Some(credential),
            Some(ConnectAnswer::TrustCertificate { .. }) | None => None,
        };
        self.open(key, &location, credential)
    }

    fn disconnect(&self, key: &ConnectionKey) {
        self.lock().sessions.remove(key);
    }

    fn create_dir(&self, path: &VfsPath) -> Result<(), VfsError> {
        self.run(path, |local| self.memory.create_dir(local))
    }

    fn create_file(&self, path: &VfsPath) -> Result<(), VfsError> {
        self.run(path, |local| self.memory.create_file(local))
    }

    fn rename(&self, from: &VfsPath, to: &VfsPath, overwrite: bool) -> Result<(), VfsError> {
        if self.key_of(from)? != self.key_of(to)? {
            return Err(VfsError::CrossesDevices {
                from: from.to_location(),
                to: to.to_location(),
            });
        }
        let (target, _) = self.local(to)?;
        self.run(from, |local| {
            self.memory.rename(local, &VfsPath::File(target), overwrite)
        })
    }

    fn remove_file(&self, path: &VfsPath) -> Result<(), VfsError> {
        self.run(path, |local| self.memory.remove_file(local))
    }

    fn remove_dir(&self, path: &VfsPath) -> Result<(), VfsError> {
        self.run(path, |local| self.memory.remove_dir(local))
    }

    fn open_read(&self, path: &VfsPath) -> Result<ReadStream, VfsError> {
        self.run(path, |local| self.memory.open_read(local))
    }

    fn create_write(
        &self,
        path: &VfsPath,
        options: WriteOptions,
    ) -> Result<Box<dyn WriteStream>, VfsError> {
        self.run(path, |local| self.memory.create_write(local, options))
    }

    fn resume_write(&self, path: &VfsPath, offset: u64) -> Result<Box<dyn WriteStream>, VfsError> {
        self.run(path, |local| {
            let mut kept = Vec::new();
            self.memory
                .open_read(local)?
                .take(offset)
                .read_to_end(&mut kept)
                .map_err(|error| crate::from_io(&error, &local.to_location()))?;
            if (kept.len() as u64) < offset {
                return Err(VfsError::Io {
                    message: format!("the partial file holds {} bytes, not {offset}", kept.len()),
                    location: Some(local.to_location()),
                });
            }
            let mut stream = self.memory.create_write(local, WriteOptions::truncate())?;
            stream
                .write_all(&kept)
                .map_err(|error| crate::from_io(&error, &local.to_location()))?;
            Ok(Box::new(Resumed(stream)) as Box<dyn WriteStream>)
        })
    }

    fn set_times(&self, path: &VfsPath, times: FileTimes) -> Result<(), VfsError> {
        self.run(path, |local| self.memory.set_times(local, times))
    }

    fn permissions(&self, path: &VfsPath) -> Result<Permissions, VfsError> {
        self.run(path, |local| self.memory.permissions(local))
    }

    fn set_permissions(&self, path: &VfsPath, permissions: Permissions) -> Result<(), VfsError> {
        self.run(path, |local| {
            self.memory.set_permissions(local, permissions)
        })
    }

    fn symlink(&self, link: &VfsPath, target: &OsStr) -> Result<(), VfsError> {
        self.run(link, |local| self.memory.symlink(local, target))
    }

    fn read_link(&self, path: &VfsPath) -> Result<OsString, VfsError> {
        self.run(path, |local| self.memory.read_link(local))
    }

    fn volume_id(&self, path: &VfsPath) -> Option<VolumeId> {
        // One server, one volume: a rename within a connection never crosses devices.
        self.key_of(path).ok().map(|_| VolumeId(7))
    }

    fn free_space(&self, _: &VfsPath) -> Option<VolumeSpace> {
        None
    }

    fn copy_file_within(
        &self,
        src: &VfsPath,
        dst: &VfsPath,
        progress: &mut dyn FnMut(u64),
        cancel: &CancelToken,
    ) -> Option<Result<u64, VfsError>> {
        if !self.memory.capabilities().server_copy {
            return None;
        }
        let target = match self.local(dst) {
            Ok((target, _)) => target,
            Err(error) => return Some(Err(error)),
        };
        let mut handled = true;
        let result = self.run(src, |local| {
            match self.memory.copy_file_within(
                local,
                &VfsPath::File(target.clone()),
                progress,
                cancel,
            ) {
                Some(result) => result,
                None => {
                    handled = false;
                    Ok(0)
                }
            }
        });
        handled.then_some(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::remote::Secret;

    fn key(fake: &FakeRemoteProvider) -> ConnectionKey {
        fake.root("me@fake.test").connection_key().unwrap()
    }

    #[test]
    fn a_session_opens_lazily_on_the_first_call() {
        let fake = FakeRemoteProvider::sftp();
        let root = fake.root("me@fake.test");
        assert_eq!(fake.connection_state(&key(&fake)), ConnectionState::Idle);
        fake.put_file(&root.join("a/b.txt").unwrap(), b"hi");
        let names: Vec<_> = fake
            .list(
                &root.join("a").unwrap(),
                &CancelToken::new(),
                0,
                &mut |_| {},
            )
            .unwrap()
            .into_iter()
            .map(|e| e.name)
            .collect();
        assert_eq!(names, ["b.txt"]);
        assert_eq!(
            fake.connection_state(&key(&fake)),
            ConnectionState::Connected
        );
        assert_eq!(fake.connects(), 1);
        fake.disconnect(&key(&fake));
        assert_eq!(fake.connection_state(&key(&fake)), ConnectionState::Idle);
        fake.stat(&root).unwrap();
        assert_eq!(fake.connects(), 2);
    }

    #[test]
    fn a_login_is_asked_for_then_answered() {
        let fake = FakeRemoteProvider::sftp();
        fake.require_password(Some("me"), "hunter2");
        let root = fake.root("me@fake.test");
        let error = fake.stat(&root).unwrap_err();
        assert_eq!(
            error,
            VfsError::AuthRequired {
                location: root.to_location(),
                prompt: Box::new(AuthPrompt::Password {
                    user: Some("me".into())
                }),
            }
        );
        assert!(matches!(
            fake.connection_state(&key(&fake)),
            ConnectionState::Failed { .. }
        ));
        let wrong = ConnectAnswer::Credential(Credential::Password {
            user: Some("me".into()),
            password: Secret::from("nope"),
        });
        assert!(matches!(
            fake.connect(&key(&fake), Some(wrong), &CancelToken::new()),
            Err(VfsError::AuthFailed { .. })
        ));
        let right = ConnectAnswer::Credential(Credential::Password {
            user: Some("me".into()),
            password: Secret::from("hunter2"),
        });
        fake.connect(&key(&fake), Some(right), &CancelToken::new())
            .unwrap();
        fake.stat(&root).unwrap();
        // No error, event or state ever carries the password.
        let state = format!("{:?}", fake.connection_state(&key(&fake)));
        assert!(!state.contains("hunter2") && !format!("{error:?}").contains("hunter2"));
    }

    struct Keyring(String);

    impl CredentialSource for Keyring {
        fn credential(&self, _: &ConnectionKey, _: &AuthPrompt) -> Option<Credential> {
            Some(Credential::Password {
                user: None,
                password: Secret::from(self.0.as_str()),
            })
        }
    }

    #[test]
    fn a_remembered_login_connects_without_a_question() {
        let fake = FakeRemoteProvider::sftp().with_credentials(Arc::new(Keyring("pw".into())));
        fake.require_password(None, "pw");
        fake.stat(&fake.root("me@fake.test")).unwrap();
    }

    #[test]
    fn an_unknown_host_key_must_be_trusted_by_its_fingerprint() {
        let fake = FakeRemoteProvider::sftp();
        let host_key = HostKey {
            host: "fake.test".into(),
            algorithm: "ssh-ed25519".into(),
            fingerprint: "SHA256:abc".into(),
        };
        fake.require_host_key(host_key.clone());
        let root = fake.root("me@fake.test");
        assert!(matches!(
            fake.stat(&root),
            Err(VfsError::HostKeyUnknown { key, .. }) if *key == host_key
        ));
        let other = ConnectAnswer::TrustHostKey {
            fingerprint: "SHA256:other".into(),
            remember: false,
        };
        assert!(fake
            .connect(&key(&fake), Some(other), &CancelToken::new())
            .is_err());
        let trust = ConnectAnswer::TrustHostKey {
            fingerprint: "SHA256:abc".into(),
            remember: true,
        };
        fake.connect(&key(&fake), Some(trust), &CancelToken::new())
            .unwrap();
        fake.stat(&root).unwrap();
    }

    #[test]
    fn offline_slow_and_busy_servers_fail_with_their_own_errors() {
        let fake = FakeRemoteProvider::sftp();
        let root = fake.root("h");
        fake.stat(&root).unwrap();
        fake.set_fault(Some(RemoteFault::Unreachable(UnreachableReason::Offline)));
        assert!(matches!(
            fake.stat(&root),
            Err(VfsError::Unreachable {
                reason: UnreachableReason::Offline,
                ..
            })
        ));
        assert_eq!(
            fake.connection_state(&root.connection_key().unwrap()),
            ConnectionState::Idle
        );
        fake.set_fault(Some(RemoteFault::RateLimited));
        assert!(matches!(
            fake.stat(&root),
            Err(VfsError::RateLimited { .. })
        ));
        fake.set_fault(None);
        fake.set_latency(Duration::from_millis(20));
        fake.set_timeout(Duration::from_millis(10));
        assert!(matches!(fake.stat(&root), Err(VfsError::Timeout { .. })));
        fake.set_timeout(Duration::from_secs(1));
        fake.stat(&root).unwrap();
    }

    #[test]
    fn errors_are_reported_at_the_remote_location() {
        let fake = FakeRemoteProvider::sftp();
        let missing = fake.root("me@h").join("nope").unwrap();
        assert_eq!(
            fake.stat(&missing).unwrap_err(),
            VfsError::NotFound {
                location: missing.to_location()
            }
        );
    }

    #[test]
    fn a_folder_lists_in_batches() {
        let fake = FakeRemoteProvider::sftp();
        let root = fake.root("h");
        for name in ["a", "b", "c", "d", "e"] {
            fake.put_file(&root.join(name).unwrap(), b"");
        }
        fake.set_batch(2);
        let mut sizes = Vec::new();
        fake.list_batches(&root, &CancelToken::new(), 0, &mut |batch| {
            sizes.push(batch.len())
        })
        .unwrap();
        assert_eq!(sizes, [2, 2, 1]);
    }

    #[test]
    fn a_rename_between_two_logins_crosses_devices() {
        let fake = FakeRemoteProvider::sftp();
        let a = fake.root("a@h").join("x").unwrap();
        fake.put_file(&a, b"1");
        let b = fake.root("b@h").join("x").unwrap();
        assert!(matches!(
            fake.rename(&a, &b, false),
            Err(VfsError::CrossesDevices { .. })
        ));
    }
}
