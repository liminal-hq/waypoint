// Defines serialisable models for the appearance preferences, their sources and their availability
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Whether the user prefers a light or a dark look.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub enum ColourScheme {
    Light,
    Dark,
    /// The system did not say, or the preference could not be read.
    #[default]
    NoPreference,
}

/// How much contrast the user asks for.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub enum Contrast {
    /// No request for more contrast, or the preference could not be read.
    #[default]
    Normal,
    /// The user asked for more contrast (a high-contrast theme).
    More,
}

/// Where one appearance preference was read from.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub enum AppearanceSource {
    /// The xdg-desktop-portal Settings interface: the `org.freedesktop.appearance` namespace first, then the desktop's own namespaces the portal passes through.
    Portal,
    /// The `gsettings` tool, for the desktops and keys the portal does not answer for.
    Gsettings,
    /// KDE's `kdeglobals` file.
    KdeGlobals,
    /// Windows `UISettings`.
    UiSettings,
    /// Windows `AccessibilitySettings`.
    AccessibilitySettings,
    /// The Windows registry.
    Registry,
}

/// The preference sources that won, one per preference; absent where nothing could be read.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct AppearanceSources {
    pub colour_scheme: Option<AppearanceSource>,
    pub accent: Option<AppearanceSource>,
    pub contrast: Option<AppearanceSource>,
    pub reduced_motion: Option<AppearanceSource>,
    pub reduced_transparency: Option<AppearanceSource>,
    pub text_scale: Option<AppearanceSource>,
    pub icon_theme: Option<AppearanceSource>,
}

/// The appearance preferences of the running desktop.
///
/// A preference nothing could answer holds a neutral value (`noPreference`, `null`, `normal`,
/// `false`, a text scale of 1); its entry in `sources` is then absent and `get_status` says why.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct AppearanceValues {
    pub colour_scheme: ColourScheme,
    /// The accent colour as lower-case `#rrggbb`; absent when the system has none.
    pub accent: Option<String>,
    pub contrast: Contrast,
    pub reduced_motion: bool,
    pub reduced_transparency: bool,
    /// The text scale as a multiplier, where 1 is the default size.
    pub text_scale: f64,
    /// The name of the icon theme, as the desktop spells it.
    pub icon_theme: Option<String>,
    pub sources: AppearanceSources,
}

impl Default for AppearanceValues {
    fn default() -> Self {
        Self {
            colour_scheme: ColourScheme::NoPreference,
            accent: None,
            contrast: Contrast::Normal,
            reduced_motion: false,
            reduced_transparency: false,
            text_scale: 1.0,
            icon_theme: None,
            sources: AppearanceSources::default(),
        }
    }
}

/// What `get_appearance` resolves to and the change event carries: the preferences plus a
/// revision. The revision starts at 1 and increases whenever a value or a source changes; the
/// JSON is the preferences object with a `revision` field added.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct AppearancePreferences {
    pub revision: u32,
    #[serde(flatten)]
    #[ts(flatten)]
    pub values: AppearanceValues,
}

/// One appearance preference whose availability is reported.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub enum AppearanceFeature {
    ColourScheme,
    Accent,
    Contrast,
    ReducedMotion,
    ReducedTransparency,
    TextScale,
    IconTheme,
}

impl AppearanceFeature {
    /// Every feature, in the order `get_status` lists them.
    pub const ALL: [AppearanceFeature; 7] = [
        AppearanceFeature::ColourScheme,
        AppearanceFeature::Accent,
        AppearanceFeature::Contrast,
        AppearanceFeature::ReducedMotion,
        AppearanceFeature::ReducedTransparency,
        AppearanceFeature::TextScale,
        AppearanceFeature::IconTheme,
    ];

    /// The name the feature has on the wire.
    pub fn name(self) -> &'static str {
        match self {
            AppearanceFeature::ColourScheme => "colourScheme",
            AppearanceFeature::Accent => "accent",
            AppearanceFeature::Contrast => "contrast",
            AppearanceFeature::ReducedMotion => "reducedMotion",
            AppearanceFeature::ReducedTransparency => "reducedTransparency",
            AppearanceFeature::TextScale => "textScale",
            AppearanceFeature::IconTheme => "iconTheme",
        }
    }
}

/// Why an appearance preference could not be read.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub enum UnavailableReason {
    /// The operating system has no such preference, or this plugin does not read it on this platform.
    PlatformUnsupported,
    /// No source on this desktop environment offers the preference.
    NoSource,
    /// The xdg-desktop-portal could not be reached and no other source answers.
    PortalUnavailable,
    /// The command-line tool a source needs (`gsettings`) is not installed.
    ToolMissing,
    /// A source answered but does not hold the preference: an older portal, or a schema or key the desktop does not install.
    SourceMissing,
    /// A source failed, or answered with a value that could not be understood.
    ReadFailed,
}

/// Whether one appearance preference could be read, and from where.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct AppearanceFeatureStatus {
    pub feature: AppearanceFeature,
    pub available: bool,
    /// The source the value comes from; absent when unavailable.
    pub source: Option<AppearanceSource>,
    /// Why the preference is unavailable; absent when it works.
    pub reason: Option<UnavailableReason>,
    /// Human-readable detail for `reason`, such as the failing key or error.
    pub detail: Option<String>,
}
