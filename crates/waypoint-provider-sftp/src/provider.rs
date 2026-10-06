// `SftpProvider`: the `Provider` contract over the session pool.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::future::Future;
use std::sync::{Arc, Mutex, OnceLock};

use russh_sftp::protocol::{FileAttributes, OpenFlags};
use waypoint_path::{CaseRule, ConnectionKey, RemotePath, VfsPath};
use waypoint_protocol::{ConnectionState, Location, VfsError};
use waypoint_vfs::{
    guess_mime, validate_name, CancelToken, Capabilities, ConnectAnswer, DetailField, EntryDetails,
    EntryKind, FileTimes, PermissionModel, Permissions, Provider, ReadStream, RenameSupport,
    ScannedEntry, VolumeSpace, WriteOptions, WriteStream,
};

use crate::changes::{self, Rename};
use crate::client::SessionTrust;
use crate::errors::{ends_session, from_sftp};
use crate::listing;
use crate::options::{default_identity_files, SftpConfig, SftpOptions};
use crate::paths::{remote, server_path};
use crate::pool::Pool;
use crate::read::SftpReader;
use crate::session::{OpenStop, Opening, Session, Target};
use crate::write::SftpWriter;

struct Inner {
    config: SftpConfig,
    pool: Pool,
    options: Mutex<HashMap<ConnectionKey, SftpOptions>>,
    trust: Arc<SessionTrust>,
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

/// The SFTP provider. Cloning shares its sessions.
#[derive(Clone)]
pub struct SftpProvider {
    inner: Arc<Inner>,
}

impl SftpProvider {
    pub fn new(config: SftpConfig) -> Self {
        Self {
            inner: Arc::new(Inner {
                config,
                pool: Pool::default(),
                options: Mutex::new(HashMap::new()),
                trust: Arc::new(SessionTrust::default()),
                runtime: OnceLock::new(),
            }),
        }
    }

    /// The tuning of one connection, used from its next session on.
    pub fn set_options(&self, key: &ConnectionKey, options: SftpOptions) {
        self.inner
            .options
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(key.clone(), options);
    }

    fn options(&self, key: &ConnectionKey) -> SftpOptions {
        self.inner
            .options
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(key)
            .copied()
            .unwrap_or(self.inner.config.options)
    }

    fn runtime(&self) -> &tokio::runtime::Runtime {
        self.inner.runtime.get_or_init(|| {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .thread_name("waypoint-sftp")
                .enable_all()
                .build()
                .expect("the SFTP runtime starts")
        })
    }

    /// Runs `future` to its end on the provider's runtime. Callers are worker threads, never the
    /// runtime's own.
    fn block<F: Future>(&self, future: F) -> F::Output {
        self.runtime().block_on(future)
    }

    fn target(&self, path: &RemotePath) -> Target {
        let config = &self.inner.config;
        crate::ssh_config::resolve(
            path,
            config.ssh_config.as_deref(),
            config.identity_files.as_deref(),
            &default_identity_files,
        )
    }

