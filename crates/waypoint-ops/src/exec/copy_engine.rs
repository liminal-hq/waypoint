// Copying the bytes of one file (A50): a provider's fast path when it offers one, otherwise a
// chunked read and write loop with progress and a cancel check after every chunk, hashing what it
// reads when the copy is to be verified (A51), plus the read-back that hashes a file that was
// written.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::io::{self, Read, Write};

use waypoint_path::VfsPath;
use waypoint_protocol::VfsError;
use waypoint_vfs::{from_io, CancelToken, Provider, WriteOptions};

use crate::model::VerifyAlgorithm;
use crate::throttle::Throttle;
use crate::verify::Hasher;

/// The size of one read and write: 8 MiB (A50).
pub const CHUNK_BYTES: usize = 8 * 1024 * 1024;

/// One file to copy, from `src` to `dst`: a partial name, or the final name on a provider that
/// shows a file only once it is complete or cannot rename (A84).
pub struct FileCopy<'a> {
    pub src_provider: &'a dyn Provider,
    pub src: &'a VfsPath,
    pub dst_provider: &'a dyn Provider,
    /// Created as `options` say (exclusively, unless an existing file is replaced in place); on a
    /// failure of any kind nothing is left under this name that the copy wrote.
    pub dst: &'a VfsPath,
    /// How `dst` is opened: exclusively for a new name, truncating for a file replaced in place.
    pub options: WriteOptions,
    /// The destination shows a file only when its stream finishes (`atomic_write`), so a failure
    /// leaves nothing to remove, and removing the name would take away the file it replaces.
    pub atomic: bool,
    /// Both paths are served by one provider under one login, so it may offer a fast path.
    pub same_provider: bool,
    /// The provider's fast path is a copy on the server (`server_copy`), which commits there: it
    /// is used for a move too, where a local fast path is not, since it never syncs.
    pub server_copy: bool,
    /// Hash what is read (and so skip the fast path, which never shows the bytes).
    pub verify: Option<VerifyAlgorithm>,
    /// The source is to be removed once this copy is in place (a move across volumes): the data is
    /// synced to storage before the copy is renamed into place, and no fast path is used, since a
    /// fast path never syncs.
    pub durable: bool,
    /// The size of one read and write.
    pub chunk: usize,
    /// How big the file is believed to be, to size the buffer.
    pub size_hint: u64,
    /// The speed limits to obey, which cut the loop into paced pieces and keep the fast path out of
    /// it while one is in force. The bytes copied are the same with or without it.
    pub throttle: Option<&'a Throttle>,
    /// Continue a partial `dst` that holds the first `offset` bytes of the source (D165): the
    /// source is read from there and the destination written on with `resume_write`. Nothing is
    /// hashed while copying, so a verified copy reads the whole file back afterwards. 0 starts afresh.
    pub offset: u64,
    /// `dst` is kept when the copy stops on a lost connection, so it can be continued: the caller
    /// records it instead of the engine removing it.
    pub resumable: bool,
}

/// What a finished copy made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Copied {
    pub bytes: u64,
    /// The digest of what was read from the source, when the copy hashed it.
    pub digest: Option<Vec<u8>>,
}

