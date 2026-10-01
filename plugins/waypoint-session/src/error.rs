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
    /// Opening one more window would pass `MAX_WINDOWS`; nothing changed.
    #[error("cannot open more than {limit} windows")]
    TooManyWindows { limit: usize },
}

impl Error {
    /// The stable name the frontend tells errors apart by.
    fn kind(&self) -> &'static str {
        match self {
            Error::Internal(_) => "internal",
            Error::Session(_) => "session",
            Error::Window(_) => "window",
            Error::TooManyWindows { .. } => "tooManyWindows",
        }
    }
}

impl serde::Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeMap;
        // `{ kind, message }`, plus `limit` for `tooManyWindows`, so the page can tell the refusal
        // from a failure and say it in its own words.
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("kind", self.kind())?;
        map.serialize_entry("message", &self.to_string())?;
        if let Error::TooManyWindows { limit } = self {
            map.serialize_entry("limit", limit)?;
        }
        map.end()
    }
}