    /// The open session of `path`'s connection, opening one when there is none. With an `answer`
    /// a new session is opened (or the waiting login answered) whatever is open.
    async fn session(
        &self,
        path: &RemotePath,
        location: &Location,
        answer: Option<&ConnectAnswer>,
        cancel: Option<&CancelToken>,
    ) -> Result<Arc<Session>, VfsError> {
        let key = path.connection_key();
        let slot = self.inner.pool.slot(&key);
        if answer.is_none() {
            if let Some(session) = slot.live() {
                return Ok(session);
            }
        }
        let mut gate = slot.gate.lock().await;
        if answer.is_none() {
            if let Some(session) = slot.live() {
                return Ok(session);
            }
        }
        slot.set_state(ConnectionState::Connecting);
        let target = self.target(path);
        let opening = Opening {
            config: &self.inner.config,
            options: self.options(&key),
            session_trust: &self.inner.trust,
            answer,
            location,
        };
        let pending = gate.take();
        let opened = match cancel {
            Some(cancel) => tokio::select! {
                opened = opening.open(&target, pending) => Some(opened),
                () = listing::cancelled(cancel) => None,
            },
            None => Some(opening.open(&target, pending).await),
        };
        match opened {
            None => {
                slot.set_state(ConnectionState::Idle);
                Err(VfsError::Cancelled)
            }
            Some(Ok(session)) => {
                if let Some(old) = slot.take() {
                    self.runtime().spawn(async move { old.close().await });
                }
                let session = Arc::new(session);
                slot.connected(session.clone());
                Ok(session)
            }
            Some(Err(OpenStop::Waiting(pending, error))) => {
                *gate = Some(pending);
                slot.set_state(ConnectionState::Failed {
                    error: error.clone(),
                });
                Err(error)
            }
            Some(Err(OpenStop::Error(error))) => {
                slot.set_state(ConnectionState::Failed {
                    error: error.clone(),
                });
                Err(error)
            }
        }
    }

    /// Runs `op` on `path`'s session with the path as the server reads it. A session that turns
    /// out to be gone is forgotten, so the next call reconnects; an `idempotent` call reconnects
    /// and tries once more at once.
    fn with_session<T, F, Fut>(
        &self,
        path: &VfsPath,
        idempotent: bool,
        op: F,
    ) -> Result<T, VfsError>
    where
        F: Fn(Arc<Session>, String) -> Fut,
        Fut: Future<Output = Result<T, VfsError>>,
    {
        let remote = remote(path)?;
        let server = server_path(remote)?;
        let location = path.to_location();
        let slot = self.inner.pool.slot(&remote.connection_key());
        self.block(async {
            let mut retried = false;
            loop {
                let session = self.session(remote, &location, None, None).await?;
                match op(session.clone(), server.clone()).await {
                    Err(error) if ends_session(&error) => {
                        slot.lost(&session, error.clone());
                        if idempotent && !retried && matches!(error, VfsError::Disconnected { .. })
                        {
                            retried = true;
                            continue;
                        }
                        return Err(error);
                    }
                    result => return result,
                }
            }
        })
    }

    /// Checks the name `path` would create, before anything is sent.
    fn check_name(path: &VfsPath) -> Result<(), VfsError> {
        match path.file_name() {
            Some(name) => validate_name(&name, CaseRule::Sensitive),
            None => Err(VfsError::InvalidName {
                name: String::new(),
                reason: "the root of a server has no name".to_owned(),
            }),
        }
    }

    /// Runs a change on `path`'s session; a change is never retried by itself.
    fn change<T, F, Fut>(&self, path: &VfsPath, op: F) -> Result<T, VfsError>
    where
        F: Fn(Arc<Session>, String, Location) -> Fut,
        Fut: Future<Output = Result<T, VfsError>>,
    {
        let location = path.to_location();
        self.with_session(path, false, |session, server| {
            op(session, server, location.clone())
        })
    }

    fn name_of(path: &VfsPath) -> String {
        path.file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default()
    }
}

impl Provider for SftpProvider {
    fn scheme(&self) -> &'static str {
        "sftp"
    }

    fn capabilities(&self) -> Capabilities {
        let mut caps = Capabilities::new(CaseRule::Sensitive);
        caps.remote = true;
        caps.range_read = true;
        caps.permissions = PermissionModel::Unix;
        caps.symlinks = true;
        caps.write = true;
        // The draft's rename refuses an existing target, but not every server keeps to it.
        caps.rename = RenameSupport::Replacing;
        caps.resume_write = true;
        caps.set_times = true;
        caps
    }

