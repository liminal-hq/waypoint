// Listing a folder with a window of `readdir` requests in flight, and turning SFTP attributes into
// entries, filling in what a server leaves out.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::ffi::OsString;
use std::time::{Duration, Instant};

use futures::stream::{FuturesOrdered, StreamExt};
use russh_sftp::client::error::Error as SftpError;
use russh_sftp::protocol::FileAttributes;
use waypoint_protocol::{Location, VfsError};
use waypoint_vfs::{group_for_scan, CancelToken, EntryKind, ScannedEntry};

use crate::errors::{from_sftp, is_eof, is_failure, is_not_found};
use crate::paths::child;
use crate::session::Session;

/// Entries handed over at once after the first reply (A18's batches are 2,000 to 5,000).
const BATCH: usize = 2_000;
/// The longest a batch waits for more entries before it is handed over anyway.
const BATCH_WAIT: Duration = Duration::from_millis(100);
/// Symlink targets and missing attributes looked up at once.
const LOOKUPS_IN_FLIGHT: usize = 16;
/// Refusals of pipelined requests before a listing gives up.
const MAX_REFUSALS: usize = 4;

const S_IFMT: u32 = 0o170_000;

/// The kind the mode bits say, if the server sent them.
fn kind_from_mode(mode: u32) -> EntryKind {
    match mode & S_IFMT {
        0o040_000 => EntryKind::Directory,
        0o120_000 => EntryKind::Symlink,
        0o100_000 => EntryKind::File,
        _ => EntryKind::Other,
    }
}

/// The kind an `ls -l` style long name starts with, for a server that sends no mode.
fn kind_from_longname(longname: &str) -> Option<EntryKind> {
    match longname.chars().next()? {
        'd' => Some(EntryKind::Directory),
        'l' => Some(EntryKind::Symlink),
        '-' => Some(EntryKind::File),
        'b' | 'c' | 'p' | 's' => Some(EntryKind::Other),
        _ => None,
    }
}

fn kind_of(attrs: &FileAttributes, longname: Option<&str>) -> Option<EntryKind> {
    attrs
        .permissions
        .map(kind_from_mode)
        .or_else(|| longname.and_then(kind_from_longname))
}

/// An entry from its name and attributes. `kind` is what is known about it; a symlink's target
/// is filled in by `resolve`.
pub(crate) fn entry(name: &str, kind: EntryKind, attrs: &FileAttributes) -> ScannedEntry {
    let executable = attrs.permissions.is_some_and(|mode| mode & 0o111 != 0);
    let link_pending = kind == EntryKind::Symlink;
    ScannedEntry {
        name: OsString::from(name),
        kind,
        link_target: None,
        link_pending,
        group: group_for_scan(name.as_bytes(), kind, None, link_pending, executable),
        special: None,
        size: attrs.size.filter(|_| kind == EntryKind::File),
        modified_ms: attrs.mtime.map(|secs| i64::from(secs) * 1000),
        hidden: name.starts_with('.'),
        trashed: None,
    }
}

/// Fills in a symlink's target from the attributes `stat` (which follows links) returned, or marks
/// it broken when there were none.
pub(crate) fn resolved(mut link: ScannedEntry, target: Option<&FileAttributes>) -> ScannedEntry {
    link.link_pending = false;
    link.link_target = target.map(|attrs| kind_of(attrs, None).unwrap_or(EntryKind::Other));
    if let Some(attrs) = target {
        if link.link_target == Some(EntryKind::File) {
            link.size = attrs.size;
        }
        if let Some(secs) = attrs.mtime {
            link.modified_ms = Some(i64::from(secs) * 1000);
        }
    }
    let executable = target
        .and_then(|attrs| attrs.permissions)
        .is_some_and(|mode| mode & 0o111 != 0);
    let name = link.name.to_string_lossy().into_owned();
    link.group = group_for_scan(
        name.as_bytes(),
        link.kind,
        link.link_target,
        false,
        executable,
    );
    link
}

/// Looks up what a symlink points at. A dangling link comes back broken; one that is gone is
/// `NotFound`.
pub(crate) async fn resolve(
    session: &Session,
    path: &str,
    link: ScannedEntry,
    location: &Location,
) -> Result<ScannedEntry, VfsError> {
    match session.sftp.stat(path).await {
        Ok(attrs) => Ok(resolved(link, Some(&attrs.attrs))),
        Err(error) if is_not_found(&error) => match session.sftp.lstat(path).await {
            Ok(_) => Ok(resolved(link, None)),
            Err(error) => Err(from_sftp(&error, location)),
        },
        // A target the server will not show (permissions, a loop) reads as broken.
        Err(_) => Ok(resolved(link, None)),
    }
}

