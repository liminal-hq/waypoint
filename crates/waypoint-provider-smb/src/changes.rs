// The changes the library has no call for (rename that may replace, times, truncating), sent as
// one compound request of open, set information and close; and the calls it has, with their
// errors narrowed to the contract's.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use smb2::client::connection::CompoundOp;
use smb2::msg::close::CloseRequest;
use smb2::msg::create::{CreateDisposition, CreateRequest, ImpersonationLevel, ShareAccess};
use smb2::msg::query_info::InfoType;
use smb2::msg::set_info::SetInfoRequest;
use smb2::pack::FileTime;
use smb2::types::flags::FileAccessMask;
use smb2::types::status::NtStatus;
use smb2::types::{Command, CreditCharge, FileId, OplockLevel};
use smb2::{Error, SmbClient, Tree};
use waypoint_protocol::{Location, VfsError};
use waypoint_vfs::{CancelToken, FileTimes, VolumeSpace};

use crate::errors::from_smb2;
use crate::session::{timed_out, Session};

/// `FileBasicInformation`, the class that holds the times and the attributes.
const FILE_BASIC_INFORMATION: u8 = 4;
/// `FileEndOfFileInformation`, the class that sets a file's length.
const FILE_END_OF_FILE_INFORMATION: u8 = 20;
/// `FileRenameInformation`.
const FILE_RENAME_INFORMATION: u8 = 10;

/// What the compound opens the entry for, and the information it sets on it.
struct Change {
    access: u32,
    class: u8,
    buffer: Vec<u8>,
}

/// Opens `path`, sets one piece of information on it and closes it, in one round trip.
async fn set_info(
    client: &mut SmbClient,
    tree: &Tree,
    path: &str,
    change: Change,
    location: &Location,
) -> Result<(), VfsError> {
    if tree.is_dfs {
        return Err(VfsError::Unsupported {
            what: "changing entries on a DFS share".to_owned(),
        });
    }
    let create = CreateRequest {
        requested_oplock_level: OplockLevel::None,
        impersonation_level: ImpersonationLevel::Impersonation,
        desired_access: FileAccessMask::new(change.access | FileAccessMask::FILE_READ_ATTRIBUTES),
        file_attributes: 0,
        share_access: ShareAccess(
            ShareAccess::FILE_SHARE_READ
                | ShareAccess::FILE_SHARE_WRITE
                | ShareAccess::FILE_SHARE_DELETE,
        ),
        create_disposition: CreateDisposition::FileOpen,
        create_options: 0,
        name: smb2::encode_path(path),
        create_contexts: Vec::new(),
    };
    let set = SetInfoRequest {
        info_type: InfoType::File,
        file_info_class: change.class,
        additional_information: 0,
        file_id: FileId::SENTINEL,
        buffer: change.buffer,
    };
    let close = CloseRequest {
        flags: 0,
        file_id: FileId::SENTINEL,
    };
    let ops = [
        CompoundOp {
            command: Command::Create,
            body: &create,
            tree_id: Some(tree.tree_id),
            credit_charge: CreditCharge(1),
        },
        CompoundOp {
            command: Command::SetInfo,
            body: &set,
            tree_id: Some(tree.tree_id),
            credit_charge: CreditCharge(1),
        },
        CompoundOp {
            command: Command::Close,
            body: &close,
            tree_id: Some(tree.tree_id),
            credit_charge: CreditCharge(1),
        },
    ];
    let frames = client
        .connection()
        .execute_compound(&ops)
        .await
        .map_err(|error| from_smb2(&error, location))?;
    // The first failure decides: a failed open fails the rest of the chain with it.
    let mut statuses = Vec::new();
    for frame in frames {
        statuses.push(
            frame
                .map_err(|error| from_smb2(&error, location))?
                .header
                .status,
        );
    }
    for (status, command) in statuses.iter().zip([Command::Create, Command::SetInfo]) {
        if *status != NtStatus::SUCCESS {
            return Err(from_smb2(
                &Error::Protocol {
                    status: *status,
                    command,
                },
                location,
            ));
        }
    }
    Ok(())
}

