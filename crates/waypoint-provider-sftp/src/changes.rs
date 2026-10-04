// The write primitives over one session: creating, renaming, removing, times, permissions and
// links, with SFTP version 3's generic failures narrowed into the contract's typed errors.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::time::{SystemTime, UNIX_EPOCH};

use russh_sftp::client::error::Error as SftpError;
use russh_sftp::protocol::{FileAttributes, OpenFlags, Packet, StatusCode};
use waypoint_protocol::{Location, VfsError};
use waypoint_vfs::{FileTimes, Permissions, VolumeSpace};

use crate::errors::{from_sftp, from_status, is_failure};
use crate::session::Session;

/// An SSH `string`: a big-endian length and the bytes.
fn put_string(out: &mut Vec<u8>, text: &str) {
    out.extend_from_slice(&(text.len() as u32).to_be_bytes());
    out.extend_from_slice(text.as_bytes());
}

fn seconds(time: SystemTime) -> u32 {
    time.duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs().min(u64::from(u32::MAX)) as u32)
        .unwrap_or(0)
}

/// The status an extended request answered with.
fn status_of(reply: Result<Packet, SftpError>, location: &Location) -> Result<(), VfsError> {
    match reply {
        Ok(Packet::Status(status)) if status.status_code == StatusCode::Ok => Ok(()),
        Ok(Packet::Status(status)) => {
            log::debug!(
                "sftp: {} at {}: {}",
                status.status_code,
                location.uri,
                status.error_message
            );
            Err(from_status(status.status_code, location))
        }
        Ok(_) => Err(VfsError::Io {
            message: "the server answered with an unexpected packet".to_owned(),
            location: Some(location.clone()),
        }),
        Err(error) => Err(from_sftp(&error, location)),
    }
}

async fn lstat(session: &Session, path: &str) -> Option<FileAttributes> {
    session.sftp.lstat(path).await.ok().map(|attrs| attrs.attrs)
}

/// A failure to create `path`: a generic failure where something already has the name is a clash.
async fn not_created(
    session: &Session,
    path: &str,
    error: &SftpError,
    location: &Location,
) -> VfsError {
    if is_failure(error) && lstat(session, path).await.is_some() {
        return VfsError::AlreadyExists {
            location: location.clone(),
        };
    }
    from_sftp(error, location)
}

pub(crate) async fn create_dir(
    session: &Session,
    path: &str,
    location: &Location,
) -> Result<(), VfsError> {
    match session.sftp.mkdir(path, FileAttributes::empty()).await {
        Ok(_) => Ok(()),
        Err(error) => Err(not_created(session, path, &error, location).await),
    }
}

/// Opens `path` for writing: created exclusively, created or truncated, or (for a resumed write)
/// only opened.
pub(crate) async fn open_for_writing(
    session: &Session,
    path: &str,
    flags: OpenFlags,
    mode: Option<u32>,
    location: &Location,
) -> Result<String, VfsError> {
    let attrs = FileAttributes {
        permissions: mode,
        ..FileAttributes::empty()
    };
    match session.sftp.open(path, flags, attrs).await {
        Ok(handle) => Ok(handle.handle),
        Err(error) => {
            if let Some(found) = lstat(session, path).await {
                if found.is_dir() {
                    return Err(VfsError::IsADirectory {
                        location: location.clone(),
                    });
                }
                if flags.contains(OpenFlags::EXCLUDE) && is_failure(&error) {
                    return Err(VfsError::AlreadyExists {
                        location: location.clone(),
                    });
                }
            }
            Err(from_sftp(&error, location))
        }
    }
}

pub(crate) async fn create_file(
    session: &Session,
    path: &str,
    location: &Location,
) -> Result<(), VfsError> {
    let flags = OpenFlags::WRITE | OpenFlags::CREATE | OpenFlags::EXCLUDE;
    let handle = open_for_writing(session, path, flags, None, location).await?;
    session
        .sftp
        .close(handle)
        .await
        .map(|_| ())
        .map_err(|error| from_sftp(&error, location))
}