/// Describes one path without following it, resolving a symlink's target.
pub(crate) async fn stat(
    session: &Session,
    path: &str,
    name: &str,
    location: &Location,
) -> Result<ScannedEntry, VfsError> {
    let attrs = session
        .sftp
        .lstat(path)
        .await
        .map_err(|error| from_sftp(&error, location))?
        .attrs;
    let kind = kind_of(&attrs, None).unwrap_or(EntryKind::Other);
    let found = entry(name, kind, &attrs);
    if kind == EntryKind::Symlink {
        return resolve(session, path, found, location).await;
    }
    Ok(found)
}

/// Lists `dir` into `sink` in batches. Up to `link_budget` symlinks are resolved on the way; the
/// rest come back pending.
pub(crate) async fn list(
    session: &Session,
    dir: &str,
    location: &Location,
    cancel: &CancelToken,
    link_budget: usize,
    sink: &mut dyn FnMut(Vec<ScannedEntry>),
) -> Result<(), VfsError> {
    let handle = match session.sftp.opendir(dir).await {
        Ok(handle) => handle.handle,
        Err(error) => return Err(why_not_listed(session, dir, &error, location).await),
    };
    let result = tokio::select! {
        result = read(session, &handle, dir, location, link_budget, sink) => result,
        () = cancelled(cancel) => Err(VfsError::Cancelled),
    };
    // Close in any case, so a cancelled listing does not leave a handle open on the server.
    let _ = tokio::time::timeout(Duration::from_secs(5), session.sftp.close(handle)).await;
    result
}