/// The wire form of `FileRenameInformation`: whether to replace, and the new name from the share's
/// root with `\` between names.
fn rename_buffer(to: &str, replace: bool) -> Vec<u8> {
    let name: Vec<u16> = smb2::encode_path(to).encode_utf16().collect();
    let mut buffer = Vec::with_capacity(20 + name.len() * 2);
    buffer.push(u8::from(replace));
    buffer.extend_from_slice(&[0u8; 7]);
    buffer.extend_from_slice(&0u64.to_le_bytes());
    buffer.extend_from_slice(&((name.len() * 2) as u32).to_le_bytes());
    for unit in name {
        buffer.extend_from_slice(&unit.to_le_bytes());
    }
    buffer
}

/// Renames within a share. With `overwrite` false the server refuses an existing target in the
/// same step (`AlreadyExists`).
pub(crate) async fn rename(
    session: &Session,
    share: &str,
    from: &str,
    to: &str,
    overwrite: bool,
    location: &Location,
) -> Result<(), VfsError> {
    let (mut client, tree) = session.share(share, location).await?;
    let change = Change {
        access: FileAccessMask::DELETE,
        class: FILE_RENAME_INFORMATION,
        buffer: rename_buffer(to, overwrite),
    };
    bounded(
        session,
        location,
        set_info(&mut client, &tree, from, change, location),
    )
    .await
}

/// The wire form of `FileBasicInformation` with only the given times set (a zero leaves a field as
/// it is).
fn times_buffer(times: FileTimes) -> Vec<u8> {
    let field = |time: Option<std::time::SystemTime>| {
        time.map(FileTime::from_system_time)
            .unwrap_or(FileTime::ZERO)
            .0
    };
    let mut buffer = Vec::with_capacity(40);
    buffer.extend_from_slice(&0u64.to_le_bytes());
    buffer.extend_from_slice(&field(times.accessed).to_le_bytes());
    buffer.extend_from_slice(&field(times.modified).to_le_bytes());
    buffer.extend_from_slice(&0u64.to_le_bytes());
    buffer.extend_from_slice(&0u32.to_le_bytes());
    buffer.extend_from_slice(&0u32.to_le_bytes());
    buffer
}

pub(crate) async fn set_times(
    session: &Session,
    share: &str,
    path: &str,
    times: FileTimes,
    location: &Location,
) -> Result<(), VfsError> {
    let (mut client, tree) = session.share(share, location).await?;
    let change = Change {
        access: FileAccessMask::FILE_WRITE_ATTRIBUTES,
        class: FILE_BASIC_INFORMATION,
        buffer: times_buffer(times),
    };
    bounded(
        session,
        location,
        set_info(&mut client, &tree, path, change, location),
    )
    .await
}

/// Cuts a file to `length` bytes, for a resumed write that must not keep what follows the offset.
pub(crate) async fn truncate(
    session: &Session,
    share: &str,
    path: &str,
    length: u64,
    location: &Location,
) -> Result<(), VfsError> {
    let (mut client, tree) = session.share(share, location).await?;
    let change = Change {
        access: FileAccessMask::FILE_WRITE_DATA,
        class: FILE_END_OF_FILE_INFORMATION,
        buffer: length.to_le_bytes().to_vec(),
    };
    bounded(
        session,
        location,
        set_info(&mut client, &tree, path, change, location),
    )
    .await
}

/// Runs a request, failing with `Timeout` when it takes longer than the connection's timeout.
pub(crate) async fn bounded<T>(
    session: &Session,
    location: &Location,
    request: impl std::future::Future<Output = Result<T, VfsError>>,
) -> Result<T, VfsError> {
    tokio::time::timeout(session.options.timeout, request)
        .await
        .map_err(|_| timed_out(location))?
}

pub(crate) async fn create_dir(
    session: &Session,
    share: &str,
    path: &str,
    location: &Location,
) -> Result<(), VfsError> {
    let (mut client, tree) = session.share(share, location).await?;
    bounded(session, location, async {
        client
            .create_directory(&tree, path)
            .await
            .map_err(|error| from_smb2(&error, location))
    })
    .await
}

pub(crate) async fn remove_file(
    session: &Session,
    share: &str,
    path: &str,
    location: &Location,
) -> Result<(), VfsError> {
    let (mut client, tree) = session.share(share, location).await?;
    bounded(session, location, async {
        client
            .delete_file(&tree, path)
            .await
            .map_err(|error| from_smb2(&error, location))
    })
    .await
}

