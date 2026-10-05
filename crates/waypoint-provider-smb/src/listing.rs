// Listing and describing entries: a share's folders, and the share browser at the server's root.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::ffi::OsString;
use std::time::{Duration, UNIX_EPOCH};

use smb2::pack::FileTime;
use waypoint_protocol::{Location, VfsError};
use waypoint_vfs::{group_for_scan, CancelToken, EntryKind, ScannedEntry};

use crate::entries::folder;
use crate::errors::from_smb2;
use crate::paths::Target;
use crate::session::{timed_out, Session};

/// The modification time as milliseconds since the Unix epoch, when the server sent one.
pub(crate) fn millis(time: FileTime) -> Option<i64> {
    if time == FileTime::ZERO {
        return None;
    }
    let time = time.to_system_time()?;
    Some(match time.duration_since(UNIX_EPOCH) {
        Ok(after) => i64::try_from(after.as_millis()).ok()?,
        Err(before) => -i64::try_from(before.duration().as_millis()).ok()?,
    })
}

pub(crate) fn entry(name: &str, is_directory: bool, size: u64, modified: FileTime) -> ScannedEntry {
    let kind = if is_directory {
        EntryKind::Directory
    } else {
        EntryKind::File
    };
    ScannedEntry {
        name: OsString::from(name),
        kind,
        link_target: None,
        link_pending: false,
        group: group_for_scan(name.as_bytes(), kind, None, false, false),
        special: None,
        size: (!is_directory).then_some(size),
        modified_ms: millis(modified),
        hidden: name.starts_with('.'),
        trashed: None,
    }
}

/// Resolves when `cancel` is set.
pub(crate) async fn cancelled(cancel: &CancelToken) {
    while !cancel.is_cancelled() {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// Lists a folder, handing the entries over in batches of at most `batch`. The library reads a
/// whole folder before it returns one (its directory queries are not pipelined), so the first
/// batch waits for the last entry; the batches keep the page's work in steps.
pub(crate) async fn list(
    session: &Session,
    target: &Target,
    location: &Location,
    cancel: &CancelToken,
    sink: &mut dyn FnMut(Vec<ScannedEntry>),
) -> Result<(), VfsError> {
    let batch = session.options.listing_batch.max(1);
    let scanned: Vec<ScannedEntry> = match target {
        Target::Shares => {
            let mut client = session.control().await;
            let shares = tokio::time::timeout(session.options.timeout, client.list_shares())
                .await
                .map_err(|_| timed_out(location))?
                .map_err(|error| from_smb2(&error, location))?;
            shares.iter().map(|share| folder(&share.name)).collect()
        }
        Target::Inside { share, inner } => {
            let (mut client, mut tree) = session.share(share, location).await?;
            let request = client.list_directory(&mut tree, inner);
            let entries = tokio::select! {
                result = tokio::time::timeout(session.options.timeout, request) => result,
                () = cancelled(cancel) => return Err(VfsError::Cancelled),
            }
            .map_err(|_| timed_out(location))?
            .map_err(|error| from_smb2(&error, location))?;
            entries
                .iter()
                .filter(|e| e.name != "." && e.name != "..")
                .map(|e| entry(&e.name, e.is_directory, e.size, e.modified))
                .collect()
        }
    };
    let mut scanned = scanned.into_iter().peekable();
    while scanned.peek().is_some() {
        if cancel.is_cancelled() {
            return Err(VfsError::Cancelled);
        }
        sink(scanned.by_ref().take(batch).collect());
    }
    Ok(())
}

/// Describes one entry. `name` is the last name of the location; the server's root has none.
pub(crate) async fn stat(
    session: &Session,
    target: &Target,
    name: &str,
    location: &Location,
) -> Result<ScannedEntry, VfsError> {
    match target {
        Target::Shares => Ok(folder("")),
        Target::Inside { share, inner } if inner.is_empty() => {
            // A share is a folder as soon as it can be connected.
            drop(session.share(share, location).await?);
            Ok(folder(share))
        }
        Target::Inside { share, inner } => {
            let (mut client, mut tree) = session.share(share, location).await?;
            let info = tokio::time::timeout(session.options.timeout, client.stat(&mut tree, inner))
                .await
                .map_err(|_| timed_out(location))?
                .map_err(|error| from_smb2(&error, location))?;
            Ok(entry(name, info.is_directory, info.size, info.modified))
        }
    }
}
