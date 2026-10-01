// The errors a virtual file system operation reports across the Rust <-> JS boundary.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::Location;

/// Why a listing or location could not be opened or kept up to date. Each variant is something
/// the UI shows as a distinct state (SPEC 5.3b, 5.5), never a blank view.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
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
}
