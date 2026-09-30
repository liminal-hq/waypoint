// Defines serialisable models for titlebar preference IPC payloads
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use serde::{Deserialize, Serialize};
use ts_rs::TS;

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
    ToggleShade,
    Minimise,
    Lower,
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct PluginStatus {
    pub available: bool,
    pub reason: Option<String>,
    pub features: Vec<String>,
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
            },
        }
    }
}
