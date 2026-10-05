// `WebDavProvider`: the `Provider` contract over HTTP sessions.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::SystemTime;

use waypoint_path::{CaseRule, ConnectionKey, RemotePath, RemoteScheme, VfsPath};
use waypoint_protocol::{ConnectionState, Location, VfsError};
use waypoint_vfs::{
    CancelToken, Capabilities, ConnectAnswer, FileTimes, Provider, ReadStream, RenameSupport,
    ScannedEntry, VolumeSpace, WriteOptions, WriteStream,
};

use crate::client::{Reply, Request, Session};
use crate::errors::{ends_session, from_status, Op};
use crate::options::{Preset, WebDavConfig, WebDavOptions};
use crate::paths::{is_nextcloud, remote, url};
use crate::pool::{lock, Pool};

pub(crate) struct Inner {
    pub(crate) config: WebDavConfig,
    pub(crate) scheme: RemoteScheme,
    pub(crate) pool: Pool,
    options: Mutex<HashMap<ConnectionKey, WebDavOptions>>,
    /// Modification times to send with the next upload of a path (Nextcloud's `X-OC-MTime`).
    hints: Mutex<HashMap<String, SystemTime>>,
    runtime: OnceLock<tokio::runtime::Runtime>,
}

impl Drop for Inner {
    fn drop(&mut self) {
        // Dropping a runtime waits for its tasks unless told not to, which may be on a thread
        // that must not block.
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_background();
        }
    }
}

/// The WebDAV provider for one scheme, `davs://` or `dav://`. Cloning shares its sessions.
#[derive(Clone)]
pub struct WebDavProvider {
    pub(crate) inner: Arc<Inner>,
}

/// A path resolved to the session that serves it.
#[derive(Clone)]
pub(crate) struct Target {
    pub remote: RemotePath,
    pub location: Location,
    pub session: Arc<Session>,
    pub nextcloud: bool,
}

impl Target {
    /// The URL of the thing itself; a folder's ends in a slash.
    pub(crate) fn url(&self, folder: bool) -> String {
        url(&self.remote, folder)
    }

    pub(crate) fn name(&self) -> Vec<u8> {
        self.remote.segments().last().cloned().unwrap_or_default()
    }
}

impl WebDavProvider {
    /// The provider of `davs://` locations, over HTTPS.
    pub fn davs(config: WebDavConfig) -> Self {
        Self::new(config, RemoteScheme::Davs)
    }

    /// The provider of `dav://` locations, over plain HTTP: for servers on the local network, with
    /// the Connect dialog's warning.
    pub fn dav(config: WebDavConfig) -> Self {
        Self::new(config, RemoteScheme::Dav)
    }

    fn new(config: WebDavConfig, scheme: RemoteScheme) -> Self {
        Self {
            inner: Arc::new(Inner {
                config,
                scheme,
                pool: Pool::default(),
                options: Mutex::new(HashMap::new()),
                hints: Mutex::new(HashMap::new()),
                runtime: OnceLock::new(),
            }),
        }
    }

    /// The tuning of one connection, used from its next session on.
    pub fn set_options(&self, key: &ConnectionKey, options: WebDavOptions) {
        lock(&self.inner.options).insert(key.clone(), options);
        // The next call builds a session with them; a login made on the old one is made again.
        if let Some(slot) = self.inner.pool.existing(key) {
            slot.close();
        }
    }

    fn options(&self, key: &ConnectionKey) -> WebDavOptions {
        lock(&self.inner.options)
            .get(key)
            .copied()
            .unwrap_or(self.inner.config.options)
    }

    /// Trusts the certificate with this SHA-256 fingerprint on this connection, as a pin saved with
    /// the connection does (D148). Nothing is written and the system's store is not touched.
    pub fn pin_certificate(&self, key: &ConnectionKey, fingerprint: &str) {
        self.inner.pool.slot(key).trust.trust(fingerprint);
    }

    /// Asks the server to give the next upload to `path` this modification time, where the server
    /// has a way to take one (Nextcloud's `X-OC-MTime`). The operations engine sets times after a
    /// copy on providers that report `set_times`; this is the way to keep them on one that does not.
    pub fn hint_modified(&self, path: &VfsPath, modified: SystemTime) {
        lock(&self.inner.hints).insert(path.to_uri(), modified);
    }

