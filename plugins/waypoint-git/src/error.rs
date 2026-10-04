// What a Git command rejects with
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use thiserror::Error;
use waypoint_protocol::VfsError;

/// A rejection serialises as the tagged `VfsError` object, as the file system plugin's do, so the
/// frontend reads both the same way.
#[derive(Debug, Error)]
pub enum Error {
    #[error("{0:?}")]
    Vfs(VfsError),
    /// A fault in the plugin itself (a background task that panicked).
    #[error("internal error: {0}")]
    Internal(String),
}

impl From<VfsError> for Error {
    fn from(error: VfsError) -> Self {
        Error::Vfs(error)
    }
}

impl serde::Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Error::Vfs(error) => error.serialize(serializer),
            Error::Internal(message) => VfsError::Io {
                message: message.clone(),
                location: None,
            }
            .serialize(serializer),
        }
    }
}
