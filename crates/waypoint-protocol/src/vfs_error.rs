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