    fn stat(&self, path: &VfsPath) -> Result<ScannedEntry, VfsError> {
        let location = path.to_location();
        let name = Self::name_of(path);
        self.with_session(path, true, |session, server| {
            let (location, name) = (location.clone(), name.clone());
            async move { listing::stat(&session, &server, &name, &location).await }
        })
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
        inline_link_budget: usize,
        sink: &mut dyn FnMut(Vec<ScannedEntry>),
    ) -> Result<(), VfsError> {
        if cancel.is_cancelled() {
            return Err(VfsError::Cancelled);
        }
        let remote = remote(path)?;
        let server = server_path(remote)?;
        let location = path.to_location();
        let slot = self.inner.pool.slot(&remote.connection_key());
        self.block(async {
            let mut retried = false;
            loop {
                let session = self.session(remote, &location, None, Some(cancel)).await?;
                let mut delivered = false;
                let mut counting = |batch: Vec<ScannedEntry>| {
                    delivered = true;
                    sink(batch);
                };
                let result = listing::list(
                    &session,
                    &server,
                    &location,
                    cancel,
                    inline_link_budget,
                    &mut counting,
                )
                .await;
                match result {
                    Err(error) if ends_session(&error) => {
                        slot.lost(&session, error.clone());
                        // Start over only when nothing has been handed over yet.
                        if !delivered && !retried && matches!(error, VfsError::Disconnected { .. })
                        {
                            retried = true;
                            continue;
                        }
                        return Err(error);
                    }
                    result => return result,
                }
            }
        })
    }

    fn resolve_link(
        &self,
        folder: &VfsPath,
        entry: &ScannedEntry,
    ) -> Result<ScannedEntry, VfsError> {
        let path = folder
            .join(&entry.name)
            .map_err(|_| VfsError::InvalidLocation {
                input: entry.name.to_string_lossy().into_owned(),
            })?;
        let location = path.to_location();
        self.with_session(&path, true, |session, server| {
            let (location, entry) = (location.clone(), entry.clone());
            async move { listing::resolve(&session, &server, entry, &location).await }
        })
    }

    fn connection_key(&self, path: &VfsPath) -> Option<ConnectionKey> {
        remote(path).ok().map(RemotePath::connection_key)
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
        let root = VfsPath::from_uri(key.as_str()).map_err(|_| VfsError::InvalidLocation {
            input: key.to_string(),
        })?;
        let remote = remote(&root)?;
        let location = root.to_location();
        self.block(self.session(remote, &location, answer.as_ref(), Some(cancel)))
            .map(|_| ())
    }

    fn disconnect(&self, key: &ConnectionKey) {
        let Some(slot) = self.inner.pool.existing(key) else {
            return;
        };
        if let Ok(mut pending) = slot.gate.try_lock() {
            *pending = None;
        }
        if let Some(session) = slot.take() {
            self.runtime().spawn(async move { session.close().await });
        }
    }

    fn open_read(&self, path: &VfsPath) -> Result<ReadStream, VfsError> {
        self.open_read_at(path, 0)
    }

    fn open_read_at(&self, path: &VfsPath, start: u64) -> Result<ReadStream, VfsError> {
        let location = path.to_location();
        let runtime = self.runtime().handle().clone();
        self.with_session(path, true, |session, server| {
            let (location, runtime) = (location.clone(), runtime.clone());
            async move {
                let handle = session
                    .sftp
                    .open(server, OpenFlags::READ, FileAttributes::empty())
                    .await
                    .map_err(|error| from_sftp(&error, &location))?
                    .handle;
                // Opening a folder for reading succeeds on some servers; reading it does not.
                let attrs = session.sftp.fstat(handle.clone()).await;
                if let Ok(attrs) = &attrs {
                    if attrs.attrs.is_dir() {
                        let _ = session.sftp.close(handle).await;
                        return Err(VfsError::IsADirectory { location });
                    }
                }
                let reader = SftpReader::start(&runtime, session, handle, start, location);
                Ok(Box::new(reader) as ReadStream)
            }
        })
    }

