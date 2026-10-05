// `SmbProvider` on Linux: the `Provider` contract over the session pool and the `smb2` client.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::HashMap;
use std::future::Future;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex, OnceLock};

use waypoint_path::{CaseRule, ConnectionKey, RemotePath, VfsPath};
use waypoint_protocol::{ConnectionState, Location, VfsError};
use waypoint_vfs::{
    validate_name, CancelToken, Capabilities, ConnectAnswer, FileTimes, PermissionModel, Provider,
    ReadStream, RenameSupport, ScannedEntry, VolumeId, VolumeSpace, WriteOptions, WriteStream,
};

use crate::changes::{self, bounded};
use crate::errors::{ends_session, from_smb2};
use crate::listing;
use crate::options::{SmbConfig, SmbOptions};
use crate::paths::{remote, target, Target};
use crate::pool::Pool;
use crate::read::SmbReader;
use crate::session::{Opening, Session};
use crate::write::SmbWriter;

struct Inner {
    config: SmbConfig,
    pool: Pool,
    options: Mutex<HashMap<ConnectionKey, SmbOptions>>,
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

/// The SMB provider. Cloning shares its sessions.
#[derive(Clone)]
pub struct SmbProvider {
    inner: Arc<Inner>,
}

impl SmbProvider {
    pub fn new(config: SmbConfig) -> Self {
        Self {
            inner: Arc::new(Inner {
                config,
                pool: Pool::default(),
                options: Mutex::new(HashMap::new()),
                runtime: OnceLock::new(),
            }),
        }
    }

    /// The tuning of one connection, used from its next session on.
    pub fn set_options(&self, key: &ConnectionKey, options: SmbOptions) {
        self.inner
            .options
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(key.clone(), options);
    }

