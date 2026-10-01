// Maps the display server the plugin found to the status it reports
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::models::{DisplayServer, FeatureStatus, Features, PluginStatus};

/// Why modifier keys are not readable during a drag on Wayland.
pub const WAYLAND_MODIFIERS: &str = "a Wayland compositor takes the keyboard during a drag and sends the app no modifier state, so the events report every modifier as released";

fn on() -> FeatureStatus {
    FeatureStatus {
        available: true,
        reason: None,
    }
}

fn off(reason: &str) -> FeatureStatus {
    FeatureStatus {
        available: false,
        reason: Some(reason.to_string()),
    }
}

/// The status for a display server. `unavailable_reason` says why there is none, and is used for every feature then.
pub fn status_for(server: DisplayServer, unavailable_reason: &str) -> PluginStatus {
    if server == DisplayServer::None {
        return PluginStatus {
            available: false,
            reason: Some(unavailable_reason.to_string()),
            display_server: server,
            features: Features {
                inbound: off(unavailable_reason),
                outbound: off(unavailable_reason),
                positions: off(unavailable_reason),
                modifiers: off(unavailable_reason),
                clipboard: off(unavailable_reason),
                self_drop_filter: off(unavailable_reason),
            },
        };
    }
    PluginStatus {
        available: true,
        reason: None,
        display_server: server,
        features: Features {
            inbound: on(),
            outbound: on(),
            positions: on(),
            modifiers: if server == DisplayServer::Wayland {
                off(WAYLAND_MODIFIERS)
            } else {
                on()
            },
            clipboard: on(),
            self_drop_filter: on(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_display_reports_every_feature_unavailable_with_the_reason() {
        let status = status_for(DisplayServer::None, "no display is available");
        assert!(!status.available);
        assert_eq!(status.reason.as_deref(), Some("no display is available"));
        let f = &status.features;
        for feature in [
            &f.inbound,
            &f.outbound,
            &f.positions,
            &f.modifiers,
            &f.clipboard,
            &f.self_drop_filter,
        ] {
            assert!(!feature.available);
            assert_eq!(feature.reason.as_deref(), Some("no display is available"));
        }
    }

    #[test]
    fn a_display_server_reports_every_feature_available() {
        for server in [
            DisplayServer::Wayland,
            DisplayServer::X11,
            DisplayServer::Windows,
        ] {
            let status = status_for(server, "unused");
            assert!(status.available);
            assert_eq!(status.display_server, server);
            assert!(status.features.outbound.available && status.features.clipboard.available);
            assert_eq!(status.features.inbound.reason, None);
        }
    }

    #[test]
    fn the_status_serialises_with_the_documented_names() {
        let json = serde_json::to_value(status_for(DisplayServer::Wayland, "")).unwrap();
        assert_eq!(json["displayServer"], "wayland");
        assert_eq!(json["features"]["self-drop-filter"]["available"], true);
        assert!(json["features"]["inbound"]["reason"].is_null());
    }
}
