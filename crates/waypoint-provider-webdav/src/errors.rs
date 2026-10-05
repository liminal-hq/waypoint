// What HTTP statuses and transport failures mean for the UI: each becomes the typed `VfsError` the
// page shows.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::error::Error as _;
use std::io;
use std::time::{Duration, SystemTime};

use reqwest::StatusCode;
use waypoint_protocol::{Location, UnreachableReason, VfsError};

/// The request whose answer is being read, since one status means different things to each.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Op {
    /// `PROPFIND`, `GET`, `HEAD`: asking.
    Read,
    Mkcol,
    /// `PUT` that replaces a file (`If-Match`).
    Put,
    /// `PUT` that must create a file (`If-None-Match: *`).
    PutNew,
    Delete,
    /// `MOVE` or `COPY`
    Transfer,
}

/// How long a `Retry-After` header (seconds, or an HTTP date) asks to wait.
pub(crate) fn retry_after(value: &str) -> Option<Duration> {
    let value = value.trim();
    if let Ok(seconds) = value.parse::<u64>() {
        return Some(Duration::from_secs(seconds));
    }
    let date = httpdate::parse_http_date(value).ok()?;
    Some(date.duration_since(SystemTime::now()).unwrap_or_default())
}

/// The error for a status that is not a success. A server's own words may echo a user name, so
/// they never enter the error.
pub(crate) fn from_status(
    op: Op,
    status: StatusCode,
    retry: Option<Duration>,
    location: &Location,
) -> VfsError {
    let location = location.clone();
    match status.as_u16() {
        401 => VfsError::AuthFailed { location },
        403 => VfsError::PermissionDenied { location },
        404 | 410 => VfsError::NotFound { location },
        // A collection already there (`MKCOL`), or a method the resource does not take.
        405 if op == Op::Mkcol => VfsError::AlreadyExists { location },
        405 | 501 => VfsError::Unsupported {
            what: format!("{} on this server", method_name(op)),
        },
        // The parent is missing.
        409 => VfsError::NotFound { location },
        // `Overwrite: F` or `If-None-Match: *` found the name taken.
        412 if matches!(op, Op::PutNew | Op::Transfer | Op::Mkcol) => {
            VfsError::AlreadyExists { location }
        }
        412 => VfsError::Io {
            message: "the item changed on the server since it was read".to_owned(),
            location: Some(location),
        },
        423 => VfsError::InUse { location },
        413 => VfsError::Io {
            message: "the server does not accept a file this large".to_owned(),
            location: Some(location),
        },
        429 => VfsError::RateLimited {
            location,
            retry_after_ms: retry.map(millis),
        },
        503 if retry.is_some() => VfsError::RateLimited {
            location,
            retry_after_ms: retry.map(millis),
        },
        504 => VfsError::Timeout { location },
        507 => VfsError::StorageFull { location },
        other => {
            log::debug!("webdav: {other} for {}", location.uri);
            VfsError::Io {
                message: format!("the server answered {}", status_text(status)),
                location: Some(location),
            }
        }
    }
}

fn method_name(op: Op) -> &'static str {
    match op {
        Op::Read => "this request",
        Op::Mkcol => "creating folders",
        Op::Put | Op::PutNew => "uploading",
        Op::Delete => "deleting",
        Op::Transfer => "moving and copying",
    }
}

fn status_text(status: StatusCode) -> String {
    match status.canonical_reason() {
        Some(reason) => format!("{} {reason}", status.as_u16()),
        None => status.as_u16().to_string(),
    }
}

fn millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

