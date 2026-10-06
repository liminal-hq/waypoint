// Defines serialisable models for the OS colour palette, where each colour came from and why one is missing
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::appearance::models::UnavailableReason;

/// Where one palette colour was read from.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub enum PaletteSource {
    /// A named colour of the running GTK theme (`theme_bg_color`, `accent_bg_color`, ...).
    GtkTheme,
    /// The colour sections of KDE's `kdeglobals` file.
    KdeGlobals,
    /// The xdg-desktop-portal's accent colour.
    Portal,
    /// Windows `UISettings.GetColorValue`.
    UiSettings,
    /// Windows `GetSysColor`, the classic colours (the user's own in a high-contrast theme).
    SysColor,
}

/// One colour of the palette.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub enum PaletteColour {
    WindowBackground,
    WindowForeground,
    ViewBackground,
    ViewForeground,
    SurfaceBackground,
    SelectionBackground,
    SelectionForeground,
    Border,
    Focus,
    Warning,
    Error,
    Success,
    /// The top (or only) colour of the title bar.
    TitleBarBackground,
    /// The colour the title bar shades to at its bottom edge; the same as the top for a flat one.
    TitleBarBackgroundEnd,
}

impl PaletteColour {
    /// Every colour, in the order the palette lists them.
    pub const ALL: [PaletteColour; 14] = [
        PaletteColour::WindowBackground,
        PaletteColour::WindowForeground,
        PaletteColour::ViewBackground,
        PaletteColour::ViewForeground,
        PaletteColour::SurfaceBackground,
        PaletteColour::SelectionBackground,
        PaletteColour::SelectionForeground,
        PaletteColour::Border,
        PaletteColour::Focus,
        PaletteColour::Warning,
        PaletteColour::Error,
        PaletteColour::Success,
        PaletteColour::TitleBarBackground,
        PaletteColour::TitleBarBackgroundEnd,
    ];
}

/// One palette colour with the source that supplied it, or the typed reason it is unavailable.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct PaletteEntry {
    /// The colour as lower-case `#rrggbb`; absent when unavailable.
    pub colour: Option<String>,
    /// The source the colour comes from; absent when unavailable.
    pub source: Option<PaletteSource>,
    /// Why the colour is unavailable; absent when it works.
    pub reason: Option<UnavailableReason>,
    /// Human-readable detail for `reason`, such as the names the theme lacks.
    pub detail: Option<String>,
}

impl PaletteEntry {
    pub fn found(colour: String, source: PaletteSource) -> Self {
        Self {
            colour: Some(colour),
            source: Some(source),
            reason: None,
            detail: None,
        }
    }

    pub fn missing(reason: UnavailableReason, detail: impl Into<String>) -> Self {
        Self {
            colour: None,
            source: None,
            reason: Some(reason),
            detail: Some(detail.into()),
        }
    }

    pub fn is_found(&self) -> bool {
        self.colour.is_some()
    }
}

/// Every colour of the palette. An entry nothing could answer has no colour and says why.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct PaletteColours {
    /// The window's own background (a GTK theme's `theme_bg_color`).
    pub window_background: PaletteEntry,
    /// Text on the window background.
    pub window_foreground: PaletteEntry,
    /// The background of content views, lists and text fields (`theme_base_color`).
    pub view_background: PaletteEntry,
    /// Text on a view.
    pub view_foreground: PaletteEntry,
    /// The background of raised surfaces: popovers, cards, buttons.
    pub surface_background: PaletteEntry,
    pub selection_background: PaletteEntry,
    /// Text on the selection.
    pub selection_foreground: PaletteEntry,
    pub border: PaletteEntry,
    /// The focus ring or focus decoration.
    pub focus: PaletteEntry,
    pub warning: PaletteEntry,
    pub error: PaletteEntry,
    pub success: PaletteEntry,
    /// The top of the title bar (a GTK theme's `headerbar_bg_color`, KDE's header background). A
    /// platform or theme with no title bar colour reports a miss, and the title bar stays flat.
    pub title_bar_background: PaletteEntry,
    /// The bottom of the title bar, where the theme shades it in two tones. Only present with a
    /// top colour.
    pub title_bar_background_end: PaletteEntry,
}

impl PaletteColours {
    /// A palette in which every colour is unavailable for the same reason.
    pub fn unavailable(reason: UnavailableReason, detail: &str) -> Self {
        let missing = PaletteEntry::missing(reason, detail);
        Self {
            window_background: missing.clone(),
            window_foreground: missing.clone(),
            view_background: missing.clone(),
            view_foreground: missing.clone(),
            surface_background: missing.clone(),
            selection_background: missing.clone(),
            selection_foreground: missing.clone(),
            border: missing.clone(),
            focus: missing.clone(),
            warning: missing.clone(),
            error: missing.clone(),
            success: missing.clone(),
            title_bar_background: missing.clone(),
            title_bar_background_end: missing,
        }
    }

