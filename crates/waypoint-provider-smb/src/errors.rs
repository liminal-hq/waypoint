// What the SMB library's failures mean for the UI: each becomes the typed `VfsError` the page shows.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::io;

use smb2::types::status::NtStatus;
use smb2::types::Command;
use smb2::{Error, ErrorKind};
use waypoint_protocol::{Location, UnreachableReason, VfsError};

use crate::failure::SmbFailure;

/// A request's failure. A server's own message may echo a user name, so it goes to the log at
/// `debug` and never into the error.
pub(crate) fn from_smb2(error: &Error, location: &Location) -> VfsError {
    log::debug!("smb: {error} at {}", location.uri);
    let at = location.clone();
    match error {
        Error::Io(error) => return from_connect_io(error, location),
        Error::ConnectFailed { attempts, .. } => {
            return connect_failed(attempts.iter().map(|a| a.error_kind), location)
        }
        _ => {}
    }
    match error.kind() {
        ErrorKind::AuthRequired => SmbFailure::BadCredentials.into_error(location),
        ErrorKind::SigningRequired => {
            SmbFailure::SigningRequired { user: None }.into_error(location)
        }
        ErrorKind::AccessDenied => VfsError::PermissionDenied { location: at },
        ErrorKind::NotFound => VfsError::NotFound { location: at },
        ErrorKind::AlreadyExists => VfsError::AlreadyExists { location: at },
        ErrorKind::SharingViolation => VfsError::InUse { location: at },
        ErrorKind::IsADirectory => VfsError::IsADirectory { location: at },
        ErrorKind::NotADirectory => VfsError::NotADirectory { location: at },
        ErrorKind::DiskFull => VfsError::StorageFull { location: at },
        ErrorKind::ConnectionLost | ErrorKind::SessionExpired => {
            VfsError::Disconnected { location: at }
        }
        ErrorKind::TimedOut => VfsError::Timeout { location: at },
        ErrorKind::Cancelled => VfsError::Cancelled,
        ErrorKind::InvalidName => VfsError::InvalidName {
            name: location_name(location),
            reason: "the server does not accept this name".to_owned(),
        },
        ErrorKind::Unsupported => VfsError::Unsupported {
            what: "this request on this server".to_owned(),
        },
        _ => match error.status() {
            Some(NtStatus::DIRECTORY_NOT_EMPTY) => VfsError::NotEmpty { location: at },
            Some(NtStatus::ACCESS_DENIED) => VfsError::PermissionDenied { location: at },
            Some(status) => VfsError::Io {
                message: format!("the server refused the request ({status})"),
                location: Some(at),
            },
            None => VfsError::Io {
                message: error.to_string(),
                location: Some(at),
            },
        },
    }
}

fn location_name(location: &Location) -> String {
    location
        .uri
        .rsplit('/')
        .next()
        .unwrap_or_default()
        .to_owned()
}

/// A failure while connecting and logging in, told apart from a request's: which login was tried
/// and whether it was a guest session decide what the person is asked.
pub(crate) fn from_login(
    error: &Error,
    user: Option<&str>,
    offered: bool,
    location: &Location,
) -> (VfsError, bool) {
    let user = user.map(str::to_owned);
    match error {
        // The server's answer to the negotiation: nothing it speaks is an SMB 2 dialect.
        Error::Protocol {
            command: Command::Negotiate,
            ..
        } => (SmbFailure::NoCommonDialect.into_error(location), false),
        Error::InvalidData { .. } | Error::Disconnected => {
            (SmbFailure::NoCommonDialect.into_error(location), false)
        }
        // A server that knows the account refuses the session because of a protection it asks
        // for (encryption or signing that this connection could not set up); a wrong password is
        // `STATUS_LOGON_FAILURE`.
        Error::Protocol {
            status: NtStatus::ACCESS_DENIED,
            command: Command::SessionSetup,
        } => {
            if offered {
                (SmbFailure::EncryptionRequired.into_error(location), false)
            } else {
                (
                    SmbFailure::SigningRequired { user }.into_error(location),
                    false,
                )
            }
        }
        _ => match error.kind() {
            ErrorKind::AuthRequired | ErrorKind::AccessDenied => {
                if offered {
                    (SmbFailure::BadCredentials.into_error(location), true)
                } else {
                    (
                        SmbFailure::CredentialsNeeded { user }.into_error(location),
                        false,
                    )
                }
            }
            ErrorKind::SigningRequired => (
                SmbFailure::SigningRequired { user }.into_error(location),
                false,
            ),
            _ => (from_smb2(error, location), false),
        },
    }
}

fn connect_failed(
    kinds: impl Iterator<Item = Option<io::ErrorKind>>,
    location: &Location,
) -> VfsError {
    let kinds: Vec<_> = kinds.collect();
    let at = location.clone();
    if !kinds.is_empty()
        && kinds
            .iter()
            .all(|kind| *kind == Some(io::ErrorKind::ConnectionRefused))
    {
        return VfsError::Unreachable {
            location: at,
            reason: UnreachableReason::Refused,
        };
    }
    if kinds.iter().any(|kind| kind.is_none()) {
        return VfsError::Timeout { location: at };
    }
    if let Some(kind) = kinds.into_iter().flatten().next() {
        return from_connect_io(&io::Error::from(kind), location);
    }
    VfsError::Unreachable {
        location: at,
        reason: UnreachableReason::NoRoute,
    }
}

