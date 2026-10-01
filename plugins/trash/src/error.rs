// Defines the typed errors the trash plugin reports for one path or one item
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::io;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::models::lossy_path;

/// Why one trash operation failed. Serialised as `{ "kind": "originExists", "path": "…" }`, so the front end can branch on `kind` and prompt (for example to restore somewhere else).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub enum TrashError {
    /// The file, or the trashed item, does not exist (any more).
    #[error("not found")]
    NotFound,
    /// The system refused access to a file or a trash directory.
    #[error("permission denied")]
    PermissionDenied,
    /// There is no usable trash for this file, for example a volume whose trash cannot be created. Nothing was moved; the plugin never copies a file across volumes into the home trash.
    #[error("no usable trash: {reason}")]
    TrashUnavailable { reason: String },
    /// The plugin will not trash this path on purpose: the home folder, a mount point, or something inside the trash.
    #[error("refused: {reason}")]
    Refused { reason: String },
    /// Restoring would overwrite something: the original location (or the chosen one) is taken.
    #[error("{} already exists", path.display())]
    OriginExists {
        #[serde(with = "lossy_path")]
        #[ts(type = "string")]
        path: PathBuf,
    },
    /// Restoring needs a folder that no longer exists.
    #[error("{} does not exist", path.display())]
    OriginMissingParent {
        #[serde(with = "lossy_path")]
        #[ts(type = "string")]
        path: PathBuf,
    },
    /// This platform has no trash support.
    #[error("the trash is not supported on this system")]
    Unsupported,
    /// Anything else the operating system reported.
    #[error("{message}")]
    Io { message: String },
}

impl TrashError {
    /// Maps an operating system error to the closest typed error.
    pub fn from_io(error: &io::Error) -> Self {
        match error.kind() {
            io::ErrorKind::NotFound => TrashError::NotFound,
            io::ErrorKind::PermissionDenied => TrashError::PermissionDenied,
            _ => TrashError::Io {
                message: error.to_string(),
            },
        }
    }

    /// An `Io` error with a message of the plugin's own.
    pub fn io(message: impl Into<String>) -> Self {
        TrashError::Io {
            message: message.into(),
        }
    }
}

impl From<io::Error> for TrashError {
    fn from(error: io::Error) -> Self {
        TrashError::from_io(&error)
    }
}

pub type Result<T> = std::result::Result<T, TrashError>;
