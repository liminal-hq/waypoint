// Copies of a server's files on this computer, for a drag out of the window: another application
// can only be handed local files, so files dragged from a server are downloaded first (D151).
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::HashSet;
use std::ffi::OsStr;

use waypoint_path::VfsPath;
use waypoint_protocol::Location;
use waypoint_vfs::{child_path, CancelToken, EntryKind, WriteOptions};

use crate::exec::{copy_file_bytes, FileCopy, CHUNK_BYTES};
use crate::model::OpsError;
use crate::names::{fold_name, unique_full_name};
use crate::traits::Providers;

/// How much a drag out may download before it starts: a drag waits for it with the button held, so
/// it stays small. Larger files are copied with Copy and Paste or Copy To….
pub const STAGE_LIMIT_BYTES: u64 = 64 * 1024 * 1024;

/// Downloads each of `items` into `folder` (a local folder that exists and holds nothing the
/// caller wants kept) and returns the copies' locations, in order. A location already on a local
/// disk is handed back as it is. Only files are downloaded: a folder from a server, or files over
/// `limit` bytes in all, are `Unsupported` before anything is read. A failure removes what was
/// written.
pub fn stage_files(
    providers: &Providers,
    items: &[Location],
    folder: &VfsPath,
    limit: u64,
    cancel: &CancelToken,
) -> Result<Vec<Location>, OpsError> {
    let dp = providers.for_path(folder)?;
    let rule = dp.capabilities().case_rule;
    // Look first: nothing is written for a drag that cannot be made.
    let mut planned = Vec::with_capacity(items.len());
    let mut total = 0u64;
    for location in items {
        let (path, sp) = providers.for_location(location)?;
        if sp.connection_key(&path).is_none() {
            planned.push((path, None));
            continue;
        }
        let entry = sp.stat(&path)?;
        if entry.kind != EntryKind::File {
            return Err(OpsError::Unsupported {
                what: "dragging a folder from a server out of the window".to_owned(),
            });
        }
        total = total.saturating_add(entry.size.unwrap_or(0));
        if total > limit {
            return Err(OpsError::Unsupported {
                what: format!(
                    "dragging more than {} MB from a server out of the window",
                    limit / (1024 * 1024)
                ),
            });
        }
        planned.push((path, Some((sp, entry))));
    }
    let mut taken: HashSet<_> = HashSet::new();
    let mut written: Vec<VfsPath> = Vec::new();
    let mut buf = Vec::new();
    let mut out = Vec::with_capacity(planned.len());
    let result = (|| -> Result<(), OpsError> {
        for (path, remote) in &planned {
            let Some((sp, entry)) = remote else {
                out.push(path.to_location());
                continue;
            };
            let name = path.file_name().unwrap_or_else(|| "file".into());
            let name = unique_full_name(
                &mut |n| taken.contains(&fold_name(OsStr::new(n), rule)),
                &name.to_string_lossy(),
            );
            taken.insert(fold_name(OsStr::new(&name), rule));
            let dst = child_path(folder, OsStr::new(&name), rule)?;
            written.push(dst.clone());
            copy_file_bytes(
                &FileCopy {
                    src_provider: sp.as_ref(),
                    src: path,
                    dst_provider: dp.as_ref(),
                    dst: &dst,
                    options: WriteOptions::exclusive(),
                    atomic: false,
                    same_provider: false,
                    server_copy: false,
                    verify: None,
                    durable: false,
                    chunk: CHUNK_BYTES,
                    size_hint: entry.size.unwrap_or(0),
                    throttle: None,
                    offset: 0,
                    resumable: false,
                    keep_on_cancel: false,
                },
                &mut buf,
                &mut |_| {},
                cancel,
            )?;
            out.push(dst.to_location());
        }
        Ok(())
    })();
    match result {
        Ok(()) => Ok(out),
        Err(error) => {
            for path in written {
                let _ = dp.remove_file(&path);
            }
            Err(error)
        }
    }
}