/// Cuts an open file to `length`, for a resumed write.
pub(crate) async fn truncate(
    session: &Session,
    handle: &str,
    length: u64,
    location: &Location,
) -> Result<(), VfsError> {
    let attrs = FileAttributes {
        size: Some(length),
        ..FileAttributes::empty()
    };
    session
        .sftp
        .fsetstat(handle.to_owned(), attrs)
        .await
        .map(|_| ())
        .map_err(|error| from_sftp(&error, location))
}

pub(crate) struct Rename<'a> {
    pub from: &'a str,
    pub to: &'a str,
    pub from_location: &'a Location,
    pub to_location: &'a Location,
    pub overwrite: bool,
}

pub(crate) async fn rename(session: &Session, rename: Rename<'_>) -> Result<(), VfsError> {
    let sftp = &session.sftp;
    let clash = || VfsError::AlreadyExists {
        location: rename.to_location.clone(),
    };
    if !rename.overwrite && lstat(session, rename.to).await.is_some() {
        return Err(clash());
    }
    if rename.overwrite && session.extensions.posix_rename {
        let mut data = Vec::new();
        put_string(&mut data, rename.from);
        put_string(&mut data, rename.to);
        let reply = sftp.extended("posix-rename@openssh.com", data).await;
        return status_of(reply, rename.from_location);
    }
    if rename.overwrite {
        // Without an atomic replacing rename, a file in the way is removed first.
        if let Some(target) = lstat(session, rename.to).await {
            if !target.is_dir() {
                sftp.remove(rename.to)
                    .await
                    .map_err(|error| from_sftp(&error, rename.to_location))?;
            }
        }
    }
    match sftp.rename(rename.from, rename.to).await {
        Ok(_) => Ok(()),
        Err(error) => {
            if lstat(session, rename.from).await.is_none() {
                return Err(VfsError::NotFound {
                    location: rename.from_location.clone(),
                });
            }
            if is_failure(&error) && lstat(session, rename.to).await.is_some() {
                return Err(clash());
            }
            Err(from_sftp(&error, rename.from_location))
        }
    }
}

pub(crate) async fn remove_file(
    session: &Session,
    path: &str,
    location: &Location,
) -> Result<(), VfsError> {
    match session.sftp.remove(path).await {
        Ok(_) => Ok(()),
        Err(error) => match lstat(session, path).await {
            Some(found) if found.is_dir() => Err(VfsError::IsADirectory {
                location: location.clone(),
            }),
            _ => Err(from_sftp(&error, location)),
        },
    }
}

pub(crate) async fn remove_dir(
    session: &Session,
    path: &str,
    location: &Location,
) -> Result<(), VfsError> {
    match session.sftp.rmdir(path).await {
        Ok(_) => Ok(()),
        Err(error) => {
            let location = location.clone();
            match lstat(session, path).await {
                None => Err(VfsError::NotFound { location }),
                // A link to a folder is removed with `remove_file`.
                Some(found) if !found.is_dir() => Err(VfsError::NotADirectory { location }),
                // OpenSSH reports `ENOTEMPTY` as a generic failure.
                Some(_) if is_failure(&error) => Err(VfsError::NotEmpty { location }),
                Some(_) => Err(from_sftp(&error, &location)),
            }
        }
    }
}