/// Resolves when `cancel` is set.
pub(crate) async fn cancelled(cancel: &CancelToken) {
    while !cancel.is_cancelled() {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// An `opendir` failure in the terms of the contract. SFTP version 3 has no "not a folder":
/// OpenSSH reports one as "no such file" and other servers as a generic failure, so the path is
/// looked at to tell them apart.
async fn why_not_listed(
    session: &Session,
    dir: &str,
    error: &SftpError,
    location: &Location,
) -> VfsError {
    if is_failure(error) || is_not_found(error) {
        if let Ok(attrs) = session.sftp.stat(dir).await {
            if kind_of(&attrs.attrs, None) != Some(EntryKind::Directory) {
                return VfsError::NotADirectory {
                    location: location.clone(),
                };
            }
        }
    }
    from_sftp(error, location)
}

async fn read(
    session: &Session,
    handle: &str,
    dir: &str,
    location: &Location,
    mut link_budget: usize,
    sink: &mut dyn FnMut(Vec<ScannedEntry>),
) -> Result<(), VfsError> {
    let sftp = &session.sftp;
    let mut window = session.options.listing_requests.max(1);
    let mut in_flight = FuturesOrdered::new();
    let mut eof = false;
    let mut refusals = 0;
    let mut batch: Vec<ScannedEntry> = Vec::new();
    let mut handed_over = false;
    let mut last_hand_over = Instant::now();
    while !eof && in_flight.len() < window {
        in_flight.push_back(sftp.readdir(handle.to_owned()));
    }
    while let Some(reply) = in_flight.next().await {
        match reply {
            Ok(name) => {
                refusals = 0;
                let entries =
                    complete(session, dir, name.files, &mut link_budget, location).await?;
                batch.extend(entries);
            }
            Err(error) if is_eof(&error) => eof = true,
            // A server that limits the requests in flight refuses the extra ones: carry on one
            // at a time. The cursor only moves on a request that is answered with names.
            Err(error) if !eof && refusals < MAX_REFUSALS && is_failure(&error) => {
                log::debug!("sftp: readdir refused with {window} in flight; going one at a time");
                refusals += 1;
                window = 1;
            }
            Err(error) => return Err(from_sftp(&error, location)),
        }
        while !eof && in_flight.len() < window {
            in_flight.push_back(sftp.readdir(handle.to_owned()));
        }
        let due = !handed_over || batch.len() >= BATCH || last_hand_over.elapsed() >= BATCH_WAIT;
        if due && !batch.is_empty() {
            sink(std::mem::take(&mut batch));
            handed_over = true;
            last_hand_over = Instant::now();
        }
    }
    if !batch.is_empty() {
        sink(batch);
    }
    Ok(())
}

/// Turns one `readdir` reply into entries: drops `.` and `..`, asks for the attributes a server
/// left out, and resolves symlinks while the budget lasts.
async fn complete(
    session: &Session,
    dir: &str,
    files: Vec<russh_sftp::protocol::File>,
    link_budget: &mut usize,
    location: &Location,
) -> Result<Vec<ScannedEntry>, VfsError> {
    let mut entries = Vec::with_capacity(files.len());
    let mut unknown = Vec::new();
    for file in files {
        if file.filename == "." || file.filename == ".." {
            continue;
        }
        match kind_of(&file.attrs, Some(&file.longname)) {
            Some(kind) => entries.push(entry(&file.filename, kind, &file.attrs)),
            None => unknown.push(file.filename),
        }
    }
    if !unknown.is_empty() {
        let found: Vec<_> = futures::stream::iter(unknown.into_iter().map(|name| async move {
            let path = child(dir, &name);
            stat(session, &path, &name, location).await
        }))
        .buffered(LOOKUPS_IN_FLIGHT)
        .collect()
        .await;
        for result in found {
            match result {
                Ok(found) => entries.push(found),
                // Gone between the listing and the look: leave it out.
                Err(VfsError::NotFound { .. }) => {}
                Err(error) => return Err(error),
            }
        }
    }
    let links: Vec<usize> = entries
        .iter()
        .enumerate()
        .filter(|(_, entry)| entry.link_pending)
        .map(|(n, _)| n)
        .take(*link_budget)
        .collect();
    *link_budget -= links.len();
    if !links.is_empty() {
        let resolved: Vec<_> = futures::stream::iter(links.iter().map(|&n| {
            let link = entries[n].clone();
            async move {
                let name = link.name.to_string_lossy().into_owned();
                resolve(session, &child(dir, &name), link, location).await
            }
        }))
        .buffered(LOOKUPS_IN_FLIGHT)
        .collect()
        .await;
        for (n, result) in links.into_iter().zip(resolved) {
            if let Ok(link) = result {
                entries[n] = link;
            }
        }
    }
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use waypoint_vfs::IconGroup;

    use super::*;

    fn attrs(mode: Option<u32>, size: Option<u64>, mtime: Option<u32>) -> FileAttributes {
        FileAttributes {
            size,
            permissions: mode,
            mtime,
            ..FileAttributes::empty()
        }
    }

    #[test]
    fn kinds_come_from_the_mode_or_else_the_long_name() {
        assert_eq!(
            kind_of(&attrs(Some(0o040_755), None, None), None),
            Some(EntryKind::Directory)
        );
        assert_eq!(
            kind_of(&attrs(Some(0o120_777), None, None), None),
            Some(EntryKind::Symlink)
        );
        assert_eq!(
            kind_of(&attrs(Some(0o100_644), None, None), None),
            Some(EntryKind::File)
        );
        assert_eq!(
            kind_of(&attrs(Some(0o010_644), None, None), None),
            Some(EntryKind::Other)
        );
        let none = attrs(None, None, None);
        assert_eq!(
            kind_of(&none, Some("drwxr-xr-x 2 me me 4096 Oct 4 10:00 src")),
            Some(EntryKind::Directory)
        );
        assert_eq!(
            kind_of(&none, Some("lrwxrwxrwx 1 me me 3 x -> y")),
            Some(EntryKind::Symlink)
        );
        assert_eq!(kind_of(&none, Some("")), None);
        assert_eq!(kind_of(&none, None), None);
    }

    #[test]
    fn a_server_that_omits_size_and_time_gives_entries_without_them() {
        let found = entry(
            "notes.txt",
            EntryKind::File,
            &attrs(Some(0o100_644), None, None),
        );
        assert_eq!(found.size, None);
        assert_eq!(found.modified_ms, None);
        let full = entry(
            ".profile",
            EntryKind::File,
            &attrs(Some(0o100_755), Some(9), Some(2)),
        );
        assert_eq!(full.size, Some(9));
        assert_eq!(full.modified_ms, Some(2000));
        assert!(full.hidden);
        let folder = entry(
            "src",
            EntryKind::Directory,
            &attrs(Some(0o040_755), Some(4096), None),
        );
        assert_eq!(folder.size, None, "a folder has no size");
        assert_eq!(folder.group, IconGroup::Folder);
    }

    #[test]
    fn a_symlink_is_pending_until_resolved_and_then_takes_its_targets_details() {
        let link = entry(
            "docs",
            EntryKind::Symlink,
            &attrs(Some(0o120_777), Some(4), Some(1)),
        );
        assert!(link.link_pending);
        let to_folder = resolved(
            link.clone(),
            Some(&attrs(Some(0o040_755), Some(4096), Some(5))),
        );
        assert_eq!(to_folder.link_target, Some(EntryKind::Directory));
        assert!(!to_folder.link_pending);
        assert_eq!(to_folder.group, IconGroup::Folder);
        assert_eq!(to_folder.size, None);
        let to_file = resolved(
            link.clone(),
            Some(&attrs(Some(0o100_644), Some(70), Some(5))),
        );
        assert_eq!(to_file.size, Some(70));
        assert_eq!(to_file.modified_ms, Some(5000));
        let broken = resolved(link, None);
        assert_eq!(broken.link_target, None);
        assert!(!broken.link_pending);
    }
}