    fn details(&self, path: &VfsPath) -> Result<EntryDetails, VfsError> {
        let location = path.to_location();
        let name = Self::name_of(path);
        self.with_session(path, true, |session, server| {
            let (location, name) = (location.clone(), name.clone());
            async move {
                let attrs = session
                    .sftp
                    .lstat(server.clone())
                    .await
                    .map_err(|error| from_sftp(&error, &location))?
                    .attrs;
                let entry = listing::stat(&session, &server, &name, &location).await?;
                let symlink_target = if entry.kind == EntryKind::Symlink {
                    session
                        .sftp
                        .readlink(server.clone())
                        .await
                        .ok()
                        .and_then(|name| name.files.into_iter().next())
                        .map(|file| file.filename)
                } else {
                    None
                };
                Ok(details_of(&entry, &attrs, symlink_target))
            }
        })
    }

    fn permissions(&self, path: &VfsPath) -> Result<Permissions, VfsError> {
        let location = path.to_location();
        self.with_session(path, true, |session, server| {
            let location = location.clone();
            async move {
                let attrs = session
                    .sftp
                    .stat(server)
                    .await
                    .map_err(|error| from_sftp(&error, &location))?
                    .attrs;
                let mode = attrs.permissions.map(|mode| mode & 0o7777);
                Ok(Permissions {
                    mode,
                    readonly: mode.is_some_and(|mode| mode & 0o222 == 0),
                })
            }
        })
    }

    fn read_link(&self, path: &VfsPath) -> Result<OsString, VfsError> {
        let location = path.to_location();
        self.with_session(path, true, |session, server| {
            let location = location.clone();
            async move {
                let name = session
                    .sftp
                    .readlink(server)
                    .await
                    .map_err(|error| from_sftp(&error, &location))?;
                name.files
                    .into_iter()
                    .next()
                    .map(|file| OsString::from(file.filename))
                    .ok_or(VfsError::Io {
                        message: "the server sent no link text".to_owned(),
                        location: Some(location),
                    })
            }
        })
    }

    fn create_dir(&self, path: &VfsPath) -> Result<(), VfsError> {
        Self::check_name(path)?;
        self.change(path, |session, server, location| async move {
            changes::create_dir(&session, &server, &location).await
        })
    }

    fn create_file(&self, path: &VfsPath) -> Result<(), VfsError> {
        Self::check_name(path)?;
        self.change(path, |session, server, location| async move {
            changes::create_file(&session, &server, &location).await
        })
    }

    fn rename(&self, from: &VfsPath, to: &VfsPath, overwrite: bool) -> Result<(), VfsError> {
        Self::check_name(to)?;
        let target = remote(to)?;
        if Some(target.connection_key()) != remote(from).ok().map(RemotePath::connection_key) {
            return Err(VfsError::CrossesDevices {
                from: from.to_location(),
                to: to.to_location(),
            });
        }
        let to_server = server_path(target)?;
        let to_location = to.to_location();
        self.change(from, |session, server, location| {
            let (to_server, to_location) = (to_server.clone(), to_location.clone());
            async move {
                let rename = Rename {
                    from: &server,
                    to: &to_server,
                    from_location: &location,
                    to_location: &to_location,
                    overwrite,
                };
                changes::rename(&session, rename).await
            }
        })
    }

    fn remove_file(&self, path: &VfsPath) -> Result<(), VfsError> {
        self.change(path, |session, server, location| async move {
            changes::remove_file(&session, &server, &location).await
        })
    }

    fn remove_dir(&self, path: &VfsPath) -> Result<(), VfsError> {
        self.change(path, |session, server, location| async move {
            changes::remove_dir(&session, &server, &location).await
        })
    }

