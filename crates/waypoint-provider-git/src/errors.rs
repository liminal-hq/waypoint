// How Git failures become the typed errors the rest of Waypoint speaks.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use waypoint_path::VfsPath;
use waypoint_protocol::VfsError;

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

pub(crate) fn unsupported(what: &str) -> VfsError {
    VfsError::Unsupported {
        what: what.to_owned(),
    }
}
