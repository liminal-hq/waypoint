// Defines serialisable models for window manager IPC payloads
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The windowing system the app is running under.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub enum Session {
    Wayland,
    X11,
    Windows,
    Macos,
    Unknown,
}

/// Window manager features that work on this system.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct WindowCapabilities {
    pub session: Session,
    /// Whether the app can keep a window above others itself. False on Wayland, where only the compositor's own window menu can do it.
    pub always_on_top: bool,
    /// Whether `show_system_window_menu` can ask the compositor for its window menu.
    pub system_window_menu: bool,
}

/// A point in CSS pixels relative to the window's top-left corner; the webview fills the window.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct WindowPosition {
    pub x: f64,
    pub y: f64,
}

/// Whether the plugin can do anything on this system, and which features work.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct PluginStatus {
    pub available: bool,
    pub reason: Option<String>,
    pub features: Vec<String>,
}

impl WindowCapabilities {
    pub fn new(session: Session, always_on_top: bool, system_window_menu: bool) -> Self {
        Self {
            session,
            always_on_top,
            system_window_menu,
        }
    }

    /// No window-manager integration exists on this target (mobile and any other platform without
    /// a module of its own), so nothing is offered: the plugin then reports itself unavailable.
    pub fn unsupported() -> Self {
        Self::new(Session::Unknown, false, false)
    }

    /// Summarises the capabilities as a status: available when any feature works.
    pub fn status(&self) -> PluginStatus {
        let mut features = Vec::new();
        if self.always_on_top {
            features.push("always-on-top".to_string());
        }
        if self.system_window_menu {
            features.push("system-window-menu".to_string());
        }
        let available = !features.is_empty();
        PluginStatus {
            available,
            reason: (!available)
                .then(|| "no window manager features are supported on this system".to_string()),
            features,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capabilities_serialise_as_camel_case() {
        let json =
            serde_json::to_value(WindowCapabilities::new(Session::Wayland, false, true)).unwrap();
        assert_eq!(
            json,
            serde_json::json!({ "session": "wayland", "alwaysOnTop": false, "systemWindowMenu": true })
        );
    }

    #[test]
    fn status_lists_working_features() {
        let status = WindowCapabilities::new(Session::Wayland, false, true).status();
        assert!(status.available);
        assert_eq!(status.features, vec!["system-window-menu"]);
        assert_eq!(status.reason, None);

        let none = WindowCapabilities::new(Session::Unknown, false, false).status();
        assert!(!none.available);
        assert!(none.reason.is_some());
    }

    #[test]
    fn an_unsupported_target_offers_nothing_and_reports_unavailable() {
        let capabilities = WindowCapabilities::unsupported();
        assert!(!capabilities.always_on_top);
        assert!(!capabilities.system_window_menu);
        let status = capabilities.status();
        assert!(!status.available);
        assert!(status.features.is_empty());
        assert!(status.reason.is_some());
    }
}
