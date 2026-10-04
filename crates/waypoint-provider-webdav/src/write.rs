// The writes: `MKCOL`, `PUT`, `MOVE`, `COPY` and `DELETE`, each guarded by the conditions that keep
// it from clobbering what another writer did in the meantime.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::io::{self, Write};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use waypoint_path::VfsPath;
use waypoint_protocol::{Location, VfsError};
use waypoint_vfs::{CancelToken, EntryKind, ScannedEntry, WriteOptions, WriteStream};

use crate::client::Request;
use crate::errors::{from_status, Op};
use crate::provider::{Target, WebDavProvider};
use crate::read::guard;
use crate::spool::{Spool, Upload};
use crate::xml::Props;

/// The headers of an upload: what it must not replace, and for Nextcloud what the server can check
/// and keep.
struct Conditions {
    /// `If-None-Match: *`: only a name nothing has.
    new_only: bool,
    /// `If-Match` (or `If-Unmodified-Since`): only the version that was seen.
    guard: Option<(&'static str, String)>,
    modified: Option<SystemTime>,
}

fn put(target: &Target, upload: Arc<Upload>, conditions: &Conditions) -> Request {
    let mut request = Request::new("PUT", target.url(false))
        .header("Content-Type", "application/octet-stream")
        .upload(upload.clone());
    if conditions.new_only {
        request = request.header("If-None-Match", "*");
    }
    if let Some((name, value)) = &conditions.guard {
        request = request.header(name, value.clone());
    }
    if target.nextcloud {
        // The server checks the bytes it received against this and refuses a mismatch.
        request = request.header("OC-Checksum", format!("SHA256:{}", upload.sha256));
        if let Some(seconds) = conditions
            .modified
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map(|since| since.as_secs())
        {
            request = request.header("X-OC-MTime", seconds.to_string());
        }
    }
    request
}

/// A stream of an upload, which waits until `finish` and then sends it in one request.
struct DavWriter {
    provider: WebDavProvider,
    target: Target,
    conditions: Conditions,
    spool: Spool,
}

impl Write for DavWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.spool.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.spool.flush()
    }
}

impl WriteStream for DavWriter {
    fn finish(self: Box<Self>, _sync: bool) -> Result<(), VfsError> {
        let Self {
            provider,
            target,
            conditions,
            spool,
        } = *self;
        let upload = spool.finish().map_err(|error| VfsError::Io {
            message: format!("the upload could not be kept until it was sent: {error}"),
            location: Some(target.location.clone()),
        })?;
        provider.send_upload(&target, Arc::new(upload), &conditions)
    }
}

impl WebDavProvider {
    fn send_upload(
        &self,
        target: &Target,
        upload: Arc<Upload>,
        conditions: &Conditions,
    ) -> Result<(), VfsError> {
        let request = put(target, upload, conditions);
        let op = if conditions.new_only {
            Op::PutNew
        } else {
            Op::Put
        };
        let reply = self.exec(target, &request, op, None)?;
        // A `204` or `201`, whichever the server says, is the file in place.
        drop(reply);
        Ok(())
    }

    pub(crate) fn dav_create_dir(&self, path: &VfsPath) -> Result<(), VfsError> {
        Self::check_name(path)?;
        let target = self.target(path)?;
        // Some servers make a folder that is there already without complaint, so look first.
        match self.dav_stat(path) {
            Ok(_) => {
                return Err(VfsError::AlreadyExists {
                    location: target.location,
                })
            }
            Err(VfsError::NotFound { .. }) => {}
            Err(error) => return Err(error),
        }
        let request = Request::new("MKCOL", target.url(true));
        self.exec(&target, &request, Op::Mkcol, None).map(drop)
    }

    pub(crate) fn dav_create_file(&self, path: &VfsPath) -> Result<(), VfsError> {
        Self::check_name(path)?;
        let target = self.target(path)?;
        // A server that ignores `If-None-Match` would replace a file here, so look first.
        match self.dav_stat(path) {
            Ok(_) => {
                return Err(VfsError::AlreadyExists {
                    location: target.location,
                })
            }
            Err(VfsError::NotFound { .. }) => {}
            Err(error) => return Err(error),
        }
        let conditions = Conditions {
            new_only: true,
            guard: None,
            modified: self.take_hint(path),
        };
        self.send_upload(&target, Upload::empty(), &conditions)
    }

    pub(crate) fn dav_create_write(
        &self,
        path: &VfsPath,
        options: WriteOptions,
    ) -> Result<Box<dyn WriteStream>, VfsError> {
        Self::check_name(path)?;
        let target = self.target(path)?;
        let conditions = match self.dav_stat(path) {
            Ok(_) if options.exclusive => {
                return Err(VfsError::AlreadyExists {
                    location: target.location,
                });
            }
            Ok((entry, _)) if entry.kind == EntryKind::Directory => {
                return Err(VfsError::IsADirectory {
                    location: target.location,
                })
            }
            // Replacing a file replaces the version that was seen, and nothing newer.
            Ok((_, props)) => Conditions {
                new_only: false,
                guard: guard(&props),
                modified: self.take_hint(path),
            },
            Err(VfsError::NotFound { .. }) => Conditions {
                new_only: true,
                guard: None,
                modified: self.take_hint(path),
            },
            Err(error) => return Err(error),
        };
        Ok(Box::new(DavWriter {
            provider: self.clone(),
            target,
            conditions,
            spool: Spool::new(self.inner.config.spool_dir.clone()),
        }))
    }

