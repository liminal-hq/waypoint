// The availability status every native plugin reports to the app.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// What a native plugin reports through its `get_status` command, so the UI can
/// hide options that do not work on the current system and the Services panel
/// can explain why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PluginStatus {
    /// Whether the plugin's concern works on this system.
    pub available: bool,
    /// A short human-readable explanation when `available` is false
    /// (for example "needs Samba").
    pub reason: Option<String>,
    /// Individually-detected capabilities, by stable identifier.
    pub features: Vec<String>,
}

impl PluginStatus {
    pub fn available(features: Vec<String>) -> Self {
        Self {
            available: true,
            reason: None,
            features,
        }
    }

    pub fn unavailable(reason: impl Into<String>) -> Self {
        Self {
            available: false,
            reason: Some(reason.into()),
            features: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unavailable_carries_its_reason_and_no_features() {
        let status = PluginStatus::unavailable("needs Samba");
        assert!(!status.available);
        assert_eq!(status.reason.as_deref(), Some("needs Samba"));
        assert!(status.features.is_empty());
    }

    #[test]
    fn serialises_with_stable_field_names() {
        let json = serde_json::to_string(&PluginStatus::available(vec!["trash".into()])).unwrap();
        assert_eq!(
            json,
            r#"{"available":true,"reason":null,"features":["trash"]}"#
        );
    }
}
