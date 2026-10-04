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
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
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

/// Whether one switch on the Integrations page can work on this system, and the sentence for why
/// not, which the page shows beside the switch and the Services panel shows as the service's reason.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct Availability {
    pub available: bool,
    /// Why not; `None` when it works.
    pub reason: Option<String>,
}

impl Availability {
    pub fn yes() -> Self {
        Self {
            available: true,
            reason: None,
        }
    }

    pub fn no(reason: impl Into<String>) -> Self {
        Self {
            available: false,
            reason: Some(reason.into()),
        }
    }
}

/// What each integration can do here (D118: each is off until enabled, and only offered when it
/// can work), worked out in Rust from the shared plugins' own statuses (A65, A66).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct IntegrationAvailability {
    pub notifications: Availability,
    /// Buttons on a notification: whether the route notifications take accepts them. The portal
    /// cannot say whether the desktop draws them, so "available" means they are sent.
    pub notification_actions: Availability,
    pub launcher_progress: Availability,
    pub prevent_sleep: Availability,
    /// Owning `org.freedesktop.FileManager1`, which only Linux has.
    pub file_manager_service: Availability,
    pub global_shortcut: Availability,
    /// Keeping the passphrase of an encrypted volume in the keyring: whether a keyring answers
    /// and may be asked. Asked of the `secrets` plugin, not of the portal or the desktop service.
    pub remember_passphrases: Availability,
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

    #[test]
    fn availability_serialises_with_stable_field_names() {
        let json = serde_json::to_string(&Availability::no("no portal")).unwrap();
        assert_eq!(json, r#"{"available":false,"reason":"no portal"}"#);
        assert!(Availability::yes().available);
    }
}