    fn create_write(
        &self,
        path: &VfsPath,
        options: WriteOptions,
    ) -> Result<Box<dyn WriteStream>, VfsError> {
        Self::check_name(path)?;
        let runtime = self.runtime().handle().clone();
        self.change(path, |session, server, location| {
            let runtime = runtime.clone();
            async move {
                let mut flags = OpenFlags::WRITE | OpenFlags::CREATE;
                flags |= if options.exclusive {
                    OpenFlags::EXCLUDE
                } else {
                    OpenFlags::TRUNCATE
                };
                let handle =
                    changes::open_for_writing(&session, &server, flags, options.mode, &location)
                        .await?;
                let writer = SftpWriter::start(&runtime, session, handle, 0, location);
                Ok(Box::new(writer) as Box<dyn WriteStream>)
            }
        })
    }

    fn resume_write(&self, path: &VfsPath, offset: u64) -> Result<Box<dyn WriteStream>, VfsError> {
        let runtime = self.runtime().handle().clone();
        self.change(path, |session, server, location| {
            let runtime = runtime.clone();
            async move {
                let handle =
                    changes::open_for_writing(&session, &server, OpenFlags::WRITE, None, &location)
                        .await?;
                // A partial file shorter than the part already sent is not the one that was
                // left: cutting it to the offset would fill the gap with zeros.
                let length = session
                    .sftp
                    .fstat(handle.clone())
                    .await
                    .ok()
                    .and_then(|attrs| attrs.attrs.size);
                if length.is_none_or(|length| length < offset) {
                    let _ = session.sftp.close(handle).await;
                    return Err(VfsError::Io {
                        message: format!(
                            "the partial file is shorter than the {offset} bytes already sent, \
                             so it cannot be resumed"
                        ),
                        location: Some(location),
                    });
                }
                // Whatever is past the offset was not confirmed and is written again.
                if let Err(error) = changes::truncate(&session, &handle, offset, &location).await {
                    let _ = session.sftp.close(handle).await;
                    return Err(error);
                }
                let writer = SftpWriter::start(&runtime, session, handle, offset, location);
                Ok(Box::new(writer) as Box<dyn WriteStream>)
            }
        })
    }

    fn set_times(&self, path: &VfsPath, times: FileTimes) -> Result<(), VfsError> {
        self.change(path, |session, server, location| async move {
            changes::set_times(&session, &server, times, &location).await
        })
    }

    fn set_permissions(&self, path: &VfsPath, permissions: Permissions) -> Result<(), VfsError> {
        self.change(path, |session, server, location| async move {
            changes::set_permissions(&session, &server, permissions, &location).await
        })
    }

    fn symlink(&self, link: &VfsPath, target: &OsStr) -> Result<(), VfsError> {
        Self::check_name(link)?;
        let target = target.to_str().ok_or_else(|| VfsError::InvalidName {
            name: target.to_string_lossy().into_owned(),
            reason: "SFTP link texts that are not UTF-8 are not supported".to_owned(),
        })?;
        self.change(link, |session, server, location| async move {
            changes::symlink(&session, &server, target, &location).await
        })
    }

    fn free_space(&self, path: &VfsPath) -> Option<VolumeSpace> {
        self.with_session(path, true, |session, server| async move {
            Ok(changes::free_space(&session, &server).await)
        })
        .ok()
        .flatten()
    }

    fn canonicalize(&self, path: &VfsPath) -> Result<VfsPath, VfsError> {
        let location = path.to_location();
        let mut root = remote(path)?.clone();
        while let Some(parent) = root.parent() {
            root = parent;
        }
        let resolved = self.with_session(path, true, |session, server| {
            let location = location.clone();
            async move {
                session
                    .sftp
                    .lstat(server.clone())
                    .await
                    .map_err(|error| from_sftp(&error, &location))?;
                let name = session
                    .sftp
                    .realpath(server)
                    .await
                    .map_err(|error| from_sftp(&error, &location))?;
                name.files
                    .into_iter()
                    .next()
                    .map(|file| file.filename)
                    .ok_or(VfsError::Io {
                        message: "the server sent no path".to_owned(),
                        location: Some(location),
                    })
            }
        })?;
        root.join(&resolved)
            .map(VfsPath::Remote)
            .map_err(|_| VfsError::InvalidLocation { input: resolved })
    }
}

