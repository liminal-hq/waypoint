// The inverse of a move across volumes: copy the entry the move left at its destination back to
// where it came from, and remove the destination copy once the copy is safely in place.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Nothing is overwritten and nothing newer is lost. The copy is built under a partial name beside
// its place and renamed in without overwriting; the entry being copied is fingerprinted again once
// the copy is in place, and only if it is still what the move left is it removed. If the removal
// fails, the copy is taken away again so the step is as it was and can be retried. A copy that
// will outlive its source is synced to storage and read back against the digest of what was read
// from the source, and its length must be the size the move left, before the source is removed.

use std::io::Read;

use waypoint_path::VfsPath;
use waypoint_protocol::{Location, VfsError};
use waypoint_vfs::{CancelToken, EntryKind, Provider, ScannedEntry};

use super::apply::{aside_path, remove_entry};
use super::fingerprint::verify_excluding;
use super::model::{Fingerprint, StaleReason};
use crate::exec::{copy_file_bytes, copy_metadata, remove_all, ExecEnv, FileCopy, CHUNK_BYTES};
use crate::model::{JobId, OpsError, VerifyAlgorithm};
use crate::verify::{hex, Hasher};

fn name_error(name: &std::ffi::OsStr) -> OpsError {
    OpsError::Io {
        message: format!("{name:?} is not a usable name"),
    }
}

/// Copies the tree at `src` to `dst`, which must not exist, keeping times and modes. Links stay
/// links. The caller removes `dst` if this fails.
fn copy_tree(
    sp: &dyn Provider,
    src: &VfsPath,
    dp: &dyn Provider,
    dst: &VfsPath,
    same_provider: bool,
    cancel: &CancelToken,
    buf: &mut Vec<u8>,
) -> Result<(), OpsError> {
    if cancel.is_cancelled() {
        return Err(OpsError::Cancelled);
    }
    let entry = sp.stat(src)?;
    match entry.kind {
        EntryKind::Directory => {
            dp.create_dir(dst)?;
            for child in sp.list(src, cancel, 0, &mut |_| {})? {
                let from = src.join(&child.name).map_err(|_| name_error(&child.name))?;
                let to = dst.join(&child.name).map_err(|_| name_error(&child.name))?;
                copy_tree(sp, &from, dp, &to, same_provider, cancel, buf)?;
            }
            copy_metadata(sp, src, dp, dst, Some(entry.modified_ms))?;
        }
        EntryKind::Symlink => {
            let text = sp.read_link(src)?;
            dp.symlink(dst, &text)?;
        }
        EntryKind::File => {
            // The source is removed once the copy is in place, so the copy must be whole and on
            // storage: a verified copy is synced and has no fast path, and what was written is
            // read back and compared below.
            let copied = copy_file_bytes(
                &FileCopy {
                    src_provider: sp,
                    src,
                    dst_provider: dp,
                    dst,
                    same_provider,
                    verify: Some(VerifyAlgorithm::Blake3),
                    chunk: CHUNK_BYTES,
                    size_hint: entry.size.unwrap_or(0),
                    durable: true,
                    throttle: None,
                },
                buf,
                &mut |_| {},
                cancel,
            )?;
            let digest = copied.digest.unwrap_or_default();
            check_written(src, &entry, dp, dst, copied.bytes, &digest, cancel, buf)?;
            copy_metadata(sp, src, dp, dst, Some(entry.modified_ms))?;
        }
        EntryKind::Other => {
            return Err(OpsError::Unsupported {
                what: "copying a special file".to_owned(),
            })
        }
    }
    Ok(())
}

/// Checks that the copy holds what was read from the source and that the source is the size the
/// move left (else it changed under the copy): the length, then a read-back of every byte.
#[allow(clippy::too_many_arguments)]
fn check_written(
    src: &VfsPath,
    entry: &ScannedEntry,
    dp: &dyn Provider,
    dst: &VfsPath,
    bytes: u64,
    expected: &[u8],
    cancel: &CancelToken,
    buf: &mut Vec<u8>,
) -> Result<(), OpsError> {
    if entry.size.is_some_and(|size| size != bytes) {
        return Err(OpsError::UndoStale {
            location: src.to_location(),
            reason: StaleReason::Changed,
        });
    }
    let short = |held: Option<u64>| OpsError::Io {
        message: format!(
            "{} holds {} of the {} bytes copied to it",
            dst.display(),
            held.map_or_else(|| "an unknown number".to_owned(), |n| n.to_string()),
            bytes
        ),
    };
    let held = dp.stat(dst)?.size;
    if held != Some(bytes) {
        return Err(short(held));
    }
    let location = dst.to_location();
    let mut reader = dp.open_read(dst)?;
    let mut hasher = Hasher::new(VerifyAlgorithm::Blake3);
    buf.resize(CHUNK_BYTES.min(bytes.max(1) as usize), 0);
    loop {
        if cancel.is_cancelled() {
            return Err(OpsError::Cancelled);
        }
        let read = reader
            .read(buf)
            .map_err(|e| waypoint_vfs::from_io(&e, &location))?;
        if read == 0 {
            break;
        }
        hasher.update(&buf[..read]);
    }
    let actual = hasher.finish();
    if actual != expected {
        return Err(OpsError::VerifyFailed {
            location,
            expected: hex(expected),
            actual: hex(&actual),
        });
    }
    Ok(())
}

fn discard(provider: &dyn Provider, path: &VfsPath) {
    let _ = remove_all(provider, path);
}

/// Puts the entry at `from` back at `to`, as the step `CopyBack` says.
pub(super) fn copy_back(
    env: &ExecEnv,
    job: JobId,
    from: &Location,
    to: &Location,
    expected: Option<&Fingerprint>,
    excluded: &[String],
    cancel: &CancelToken,
) -> Result<(), OpsError> {
    let (from_path, sp) = env.providers.for_location(from)?;
    let (to_path, dp) = env.providers.for_location(to)?;
    let same_provider = std::sync::Arc::ptr_eq(&sp, &dp);
    let partial = aside_path(env, job, &to_path)?;
    let mut buf = Vec::new();
    if let Err(error) = copy_tree(
        sp.as_ref(),
        &from_path,
        dp.as_ref(),
        &partial,
        same_provider,
        cancel,
        &mut buf,
    ) {
        discard(dp.as_ref(), &partial);
        return Err(error);
    }
    if let Err(error) = dp.rename(&partial, &to_path, false) {
        discard(dp.as_ref(), &partial);
        return Err(match error {
            VfsError::AlreadyExists { .. } => OpsError::UndoStale {
                location: to.clone(),
                reason: StaleReason::NameTaken,
            },
            other => other.into(),
        });
    }
    // What was copied must still be what the move left, or the copy lacks newer work.
    if let Err(reason) = verify_excluding(sp.as_ref(), &from_path, expected, excluded) {
        let _ = remove_entry(env, job, to);
        return Err(OpsError::UndoStale {
            location: from.clone(),
            reason,
        });
    }
    if let Err(error) = remove_entry(env, job, from) {
        // The copy is in place and the original would not go: take the copy away again so the
        // step is as it was and undoing again can retry it.
        let _ = remove_entry(env, job, to);
        return Err(error);
    }
    Ok(())
}
