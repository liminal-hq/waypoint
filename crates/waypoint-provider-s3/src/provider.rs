// `S3Provider`: the `Provider` contract over buckets and keys.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::future::Future;
use std::io::Read;
use std::sync::Arc;

use waypoint_path::{CaseRule, ConnectionKey, RemotePath, VfsPath};
use waypoint_protocol::{ConnectionState, VfsError};
use waypoint_vfs::{
    CancelToken, Capabilities, ConnectAnswer, Credential, Provider, ReadStream, RenameSupport,
    ScannedEntry, WriteOptions, WriteStream,
};

use crate::address::{address, remote, Address, MAX_KEY_BYTES};
use crate::ops::{BucketInfo, ListedEntry};
use crate::options::{S3Config, S3Options};
use crate::reader::{self, S3Reader};
use crate::service::{Creds, Failed, Inner};
use crate::storage_class::ObjectAttributes;
use crate::writer::S3Writer;

/// The S3 provider. Cloning shares its connections.
#[derive(Clone)]
pub struct S3Provider {
    inner: Arc<Inner>,
}

impl S3Provider {
    pub fn new(config: S3Config) -> Self {
        Self {
            inner: Arc::new(Inner::new(config)),
        }
    }

    /// The tuning and extra login details of one connection, used from its next request on.
    pub fn set_options(&self, key: &ConnectionKey, options: S3Options) {
        self.inner.set_options(key, options);
    }

    /// Runs `future` to its end on the provider's runtime. Callers are worker threads, never the
    /// runtime's own.
    fn block<F: Future>(&self, future: F) -> F::Output {
        self.inner.runtime().block_on(future)
    }

    fn typed<T>(&self, address: &Address, result: Result<T, Failed>) -> Result<T, VfsError> {
        result.map_err(|failed| failed.into_vfs(&address.location))
    }

    /// Lists a folder in batches that carry the storage class and ETag of each object, so the
    /// Storage class column has them without a request per row.
    pub fn list_batches_with_attributes(
        &self,
        path: &VfsPath,
        cancel: &CancelToken,
        sink: &mut dyn FnMut(Vec<ListedEntry>),
    ) -> Result<(), VfsError> {
        let a = address(path)?;
        let result = self.block(self.inner.list_stream(&a, cancel, sink));
        self.typed(&a, result)
    }

    /// `stat`, with the storage class and ETag when the entry is an object.
    pub fn stat_with_attributes(
        &self,
        path: &VfsPath,
    ) -> Result<(ScannedEntry, Option<ObjectAttributes>), VfsError> {
        let a = address(path)?;
        let result = self.block(self.inner.stat(&a));
        self.typed(&a, result)
    }

    /// The buckets of the service `path` is on, signed in as `path`'s connection does. A service
    /// root has no address of its own (a location's authority is a bucket), so the Connect dialog
    /// asks through any bucket it already knows, or the one the person typed.
    pub fn list_buckets(&self, path: &VfsPath) -> Result<Vec<BucketInfo>, VfsError> {
        let a = address(path)?;
        let result = self.block(self.inner.list_buckets(&a));
        self.typed(&a, result)
    }

    /// Creates the bucket `path` names, for a service where the person owns the buckets (MinIO,
    /// a private cloud). `AlreadyExists` when it is there.
    pub fn create_bucket(&self, path: &VfsPath) -> Result<(), VfsError> {
        let a = address(path)?;
        let result = self.block(self.inner.create_bucket(&a));
        self.typed(&a, result)
    }

    /// Whether the exclusive create of `address` would be atomic on its service.
    fn conditional(&self, a: &Address, exclusive: bool) -> bool {
        exclusive && self.inner.conditional_writes(a)
    }

    fn writer(&self, a: &Address, exclusive: bool) -> S3Writer {
        S3Writer::new(
            self.inner.clone(),
            self.inner.runtime().handle().clone(),
            a.clone(),
            &self.inner.options(&a.connection),
            self.conditional(a, exclusive),
        )
    }

    fn not_a_file(&self, a: &Address) -> VfsError {
        VfsError::IsADirectory {
            location: a.location.clone(),
        }
    }
}

