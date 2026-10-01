// The inverse of a move across volumes: copy the entry the move left at its destination back to
// where it came from, and remove the destination copy once the copy is safely in place.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Nothing is overwritten and nothing newer is lost. The copy is built under a partial name beside
// its place and renamed in without overwriting; the entry being copied is fingerprinted again once
// the copy is in place, and only if it is still what the move left is it removed. If the removal
// fails, the copy is taken away again so the step is as it was and can be retried.

use waypoint_path::VfsPath;
use waypoint_protocol::{Location, VfsError};
use waypoint_vfs::{CancelToken, EntryKind, Provider};

use super::apply::{aside_path, remove_entry};
use super::fingerprint::verify_excluding;
use super::model::{Fingerprint, StaleReason};
use crate::exec::{copy_file_bytes, copy_metadata, remove_all, ExecEnv, FileCopy, CHUNK_BYTES};
use crate::model::{JobId, OpsError};

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
            copy_file_bytes(
                &FileCopy {
                    src_provider: sp,
                    src,
                    dst_provider: dp,
                    dst,
                    same_provider,
                    verify: None,
                    chunk: CHUNK_BYTES,
                    size_hint: entry.size.unwrap_or(0),
                    durable: true,
                },
                buf,
                &mut |_| {},
                cancel,
            )?;
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
