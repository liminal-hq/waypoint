// What SFTP and SSH failures mean for the UI: each becomes the typed `VfsError` the page shows.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::io;

use russh_sftp::client::error::Error as SftpError;
use russh_sftp::protocol::StatusCode;
use waypoint_protocol::{Location, UnreachableReason, VfsError};

/// A request's failure. A server's own message may echo a user name, so it goes to the log at
/// `debug` and never into the error.
pub(crate) fn from_sftp(error: &SftpError, location: &Location) -> VfsError {
    match error {
        SftpError::Status(status) => {
            log::debug!(
                "sftp: {} at {}: {}",
                status.status_code,
                location.uri,
                status.error_message
            );
            from_status(status.status_code, location)
        }
        SftpError::Timeout => VfsError::Timeout {
            location: location.clone(),
        },
        // The channel or the connection under it is gone.
        SftpError::IO(message) | SftpError::UnexpectedBehavior(message) => {
            log::debug!("sftp: transport failed at {}: {message}", location.uri);
            VfsError::Disconnected {
                location: location.clone(),
            }
        }
        SftpError::Limited(message) => VfsError::Io {
            message: format!("the server's limit was reached: {message}"),
            location: Some(location.clone()),
        },
        SftpError::UnexpectedPacket => VfsError::Io {
            message: "the server answered with an unexpected packet".to_owned(),
            location: Some(location.clone()),
        },
    }
}

pub(crate) fn from_status(code: StatusCode, location: &Location) -> VfsError {
    let location = location.clone();
    match code {
        StatusCode::NoSuchFile => VfsError::NotFound { location },
        StatusCode::PermissionDenied => VfsError::PermissionDenied { location },
        StatusCode::NoConnection | StatusCode::ConnectionLost => {
            VfsError::Disconnected { location }
        }
        StatusCode::OpUnsupported => VfsError::Unsupported {
            what: "this request on this server".to_owned(),
        },
        other => VfsError::Io {
            message: format!("the server refused the request ({other})"),
            location: Some(location),
        },
    }
}

/// Whether the failure is the server's generic "it did not work" (SFTP version 3 reports most
/// errors so), which a caller narrows down by looking at the path.
pub(crate) fn is_failure(error: &SftpError) -> bool {
    matches!(error, SftpError::Status(status) if status.status_code == StatusCode::Failure)
}

pub(crate) fn is_eof(error: &SftpError) -> bool {
    matches!(error, SftpError::Status(status) if status.status_code == StatusCode::Eof)
}

pub(crate) fn is_not_found(error: &SftpError) -> bool {
    matches!(error, SftpError::Status(status) if status.status_code == StatusCode::NoSuchFile)
}

/// Whether the error means the session is gone, so the next call should reconnect.
pub(crate) fn ends_session(error: &VfsError) -> bool {
    matches!(
        error,
        VfsError::Disconnected { .. } | VfsError::Timeout { .. }
    )
}

/// A failure to reach the server's address.
pub(crate) fn from_connect_io(error: &io::Error, location: &Location) -> VfsError {
    let location = location.clone();
    let reason = match error.kind() {
        io::ErrorKind::ConnectionRefused => UnreachableReason::Refused,
        io::ErrorKind::HostUnreachable | io::ErrorKind::NetworkUnreachable => {
            UnreachableReason::NoRoute
        }
        io::ErrorKind::NetworkDown => UnreachableReason::Offline,
        io::ErrorKind::TimedOut => return VfsError::Timeout { location },
        _ => {
            return VfsError::Io {
                message: error.to_string(),
                location: Some(location),
            }
        }
    };
    VfsError::Unreachable { location, reason }
}

/// A failure of the SSH session itself (the handshake, a channel, authentication's transport).
pub(crate) fn from_russh(error: &russh::Error, location: &Location) -> VfsError {
    let location = location.clone();
    match error {
        russh::Error::IO(error) => from_connect_io(error, &location),
        russh::Error::ConnectionTimeout
        | russh::Error::KeepaliveTimeout
        | russh::Error::InactivityTimeout
        | russh::Error::Elapsed(_) => VfsError::Timeout { location },
        russh::Error::Disconnect
        | russh::Error::HUP
        | russh::Error::SendError
        | russh::Error::RecvError => VfsError::Disconnected { location },
        russh::Error::NoCommonAlgo { kind, .. } => VfsError::Io {
            message: format!("the server and Waypoint share no {kind:?} algorithm"),
            location: Some(location),
        },
        other => {
            log::debug!("ssh: {other}");
            VfsError::Io {
                message: format!("the SSH connection failed: {other}"),
                location: Some(location),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use russh_sftp::protocol::Status;

    use super::*;

    fn status(code: StatusCode) -> SftpError {
        SftpError::Status(Status {
            id: 1,
            status_code: code,
            error_message: "secret user name".to_owned(),
            language_tag: "en".to_owned(),
        })
    }

    #[test]
    fn statuses_become_typed_errors_without_the_servers_words() {
        let at = Location::new("sftp://h/x", "sftp://h/x");
        assert!(matches!(
            from_sftp(&status(StatusCode::NoSuchFile), &at),
            VfsError::NotFound { .. }
        ));
        assert!(matches!(
            from_sftp(&status(StatusCode::PermissionDenied), &at),
            VfsError::PermissionDenied { .. }
        ));
        assert!(matches!(
            from_sftp(&status(StatusCode::ConnectionLost), &at),
            VfsError::Disconnected { .. }
        ));
        let failure = from_sftp(&status(StatusCode::Failure), &at);
        assert!(!format!("{failure:?}").contains("secret"));
        assert!(matches!(
            from_sftp(&SftpError::Timeout, &at),
            VfsError::Timeout { .. }
        ));
        assert!(matches!(
            from_sftp(&SftpError::UnexpectedBehavior("session closed".into()), &at),
            VfsError::Disconnected { .. }
        ));
        assert!(is_failure(&status(StatusCode::Failure)));
        assert!(is_eof(&status(StatusCode::Eof)));
    }

    #[test]
    fn unreachable_servers_say_why() {
        let at = Location::new("sftp://h/", "sftp://h/");
        let refused = io::Error::from(io::ErrorKind::ConnectionRefused);
        assert_eq!(
            from_connect_io(&refused, &at),
            VfsError::Unreachable {
                location: at.clone(),
                reason: UnreachableReason::Refused
            }
        );
        let down = io::Error::from(io::ErrorKind::HostUnreachable);
        assert!(matches!(
            from_connect_io(&down, &at),
            VfsError::Unreachable {
                reason: UnreachableReason::NoRoute,
                ..
            }
        ));
    }
}