pub(crate) async fn remove_dir(
    session: &Session,
    share: &str,
    path: &str,
    location: &Location,
) -> Result<(), VfsError> {
    let (mut client, tree) = session.share(share, location).await?;
    bounded(session, location, async {
        client
            .delete_directory(&tree, path)
            .await
            .map_err(|error| from_smb2(&error, location))
    })
    .await
}

/// Creates an empty file, exclusively.
pub(crate) async fn create_file(
    session: &Session,
    share: &str,
    path: &str,
    location: &Location,
) -> Result<(), VfsError> {
    let (mut client, tree) = session.share(share, location).await?;
    bounded(session, location, async {
        let writer = client
            .create_file_writer_exclusive(&tree, path)
            .await
            .map_err(|error| from_smb2(&error, location))?;
        writer
            .finish()
            .await
            .map(|_| ())
            .map_err(|error| from_smb2(&error, location))
    })
    .await
}

/// Copies a file inside a share on the server. `None` when the server cannot (nothing was
/// touched), so the caller streams the bytes instead.
pub(crate) async fn copy_within(
    session: &Session,
    share: &str,
    from: &str,
    to: &str,
    cancel: &CancelToken,
    location: &Location,
) -> Option<Result<u64, VfsError>> {
    if cancel.is_cancelled() {
        return Some(Err(VfsError::Cancelled));
    }
    let (mut client, tree) = match session.share(share, location).await {
        Ok(opened) => opened,
        Err(error) => return Some(Err(error)),
    };
    // The destination is made exclusively first, then filled: the library's copy would truncate a
    // file that is already there.
    match client.create_file_writer_exclusive(&tree, to).await {
        Ok(writer) => {
            if let Err(error) = writer.finish().await {
                return Some(Err(from_smb2(&error, location)));
            }
        }
        Err(error) => return Some(Err(from_smb2(&error, location))),
    }
    let copied = tokio::time::timeout(
        // A copy moves whole files on the server: the timeout is for silence, not for the file.
        session.options.timeout * 20,
        client.server_side_copy_file(&tree, from, to),
    )
    .await;
    let failure = match copied {
        Ok(Ok(bytes)) => return Some(Ok(bytes)),
        Ok(Err(error)) => error,
        Err(_) => Error::Timeout,
    };
    let _ = client.delete_file(&tree, to).await;
    if failure.kind() == smb2::ErrorKind::Unsupported {
        return None;
    }
    Some(Err(from_smb2(&failure, location)))
}

pub(crate) async fn free_space(
    session: &Session,
    share: &str,
    location: &Location,
) -> Option<VolumeSpace> {
    let (mut client, mut tree) = session.share(share, location).await.ok()?;
    let info = tokio::time::timeout(session.options.timeout, client.fs_info(&mut tree))
        .await
        .ok()?
        .ok()?;
    Some(VolumeSpace {
        free_bytes: info.free_bytes,
        total_bytes: info.total_bytes,
    })
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, UNIX_EPOCH};

    use super::*;

    #[test]
    fn a_rename_buffer_carries_the_replace_flag_and_the_new_name() {
        let keep = rename_buffer("a/b c", false);
        assert_eq!(keep[0], 0);
        let replace = rename_buffer("a/b c", true);
        assert_eq!(replace[0], 1);
        let length = u32::from_le_bytes(replace[16..20].try_into().unwrap()) as usize;
        let units: Vec<u16> = replace[20..20 + length]
            .chunks(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect();
        assert_eq!(String::from_utf16(&units).unwrap(), "a\\b c");
    }

    #[test]
    fn a_times_buffer_sets_only_what_it_is_given() {
        let modified = UNIX_EPOCH + Duration::from_secs(1_600_000_000);
        let buffer = times_buffer(FileTimes {
            accessed: None,
            modified: Some(modified),
        });
        assert_eq!(buffer.len(), 40);
        let at = |from: usize| u64::from_le_bytes(buffer[from..from + 8].try_into().unwrap());
        assert_eq!(at(0), 0, "the creation time is left alone");
        assert_eq!(at(8), 0, "the access time is left alone");
        assert_eq!(at(16), FileTime::from_system_time(modified).0);
        assert_eq!(at(24), 0);
    }
}
