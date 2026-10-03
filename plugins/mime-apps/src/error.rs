// Defines the typed errors the mime-apps plugin reports
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Why one call failed. Serialised as `{ "kind": "noHandler", "mime": "image/png" }`, so the front end can branch on `kind`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub enum MimeAppsError {
    /// No application with that id is installed.
    #[error("no such application")]
    AppNotFound,
    /// Nothing is registered to open the type.
    #[error("no application opens {mime}")]
    NoHandler { mime: String },
    /// The text is not a path or a URI the plugin can read.
    #[error("not a valid location: {uri}")]
    InvalidUri { uri: String },
    /// The call was made with nothing to act on.
    #[error("nothing to open")]
    Empty,
    /// The person dismissed the chooser.
    #[error("cancelled")]
    Cancelled,
    /// This system, or this build, cannot do that.
    #[error("not supported")]
    Unsupported,
    /// The application did not start, or the system refused.
    #[error("{message}")]
    Failed { message: String },
}

impl MimeAppsError {
    pub fn failed(message: impl Into<String>) -> Self {
        MimeAppsError::Failed {
            message: message.into(),
        }
    }
}

pub type Result<T> = std::result::Result<T, MimeAppsError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_serialise_with_a_kind_to_branch_on() {
        let json = serde_json::to_value(MimeAppsError::NoHandler {
            mime: "image/png".into(),
        })
        .unwrap();
        assert_eq!(
            json,
            serde_json::json!({ "kind": "noHandler", "mime": "image/png" })
        );
        let json = serde_json::to_value(MimeAppsError::AppNotFound).unwrap();
        assert_eq!(json, serde_json::json!({ "kind": "appNotFound" }));
    }

    #[test]
    fn errors_read_as_sentences() {
        assert_eq!(MimeAppsError::Cancelled.to_string(), "cancelled");
        assert_eq!(MimeAppsError::failed("boom").to_string(), "boom");
    }
}