    pub(crate) fn take_hint(&self, path: &VfsPath) -> Option<SystemTime> {
        lock(&self.inner.hints).remove(&path.to_uri())
    }

    pub(crate) fn runtime(&self) -> &tokio::runtime::Runtime {
        self.inner.runtime.get_or_init(|| {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .thread_name("waypoint-webdav")
                .enable_all()
                .build()
                .expect("the WebDAV runtime starts")
        })
    }

    /// Runs `future` to its end on the provider's runtime. Callers are worker threads, never the
    /// runtime's own.
    pub(crate) fn block<F: Future>(&self, future: F) -> F::Output {
        self.runtime().block_on(future)
    }

    fn session(&self, remote: &RemotePath) -> Result<Arc<Session>, VfsError> {
        let key = remote.connection_key();
        let slot = self.inner.pool.slot(&key);
        let mut held = lock(&slot.session);
        if let Some(session) = held.clone() {
            return Ok(session);
        }
        let origin = format!("{}/", crate::paths::origin(remote));
        let session = self.block(async {
            Session::new(
                key.clone(),
                remote.authority().user.clone(),
                origin,
                self.options(&key),
                self.inner.config.credentials.clone(),
                slot.trust.clone(),
            )
        })?;
        let session = Arc::new(session);
        *held = Some(session.clone());
        Ok(session)
    }

    pub(crate) fn target(&self, path: &VfsPath) -> Result<Target, VfsError> {
        let remote = remote(path, self.inner.scheme)?.clone();
        let session = self.session(&remote)?;
        let nextcloud = match session.options.preset {
            Preset::Nextcloud => true,
            Preset::Generic => false,
            Preset::Auto => is_nextcloud(remote.segments()),
        };
        Ok(Target {
            remote,
            location: path.to_location(),
            session,
            nextcloud,
        })
    }

    /// Sends a request and returns the response whatever its status; a failure to get one is the
    /// error that says why. A request that changes nothing is sent again, once, on a connection
    /// that turns out to have been dropped.
    pub(crate) fn exec_raw(
        &self,
        target: &Target,
        request: &Request,
        cancel: Option<&CancelToken>,
    ) -> Result<Reply, VfsError> {
        let slot = self.inner.pool.slot(&target.remote.connection_key());
        let mut retried = false;
        loop {
            let sent = self.block(target.session.send(request, &target.location, cancel));
            match sent {
                Ok(reply) => {
                    slot.set_state(ConnectionState::Connected);
                    *lock(&slot.probe) =
                        Some(crate::paths::encode_path(target.remote.segments(), true));
                    return Ok(reply);
                }
                Err(error) => {
                    if matches!(error, VfsError::Disconnected { .. })
                        && request.idempotent()
                        && !retried
                    {
                        retried = true;
                        continue;
                    }
                    if !matches!(error, VfsError::Cancelled) {
                        slot.set_state(ConnectionState::Failed {
                            error: error.clone(),
                        });
                    }
                    if ends_session(&error) {
                        log::debug!("webdav: session ended at {}", target.location.uri);
                    }
                    return Err(error);
                }
            }
        }
    }

    /// Like `exec_raw`, with every status that is not a success turned into its typed error.
    pub(crate) fn exec(
        &self,
        target: &Target,
        request: &Request,
        op: Op,
        cancel: Option<&CancelToken>,
    ) -> Result<Reply, VfsError> {
        let reply = self.exec_raw(target, request, cancel)?;
        let status = reply.status();
        if status.is_success() {
            return Ok(reply);
        }
        Err(from_status(op, status, None, &target.location))
    }

    /// Checks the name `path` would create, before anything is sent.
    pub(crate) fn check_name(path: &VfsPath) -> Result<(), VfsError> {
        match path.file_name() {
            Some(name) => waypoint_vfs::validate_name(&name, CaseRule::Sensitive),
            None => Err(VfsError::InvalidName {
                name: String::new(),
                reason: "the root of a server has no name".to_owned(),
            }),
        }
    }
}

