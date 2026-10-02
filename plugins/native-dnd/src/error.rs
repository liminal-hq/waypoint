// Defines the typed errors native drag and drop commands fail with
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use serde::{Serialize, Serializer};
use thiserror::Error;

use crate::models::{ErrorKind, NativeDndError};

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum Error {
    /// Native drag and drop does not work here, for the reason given.
    #[error("native drag and drop is unavailable: {0}")]
    Unsupported(String),
    /// The primary mouse button is not down, so a drag cannot start.
    #[error("the primary mouse button is not down")]
    ButtonNotPressed,
    /// An outbound drag is already running.
    #[error("an outbound drag is already active")]
    AlreadyActive,
    #[error("invalid request: {0}")]
    Invalid(String),
    #[error("{0}")]
    Failed(String),
}

impl Error {
    pub fn kind(&self) -> ErrorKind {
        match self {
            Error::Unsupported(_) => ErrorKind::Unsupported,
            Error::ButtonNotPressed => ErrorKind::ButtonNotPressed,
            Error::AlreadyActive => ErrorKind::AlreadyActive,
            Error::Invalid(_) => ErrorKind::Invalid,
            Error::Failed(_) => ErrorKind::Failed,
        }
    }
}

impl Serialize for Error {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        NativeDndError {
            kind: self.kind(),
            message: self.to_string(),
        }
        .serialize(serializer)
    }
}

pub type Result<T> = std::result::Result<T, Error>;