/// The Inspector's details of an entry from what the server says about it.
fn details_of(
    entry: &ScannedEntry,
    attrs: &FileAttributes,
    symlink_target: Option<String>,
) -> EntryDetails {
    let name = entry.name.to_string_lossy().into_owned();
    let is_folder =
        entry.kind == EntryKind::Directory || entry.link_target == Some(EntryKind::Directory);
    let mode = attrs.permissions.map(|mode| mode & 0o7777);
    let owner = attrs
        .user
        .clone()
        .or_else(|| attrs.uid.map(|uid| uid.to_string()));
    let group = attrs
        .group
        .clone()
        .or_else(|| attrs.gid.map(|gid| gid.to_string()));
    let mut unavailable = vec![DetailField::AllocatedSize, DetailField::Created];
    if attrs.atime.is_none() {
        unavailable.push(DetailField::Accessed);
    }
    if owner.is_none() {
        unavailable.push(DetailField::Owner);
    }
    if group.is_none() {
        unavailable.push(DetailField::Group);
    }
    if mode.is_none() {
        unavailable.push(DetailField::Permissions);
    }
    EntryDetails {
        mime_type: if is_folder {
            Some("inode/directory".to_owned())
        } else {
            guess_mime(&name, None)
        },
        name,
        kind: entry.kind,
        resolves_to: entry.link_target,
        symlink_target,
        size: entry.size,
        allocated_size: None,
        created_ms: None,
        modified_ms: entry.modified_ms,
        accessed_ms: attrs.atime.map(|secs| i64::from(secs) * 1000),
        owner,
        group,
        mode,
        read_only: mode.is_some_and(|mode| mode & 0o222 == 0),
        hidden: entry.hidden,
        unavailable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn details_say_what_the_server_did_not() {
        let attrs = FileAttributes {
            size: Some(5),
            uid: Some(1000),
            gid: None,
            permissions: Some(0o100_444),
            atime: Some(3),
            mtime: Some(4),
            ..FileAttributes::empty()
        };
        let entry = listing::entry("a.txt", EntryKind::File, &attrs);
        let details = details_of(&entry, &attrs, None);
        assert_eq!(details.owner.as_deref(), Some("1000"));
        assert_eq!(details.group, None);
        assert_eq!(details.mode, Some(0o444));
        assert!(details.read_only);
        assert_eq!(details.accessed_ms, Some(3000));
        assert_eq!(details.mime_type.as_deref(), Some("text/plain"));
        assert!(details.unavailable.contains(&DetailField::Group));
        assert!(!details.unavailable.contains(&DetailField::Owner));
    }

    #[test]
    fn the_provider_serves_sftp_locations_only_and_claims_no_watching() {
        let provider = SftpProvider::new(SftpConfig::new(Arc::new(
            crate::host_keys::MemoryKnownHosts::new(),
        )));
        let caps = provider.capabilities();
        assert!(caps.remote && !caps.watch && caps.range_read);
        assert_eq!(caps.case_rule, CaseRule::Sensitive);
        let path = VfsPath::from_uri("sftp://me@h:2222/x").unwrap();
        assert_eq!(
            provider.connection_key(&path).unwrap().as_str(),
            "sftp://me@h:2222"
        );
        let local = VfsPath::from_uri(if cfg!(windows) {
            "file:///C:/tmp"
        } else {
            "file:///tmp"
        })
        .unwrap();
        assert_eq!(provider.connection_key(&local), None);
        assert!(matches!(
            provider.stat(&local),
            Err(VfsError::Unsupported { .. })
        ));
        assert_eq!(
            provider.connection_state(&path.connection_key().unwrap()),
            ConnectionState::Idle
        );
    }
}