/// A failure to complete a request: nothing answered, the answer broke off, or the secure
/// connection could not be made.
pub(crate) fn from_transport(error: &reqwest::Error, location: &Location) -> VfsError {
    let location = location.clone();
    if error.is_timeout() {
        return VfsError::Timeout { location };
    }
    let mut source = error.source();
    let mut text = String::new();
    while let Some(cause) = source {
        if let Some(io) = cause.downcast_ref::<io::Error>() {
            match io.kind() {
                io::ErrorKind::ConnectionRefused => {
                    return VfsError::Unreachable {
                        location,
                        reason: UnreachableReason::Refused,
                    }
                }
                io::ErrorKind::HostUnreachable | io::ErrorKind::NetworkUnreachable => {
                    return VfsError::Unreachable {
                        location,
                        reason: UnreachableReason::NoRoute,
                    }
                }
                io::ErrorKind::NetworkDown => {
                    return VfsError::Unreachable {
                        location,
                        reason: UnreachableReason::Offline,
                    }
                }
                io::ErrorKind::TimedOut => return VfsError::Timeout { location },
                _ => {}
            }
        }
        text.push_str(&cause.to_string());
        text.push(' ');
        source = cause.source();
    }
    let text = text.to_lowercase();
    if error.is_connect() {
        if text.contains("dns error")
            || text.contains("failed to lookup")
            || text.contains("name or service not known")
            || text.contains("no such host")
            || text.contains("nodename nor servname")
            || text.contains("temporary failure in name resolution")
            || text.contains("no address associated")
        {
            return VfsError::Unreachable {
                location,
                reason: UnreachableReason::NameNotResolved,
            };
        }
        if text.contains("refused") {
            return VfsError::Unreachable {
                location,
                reason: UnreachableReason::Refused,
            };
        }
        if text.contains("unreachable") {
            return VfsError::Unreachable {
                location,
                reason: UnreachableReason::NoRoute,
            };
        }
    }
    log::debug!(
        "webdav: transport failed at {}: {error} {text}",
        location.uri
    );
    if error.is_connect() || error.is_request() || error.is_body() || error.is_decode() {
        // The connection was made and then lost, or never completed: reconnecting may help.
        if error.is_connect() {
            return VfsError::Io {
                message: "the connection to the server could not be made".to_owned(),
                location: Some(location),
            };
        }
        return VfsError::Disconnected { location };
    }
    VfsError::Io {
        message: "the request failed".to_owned(),
        location: Some(location),
    }
}

/// Whether the error means the session is gone, so the next call should start afresh.
pub(crate) fn ends_session(error: &VfsError) -> bool {
    matches!(
        error,
        VfsError::Disconnected { .. } | VfsError::Timeout { .. }
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at() -> Location {
        Location::new("dav://h/x", "dav://h/x")
    }

    #[test]
    fn statuses_become_the_states_the_page_shows() {
        let code =
            |op, status: u16| from_status(op, StatusCode::from_u16(status).unwrap(), None, &at());
        assert!(matches!(code(Op::Read, 404), VfsError::NotFound { .. }));
        assert!(matches!(
            code(Op::Read, 403),
            VfsError::PermissionDenied { .. }
        ));
        assert!(matches!(
            code(Op::Mkcol, 405),
            VfsError::AlreadyExists { .. }
        ));
        assert!(matches!(
            code(Op::Delete, 405),
            VfsError::Unsupported { .. }
        ));
        assert!(matches!(code(Op::Mkcol, 409), VfsError::NotFound { .. }));
        assert!(matches!(
            code(Op::Transfer, 412),
            VfsError::AlreadyExists { .. }
        ));
        assert!(matches!(code(Op::Delete, 412), VfsError::Io { .. }));
        assert!(matches!(code(Op::Put, 412), VfsError::Io { .. }));
        assert!(matches!(
            code(Op::PutNew, 412),
            VfsError::AlreadyExists { .. }
        ));
        assert!(matches!(code(Op::Put, 423), VfsError::InUse { .. }));
        assert!(matches!(code(Op::Put, 507), VfsError::StorageFull { .. }));
        assert!(matches!(code(Op::Read, 504), VfsError::Timeout { .. }));
        assert!(matches!(code(Op::Read, 500), VfsError::Io { .. }));
        // Without a Retry-After, a 503 is an ordinary failure; a 429 is always a request to slow down.
        assert!(matches!(code(Op::Read, 503), VfsError::Io { .. }));
        assert_eq!(
            code(Op::Read, 429),
            VfsError::RateLimited {
                location: at(),
                retry_after_ms: None
            }
        );
    }

    #[test]
    fn retry_after_is_read_in_seconds_or_as_a_date() {
        let error = from_status(
            Op::Read,
            StatusCode::SERVICE_UNAVAILABLE,
            Some(Duration::from_secs(7)),
            &at(),
        );
        assert_eq!(
            error,
            VfsError::RateLimited {
                location: at(),
                retry_after_ms: Some(7000)
            }
        );
        assert_eq!(retry_after(" 120 "), Some(Duration::from_secs(120)));
        // A date in the past asks for no wait.
        assert_eq!(
            retry_after("Sun, 06 Nov 1994 08:49:37 GMT"),
            Some(Duration::ZERO)
        );
        assert_eq!(retry_after("soon"), None);
    }

    #[test]
    fn a_servers_words_never_enter_an_error() {
        let error = from_status(Op::Read, StatusCode::INTERNAL_SERVER_ERROR, None, &at());
        assert!(!format!("{error:?}").contains("secret"));
    }
}