    pub fn entry(&self, colour: PaletteColour) -> &PaletteEntry {
        match colour {
            PaletteColour::WindowBackground => &self.window_background,
            PaletteColour::WindowForeground => &self.window_foreground,
            PaletteColour::ViewBackground => &self.view_background,
            PaletteColour::ViewForeground => &self.view_foreground,
            PaletteColour::SurfaceBackground => &self.surface_background,
            PaletteColour::SelectionBackground => &self.selection_background,
            PaletteColour::SelectionForeground => &self.selection_foreground,
            PaletteColour::Border => &self.border,
            PaletteColour::Focus => &self.focus,
            PaletteColour::Warning => &self.warning,
            PaletteColour::Error => &self.error,
            PaletteColour::Success => &self.success,
            PaletteColour::TitleBarBackground => &self.title_bar_background,
            PaletteColour::TitleBarBackgroundEnd => &self.title_bar_background_end,
        }
    }

    pub fn entry_mut(&mut self, colour: PaletteColour) -> &mut PaletteEntry {
        match colour {
            PaletteColour::WindowBackground => &mut self.window_background,
            PaletteColour::WindowForeground => &mut self.window_foreground,
            PaletteColour::ViewBackground => &mut self.view_background,
            PaletteColour::ViewForeground => &mut self.view_foreground,
            PaletteColour::SurfaceBackground => &mut self.surface_background,
            PaletteColour::SelectionBackground => &mut self.selection_background,
            PaletteColour::SelectionForeground => &mut self.selection_foreground,
            PaletteColour::Border => &mut self.border,
            PaletteColour::Focus => &mut self.focus,
            PaletteColour::Warning => &mut self.warning,
            PaletteColour::Error => &mut self.error,
            PaletteColour::Success => &mut self.success,
            PaletteColour::TitleBarBackground => &mut self.title_bar_background,
            PaletteColour::TitleBarBackgroundEnd => &mut self.title_bar_background_end,
        }
    }

    /// Sets `colour` from `source`.
    pub fn set(&mut self, colour: PaletteColour, hex: String, source: PaletteSource) {
        *self.entry_mut(colour) = PaletteEntry::found(hex, source);
    }

    /// Fills every colour that has none from `other`, keeping the colours already found.
    pub fn fill_from(&mut self, other: &PaletteColours) {
        for colour in PaletteColour::ALL {
            if !self.entry(colour).is_found() && other.entry(colour).is_found() {
                *self.entry_mut(colour) = other.entry(colour).clone();
            }
        }
    }

    /// Whether the palette can stand in for the window's colours at all: the window background
    /// and the text on it are both known. Without them no mapping is safe.
    pub fn is_usable(&self) -> bool {
        self.window_background.is_found() && self.window_foreground.is_found()
    }

    /// Where the usable palette comes from (the window background's source), and why it is
    /// unavailable when it is not.
    pub fn status(&self) -> PaletteStatus {
        if self.is_usable() {
            PaletteStatus {
                available: true,
                source: self.window_background.source,
                reason: None,
                detail: None,
            }
        } else {
            let cause = if self.window_background.is_found() {
                &self.window_foreground
            } else {
                &self.window_background
            };
            PaletteStatus {
                available: false,
                source: None,
                reason: Some(cause.reason.unwrap_or(UnavailableReason::NoSource)),
                detail: cause.detail.clone(),
            }
        }
    }
}

/// Whether the palette could be read here, and from where.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct PaletteStatus {
    pub available: bool,
    pub source: Option<PaletteSource>,
    pub reason: Option<UnavailableReason>,
    pub detail: Option<String>,
}

impl Default for PaletteStatus {
    fn default() -> Self {
        Self {
            available: false,
            source: None,
            reason: Some(UnavailableReason::NoSource),
            detail: Some("the palette has not been read yet".to_string()),
        }
    }
}

/// What `get_palette` resolves to and the change event carries: the colours plus a revision. The
/// revision starts at 1 and increases whenever a colour or its source changes; the JSON is the
/// colours object with a `revision` field and the `status` added.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct Palette {
    pub revision: u32,
    pub status: PaletteStatus,
    #[serde(flatten)]
    #[ts(flatten)]
    pub colours: PaletteColours,
}