/// Reads until `buf` is full or the stream ends, retrying a read an interrupt cut short.
fn fill(reader: &mut dyn Read, buf: &mut [u8]) -> io::Result<usize> {
    let mut filled = 0;
    while filled < buf.len() {
        match reader.read(&mut buf[filled..]) {
            Ok(0) => break,
            Ok(n) => filled += n,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
    Ok(filled)
}

/// Sizes the reusable buffer for a file of about `size_hint` bytes without allocating a whole chunk
/// for a small one.
fn buffer_for(buf: &mut Vec<u8>, chunk: usize, size_hint: u64) -> &mut [u8] {
    let wanted = (chunk.max(1) as u64).min(size_hint.max(1)) as usize;
    if buf.len() < wanted {
        buf.resize(wanted, 0);
    }
    &mut buf[..wanted]
}

/// Copies one file's bytes. `progress` receives the bytes copied so far after each chunk (a fast
/// path reports as it likes), and a cancel is `VfsError::Cancelled`. On any failure the partial
/// file is removed before the error is returned.
pub fn copy_file_bytes(
    request: &FileCopy<'_>,
    buf: &mut Vec<u8>,
    progress: &mut dyn FnMut(u64),
    cancel: &CancelToken,
) -> Result<Copied, VfsError> {
    let limited = request.throttle.is_some_and(Throttle::is_limited);
    let fast = request.same_provider
        && request.offset == 0
        && request.verify.is_none()
        && (!request.durable || request.server_copy)
        && request.options.exclusive
        && !limited;
    if fast {
        let attempt = request.src_provider.copy_file_within(
            request.src,
            request.dst,
            &mut |copied| progress(copied),
            cancel,
        );
        match attempt {
            Some(Ok(bytes)) => {
                return Ok(Copied {
                    bytes,
                    digest: None,
                })
            }
            // The fast path declined or could not cross what it met: the loop below can.
            Some(Err(VfsError::CrossesDevices { .. } | VfsError::Unsupported { .. })) => {
                let _ = request.dst_provider.remove_file(request.dst);
            }
            Some(Err(error)) => {
                let _ = request.dst_provider.remove_file(request.dst);
                return Err(error);
            }
            None => {}
        }
    }
    copy_loop(request, buf, progress, cancel)
}

fn copy_loop(
    request: &FileCopy<'_>,
    buf: &mut Vec<u8>,
    progress: &mut dyn FnMut(u64),
    cancel: &CancelToken,
) -> Result<Copied, VfsError> {
    let src_location = request.src.to_location();
    let dst_location = request.dst.to_location();
    let resuming = request.offset > 0;
    let mut reader = if resuming {
        request
            .src_provider
            .open_read_at(request.src, request.offset)?
    } else {
        request.src_provider.open_read(request.src)?
    };
    let opened = if resuming {
        request
            .dst_provider
            .resume_write(request.dst, request.offset)
    } else {
        request
            .dst_provider
            .create_write(request.dst, request.options)
    };
    let mut writer = opened?;
    let buffer = buffer_for(buf, request.chunk, request.size_hint);
    let mut hasher = request.verify.filter(|_| !resuming).map(Hasher::new);
    let mut copied = request.offset;
    let result = (|| -> Result<(), VfsError> {
        loop {
            if cancel.is_cancelled() {
                return Err(VfsError::Cancelled);
            }
            // A limited copy moves a piece at a time, sized by the limit as it is now.
            let piece = request
                .throttle
                .map_or(buffer.len(), |t| t.piece(buffer.len()));
            let read =
                fill(&mut reader, &mut buffer[..piece]).map_err(|e| from_io(&e, &src_location))?;
            if read == 0 {
                return Ok(());
            }
            if let Some(throttle) = request.throttle {
                throttle.pace(read, cancel)?;
            }
            if let Some(hasher) = hasher.as_mut() {
                hasher.update(&buffer[..read]);
            }
            writer
                .write_all(&buffer[..read])
                .map_err(|e| from_io(&e, &dst_location))?;
            copied += read as u64;
            progress(copied);
        }
    })();
    let finished = match result {
        // A verified copy asks the storage to commit the data before it is read back, and so does a
        // copy whose source is about to go: the copy must outlive a power loss.
        Ok(()) => writer.finish(request.verify.is_some() || request.durable),
        Err(error) => {
            drop(writer);
            Err(error)
        }
    };
    match finished {
        Ok(()) => Ok(Copied {
            bytes: copied,
            digest: hasher.map(Hasher::finish),
        }),
        Err(error) => {
            let keep = request.atomic || (request.resumable && is_lost(&error));
            if !keep {
                let _ = request.dst_provider.remove_file(request.dst);
            }
            Err(error)
        }
    }
}

/// Whether a copy stopped because the connection went away, which leaves a partial file worth
/// continuing.
pub(crate) fn is_lost(error: &VfsError) -> bool {
    matches!(
        error,
        VfsError::Disconnected { .. }
            | VfsError::Unreachable { .. }
            | VfsError::Timeout { .. }
            | VfsError::RateLimited { .. }
    )
}

/// Reads a file through and returns its digest (the read-back of a verified copy, and the second
/// look at a source that is about to be removed).
pub fn hash_file(
    provider: &dyn Provider,
    path: &VfsPath,
    algorithm: VerifyAlgorithm,
    chunk: usize,
    size_hint: u64,
    buf: &mut Vec<u8>,
    cancel: &CancelToken,
) -> Result<Vec<u8>, VfsError> {
    let location = path.to_location();
    let mut reader = provider.open_read(path)?;
    let buffer = buffer_for(buf, chunk, size_hint);
    let mut hasher = Hasher::new(algorithm);
    loop {
        if cancel.is_cancelled() {
            return Err(VfsError::Cancelled);
        }
        let read = fill(&mut reader, buffer).map_err(|e| from_io(&e, &location))?;
        if read == 0 {
            return Ok(hasher.finish());
        }
        hasher.update(&buffer[..read]);
    }
}
