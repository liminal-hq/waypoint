// Defines the typed errors the window-effects plugin reports for one request on one window
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::models::Reason;

/// Why a request failed. Serialised as `{ "kind": "unsupported", "reason": "compositor-has-no-blur", "message": "…" }`, so the front end can branch on `kind` and `reason`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub enum WindowEffectsError {
    /// This system cannot do that. Nothing was changed.
    #[error("unsupported: {message}")]
    Unsupported { reason: Reason, message: String },
    /// No window has that label.
    #[error("no window is labelled {label}")]
    WindowNotFound { label: String },
    /// The region is empty, has a rectangle with no size, or reaches outside the window.
    #[error("invalid region: {message}")]
    InvalidRegion { message: String },
    /// An inset is negative, or they leave no room in the window.
    #[error("invalid insets: {message}")]
    InvalidInsets { message: String },
    /// The system refused or failed the request.
    #[error("{message}")]
    Failed { message: String },
}

impl WindowEffectsError {
    pub fn failed(message: impl Into<String>) -> Self {
        WindowEffectsError::Failed {
            message: message.into(),
        }
    }
}

pub type Result<T> = std::result::Result<T, WindowEffectsError>;