/// A failure to reach the server's address.
pub(crate) fn from_connect_io(error: &io::Error, location: &Location) -> VfsError {
    let at = location.clone();
    let reason = match error.kind() {
        io::ErrorKind::ConnectionRefused => UnreachableReason::Refused,
        io::ErrorKind::HostUnreachable | io::ErrorKind::NetworkUnreachable => {
            UnreachableReason::NoRoute
        }
        io::ErrorKind::NetworkDown => UnreachableReason::Offline,
        io::ErrorKind::TimedOut => return VfsError::Timeout { location: at },
        io::ErrorKind::ConnectionReset
        | io::ErrorKind::ConnectionAborted
        | io::ErrorKind::BrokenPipe
        | io::ErrorKind::UnexpectedEof => return VfsError::Disconnected { location: at },
        _ => {
            // Name resolution has no stable kind; the text is the only mark of it.
            let text = error.to_string().to_lowercase();
            if text.contains("lookup")
                || text.contains("resolve")
                || text.contains("name or service")
            {
                return VfsError::Unreachable {
                    location: at,
                    reason: UnreachableReason::NameNotResolved,
                };
            }
            return VfsError::Io {
                message: error.to_string(),
                location: Some(at),
            };
        }
    };
    VfsError::Unreachable {
        location: at,
        reason,
    }
}

/// Whether the error means the session is gone, so the next call should reconnect.
pub(crate) fn ends_session(error: &VfsError) -> bool {
    matches!(
        error,
        VfsError::Disconnected { .. } | VfsError::Timeout { .. }
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn protocol(status: NtStatus) -> Error {
        Error::Protocol {
            status,
            command: Command::Create,
        }
    }

    fn kind_of(error: &VfsError) -> String {
        serde_json::to_value(error).unwrap()["kind"]
            .as_str()
            .unwrap()
            .to_owned()
    }

    #[test]
    fn statuses_become_typed_errors_without_the_servers_words() {
        let at = Location::new("smb://h/s/x", "smb://h/s/x");
        let cases = [
            (NtStatus::OBJECT_NAME_NOT_FOUND, "notFound"),
            (NtStatus::BAD_NETWORK_NAME, "notFound"),
            (NtStatus::ACCESS_DENIED, "permissionDenied"),
            (NtStatus::OBJECT_NAME_COLLISION, "alreadyExists"),
            (NtStatus::DIRECTORY_NOT_EMPTY, "notEmpty"),
            (NtStatus::FILE_IS_A_DIRECTORY, "isADirectory"),
            (NtStatus::NOT_A_DIRECTORY, "notADirectory"),
            (NtStatus::DISK_FULL, "storageFull"),
            (NtStatus::SHARING_VIOLATION, "inUse"),
            (NtStatus::NETWORK_NAME_DELETED, "disconnected"),
            (NtStatus::NOT_SUPPORTED, "unsupported"),
        ];
        for (status, kind) in cases {
            assert_eq!(
                kind_of(&from_smb2(&protocol(status), &at)),
                kind,
                "{status}"
            );
        }
        assert_eq!(kind_of(&from_smb2(&Error::Timeout, &at)), "timeout");
        assert_eq!(
            kind_of(&from_smb2(&Error::Disconnected, &at)),
            "disconnected"
        );
    }

    #[test]
    fn a_login_failure_depends_on_whether_a_password_was_offered() {
        let at = Location::new("smb://h/", "smb://h/");
        let refused = protocol(NtStatus::LOGON_FAILURE);
        let (error, rejected) = from_login(&refused, Some("me"), true, &at);
        assert_eq!(kind_of(&error), "authFailed");
        assert!(rejected, "a refused credential is reported as rejected");
        let (error, rejected) = from_login(&refused, Some("me"), false, &at);
        assert_eq!(kind_of(&error), "authRequired");
        assert!(!rejected);
        let (error, _) = from_login(
            &Error::Protocol {
                status: NtStatus::NOT_SUPPORTED,
                command: Command::Negotiate,
            },
            None,
            false,
            &at,
        );
        assert_eq!(kind_of(&error), "unsupported");
    }

    #[test]
    fn unreachable_servers_say_why() {
        let at = Location::new("smb://h/", "smb://h/");
        let refused = io::Error::from(io::ErrorKind::ConnectionRefused);
        assert_eq!(
            from_connect_io(&refused, &at),
            VfsError::Unreachable {
                location: at.clone(),
                reason: UnreachableReason::Refused
            }
        );
        let lookup = io::Error::other("failed to lookup address information");
        assert!(matches!(
            from_connect_io(&lookup, &at),
            VfsError::Unreachable {
                reason: UnreachableReason::NameNotResolved,
                ..
            }
        ));
    }
}