    pub(crate) fn dav_rename(
        &self,
        from: &VfsPath,
        to: &VfsPath,
        overwrite: bool,
    ) -> Result<(), VfsError> {
        Self::check_name(to)?;
        let source = self.target(from)?;
        let destination = self.target(to)?;
        if source.remote.connection_key() != destination.remote.connection_key() {
            return Err(VfsError::CrossesDevices {
                from: source.location,
                to: destination.location,
            });
        }
        let request = Request::new("MOVE", source.url(false))
            .header("Destination", destination.url(false))
            .header("Overwrite", if overwrite { "T" } else { "F" });
        match self.exec(&source, &request, Op::Transfer, None) {
            Err(VfsError::AlreadyExists { .. }) => Err(VfsError::AlreadyExists {
                location: destination.location,
            }),
            // Some servers answer a missing destination folder with a plain failure.
            Err(error @ VfsError::Io { .. }) => Err(self.explain_transfer(to, error)),
            other => other.map(drop),
        }
    }

    /// A `MOVE` or `COPY` that failed without saying why: the destination's folder is looked at, as
    /// the usual reason (Apache answers `500` when it is missing, where RFC 4918 says `409`).
    fn explain_transfer(&self, to: &VfsPath, error: VfsError) -> VfsError {
        match to.parent().map(|parent| self.dav_stat(&parent)) {
            Some(Err(VfsError::NotFound { location })) => VfsError::NotFound { location },
            _ => error,
        }
    }

    /// The entry at `path` and its tag, for a delete that is conditional on both.
    fn expect_kind(
        &self,
        path: &VfsPath,
        wanted: EntryKind,
    ) -> Result<(Target, ScannedEntry, Props), VfsError> {
        let target = self.target(path)?;
        let (entry, props) = self.dav_stat(path)?;
        let is_dir = entry.kind == EntryKind::Directory;
        match (wanted, is_dir) {
            (EntryKind::File, true) => Err(VfsError::IsADirectory {
                location: target.location,
            }),
            (EntryKind::Directory, false) => Err(VfsError::NotADirectory {
                location: target.location,
            }),
            _ => Ok((target, entry, props)),
        }
    }

    fn delete(
        &self,
        target: &Target,
        folder: bool,
        guard: Option<(&'static str, String)>,
    ) -> Result<(), VfsError> {
        let mut request = Request::new("DELETE", target.url(folder));
        if let Some((name, value)) = guard {
            request = request.header(name, value);
        }
        let reply = self.exec(target, &request, Op::Delete, None)?;
        // `207` is a delete that failed for something inside what was asked to go.
        if reply.status().as_u16() == 207 {
            return Err(VfsError::Io {
                message: "the server could not remove everything".to_owned(),
                location: Some(target.location.clone()),
            });
        }
        Ok(())
    }

    pub(crate) fn dav_remove_file(&self, path: &VfsPath) -> Result<(), VfsError> {
        // A `DELETE` of a collection removes everything in it, so only a file is ever sent one here.
        let (target, _, props) = self.expect_kind(path, EntryKind::File)?;
        self.delete(&target, false, guard(&props))
    }

    pub(crate) fn dav_remove_dir(&self, path: &VfsPath) -> Result<(), VfsError> {
        let (target, _, _) = self.expect_kind(path, EntryKind::Directory)?;
        // The same reason: only a folder that holds nothing.
        let mut first = false;
        let cancel = CancelToken::new();
        self.dav_list(path, &cancel, &mut |batch| first |= !batch.is_empty())?;
        if first {
            return Err(VfsError::NotEmpty {
                location: target.location,
            });
        }
        self.delete(&target, true, None)
    }

    /// `COPY` on the server. `None` when the server cannot, or the source is not a file; nothing
    /// has been touched then.
    pub(crate) fn dav_copy(
        &self,
        src: &VfsPath,
        dst: &VfsPath,
        progress: &mut dyn FnMut(u64),
        cancel: &CancelToken,
    ) -> Option<Result<u64, VfsError>> {
        if cancel.is_cancelled() {
            return Some(Err(VfsError::Cancelled));
        }
        let (source, destination) = (self.target(src).ok()?, self.target(dst).ok()?);
        if source.remote.connection_key() != destination.remote.connection_key() {
            return None;
        }
        let (entry, _) = match self.dav_stat(src) {
            Ok(found) => found,
            Err(error) => return Some(Err(error)),
        };
        if entry.kind != EntryKind::File {
            return None;
        }
        let request = Request::new("COPY", source.url(false))
            .header("Destination", destination.url(false))
            .header("Overwrite", "F");
        let reply = match self.exec_raw(&source, &request, Some(cancel)) {
            Ok(reply) => reply,
            Err(error) => return Some(Err(error)),
        };
        let status = reply.status();
        match status.as_u16() {
            // A server that has no `COPY` leaves the caller to copy the bytes itself.
            405 | 501 => None,
            _ if status.is_success() => {
                let size = entry.size.unwrap_or(0);
                progress(size);
                Some(Ok(size))
            }
            _ => {
                let error = copy_error(status, &destination.location);
                Some(Err(match error {
                    VfsError::Io { .. } => self.explain_transfer(dst, error),
                    other => other,
                }))
            }
        }
    }
}

fn copy_error(status: reqwest::StatusCode, destination: &Location) -> VfsError {
    from_status(Op::Transfer, status, None, destination)
}