    fn options(&self, key: &ConnectionKey) -> SmbOptions {
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
                .thread_name("waypoint-smb")
                .enable_all()
                .build()
                .expect("the SMB runtime starts")
        })
    }

    /// Runs `future` to its end on the provider's runtime. Callers are worker threads, never the
    /// runtime's own.
    fn block<F: Future>(&self, future: F) -> F::Output {
        self.runtime().block_on(future)
    }

    /// The open session of `path`'s connection, opening one when there is none. With an `answer`
    /// a new session is opened whatever is open.
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
        let _gate = slot.gate.lock().await;
        if answer.is_none() {
            if let Some(session) = slot.live() {
                return Ok(session);
            }
        }
        slot.set_state(ConnectionState::Connecting);
        let opening = Opening {
            config: &self.inner.config,
            options: self.options(&key),
            answer,
            location,
        };
        let opened = match cancel {
            Some(cancel) => tokio::select! {
                opened = opening.open(path) => Some(opened),
                () = listing::cancelled(cancel) => None,
            },
            None => Some(opening.open(path).await),
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
            Some(Err(error)) => {
                slot.set_state(ConnectionState::Failed {
                    error: error.clone(),
                });
                Err(error)
            }
        }
    }

    /// Runs `op` on `path`'s session. A session that turns out to be gone is forgotten, so the
    /// next call reconnects; an `idempotent` call reconnects and tries once more at once.
    fn with_session<T, F, Fut>(
        &self,
        path: &VfsPath,
        idempotent: bool,
        op: F,
    ) -> Result<T, VfsError>
    where
        F: Fn(Arc<Session>, Target, Location) -> Fut,
        Fut: Future<Output = Result<T, VfsError>>,
    {
        let remote = remote(path)?;
        let target = target(remote)?;
        let location = path.to_location();
        let slot = self.inner.pool.slot(&remote.connection_key());
        self.block(async {
            let mut retried = false;
            loop {
                let session = self.session(remote, &location, None, None).await?;
                match op(session.clone(), target.clone(), location.clone()).await {
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

    /// The share and path of something inside a share, for a change: the server's root and the
    /// shares themselves cannot be created, renamed or removed from here.
    fn inside(target: Target, what: &str) -> Result<(String, String), VfsError> {
        match target {
            Target::Inside { share, inner } if !inner.is_empty() => Ok((share, inner)),
            _ => Err(VfsError::Unsupported {
                what: what.to_owned(),
            }),
        }
    }

    /// Checks the name `path` would create, before anything is sent.
    fn check_name(path: &VfsPath) -> Result<(), VfsError> {
        match path.file_name() {
            Some(name) => validate_name(&name, CaseRule::Insensitive),
            None => Err(VfsError::InvalidName {
                name: String::new(),
                reason: "the root of a server has no name".to_owned(),
            }),
        }
    }

    fn name_of(path: &VfsPath) -> String {
        path.file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default()
    }
}

impl Provider for SmbProvider {
    fn scheme(&self) -> &'static str {
        "smb"
    }

    fn capabilities(&self) -> Capabilities {
        let mut caps = Capabilities::new(CaseRule::Insensitive);
        caps.remote = true;
        caps.write = true;
        // The server refuses a rename onto an existing name in the same step.
        caps.rename = RenameSupport::NoReplace;
        caps.server_copy = true;
        caps.resume_write = true;
        caps.range_read = true;
        // The attributes of a file are not read, so no permission rows are shown.
        caps.permissions = PermissionModel::None;
        caps.set_times = true;
        caps.max_name_len = Some(255);
        caps
    }

    fn stat(&self, path: &VfsPath) -> Result<ScannedEntry, VfsError> {
        let name = Self::name_of(path);
        self.with_session(path, true, |session, target, location| {
            let name = name.clone();
            async move { listing::stat(&session, &target, &name, &location).await }
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
        _inline_link_budget: usize,
        sink: &mut dyn FnMut(Vec<ScannedEntry>),
    ) -> Result<(), VfsError> {
        if cancel.is_cancelled() {
            return Err(VfsError::Cancelled);
        }
        let remote = remote(path)?;
        let target = target(remote)?;
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
                let result =
                    listing::list(&session, &target, &location, cancel, &mut counting).await;
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
        _folder: &VfsPath,
        entry: &ScannedEntry,
    ) -> Result<ScannedEntry, VfsError> {
        // SMB entries are never links, so none is left pending.
        Ok(entry.clone())
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
        if let Some(session) = slot.take() {
            self.runtime().spawn(async move { session.close().await });
        }
    }

    fn open_read(&self, path: &VfsPath) -> Result<ReadStream, VfsError> {
        self.open_read_at(path, 0)
    }

    fn open_read_at(&self, path: &VfsPath, start: u64) -> Result<ReadStream, VfsError> {
        let runtime = self.runtime().handle().clone();
        self.with_session(path, true, |session, target, location| {
            let runtime = runtime.clone();
            async move {
                let (share, inner) = match target {
                    Target::Inside { share, inner } if !inner.is_empty() => (share, inner),
                    _ => return Err(VfsError::IsADirectory { location }),
                };
                let (mut client, mut tree) = session.share(&share, &location).await?;
                let file = bounded(&session, &location, async {
                    // Opening a folder for reading succeeds on the server; reading it does not.
                    let info = client
                        .stat(&mut tree, &inner)
                        .await
                        .map_err(|error| from_smb2(&error, &location))?;
                    if info.is_directory {
                        return Err(VfsError::IsADirectory {
                            location: location.clone(),
                        });
                    }
                    client
                        .open_file_reader(&tree, &inner)
                        .await
                        .map_err(|error| from_smb2(&error, &location))
                })
                .await?;
                drop(client);
                let reader = SmbReader::start(
                    &runtime,
                    file,
                    start,
                    session.options.read_requests,
                    session.options.chunk,
                    location,
                );
                Ok(Box::new(reader) as ReadStream)
            }
        })
    }

    fn create_dir(&self, path: &VfsPath) -> Result<(), VfsError> {
        Self::check_name(path)?;
        self.with_session(path, false, |session, target, location| async move {
            let (share, inner) = Self::inside(target, "creating shares")?;
            changes::create_dir(&session, &share, &inner, &location).await
        })
    }

    fn create_file(&self, path: &VfsPath) -> Result<(), VfsError> {
        Self::check_name(path)?;
        self.with_session(path, false, |session, target, location| async move {
            let (share, inner) = Self::inside(target, "creating files outside a share")?;
            changes::create_file(&session, &share, &inner, &location).await
        })
    }

    fn rename(&self, from: &VfsPath, to: &VfsPath, overwrite: bool) -> Result<(), VfsError> {
        Self::check_name(to)?;
        let destination = target(remote(to)?)?;
        let to_location = to.to_location();
        self.with_session(from, false, |session, source, location| {
            let (destination, to_location) = (destination.clone(), to_location.clone());
            async move {
                let (share, inner) = Self::inside(source, "renaming shares")?;
                let (to_share, to_inner) = Self::inside(destination, "renaming into a server")?;
                if !share.eq_ignore_ascii_case(&to_share) {
                    return Err(VfsError::CrossesDevices {
                        from: location,
                        to: to_location,
                    });
                }
                changes::rename(&session, &share, &inner, &to_inner, overwrite, &location).await
            }
        })
    }

    fn remove_file(&self, path: &VfsPath) -> Result<(), VfsError> {
        self.with_session(path, false, |session, target, location| async move {
            let (share, inner) = Self::inside(target, "removing shares")?;
            changes::remove_file(&session, &share, &inner, &location).await
        })
    }

    fn remove_dir(&self, path: &VfsPath) -> Result<(), VfsError> {
        self.with_session(path, false, |session, target, location| async move {
            let (share, inner) = Self::inside(target, "removing shares")?;
            changes::remove_dir(&session, &share, &inner, &location).await
        })
    }

    fn create_write(
        &self,
        path: &VfsPath,
        options: WriteOptions,
    ) -> Result<Box<dyn WriteStream>, VfsError> {
        Self::check_name(path)?;
        let runtime = self.runtime().handle().clone();
        self.with_session(path, false, |session, target, location| {
            let runtime = runtime.clone();
            async move {
                let (share, inner) = Self::inside(target, "writing files outside a share")?;
                let (mut client, tree) = session.share(&share, &location).await?;
                let writer = bounded(&session, &location, async {
                    let opened = if options.exclusive {
                        client.create_file_writer_exclusive(&tree, &inner).await
                    } else {
                        client.create_file_writer(&tree, &inner).await
                    };
                    opened.map_err(|error| from_smb2(&error, &location))
                })
                .await?;
                drop(client);
                Ok(Box::new(SmbWriter::new(
                    runtime,
                    writer,
                    session.options.chunk,
                    location,
                )) as Box<dyn WriteStream>)
            }
        })
    }

    fn resume_write(&self, path: &VfsPath, offset: u64) -> Result<Box<dyn WriteStream>, VfsError> {
        let runtime = self.runtime().handle().clone();
        self.with_session(path, false, |session, target, location| {
            let runtime = runtime.clone();
            async move {
                let (share, inner) = Self::inside(target, "writing files outside a share")?;
                let size = {
                    let (mut client, mut tree) = session.share(&share, &location).await?;
                    bounded(&session, &location, async {
                        client
                            .stat(&mut tree, &inner)
                            .await
                            .map_err(|error| from_smb2(&error, &location))
                    })
                    .await?
                    .size
                };
                if size < offset {
                    return Err(VfsError::Io {
                        message: "the partial file is shorter than where the write resumes"
                            .to_owned(),
                        location: Some(location),
                    });
                }
                if size > offset {
                    // What follows the offset is discarded, as the contract says.
                    changes::truncate(&session, &share, &inner, offset, &location).await?;
                }
                let (mut client, tree) = session.share(&share, &location).await?;
                let writer = bounded(&session, &location, async {
                    client
                        .create_file_writer_at(&tree, &inner, offset)
                        .await
                        .map_err(|error| from_smb2(&error, &location))
                })
                .await?;
                drop(client);
                Ok(Box::new(SmbWriter::new(
                    runtime,
                    writer,
                    session.options.chunk,
                    location,
                )) as Box<dyn WriteStream>)
            }
        })
    }

    fn set_times(&self, path: &VfsPath, times: FileTimes) -> Result<(), VfsError> {
        self.with_session(path, false, |session, target, location| async move {
            let (share, inner) = Self::inside(target, "setting the times of a share")?;
            changes::set_times(&session, &share, &inner, times, &location).await
        })
    }

    fn volume_id(&self, path: &VfsPath) -> Option<VolumeId> {
        let remote = remote(path).ok()?;
        let Target::Inside { share, .. } = target(remote).ok()? else {
            return None;
        };
        // A share is a volume: a rename across two fails, and the engine copies instead.
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        remote.connection_key().hash(&mut hasher);
        share.to_lowercase().hash(&mut hasher);
        Some(VolumeId(hasher.finish()))
    }

    fn free_space(&self, path: &VfsPath) -> Option<VolumeSpace> {
        self.with_session(path, true, |session, target, location| async move {
            match target {
                Target::Inside { share, .. } => {
                    Ok(changes::free_space(&session, &share, &location).await)
                }
                Target::Shares => Ok(None),
            }
        })
        .ok()
        .flatten()
    }

    fn copy_file_within(
        &self,
        src: &VfsPath,
        dst: &VfsPath,
        progress: &mut dyn FnMut(u64),
        cancel: &CancelToken,
    ) -> Option<Result<u64, VfsError>> {
        let (from, to) = (remote(src).ok()?, remote(dst).ok()?);
        if from.connection_key() != to.connection_key() {
            return None;
        }
        let (
            Target::Inside {
                share,
                inner: from_inner,
            },
            Target::Inside {
                share: to_share,
                inner: to_inner,
            },
        ) = (target(from).ok()?, target(to).ok()?)
        else {
            return None;
        };
        if !share.eq_ignore_ascii_case(&to_share) || from_inner.is_empty() || to_inner.is_empty() {
            return None;
        }
        if Self::check_name(dst).is_err() {
            return None;
        }
        let location = dst.to_location();
        let slot = self.inner.pool.slot(&from.connection_key());
        let copied = self.block(async {
            let session = match self.session(from, &location, None, Some(cancel)).await {
                Ok(session) => session,
                Err(error) => return Some(Err(error)),
            };
            let result =
                changes::copy_within(&session, &share, &from_inner, &to_inner, cancel, &location)
                    .await;
            if let Some(Err(error)) = &result {
                if ends_session(error) {
                    slot.lost(&session, error.clone());
                }
            }
            result
        });
        if let Some(Ok(bytes)) = &copied {
            progress(*bytes);
        }
        copied
    }
}
