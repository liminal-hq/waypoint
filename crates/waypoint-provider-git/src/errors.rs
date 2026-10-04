// How Git failures become the typed errors the rest of Waypoint speaks.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use waypoint_path::VfsPath;
use waypoint_protocol::{Location, VfsError};

pub(crate) fn not_found(path: &VfsPath) -> VfsError {
    VfsError::NotFound {
        location: path.to_location(),
    }
}

pub(crate) fn corrupt(path: &VfsPath, error: &dyn std::fmt::Display) -> VfsError {
    log::warn!("git: {} is unreadable: {error}", path.display());
    VfsError::Corrupt {
        location: path.to_location(),
    }
}

/// A failure that has no better type than a message (a repository that cannot be read for a reason
/// of its own), named at `location` when there is one.
pub(crate) fn io(error: &dyn std::fmt::Display, location: Option<Location>) -> VfsError {
    VfsError::Io {
        message: error.to_string(),
        location,
    }
}

pub(crate) fn unsupported(what: &str) -> VfsError {
    VfsError::Unsupported {
        what: what.to_owned(),
    }
}