/// Sets attributes on the entry itself: `lsetstat@openssh.com` where the server has it, otherwise
/// `setstat`, which a symlink would pass on to its target, so a symlink is `Unsupported` then.
async fn set_own_attributes(
    session: &Session,
    path: &str,
    current: &FileAttributes,
    attrs: FileAttributes,
    location: &Location,
) -> Result<(), VfsError> {
    if session.extensions.lsetstat {
        let mut data = Vec::new();
        put_string(&mut data, path);
        let encoded = russh_sftp::ser::to_bytes(&attrs).map_err(|error| VfsError::Io {
            message: format!("the attributes could not be encoded: {error}"),
            location: Some(location.clone()),
        })?;
        data.extend_from_slice(&encoded);
        return status_of(
            session.sftp.extended("lsetstat@openssh.com", data).await,
            location,
        );
    }
    if current.is_symlink() {
        return Err(VfsError::Unsupported {
            what: "changing a symlink itself on this server".to_owned(),
        });
    }
    session
        .sftp
        .setstat(path, attrs)
        .await
        .map(|_| ())
        .map_err(|error| from_sftp(&error, location))
}

pub(crate) async fn set_times(
    session: &Session,
    path: &str,
    times: FileTimes,
    location: &Location,
) -> Result<(), VfsError> {
    let current = session
        .sftp
        .lstat(path)
        .await
        .map_err(|error| from_sftp(&error, location))?
        .attrs;
    // SFTP version 3 sets both times at once.
    let attrs = FileAttributes {
        atime: times
            .accessed
            .map(seconds)
            .or(current.atime)
            .or(current.mtime),
        mtime: times
            .modified
            .map(seconds)
            .or(current.mtime)
            .or(current.atime),
        ..FileAttributes::empty()
    };
    if attrs.atime.is_none() || attrs.mtime.is_none() {
        return Err(VfsError::Unsupported {
            what: "setting one time when the server reports neither".to_owned(),
        });
    }
    set_own_attributes(session, path, &current, attrs, location).await
}

pub(crate) async fn set_permissions(
    session: &Session,
    path: &str,
    permissions: Permissions,
    location: &Location,
) -> Result<(), VfsError> {
    let current = session
        .sftp
        .lstat(path)
        .await
        .map_err(|error| from_sftp(&error, location))?
        .attrs;
    if current.is_symlink() {
        return Err(VfsError::Unsupported {
            what: "permissions on a symlink".to_owned(),
        });
    }
    let mode = match permissions.mode {
        Some(mode) => mode & 0o7777,
        None => {
            let mode = current.permissions.unwrap_or(0o644) & 0o7777;
            if permissions.readonly {
                mode & !0o222
            } else {
                mode | 0o200
            }
        }
    };
    let attrs = FileAttributes {
        permissions: Some(mode),
        ..FileAttributes::empty()
    };
    session
        .sftp
        .setstat(path, attrs)
        .await
        .map(|_| ())
        .map_err(|error| from_sftp(&error, location))
}

pub(crate) async fn symlink(
    session: &Session,
    link: &str,
    target: &str,
    location: &Location,
) -> Result<(), VfsError> {
    // OpenSSH reads the target first, against the draft's order (its `PROTOCOL` file says so).
    let sent = if session.extensions.openssh {
        session.sftp.symlink(target, link).await
    } else {
        session.sftp.symlink(link, target).await
    };
    match sent {
        Ok(_) => Ok(()),
        Err(error) => Err(not_created(session, link, &error, location).await),
    }
}

pub(crate) async fn free_space(session: &Session, path: &str) -> Option<VolumeSpace> {
    if !session.extensions.statvfs {
        return None;
    }
    let space = session.sftp.statvfs(path).await.ok()?;
    let unit = if space.fragment_size > 0 {
        space.fragment_size
    } else {
        space.block_size
    };
    Some(VolumeSpace {
        free_bytes: space.blocks_avail.saturating_mul(unit),
        total_bytes: space.blocks.saturating_mul(unit),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strings_and_times_are_encoded_as_the_protocol_wants() {
        let mut out = Vec::new();
        put_string(&mut out, "/a b");
        assert_eq!(out, [0, 0, 0, 4, b'/', b'a', b' ', b'b']);
        assert_eq!(seconds(UNIX_EPOCH + std::time::Duration::from_secs(7)), 7);
        assert_eq!(seconds(UNIX_EPOCH - std::time::Duration::from_secs(7)), 0);
    }
}
