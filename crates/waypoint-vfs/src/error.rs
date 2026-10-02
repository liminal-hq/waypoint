// Turns operating system errors into the typed errors the UI shows as distinct states.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::fmt;
use std::io::{self, ErrorKind};

use waypoint_protocol::{Location, VfsError};

/// An error a provider injects into a stream (`Read` or `Write` handle), which can only return
/// `io::Error`. `from_io` unwraps it, so the typed error survives the trip through the handle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InjectedError(pub VfsError);

impl fmt::Display for InjectedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

impl std::error::Error for InjectedError {}

impl InjectedError {
    /// Wraps a typed error so a stream can return it.
    pub fn into_io(self) -> io::Error {
        io::Error::other(self)
    }
}

/// What an operating system error means for a write, whatever platform it came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Cause {
    NotFound,
    PermissionDenied,
    NotADirectory,
    IsADirectory,
    AlreadyExists,
    NotEmpty,
    CrossesDevices,
    StorageFull,
    ReadOnly,
    InUse,
    NameTooLong,
    Other,
}

#[cfg(unix)]
fn raw_cause(code: i32) -> Option<Cause> {
    Some(match code {
        libc::ENOENT => Cause::NotFound,
        libc::EACCES | libc::EPERM => Cause::PermissionDenied,
        libc::ENOTDIR => Cause::NotADirectory,
        libc::EISDIR => Cause::IsADirectory,
        libc::EEXIST => Cause::AlreadyExists,
        libc::ENOTEMPTY => Cause::NotEmpty,
        libc::EXDEV => Cause::CrossesDevices,
        libc::ENOSPC | libc::EDQUOT => Cause::StorageFull,
        libc::EROFS => Cause::ReadOnly,
        libc::EBUSY | libc::ETXTBSY => Cause::InUse,
        libc::ENAMETOOLONG => Cause::NameTooLong,
        _ => return None,
    })
}

#[cfg(windows)]
fn raw_cause(code: i32) -> Option<Cause> {
    Some(match code as u32 {
        2 | 3 => Cause::NotFound,            // FILE_NOT_FOUND, PATH_NOT_FOUND
        5 | 1314 => Cause::PermissionDenied, // ACCESS_DENIED, PRIVILEGE_NOT_HELD
        19 => Cause::ReadOnly,               // WRITE_PROTECT
        17 => Cause::CrossesDevices,         // NOT_SAME_DEVICE
        32 | 33 => Cause::InUse,             // SHARING_VIOLATION, LOCK_VIOLATION
        39 | 112 => Cause::StorageFull,      // HANDLE_DISK_FULL, DISK_FULL
        80 | 183 => Cause::AlreadyExists,    // FILE_EXISTS, ALREADY_EXISTS
        145 => Cause::NotEmpty,              // DIR_NOT_EMPTY
        267 => Cause::NotADirectory,         // DIRECTORY
        206 => Cause::NameTooLong,           // FILENAME_EXCED_RANGE
        _ => return None,
    })
}

#[cfg(not(any(unix, windows)))]
fn raw_cause(_code: i32) -> Option<Cause> {
    None
}

/// Classifies an error: the raw operating system code first, because it is the most exact, then
/// the portable `ErrorKind`.
pub(crate) fn cause(error: &io::Error) -> Cause {
    if let Some(cause) = error.raw_os_error().and_then(raw_cause) {
        return cause;
    }
    match error.kind() {
        ErrorKind::NotFound => Cause::NotFound,
        ErrorKind::PermissionDenied => Cause::PermissionDenied,
        ErrorKind::NotADirectory => Cause::NotADirectory,
        ErrorKind::IsADirectory => Cause::IsADirectory,
        ErrorKind::AlreadyExists => Cause::AlreadyExists,
        ErrorKind::DirectoryNotEmpty => Cause::NotEmpty,
        ErrorKind::CrossesDevices => Cause::CrossesDevices,
        ErrorKind::StorageFull | ErrorKind::QuotaExceeded => Cause::StorageFull,
        ErrorKind::ReadOnlyFilesystem => Cause::ReadOnly,
        ErrorKind::ResourceBusy | ErrorKind::ExecutableFileBusy => Cause::InUse,
        ErrorKind::InvalidFilename => Cause::NameTooLong,
        _ => Cause::Other,
    }
}

