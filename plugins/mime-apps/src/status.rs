// Works out what the plugin can do on Linux from the sandbox and the display, and reports it with a typed reason for each feature
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![cfg_attr(not(target_os = "linux"), allow(dead_code))]

use crate::models::{
    FeatureStatus, Flavour, PluginStatus, Reason, FEATURE_APP_ICONS, FEATURE_CHOOSER,
    FEATURE_HANDLERS, FEATURE_OPEN_DEFAULT, FEATURE_OPEN_WITH, FEATURE_SET_DEFAULT,
    FEATURE_TYPE_INFO,
};

/// What the Linux status depends on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LinuxEnv {
    /// The process runs in a Flatpak sandbox (`/.flatpak-info` exists).
    pub flatpak: bool,
    /// A display is there to draw icons for, so the icon theme is usable.
    pub display: bool,
}

impl LinuxEnv {
    pub fn detect() -> Self {
        LinuxEnv {
            flatpak: std::path::Path::new("/.flatpak-info").exists(),
            display: std::env::var_os("WAYLAND_DISPLAY").is_some()
                || std::env::var_os("DISPLAY").is_some(),
        }
    }
}

const SANDBOX: &str = "a Flatpak sandbox sees neither the host's applications nor its defaults; the OpenURI portal opens a file in the application the person picks";

pub fn linux_status(env: LinuxEnv) -> PluginStatus {
    if env.flatpak {
        let sandboxed =
            |name: &str| FeatureStatus::unavailable(name, Reason::FlatpakSandbox, SANDBOX);
        return PluginStatus::build(
            Flavour::Portal,
            vec![
                FeatureStatus::available(FEATURE_TYPE_INFO),
                sandboxed(FEATURE_HANDLERS),
                sandboxed(FEATURE_OPEN_WITH),
                FeatureStatus::available(FEATURE_OPEN_DEFAULT),
                sandboxed(FEATURE_SET_DEFAULT),
                FeatureStatus::available(FEATURE_CHOOSER),
                sandboxed(FEATURE_APP_ICONS),
            ],
        );
    }
    PluginStatus::build(
        Flavour::Gio,
        vec![
            FeatureStatus::available(FEATURE_TYPE_INFO),
            FeatureStatus::available(FEATURE_HANDLERS),
            FeatureStatus::available(FEATURE_OPEN_WITH),
            FeatureStatus::available(FEATURE_OPEN_DEFAULT),
            FeatureStatus::available(FEATURE_SET_DEFAULT),
            FeatureStatus::unavailable(
                FEATURE_CHOOSER,
                Reason::NoSystemChooser,
                "this system has no application chooser to call; draw a list from the handlers",
            ),
            if env.display {
                FeatureStatus::available(FEATURE_APP_ICONS)
            } else {
                FeatureStatus::unavailable(
                    FEATURE_APP_ICONS,
                    Reason::NoDisplay,
                    "there is no display to read the icon theme from",
                )
            },
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::FEATURES;

    fn names(status: &PluginStatus) -> Vec<&str> {
        status.features.iter().map(|f| f.name.as_str()).collect()
    }

    #[test]
    fn a_desktop_session_can_do_everything_but_ask_the_system_to_choose() {
        let status = linux_status(LinuxEnv {
            flatpak: false,
            display: true,
        });
        assert_eq!(names(&status), FEATURES);
        assert_eq!(status.flavour, Flavour::Gio);
        assert!(status.available);
        for feature in &status.features {
            assert_eq!(
                feature.available,
                feature.name != FEATURE_CHOOSER,
                "{}",
                feature.name
            );
        }
        let chooser = status
            .features
            .iter()
            .find(|f| f.name == FEATURE_CHOOSER)
            .unwrap();
        assert_eq!(chooser.reason, Some(Reason::NoSystemChooser));
        assert_eq!(status.reason, Some(Reason::NoSystemChooser));
    }

    #[test]
    fn without_a_display_there_are_no_icons() {
        let status = linux_status(LinuxEnv {
            flatpak: false,
            display: false,
        });
        let icons = status
            .features
            .iter()
            .find(|f| f.name == FEATURE_APP_ICONS)
            .unwrap();
        assert_eq!(icons.reason, Some(Reason::NoDisplay));
        assert!(status.has(FEATURE_HANDLERS));
    }

    #[test]
    fn in_a_flatpak_the_portal_opens_and_asks_but_the_rest_is_unavailable_with_the_reason() {
        let status = linux_status(LinuxEnv {
            flatpak: true,
            display: true,
        });
        assert_eq!(status.flavour, Flavour::Portal);
        assert!(status.has(FEATURE_OPEN_DEFAULT) && status.has(FEATURE_CHOOSER));
        assert!(status.has(FEATURE_TYPE_INFO));
        for name in [
            FEATURE_HANDLERS,
            FEATURE_OPEN_WITH,
            FEATURE_SET_DEFAULT,
            FEATURE_APP_ICONS,
        ] {
            let feature = status.features.iter().find(|f| f.name == name).unwrap();
            assert!(!feature.available, "{name}");
            assert_eq!(feature.reason, Some(Reason::FlatpakSandbox), "{name}");
            assert!(feature.message.is_some());
        }
    }
}
