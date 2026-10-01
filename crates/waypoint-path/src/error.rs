// The ways text or a URI can fail to be a path.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use thiserror::Error;

/// Why a string, a URI or a native path is not a usable [`crate::FilePath`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PathError {
    #[error("the path is empty")]
    Empty,
    #[error("the path is not absolute")]
    NotAbsolute,
    #[error("the path contains a NUL character")]
    InteriorNul,
    #[error("the path is not valid on this platform: {0}")]
    Invalid(&'static str),
    #[error("the URI is malformed: {0}")]
    InvalidUri(&'static str),
    #[error("the URI scheme `{0}` is not supported")]
    UnsupportedScheme(String),
    #[error("a `file` URI with the host `{0}` does not name a local path")]
    RemoteHost(String),
    #[error("the path cannot be represented as text on this platform")]
    Unrepresentable,
}