impl Provider for S3Provider {
    fn scheme(&self) -> &'static str {
        "s3"
    }

    fn capabilities(&self) -> Capabilities {
        let mut caps = Capabilities::new(CaseRule::Sensitive);
        caps.remote = true;
        caps.write = true;
        // There is no rename on S3: a move is a copy and a delete, item by item, and is not
        // atomic, so the engine does it (and a folder move shows its warning).
        caps.rename = RenameSupport::None;
        caps.server_copy = true;
        // An object appears whole when the write finishes.
        caps.atomic_write = true;
        caps.resume_write = true;
        caps.range_read = true;
        caps.max_name_len = Some(MAX_KEY_BYTES as u32);
        caps
    }

    fn stat(&self, path: &VfsPath) -> Result<ScannedEntry, VfsError> {
        self.stat_with_attributes(path).map(|(entry, _)| entry)
    }

    fn list(
        &self,
        path: &VfsPath,
        cancel: &CancelToken,
        _inline_link_budget: usize,
        progress: &mut dyn FnMut(u32),
    ) -> Result<Vec<ScannedEntry>, VfsError> {
        let mut entries = Vec::new();
        self.list_batches_with_attributes(path, cancel, &mut |batch| {
            entries.extend(batch.into_iter().map(|listed| listed.entry));
            progress(entries.len().min(u32::MAX as usize) as u32);
        })?;
        Ok(entries)
    }

    fn resolve_link(
        &self,
        _folder: &VfsPath,
        entry: &ScannedEntry,
    ) -> Result<ScannedEntry, VfsError> {
        // S3 has no links.
        Ok(entry.clone())
    }

    fn list_batches(
        &self,
        path: &VfsPath,
        cancel: &CancelToken,
        _inline_link_budget: usize,
        sink: &mut dyn FnMut(Vec<ScannedEntry>),
    ) -> Result<(), VfsError> {
        self.list_batches_with_attributes(path, cancel, &mut |batch| {
            sink(batch.into_iter().map(|listed| listed.entry).collect())
        })
    }

    fn connection_key(&self, path: &VfsPath) -> Option<ConnectionKey> {
        remote(path).ok().map(RemotePath::connection_key)
    }

    fn connection_state(&self, key: &ConnectionKey) -> ConnectionState {
        self.inner
            .existing_conn(key)
            .map_or(ConnectionState::Idle, |conn| conn.state())
    }

    fn connect(
        &self,
        key: &ConnectionKey,
        answer: Option<ConnectAnswer>,
        cancel: &CancelToken,
    ) -> Result<(), VfsError> {
        let remote = RemotePath::from_uri(key.as_str()).map_err(|_| VfsError::InvalidLocation {
            input: key.as_str().to_owned(),
        })?;
        let a = address(&VfsPath::Remote(remote))?;
        if let Some(ConnectAnswer::Credential(Credential::AccessKey { key_id, secret })) = answer {
            let token = self.inner.options(key).session_token;
            self.inner.conn(key).set_credentials(Creds::Keys {
                key_id,
                secret,
                token,
                region: None,
            });
        }
        // One cheap request proves the login, the bucket and the endpoint together.
        let result = self.block(crate::service::cancellable(cancel, async {
            self.inner.exists(&a).await.map(|_| ())
        }));
        self.typed(&a, result)
    }

    fn disconnect(&self, key: &ConnectionKey) {
        if let Some(conn) = self.inner.existing_conn(key) {
            conn.disconnect();
        }
    }

    fn create_dir(&self, path: &VfsPath) -> Result<(), VfsError> {
        let a = address(path)?;
        if a.is_bucket() {
            return Err(VfsError::Unsupported {
                what: "creating buckets here".to_owned(),
            });
        }
        let result = self.block(async {
            if self.inner.exists(&a).await? {
                return Err(Failed::Vfs(VfsError::AlreadyExists {
                    location: a.location.clone(),
                }));
            }
            self.inner.put_marker(&a).await
        });
        self.typed(&a, result)
    }

    fn create_file(&self, path: &VfsPath) -> Result<(), VfsError> {
        let a = address(path)?;
        if a.is_bucket() {
            return Err(self.not_a_file(&a));
        }
        let stream = self.create_write(path, WriteOptions::exclusive())?;
        stream.finish(false)
    }

    // `rename` stays `Unsupported` (the default): see `capabilities`.

    fn remove_file(&self, path: &VfsPath) -> Result<(), VfsError> {
        let a = address(path)?;
        if a.is_bucket() {
            return Err(self.not_a_file(&a));
        }
        let result = self.block(async {
            if self.inner.head(&a).await?.is_some() {
                return self.inner.delete_key(&a, a.key.clone()).await;
            }
            // S3 deletes a missing key without complaint; the contract does not.
            if self.inner.has_children(&a).await? {
                Err(Failed::Vfs(self.not_a_file(&a)))
            } else {
                Err(Failed::S3(crate::errors::S3Error::NoSuchKey))
            }
        });
        self.typed(&a, result)
    }

    fn remove_dir(&self, path: &VfsPath) -> Result<(), VfsError> {
        let a = address(path)?;
        if a.is_bucket() {
            return Err(VfsError::Unsupported {
                what: "removing buckets here".to_owned(),
            });
        }
        let result = self.block(async {
            let keys = self.inner.first_keys(&a, 2).await?;
            let marker = a.marker();
            if keys.is_empty() {
                // Nothing under the prefix: a file, or nothing.
                return Err(if self.inner.head(&a).await?.is_some() {
                    Failed::Vfs(VfsError::NotADirectory {
                        location: a.location.clone(),
                    })
                } else {
                    Failed::S3(crate::errors::S3Error::NoSuchKey)
                });
            }
            if keys.iter().any(|key| *key != marker) {
                return Err(Failed::Vfs(VfsError::NotEmpty {
                    location: a.location.clone(),
                }));
            }
            self.inner.delete_key(&a, marker).await
        });
        self.typed(&a, result)
    }

    fn open_read(&self, path: &VfsPath) -> Result<ReadStream, VfsError> {
        self.open_read_at(path, 0)
    }

    fn open_read_at(&self, path: &VfsPath, start: u64) -> Result<ReadStream, VfsError> {
        let a = address(path)?;
        if a.is_bucket() {
            return Err(self.not_a_file(&a));
        }
        match self.block(self.inner.get(&a, start, None)) {
            Ok(opened) => Ok(Box::new(S3Reader::new(
                self.inner.clone(),
                self.inner.runtime().handle().clone(),
                a,
                start,
                opened,
            ))),
            Err(failed) if start > 0 && reader::past_the_end(&failed) => Ok(reader::empty()),
            Err(failed) => {
                // A key that is not an object may still be a folder.
                if failed.is(&crate::errors::S3Error::NoSuchKey) {
                    if let Ok(true) = self.block(self.inner.has_children(&a)) {
                        return Err(self.not_a_file(&a));
                    }
                }
                Err(failed.into_vfs(&a.location))
            }
        }
    }

    fn create_write(
        &self,
        path: &VfsPath,
        options: WriteOptions,
    ) -> Result<Box<dyn WriteStream>, VfsError> {
        let a = address(path)?;
        if a.is_bucket() {
            return Err(self.not_a_file(&a));
        }
        if options.exclusive {
            let taken = self.block(self.inner.exists(&a));
            if self.typed(&a, taken)? {
                return Err(VfsError::AlreadyExists {
                    location: a.location.clone(),
                });
            }
        }
        Ok(Box::new(self.writer(&a, options.exclusive)))
    }

    fn resume_write(&self, path: &VfsPath, offset: u64) -> Result<Box<dyn WriteStream>, VfsError> {
        let a = address(path)?;
        if a.is_bucket() {
            return Err(self.not_a_file(&a));
        }
        let meta = self.block(self.inner.head(&a));
        let Some(meta) = self.typed(&a, meta)? else {
            return Err(VfsError::NotFound {
                location: a.location.clone(),
            });
        };
        if meta.size < offset {
            return Err(VfsError::Io {
                message: "the partial file is shorter than the offset to resume at".to_owned(),
                location: Some(a.location.clone()),
            });
        }
        // An object cannot be appended to: the new object is the first `offset` bytes of the old
        // one, read back, and what follows. It replaces the old one only when it is complete.
        let mut writer = self.writer(&a, false);
        if offset > 0 {
            let opened = self.block(self.inner.get(&a, 0, meta.attributes.etag.as_deref()));
            let opened = self.typed(&a, opened)?;
            let source = S3Reader::new(
                self.inner.clone(),
                self.inner.runtime().handle().clone(),
                a.clone(),
                0,
                opened,
            );
            std::io::copy(&mut source.take(offset), &mut writer)
                .map_err(|error| waypoint_vfs::from_io(&error, &a.location))?;
        }
        Ok(Box::new(writer))
    }

    fn copy_file_within(
        &self,
        src: &VfsPath,
        dst: &VfsPath,
        progress: &mut dyn FnMut(u64),
        cancel: &CancelToken,
    ) -> Option<Result<u64, VfsError>> {
        let (Ok(from), Ok(to)) = (address(src), address(dst)) else {
            return None;
        };
        // A copy on the server needs both ends on one service, and signs in as the destination.
        if !from.same_service(&to) || from.is_bucket() || to.is_bucket() {
            return None;
        }
        let threshold = self
            .inner
            .options(&to.connection)
            .copy_part_threshold
            .unwrap_or(crate::options::MAX_SINGLE_COPY);
        let result = self.block(async {
            let Some(meta) = self.inner.head(&from).await? else {
                return Err(Failed::S3(crate::errors::S3Error::NoSuchKey));
            };
            if self.inner.exists(&to).await? {
                return Err(Failed::Vfs(VfsError::AlreadyExists {
                    location: to.location.clone(),
                }));
            }
            self.inner
                .copy_object(&from, &to, meta.size, threshold, cancel)
                .await?;
            Ok(meta.size)
        });
        Some(match result {
            Ok(size) => {
                progress(size);
                Ok(size)
            }
            // A missing source is the source's NotFound, not the destination's.
            Err(failed) if failed.is(&crate::errors::S3Error::NoSuchKey) => {
                Err(VfsError::NotFound {
                    location: from.location.clone(),
                })
            }
            Err(Failed::S3(crate::errors::S3Error::Archived)) => {
                Err(crate::errors::S3Error::Archived.into_vfs(&from.location))
            }
            Err(failed) => Err(failed.into_vfs(&to.location)),
        })
    }
}