/// Maps an I/O error on `location` to a `VfsError`. Anything without its own state keeps the
/// operating system's message. An error a provider injected (`InjectedError`) comes back as it was.
pub fn from_io(error: &io::Error, location: &Location) -> VfsError {
    from_io_pair(error, location, location)
}

/// Like `from_io`, for an operation that moves `from` to `to` (a rename): a cross-volume failure
/// names both ends.
pub fn from_io_pair(error: &io::Error, from: &Location, to: &Location) -> VfsError {
    if let Some(injected) = error
        .get_ref()
        .and_then(|inner| inner.downcast_ref::<InjectedError>())
    {
        return injected.0.clone();
    }
    let location = from.clone();
    match cause(error) {
        Cause::NotFound => VfsError::NotFound { location },
        Cause::PermissionDenied => VfsError::PermissionDenied { location },
        Cause::NotADirectory => VfsError::NotADirectory { location },
        Cause::IsADirectory => VfsError::IsADirectory { location },
        Cause::AlreadyExists => VfsError::AlreadyExists {
            location: to.clone(),
        },
        Cause::NotEmpty => VfsError::NotEmpty { location },
        Cause::CrossesDevices => VfsError::CrossesDevices {
            from: from.clone(),
            to: to.clone(),
        },
        Cause::StorageFull => VfsError::StorageFull { location },
        Cause::ReadOnly => VfsError::ReadOnly { location },
        Cause::InUse => VfsError::InUse { location },
        Cause::NameTooLong => VfsError::InvalidName {
            name: location.display.clone(),
            reason: "the name is too long".to_owned(),
        },
        Cause::Other => VfsError::Io {
            message: error.to_string(),
            location: Some(location),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn here() -> Location {
        Location::new("/x", "file:///x")
    }

    #[test]
    fn maps_the_kinds_that_have_their_own_state() {
        for (kind, expected) in [
            (ErrorKind::NotFound, VfsError::NotFound { location: here() }),
            (
                ErrorKind::PermissionDenied,
                VfsError::PermissionDenied { location: here() },
            ),
            (
                ErrorKind::NotADirectory,
                VfsError::NotADirectory { location: here() },
            ),
        ] {
            assert_eq!(from_io(&io::Error::from(kind), &here()), expected);
        }
    }

    #[test]
    fn keeps_the_message_for_everything_else() {
        let error = from_io(&io::Error::other("the disk is on fire"), &here());
        assert_eq!(
            error,
            VfsError::Io {
                message: "the disk is on fire".to_owned(),
                location: Some(here())
            }
        );
    }

    fn there() -> Location {
        Location::new("/there", "file:///there")
    }

    /// Each raw operating system error a write can meet, and the typed error it becomes.
    #[cfg(unix)]
    fn raw_table() -> Vec<(i32, Cause)> {
        vec![
            (libc::ENOENT, Cause::NotFound),
            (libc::EACCES, Cause::PermissionDenied),
            (libc::EPERM, Cause::PermissionDenied),
            (libc::ENOTDIR, Cause::NotADirectory),
            (libc::EISDIR, Cause::IsADirectory),
            (libc::EEXIST, Cause::AlreadyExists),
            (libc::ENOTEMPTY, Cause::NotEmpty),
            (libc::EXDEV, Cause::CrossesDevices),
            (libc::ENOSPC, Cause::StorageFull),
            (libc::EDQUOT, Cause::StorageFull),
            (libc::EROFS, Cause::ReadOnly),
            (libc::EBUSY, Cause::InUse),
            (libc::ETXTBSY, Cause::InUse),
            (libc::ENAMETOOLONG, Cause::NameTooLong),
            (libc::EIO, Cause::Other),
        ]
    }

    #[cfg(windows)]
    fn raw_table() -> Vec<(i32, Cause)> {
        vec![
            (2, Cause::NotFound),
            (3, Cause::NotFound),
            (5, Cause::PermissionDenied),
            (1314, Cause::PermissionDenied),
            (19, Cause::ReadOnly),
            (17, Cause::CrossesDevices),
            (32, Cause::InUse),
            (33, Cause::InUse),
            (39, Cause::StorageFull),
            (112, Cause::StorageFull),
            (80, Cause::AlreadyExists),
            (183, Cause::AlreadyExists),
            (145, Cause::NotEmpty),
            (267, Cause::NotADirectory),
            (206, Cause::NameTooLong),
            (1117, Cause::Other),
        ]
    }

    #[test]
    fn raw_operating_system_errors_map_to_their_causes() {
        for (code, expected) in raw_table() {
            assert_eq!(
                cause(&io::Error::from_raw_os_error(code)),
                expected,
                "raw error {code}"
            );
        }
    }

    #[test]
    fn portable_error_kinds_map_without_a_raw_code() {
        for (kind, expected) in [
            (ErrorKind::AlreadyExists, Cause::AlreadyExists),
            (ErrorKind::DirectoryNotEmpty, Cause::NotEmpty),
            (ErrorKind::CrossesDevices, Cause::CrossesDevices),
            (ErrorKind::StorageFull, Cause::StorageFull),
            (ErrorKind::QuotaExceeded, Cause::StorageFull),
            (ErrorKind::ReadOnlyFilesystem, Cause::ReadOnly),
            (ErrorKind::IsADirectory, Cause::IsADirectory),
            (ErrorKind::ResourceBusy, Cause::InUse),
            (ErrorKind::Interrupted, Cause::Other),
        ] {
            assert_eq!(cause(&io::Error::from(kind)), expected, "{kind:?}");
        }
    }

    #[test]
    fn every_cause_becomes_its_own_typed_error() {
        let here = here();
        let error = |c: Cause| {
            let raw = raw_table().into_iter().find(|(_, cause)| *cause == c);
            let error = match raw {
                Some((code, _)) => io::Error::from_raw_os_error(code),
                // Windows has no raw code for a folder where a file was wanted.
                None => {
                    assert_eq!(
                        c,
                        Cause::IsADirectory,
                        "only that cause may lack a raw code"
                    );
                    io::Error::from(ErrorKind::IsADirectory)
                }
            };
            from_io(&error, &here)
        };
        assert_eq!(
            error(Cause::AlreadyExists),
            VfsError::AlreadyExists {
                location: here.clone()
            }
        );
        assert_eq!(
            error(Cause::NotEmpty),
            VfsError::NotEmpty {
                location: here.clone()
            }
        );
        assert_eq!(
            error(Cause::IsADirectory),
            VfsError::IsADirectory {
                location: here.clone()
            },
            "or the platform has no such code"
        );
        assert_eq!(
            error(Cause::StorageFull),
            VfsError::StorageFull {
                location: here.clone()
            }
        );
        assert_eq!(
            error(Cause::ReadOnly),
            VfsError::ReadOnly {
                location: here.clone()
            }
        );
        assert_eq!(
            error(Cause::InUse),
            VfsError::InUse {
                location: here.clone()
            }
        );
        assert_eq!(
            error(Cause::CrossesDevices),
            VfsError::CrossesDevices {
                from: here.clone(),
                to: here.clone()
            }
        );
        assert!(matches!(
            error(Cause::NameTooLong),
            VfsError::InvalidName { .. }
        ));
    }

    #[test]
    fn a_rename_error_names_both_ends() {
        let cross = io::Error::from(ErrorKind::CrossesDevices);
        assert_eq!(
            from_io_pair(&cross, &here(), &there()),
            VfsError::CrossesDevices {
                from: here(),
                to: there()
            }
        );
        // An existing target is reported at the target.
        let exists = io::Error::from(ErrorKind::AlreadyExists);
        assert_eq!(
            from_io_pair(&exists, &here(), &there()),
            VfsError::AlreadyExists { location: there() }
        );
    }

    #[test]
    fn an_injected_error_survives_a_trip_through_io_error() {
        let injected = VfsError::StorageFull { location: here() };
        let through = InjectedError(injected.clone()).into_io();
        assert_eq!(from_io(&through, &there()), injected);
    }
}