impl Provider for WebDavProvider {
    fn scheme(&self) -> &'static str {
        self.inner.scheme.as_str()
    }

    fn capabilities(&self) -> Capabilities {
        let mut caps = Capabilities::new(CaseRule::Sensitive);
        caps.remote = true;
        caps.write = true;
        caps.range_read = true;
        // `MOVE` with `Overwrite: F` is atomic and refuses a name that is taken (RFC 4918 §9.9).
        caps.rename = RenameSupport::NoReplace;
        caps.server_copy = true;
        // A `PUT` shows the file as it arrives on some servers, so the engine writes to a partial
        // name and moves it into place.
        caps.atomic_write = false;
        caps
    }

    fn stat(&self, path: &VfsPath) -> Result<ScannedEntry, VfsError> {
        self.dav_stat(path).map(|(entry, _)| entry)
    }

    fn list(
        &self,
        path: &VfsPath,
        cancel: &CancelToken,
        inline_link_budget: usize,
        progress: &mut dyn FnMut(u32),
    ) -> Result<Vec<ScannedEntry>, VfsError> {
        let mut entries = Vec::new();
        self.list_batches(path, cancel, inline_link_budget, &mut |batch| {
            entries.extend(batch);
            progress(u32::try_from(entries.len()).unwrap_or(u32::MAX));
        })?;
        Ok(entries)
    }

    fn list_batches(
        &self,
        path: &VfsPath,
        cancel: &CancelToken,
        _inline_link_budget: usize,
        sink: &mut dyn FnMut(Vec<ScannedEntry>),
    ) -> Result<(), VfsError> {
        self.dav_list(path, cancel, sink)
    }

    fn resolve_link(
        &self,
        _folder: &VfsPath,
        entry: &ScannedEntry,
    ) -> Result<ScannedEntry, VfsError> {
        // Nothing here is a link.
        Ok(entry.clone())
    }

    fn connection_key(&self, path: &VfsPath) -> Option<ConnectionKey> {
        remote(path, self.inner.scheme)
            .ok()
            .map(RemotePath::connection_key)
    }

    fn connection_state(&self, key: &ConnectionKey) -> ConnectionState {
        self.inner
            .pool
            .existing(key)
            .map_or(ConnectionState::Idle, |slot| slot.state())
    }

    fn connect(
        &self,
        key: &ConnectionKey,
        answer: Option<ConnectAnswer>,
        cancel: &CancelToken,
    ) -> Result<(), VfsError> {
        self.dav_connect(key, answer, cancel)
    }

    fn disconnect(&self, key: &ConnectionKey) {
        if let Some(slot) = self.inner.pool.existing(key) {
            slot.close();
        }
    }

    fn create_dir(&self, path: &VfsPath) -> Result<(), VfsError> {
        self.dav_create_dir(path)
    }

    fn create_file(&self, path: &VfsPath) -> Result<(), VfsError> {
        self.dav_create_file(path)
    }

    fn rename(&self, from: &VfsPath, to: &VfsPath, overwrite: bool) -> Result<(), VfsError> {
        self.dav_rename(from, to, overwrite)
    }

    fn remove_file(&self, path: &VfsPath) -> Result<(), VfsError> {
        self.dav_remove_file(path)
    }

    fn remove_dir(&self, path: &VfsPath) -> Result<(), VfsError> {
        self.dav_remove_dir(path)
    }

    fn open_read(&self, path: &VfsPath) -> Result<ReadStream, VfsError> {
        self.dav_open_read(path, 0)
    }

    fn open_read_at(&self, path: &VfsPath, start: u64) -> Result<ReadStream, VfsError> {
        self.dav_open_read(path, start)
    }

    fn create_write(
        &self,
        path: &VfsPath,
        options: WriteOptions,
    ) -> Result<Box<dyn WriteStream>, VfsError> {
        self.dav_create_write(path, options)
    }

    fn set_times(&self, _path: &VfsPath, _times: FileTimes) -> Result<(), VfsError> {
        Err(VfsError::Unsupported {
            what: "setting times here".to_owned(),
        })
    }

    fn free_space(&self, path: &VfsPath) -> Option<VolumeSpace> {
        self.dav_free_space(path)
    }

    fn copy_file_within(
        &self,
        src: &VfsPath,
        dst: &VfsPath,
        progress: &mut dyn FnMut(u64),
        cancel: &CancelToken,
    ) -> Option<Result<u64, VfsError>> {
        self.dav_copy(src, dst, progress, cancel)
    }
}
