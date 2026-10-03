// Defines the serialisable models of the window-effects plugin: the effect request, the insets and the status
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Which effect to put behind a window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub enum EffectKind {
    /// No effect: the same as `clear`.
    None,
    /// The compositor blurs what is behind the window (Wayland and X11 compositors that offer it, and Windows).
    Blur,
    /// The Windows 11 Mica material: the desktop wallpaper, blurred and tinted, behind the window.
    Mica,
    /// The Windows Acrylic material: a stronger blur with noise, which can lag while the window is resized on some builds.
    Acrylic,
}

/// A rectangle in the window's own logical pixels, from its top left.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// What `apply` puts behind a window. How see-through the window is stays the page's own alpha: the effect only shows through where the page is transparent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct Effects {
    pub kind: EffectKind,
    /// The page is dark: picks the dark Mica, and the tint of Blur and Acrylic on Windows.
    #[serde(default)]
    pub dark: bool,
    /// Where on the window to blur (Linux). Absent means the whole window. Windows effects always cover the whole window.
    #[serde(default)]
    pub region: Option<Vec<Rect>>,
}

/// The margin, in logical pixels, a client-side shadow or an invisible resize border takes around the window's visible area. The compositor leaves it out of the window's geometry, so a tiled or snapped window sits flush with its neighbours and the screen edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct Insets {
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
    pub left: i32,
}

/// Which implementation is behind the plugin on this system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub enum Flavour {
    /// A Wayland compositor, through GTK's own connection.
    Wayland,
    /// An X11 server, through window properties.
    X11,
    /// Windows, through DWM.
    Windows,
    /// No window effects on this system.
    Unsupported,
}

/// Why a feature is unavailable. A code the front end can branch on; `message` beside it is a sentence for people.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub enum Reason {
    /// The compositor offers apps no way to blur behind their windows (GNOME's Mutter, Cinnamon's Muffin).
    CompositorHasNoBlur,
    /// A Wayland compositor that offers neither blur protocol the plugin speaks.
    NoBlurProtocol,
    /// An X11 desktop whose compositor is not known to honour the blur request.
    UnknownCompositor,
    /// X11 with no compositing manager running, so windows cannot be transparent.
    X11NoCompositor,
    /// Mica needs Windows 11 (build 22000 or later).
    NeedsWindows11,
    /// Acrylic needs Windows 10 version 1809 (build 17763) or later.
    NeedsWindows10,
    /// The effect is a Windows material.
    WindowsOnly,
    /// The shadow inset is a GTK feature.
    GtkOnly,
    /// This operating system has no window effects in the plugin.
    UnsupportedPlatform,
    /// The system would not say what it can do.
    ProbeFailed,
}

pub const FEATURE_OPACITY: &str = "opacity";
pub const FEATURE_BLUR: &str = "blur";
pub const FEATURE_MICA: &str = "mica";
pub const FEATURE_ACRYLIC: &str = "acrylic";
pub const FEATURE_SHADOW_INSET: &str = "shadowInset";

/// Every feature, in the order `get_status` lists them.
pub const FEATURES: [&str; 5] = [
    FEATURE_OPACITY,
    FEATURE_BLUR,
    FEATURE_MICA,
    FEATURE_ACRYLIC,
    FEATURE_SHADOW_INSET,
];

/// Whether one feature of the plugin works on this system.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct FeatureStatus {
    /// One of `opacity`, `blur`, `mica`, `acrylic` or `shadowInset`.
    pub name: String,
    pub available: bool,
    /// Why the feature is unavailable; absent when it works.
    pub reason: Option<Reason>,
    /// A sentence that explains the reason; absent when the feature works.
    pub message: Option<String>,
}

impl FeatureStatus {
    pub fn available(name: &str) -> Self {
        FeatureStatus {
            name: name.to_string(),
            available: true,
            reason: None,
            message: None,
        }
    }

    pub fn unavailable(name: &str, reason: Reason, message: impl Into<String>) -> Self {
        FeatureStatus {
            name: name.to_string(),
            available: false,
            reason: Some(reason),
            message: Some(message.into()),
        }
    }
}

/// What the plugin can do on the running system.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct PluginStatus {
    /// True when at least one feature is available.
    pub available: bool,
    /// Why something this system could do is missing: the first unavailable feature's reason, leaving out features that belong to another platform (Mica on Linux, the shadow inset on Windows). Absent when everything that applies works.
    pub reason: Option<Reason>,
    pub message: Option<String>,
    pub flavour: Flavour,
    pub features: Vec<FeatureStatus>,
}

impl PluginStatus {
    /// Builds a status from the features.
    pub fn build(flavour: Flavour, features: Vec<FeatureStatus>) -> Self {
        let available = features.iter().any(|feature| feature.available);
        let first = features.iter().find(|feature| {
            !feature.available
                && !matches!(feature.reason, Some(Reason::WindowsOnly | Reason::GtkOnly))
        });
        PluginStatus {
            available,
            reason: first.and_then(|feature| feature.reason),
            message: first.and_then(|feature| feature.message.clone()),
            flavour,
            features,
        }
    }

    /// A status in which every feature is unavailable for the same reason.
    pub fn all_unavailable(flavour: Flavour, reason: Reason, message: &str) -> Self {
        PluginStatus::build(
            flavour,
            FEATURES
                .iter()
                .map(|name| FeatureStatus::unavailable(name, reason, message))
                .collect(),
        )
    }

    /// Whether the named feature is available.
    pub fn has(&self, name: &str) -> bool {
        self.feature(name).is_some_and(|feature| feature.available)
    }

    pub fn feature(&self, name: &str) -> Option<&FeatureStatus> {
        self.features.iter().find(|feature| feature.name == name)
    }
}
