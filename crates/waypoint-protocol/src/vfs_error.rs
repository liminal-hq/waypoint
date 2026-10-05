// The errors a virtual file system operation reports across the Rust <-> JS boundary.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{AuthPrompt, Certificate, HostKey, HostKeyChange, Location, UnreachableReason};

/// Why a listing or location could not be opened or kept up to date. Each variant is something
/// the UI shows as a distinct state (SPEC 5.3b, 5.5), never a blank view.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
// The trust and login payloads are boxed so the error stays as small as it was before them: it is
// the `Err` of nearly every call.
pub enum VfsError {
    /// The location does not exist (any more).
    NotFound { location: Location },
    /// The location exists but may not be read.
    PermissionDenied { location: Location },
    /// The location is a file, or otherwise not something that can be listed.
    NotADirectory { location: Location },
    /// The text does not parse as a location.
    InvalidLocation { input: String },
    /// The listing handle is unknown or has been closed.
    StaleHandle,
    /// The operation was cancelled, for example by navigating away mid-scan.
    Cancelled,
    /// This provider cannot do what was asked.
    Unsupported { what: String },
    /// The address is of a protocol the build has but the person has not turned on (Settings →
    /// Experimental, D167): no provider is registered for `scheme`, so nothing was tried. Unlike
    /// `Unsupported`, turning the switch on makes the same address work.
    ProtocolOff { scheme: String },
    /// Something already has that name, and the operation was not allowed to replace it.
    AlreadyExists { location: Location },
    /// A folder cannot be removed because it still holds entries.
    NotEmpty { location: Location },
    /// A file was expected and the location is a folder.
    IsADirectory { location: Location },
    /// A rename cannot be atomic because the two places are on different volumes; copy and remove
    /// instead.
    CrossesDevices { from: Location, to: Location },
    /// The volume has no room left (or the user's quota is spent).
    StorageFull { location: Location },
    /// The volume is read-only.
    ReadOnly { location: Location },
    /// Another program holds the file open in a way that forbids this (a sharing violation on
    /// Windows, a busy file on Linux).
    InUse { location: Location },
    /// The name cannot be created on this provider: empty, too long, a separator, `.` or `..`, a
    /// NUL, or (under the Windows rules) a reserved device name or a trailing dot or space.
    InvalidName { name: String, reason: String },
    /// A file cannot be shown as text because it holds binary data (a NUL byte in its first bytes).
    NotText { location: Location },
    /// The connection to the location's server is closed or was lost. Reconnecting may help.
    Disconnected { location: Location },
    /// Nothing answered at the server's address.
    Unreachable {
        location: Location,
        reason: UnreachableReason,
    },
    /// The server stopped answering within the connection's timeout.
    Timeout { location: Location },
    /// The location needs a login, or a passphrase, that Waypoint does not have. `prompt` says
    /// what to ask for; it never holds a secret.
    AuthRequired {
        location: Location,
        prompt: Box<AuthPrompt>,
    },
    /// The server refused the credential it was given.
    AuthFailed { location: Location },
    /// An SSH server whose key is not known yet: the person decides whether to trust it.
    HostKeyUnknown {
        location: Location,
        key: Box<HostKey>,
    },
    /// An SSH server whose key differs from the one recorded for it. It never connects silently:
    /// only the explicit `TrustChangedHostKey` answer, after a warning that shows both
    /// fingerprints, replaces the recorded key (D148).
    HostKeyChanged {
        location: Location,
        change: Box<HostKeyChange>,
    },
    /// A TLS certificate the system does not trust.
    CertificateUntrusted {
        location: Location,
        certificate: Box<Certificate>,
    },
    /// The service asked to slow down. `retry_after_ms` is how long it asked to wait, if it said.
    RateLimited {
        location: Location,
        #[ts(type = "number | null")]
        retry_after_ms: Option<u64>,
    },
    /// The object is in an archive storage class (S3 Glacier Flexible Retrieval, Deep Archive) and
    /// needs a restore before it can be read. Waypoint never starts a restore by itself.
    Archived { location: Location },
    /// The service refused a signed request because this computer's clock and its own differ too
    /// much. `skew_ms` is how far this clock is ahead (positive) or behind (negative) the service's,
    /// when the service said.
    ClockSkew {
        location: Location,
        #[ts(type = "number | null")]
        skew_ms: Option<i64>,
    },
    /// The file cannot be read as what it should be: a damaged archive or repository.
    Corrupt { location: Location },
    /// Any other I/O failure, with the operating system's message.
    Io {
        message: String,
        location: Option<Location>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn carries_a_kind_tag() {
        let json = serde_json::to_string(&VfsError::StaleHandle).unwrap();
        assert_eq!(json, r#"{"kind":"staleHandle"}"#);
        let json = serde_json::to_string(&VfsError::NotFound {
            location: Location::new("/x", "file:///x"),
        })
        .unwrap();
        assert!(json.starts_with(r#"{"kind":"notFound","location":"#));
    }

    #[test]
    fn connection_errors_name_their_fields_in_camel_case() {
        let location = Location::new("sftp://h/", "sftp://h/");
        let json = serde_json::to_value(VfsError::RateLimited {
            location: location.clone(),
            retry_after_ms: Some(1500),
        })
        .unwrap();
        assert_eq!(json["kind"], "rateLimited");
        assert_eq!(json["retryAfterMs"], 1500);
        let json = serde_json::to_value(VfsError::Unreachable {
            location,
            reason: UnreachableReason::Refused,
        })
        .unwrap();
        assert_eq!(json["reason"], "refused");
    }
}
