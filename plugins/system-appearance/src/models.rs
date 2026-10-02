// Defines serialisable models for titlebar preference IPC payloads
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::appearance::models::AppearanceFeatureStatus;

/// A button that can appear in a window titlebar.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub enum WindowButton {
    AppMenu,
    WindowMenu,
    Minimise,
    Maximise,
    Close,
    KeepAbove,
    KeepBelow,
    Shade,
    Stick,
    Help,
}

/// Titlebar buttons on each side of the title, in display order.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct ButtonLayout {
    pub start: Vec<WindowButton>,
    pub end: Vec<WindowButton>,
}

/// An action the desktop runs for a titlebar gesture.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub enum TitlebarAction {
    ToggleMaximise,
    ToggleMaximiseHorizontally,
    ToggleMaximiseVertically,
    ToggleShade,
    Minimise,
    Lower,
    /// Raises a window that is behind others and lowers one that is in front (KWin's "Toggle raise and lower").
    ToggleRaiseLower,
    /// Brings a window in front of others without the lowering half of the toggle (KWin's `Raise`).
    Raise,
    /// Toggles whether the window appears on every desktop (KWin's `OnAllDesktops`).
    ToggleAllDesktops,
    /// Toggles keeping the window above others (xfwm4's `above`).
    ToggleAbove,
    /// Expands the window to fill the free space around it without maximising (xfwm4's `fill`).
    Fill,
    Close,
    Menu,
    None,
}

/// Actions bound to titlebar gestures.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct TitlebarActions {
    pub double_click: TitlebarAction,
    pub middle_click: TitlebarAction,
    pub right_click: TitlebarAction,
}

/// The desktop environment the preferences were read for.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub enum DesktopEnvironment {
    Gnome,
    Kde,
    Cinnamon,
    Mate,
    Xfce,
    Windows,
    Macos,
    Unknown,
}

/// Where the reported preferences came from.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub enum LayoutSource {
    Portal,
    KwinConfig,
    Gsettings,
    Xfconf,
    Platform,
    Default,
}

/// The titlebar preferences of the running desktop.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct TitlebarPreferences {
    pub button_layout: ButtonLayout,
    pub actions: TitlebarActions,
    pub desktop_environment: DesktopEnvironment,
    pub source: LayoutSource,
}

/// Whether the plugin could read the platform's preferences, and how.
///
/// `available` is true when the titlebar preferences or any appearance preference could be read;
/// `reason` says why the titlebar preferences could not. `features` names the titlebar sources
/// that worked (`portal`, `kwin-config`, ...) followed by the appearance features that did
/// (`colourScheme`, ...), and `appearance` reports every appearance feature with its reason when
/// it does not work.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct PluginStatus {
    pub available: bool,
    pub reason: Option<String>,
    pub features: Vec<String>,
    pub appearance: Vec<AppearanceFeatureStatus>,
}

impl PluginStatus {
    /// Adds the availability of the appearance features to a titlebar status.
    pub fn with_appearance(mut self, appearance: Vec<AppearanceFeatureStatus>) -> Self {
        for feature in appearance.iter().filter(|status| status.available) {
            self.features.push(feature.feature.name().to_string());
        }
        self.available = self.available || appearance.iter().any(|status| status.available);
        self.appearance = appearance;
        self
    }
}

impl TitlebarActions {
    /// The conventional actions: double-click toggles maximise, right-click opens the window menu.
    pub const DEFAULT: TitlebarActions = TitlebarActions {
        double_click: TitlebarAction::ToggleMaximise,
        middle_click: TitlebarAction::None,
        right_click: TitlebarAction::Menu,
    };
}

impl TitlebarPreferences {
    /// The layout reported when no source could be read: all buttons at the end, conventional actions.
    pub fn fallback(desktop_environment: DesktopEnvironment) -> Self {
        Self {
            button_layout: ButtonLayout {
                start: Vec::new(),
                end: vec![
                    WindowButton::Minimise,
                    WindowButton::Maximise,
                    WindowButton::Close,
                ],
            },
            actions: TitlebarActions::DEFAULT,
            desktop_environment,
            source: LayoutSource::Default,
        }
    }
}

/// The preferences stamped with a revision, so a consumer can discard a reading that is older
/// than one it already has. The revision starts at 1 and increases whenever the preferences
/// change; the JSON is the preferences object with a `revision` field added.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct TitlebarSnapshot {
    pub revision: u32,
    #[serde(flatten)]
    #[ts(flatten)]
    pub preferences: TitlebarPreferences,
}

/// One reading of the platform: the preferences plus how it went.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub preferences: TitlebarPreferences,
    pub status: PluginStatus,
}

impl Snapshot {
    /// A reading taken from a real source named by `feature`.
    pub fn from_source(preferences: TitlebarPreferences, feature: &str) -> Self {
        Self {
            preferences,
            status: PluginStatus {
                available: true,
                reason: None,
                features: vec![feature.to_string()],
                appearance: Vec::new(),
            },
        }
    }

    /// The default preferences, reporting why no source could be read.
    pub fn unavailable(desktop_environment: DesktopEnvironment, reason: impl Into<String>) -> Self {
        Self {
            preferences: TitlebarPreferences::fallback(desktop_environment),
            status: PluginStatus {
                available: false,
                reason: Some(reason.into()),
                features: Vec::new(),
                appearance: Vec::new(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::appearance::models::{AppearanceFeature, AppearanceSource, UnavailableReason};

    fn feature(feature: AppearanceFeature, available: bool) -> AppearanceFeatureStatus {
        AppearanceFeatureStatus {
            feature,
            available,
            source: available.then_some(AppearanceSource::Portal),
            reason: (!available).then_some(UnavailableReason::NoSource),
            detail: None,
        }
    }

    #[test]
    fn appearance_features_join_the_status_without_touching_the_titlebar_part() {
        let titlebar = Snapshot::from_source(
            TitlebarPreferences::fallback(DesktopEnvironment::Gnome),
            "portal",
        )
        .status;
        let status = titlebar.clone().with_appearance(vec![
            feature(AppearanceFeature::ColourScheme, true),
            feature(AppearanceFeature::Accent, false),
        ]);
        assert!(status.available);
        assert_eq!(status.reason, titlebar.reason);
        assert_eq!(status.features, vec!["portal", "colourScheme"]);
        assert_eq!(status.appearance.len(), 2);
    }

    #[test]
    fn working_appearance_makes_the_plugin_available_when_the_titlebar_is_not() {
        let titlebar = Snapshot::unavailable(DesktopEnvironment::Unknown, "no portal").status;
        let status = titlebar.with_appearance(vec![feature(AppearanceFeature::TextScale, true)]);
        assert!(status.available);
        assert_eq!(status.reason.as_deref(), Some("no portal"));
    }

    #[test]
    fn nothing_available_stays_unavailable() {
        let titlebar = Snapshot::unavailable(DesktopEnvironment::Unknown, "no portal").status;
        let status = titlebar.with_appearance(vec![feature(AppearanceFeature::TextScale, false)]);
        assert!(!status.available);
        assert!(status.features.is_empty());
    }
}
