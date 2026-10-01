// Defines plugin error types for session command failures
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use thiserror::Error;

use crate::deps::WindowError;

#[derive(Debug, Error)]
pub enum Error {
    #[error("internal error: {0}")]
    Internal(String),
    #[error(transparent)]
    Session(#[from] waypoint_session::SessionError),
    /// The window factory could not make a window; the store change was rolled back.
    #[error("could not create the window: {0}")]
    Window(#[from] WindowError),
}

impl serde::Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}
