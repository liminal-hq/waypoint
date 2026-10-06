// The settings document's types and the rules a value must satisfy
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use serde::{Deserialize, Serialize};
use thiserror::Error;
use ts_rs::TS;

use crate::accelerator::validate_accelerator;

/// The shortest spring-load delay: below this a drag passing over a folder would open it.
pub const SPRING_LOAD_MIN_MS: u32 = 200;

/// The longest spring-load delay.
pub const SPRING_LOAD_MAX_MS: u32 = 2000;

/// What a new window starts with (SPEC 11, General).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum StartupMode {
    /// Bring back every window and tab of the last session.
    #[default]
    RestoreSession,
    /// Open one window at Home.
    Home,
}

/// How a new window shows a folder until the person picks another view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum DefaultView {
    #[default]
    List,
    Grid,
}

/// How many clicks open a file or folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum ClickMode {
    Single,
    #[default]
    Double,
}

/// Which action a drop takes when no modifier key says otherwise (SPEC 6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum DropActionRule {
    /// Move within one volume, copy between volumes.
    #[default]
    ByVolume,
    AlwaysCopy,
    /// Open the action picker on every drop.
    AlwaysAsk,
}

/// The General page. Confirming before the Trash is an operations setting (`OpsSettings`) and is
/// shown on this page, not stored here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct GeneralSettings {
    /// Whether a new window lists hidden files.
    pub show_hidden_default: bool,
    pub startup: StartupMode,
    pub default_view: DefaultView,
    pub click_mode: ClickMode,
    /// Whether a folder remembers its own view, sort and grouping (SPEC 5.3b). Off, every folder
    /// shows the window's view; what was remembered is kept for when it is turned on again.
    pub remember_folder_views: bool,
    /// Whether the Git status shows in a working tree: the Git column, the marks on folders, the
    /// branch in the status bar and the Inspector's Git tab (D161). On by default; off, Waypoint
    /// reads nothing from the repository and the Services panel says it is switched off.
    pub git_decorations: bool,
}

impl Default for GeneralSettings {
    fn default() -> Self {
        Self {
            show_hidden_default: false,
            startup: StartupMode::RestoreSession,
            default_view: DefaultView::List,
            click_mode: ClickMode::Double,
            remember_folder_views: true,
            git_decorations: true,
        }
    }
}

/// The Drag & drop page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct DndSettings {
    pub default_action_rule: DropActionRule,
    /// How long a drag hovers over a folder, tab or sidebar item before it opens, in milliseconds.
    pub spring_load_ms: u32,
    /// Whether the Shelf's items are kept for the next start.
    pub shelf_persist: bool,
}

impl Default for DndSettings {
    fn default() -> Self {
        Self {
            default_action_rule: DropActionRule::ByVolume,
            spring_load_ms: 600,
            shelf_persist: true,
        }
    }
}

/// What the window chrome shows (SPEC 5.9). These are view choices of the window, kept here so
/// every window and every restart agree on them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct UiSettings {
    /// Whether the Action bar shows under the toolbar.
    pub action_bar: bool,
    /// Whether the Action bar's buttons carry their labels, or are icons only.
    pub action_bar_labels: bool,
    /// Whether the title bar's label starts with "Waypoint"; off, it is just the window's name.
    pub app_name_in_title: bool,
    /// Whether Main windows show a permanent menu bar under the title bar, carrying the application menu's menus.
    pub menu_bar: bool,
    /// Whether the list shows the Git column in a folder of a working tree (the header menu's
    /// choice; off, the marks on the rows and the branch stay).
    pub git_column: bool,
    /// Whether the list shows the Storage class column in a folder on S3 (the header menu's
    /// choice; off by default, since most folders are not on S3 and most people need the class
    /// only now and then).
    pub storage_class_column: bool,
    /// The widths the list's columns were dragged to, the same in every folder (D176).
    pub column_widths: ListColumnWidths,
}

/// The shortest and longest width a list column may be given, in pixels. The list keeps each
/// column to its own narrower range (so its heading still fits); these bounds only stop a
/// hand-edited document from asking for a column that cannot be seen or one wider than any screen.
pub const COLUMN_WIDTH_MIN: u16 = 32;
pub const COLUMN_WIDTH_MAX: u16 = 1200;

/// The widths, in pixels, a person has dragged the list's columns to. `None` is the column's own
/// width. Name is not here: it is the flexible column and takes what the others leave.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct ListColumnWidths {
    pub size: Option<u16>,
    pub modified: Option<u16>,
    pub kind: Option<u16>,
    pub git: Option<u16>,
    pub storage_class: Option<u16>,
    /// Where an item was trashed from (the Trash's own column).
    pub original: Option<u16>,
    /// When an item was trashed (the Trash's own column).
    pub deleted: Option<u16>,
}

impl ListColumnWidths {
    /// Each column with its setting name, for checks that treat them alike.
    fn named(&mut self) -> [(&'static str, &mut Option<u16>); 7] {
        [
            ("ui.columnWidths.size", &mut self.size),
            ("ui.columnWidths.modified", &mut self.modified),
            ("ui.columnWidths.kind", &mut self.kind),
            ("ui.columnWidths.git", &mut self.git),
            ("ui.columnWidths.storageClass", &mut self.storage_class),
            ("ui.columnWidths.original", &mut self.original),
            ("ui.columnWidths.deleted", &mut self.deleted),
        ]
    }
}

impl Default for UiSettings {
    fn default() -> Self {
        Self {
            action_bar: true,
            action_bar_labels: true,
            app_name_in_title: false,
            menu_bar: false,
            git_column: true,
            storage_class_column: false,
            column_widths: ListColumnWidths::default(),
        }
    }
}

/// The shortest and longest window opacity, in percent (below this text cannot be read).
pub const OPACITY_MIN: u8 = 40;
pub const OPACITY_MAX: u8 = 100;

/// The shortest menu and popup opacity, in percent.
pub const MENU_OPACITY_MIN: u8 = 60;

/// The text sizes the Accessibility page offers, in percent of the normal size.
pub const TEXT_SIZES: [u16; 3] = [100, 115, 130];

/// The largest file a thumbnail or preview is made of, in megabytes.
pub const PREVIEW_MAX_MB_MIN: u32 = 1;
pub const PREVIEW_MAX_MB_MAX: u32 = 2048;

/// The languages the app has a message catalogue for, as BCP 47 tags. `system` follows the OS.
pub const SUPPORTED_LANGUAGES: [&str; 2] = ["en-CA", "fr-CA"];

/// Developer-only pseudo-locales (accented and lengthened English, and a right-to-left mock) that
/// prove the catalogue loader, text expansion and mirrored layout. Debug builds accept them; a
/// release build treats one in a settings file as unknown, so it never reaches a person.
pub const PSEUDO_LANGUAGES: [&str; 2] = ["en-XA", "ar-XB"];

/// Whether this build has a catalogue for `language` (not `system`).
pub fn language_available(language: &str) -> bool {
    SUPPORTED_LANGUAGES.contains(&language)
        || (cfg!(debug_assertions) && PSEUDO_LANGUAGES.contains(&language))
}

/// Light, dark, or whatever the OS says (SPEC 9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum ColourMode {
    #[default]
    System,
    Light,
    Dark,
}

/// Where the colours come from: the brand tokens, or the OS's accent and colour scheme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum ThemeSource {
    #[default]
    Liminal,
    Os,
}

/// The accent colour: the brand colour, the OS's, or one the person picked (`#rrggbb`).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum AccentChoice {
    #[default]
    Ember,
    Os,
    Custom {
        hex: String,
    },
}

/// How much room the lists and the chrome take.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum Density {
    Compact,
    #[default]
    Comfortable,
    Spacious,
}

/// The weight of the app's own icons (D123): variants of one set of paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum IconStyle {
    Light,
    #[default]
    Regular,
    Bold,
    Filled,
}

/// Which set of file and folder icons the views draw.
///
/// `System` is accepted so a document that names it loads, but the page does not offer it yet and
/// the views draw the Waypoint set for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum IconTheme {
    #[default]
    Waypoint,
    Portage,
    System,
}

/// The colour of the Portage folder icons. The ids are the ones `portagePalette.ts` draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum FolderColour {
    #[default]
    Liminal,
    Gnome,
    Cinnamon,
    Kde,
    Windows11,
    Red,
    Pink,
    Orange,
    Purple,
    Rainbow,
}

/// The Appearance page.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct AppearanceSettings {
    pub mode: ColourMode,
    pub theme_source: ThemeSource,
    pub accent: AccentChoice,
    pub density: Density,
    pub icon_style: IconStyle,
    pub icon_theme: IconTheme,
    pub folder_colour: FolderColour,
    /// Whether surfaces, text and selection take the operating system's palette instead of
    /// Waypoint's own colours (D144). Off by default; the accent choice still decides the accent.
    pub match_system_colours: bool,
}

/// Whether a blur is put behind the window, where the system can. There is one strength: on Linux the
/// compositors have no other, and on Windows the quieter material (Mica) only tints the window with the
/// wallpaper and shows no blur at all, so a "low" the settings once stored reads as `High`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum BlurLevel {
    #[default]
    Off,
    High,
}

impl<'de> Deserialize<'de> for BlurLevel {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        /// What the settings file may hold, `low` included.
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        enum Stored {
            Off,
            Low,
            High,
        }
        Ok(match Stored::deserialize(deserializer)? {
            Stored::Off => BlurLevel::Off,
            Stored::Low | Stored::High => BlurLevel::High,
        })
    }
}

/// Which parts of the window are translucent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct TransparencyRegions {
    pub sidebar: bool,
    pub content: bool,
    pub title_bar: bool,
}

impl Default for TransparencyRegions {
    fn default() -> Self {
        Self {
            sidebar: true,
            content: false,
            title_bar: true,
        }
    }
}

/// The Transparency page. Off by default (D117).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct TransparencySettings {
    pub enabled: bool,
    /// The title bar's and menu bar's opacity in percent, 40 to 100.
    pub opacity: u8,
    /// The tabs', toolbar's and status bar's opacity in percent, 40 to 100.
    pub rows_opacity: u8,
    /// The sidebar's (and the Inspector's) opacity in percent, 40 to 100.
    pub sidebar_opacity: u8,
    /// The file area's opacity in percent, 40 to 100.
    pub content_opacity: u8,
    pub blur: BlurLevel,
    pub regions: TransparencyRegions,
    /// Whether menus and popups are translucent too.
    pub menus: bool,
    /// The menus' opacity in percent, 60 to 100.
    pub menu_opacity: u8,
    /// Whether a window that is not in front draws solid.
    pub solid_when_unfocused: bool,
}

impl Default for TransparencySettings {
    fn default() -> Self {
        Self {
            enabled: false,
            opacity: 82,
            rows_opacity: 90,
            sidebar_opacity: 94,
            content_opacity: 98,
            blur: BlurLevel::Off,
            regions: TransparencyRegions::default(),
            menus: false,
            menu_opacity: 96,
            solid_when_unfocused: true,
        }
    }
}

/// A preference that follows the OS unless it is forced on or off.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum OsPreference {
    #[default]
    Follow,
    On,
    Off,
}

/// Touch mode: larger hit targets, on when the last pointer was touch or no fine pointer exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum TouchMode {
    Off,
    #[default]
    Auto,
    On,
}

/// The Accessibility page (SPEC 11).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct AccessibilitySettings {
    pub high_contrast: OsPreference,
    /// The text size in percent of the normal size: 100, 115 or 130.
    pub text_size: u16,
    pub touch_mode: TouchMode,
    pub reduced_motion: OsPreference,
    pub reduced_transparency: OsPreference,
    /// A thicker, higher-contrast focus ring.
    pub strong_focus_ring: bool,
}

impl Default for AccessibilitySettings {
    fn default() -> Self {
        Self {
            high_contrast: OsPreference::Follow,
            text_size: 100,
            touch_mode: TouchMode::Auto,
            reduced_motion: OsPreference::Follow,
            reduced_transparency: OsPreference::Follow,
            strong_focus_ring: false,
        }
    }
}

/// Which way text and layout run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum Direction {
    /// Follow the language.
    #[default]
    Auto,
    Ltr,
    Rtl,
}

/// The Language & region page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct LocaleSettings {
    /// `system`, or one of `SUPPORTED_LANGUAGES` (or `PSEUDO_LANGUAGES` in a debug build).
    pub language: String,
    pub direction: Direction,
}

impl Default for LocaleSettings {
    fn default() -> Self {
        Self {
            language: "system".to_owned(),
            direction: Direction::Auto,
        }
    }
}

/// The Previews & thumbnails page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct PreviewSettings {
    pub thumbnails: bool,
    /// Files larger than this, in megabytes, get an icon, not a thumbnail.
    pub max_file_mb: u32,
    pub folder_peeks: bool,
    pub hover_to_peek: bool,
    /// Whether Overview measures Home when it opens (it can always be started by hand).
    pub measure_home_on_open: bool,
}

impl Default for PreviewSettings {
    fn default() -> Self {
        Self {
            thumbnails: true,
            max_file_mb: 50,
            folder_peeks: true,
            hover_to_peek: false,
            measure_home_on_open: true,
        }
    }
}

/// The Integrations page. Every integration is off until it is enabled (D118).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct IntegrationSettings {
    pub notifications: bool,
    /// Whether a job's notification carries action buttons ("Show in folder", "Undo", …) where the
    /// system draws them. It is on by default, so it takes effect as soon as `notifications` is.
    pub notification_actions: bool,
    pub launcher_progress: bool,
    pub prevent_sleep: bool,
    /// Take `org.freedesktop.FileManager1` while Waypoint runs, so other applications' "Show in
    /// folder" opens Waypoint. Linux only; making Waypoint the default for folders is a one-off
    /// action on the page, not a setting.
    pub default_file_manager: bool,
    /// Whether the global shortcut is registered.
    pub global_shortcut_enabled: bool,
    /// The accelerator that brings Waypoint to the front, such as `Ctrl+Alt+W`; none means the
    /// default (`DEFAULT_ACCELERATOR`).
    pub global_shortcut: Option<String>,
    /// Offer to keep the passphrase of an encrypted volume in the system keyring when it is unlocked, and unlock a remembered volume by itself when it is plugged in. Off until it is enabled (D118, D153); turning it off keeps what is in the keyring but stops using it.
    pub remember_volume_passphrases: bool,
}

impl Default for IntegrationSettings {
    fn default() -> Self {
        Self {
            notifications: false,
            notification_actions: true,
            launcher_progress: false,
            prevent_sleep: false,
            default_file_manager: false,
            global_shortcut_enabled: false,
            global_shortcut: None,
            remember_volume_passphrases: false,
        }
    }
}

/// The Experimental page: one switch per remote protocol, each off until it is turned on (D167).
/// A protocol that is off has no provider registered, so nothing connects, listens or reads a
/// credential for it, and its addresses fail with a reason that points here. `native_context_menus`
/// is the one switch that is not a protocol: it makes the file list, sidebar, Trash and tab menus
/// open as the system's own menus, which can hang past the window's edge (D196).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct ExperimentalSettings {
    pub sftp: bool,
    pub smb: bool,
    pub webdav: bool,
    pub s3: bool,
    pub native_context_menus: bool,
}

/// Everything the Settings window edits that is not an operations setting.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct Settings {
    pub general: GeneralSettings,
    pub dnd: DndSettings,
    pub ui: UiSettings,
    pub appearance: AppearanceSettings,
    pub transparency: TransparencySettings,
    pub accessibility: AccessibilitySettings,
    pub locale: LocaleSettings,
    pub previews: PreviewSettings,
    pub integrations: IntegrationSettings,
    pub experimental: ExperimentalSettings,
}

/// Why a settings value was refused. Nothing changed.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SettingsError {
    #[error("{field} must be between {min} and {max}")]
    OutOfRange {
        /// The setting, as the page names it (`dnd.springLoadMs`).
        field: &'static str,
        min: u32,
        max: u32,
    },
    /// A value that is not one of the allowed ones, or is not written the way it must be.
    #[error("{field} is not valid: {reason}")]
    Invalid {
        field: &'static str,
        reason: &'static str,
    },
}

fn out_of_range(field: &'static str, value: u32, min: u32, max: u32) -> Result<(), SettingsError> {
    if (min..=max).contains(&value) {
        Ok(())
    } else {
        Err(SettingsError::OutOfRange { field, min, max })
    }
}

/// Whether `text` is `#` and six hexadecimal digits.
fn is_hex_colour(text: &str) -> bool {
    text.len() == 7 && text.starts_with('#') && text[1..].bytes().all(|b| b.is_ascii_hexdigit())
}

impl Settings {
    /// Checks every value against its range.
    pub fn validate(&self) -> Result<(), SettingsError> {
        out_of_range(
            "dnd.springLoadMs",
            self.dnd.spring_load_ms,
            SPRING_LOAD_MIN_MS,
            SPRING_LOAD_MAX_MS,
        )?;
        out_of_range(
            "transparency.opacity",
            self.transparency.opacity.into(),
            OPACITY_MIN.into(),
            OPACITY_MAX.into(),
        )?;
        out_of_range(
            "transparency.rowsOpacity",
            self.transparency.rows_opacity.into(),
            OPACITY_MIN.into(),
            OPACITY_MAX.into(),
        )?;
        out_of_range(
            "transparency.sidebarOpacity",
            self.transparency.sidebar_opacity.into(),
            OPACITY_MIN.into(),
            OPACITY_MAX.into(),
        )?;
        out_of_range(
            "transparency.contentOpacity",
            self.transparency.content_opacity.into(),
            OPACITY_MIN.into(),
            OPACITY_MAX.into(),
        )?;
        out_of_range(
            "transparency.menuOpacity",
            self.transparency.menu_opacity.into(),
            MENU_OPACITY_MIN.into(),
            OPACITY_MAX.into(),
        )?;
        out_of_range(
            "previews.maxFileMb",
            self.previews.max_file_mb,
            PREVIEW_MAX_MB_MIN,
            PREVIEW_MAX_MB_MAX,
        )?;
        let mut widths = self.ui.column_widths;
        for (field, width) in widths.named() {
            if let Some(width) = *width {
                out_of_range(
                    field,
                    width.into(),
                    COLUMN_WIDTH_MIN.into(),
                    COLUMN_WIDTH_MAX.into(),
                )?;
            }
        }
        if !TEXT_SIZES.contains(&self.accessibility.text_size) {
            return Err(SettingsError::Invalid {
                field: "accessibility.textSize",
                reason: "use 100, 115 or 130",
            });
        }
        if let AccentChoice::Custom { hex } = &self.appearance.accent {
            if !is_hex_colour(hex) {
                return Err(SettingsError::Invalid {
                    field: "appearance.accent",
                    reason: "write it as #rrggbb",
                });
            }
        }
        if self.locale.language != "system" && !language_available(&self.locale.language) {
            return Err(SettingsError::Invalid {
                field: "locale.language",
                reason: "not a language Waypoint has",
            });
        }
        if let Some(shortcut) = &self.integrations.global_shortcut {
            validate_accelerator(shortcut).map_err(|reason| SettingsError::Invalid {
                field: "integrations.globalShortcut",
                reason,
            })?;
        }
        Ok(())
    }

    /// The settings with every out-of-range value brought to the nearest valid one, for a stored
    /// document edited by hand: a bad value must not stop the app from starting.
    pub fn clamped(mut self) -> Self {
        self.dnd.spring_load_ms = self
            .dnd
            .spring_load_ms
            .clamp(SPRING_LOAD_MIN_MS, SPRING_LOAD_MAX_MS);
        self.transparency.opacity = self.transparency.opacity.clamp(OPACITY_MIN, OPACITY_MAX);
        for opacity in [
            &mut self.transparency.rows_opacity,
            &mut self.transparency.sidebar_opacity,
            &mut self.transparency.content_opacity,
        ] {
            *opacity = (*opacity).clamp(OPACITY_MIN, OPACITY_MAX);
        }
        self.transparency.menu_opacity = self
            .transparency
            .menu_opacity
            .clamp(MENU_OPACITY_MIN, OPACITY_MAX);
        self.previews.max_file_mb = self
            .previews
            .max_file_mb
            .clamp(PREVIEW_MAX_MB_MIN, PREVIEW_MAX_MB_MAX);
        for (_, width) in self.ui.column_widths.named() {
            *width = width.map(|width| width.clamp(COLUMN_WIDTH_MIN, COLUMN_WIDTH_MAX));
        }
        if !TEXT_SIZES.contains(&self.accessibility.text_size) {
            let size = self.accessibility.text_size;
            self.accessibility.text_size = TEXT_SIZES
                .into_iter()
                .min_by_key(|candidate| candidate.abs_diff(size))
                .unwrap_or(100);
        }
        if matches!(&self.appearance.accent, AccentChoice::Custom { hex } if !is_hex_colour(hex)) {
            self.appearance.accent = AccentChoice::Ember;
        }
        if self.locale.language != "system" && !language_available(&self.locale.language) {
            self.locale.language = "system".to_owned();
        }
        if self
            .integrations
            .global_shortcut
            .as_deref()
            .is_some_and(|text| validate_accelerator(text).is_err())
        {
            self.integrations.global_shortcut = None;
        }
        self
    }
}

/// The settings with the revision that produced them, so a window can tell a newer state from an
/// older one that arrives late.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct SettingsSnapshot {
    /// Starts at 0 for the settings loaded at start-up and grows by one on every change.
    #[ts(type = "number")]
    pub revision: u64,
    pub settings: Settings,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_the_documented_ones() {
        let s = Settings::default();
        assert_eq!(s.general.startup, StartupMode::RestoreSession);
        assert_eq!(s.general.default_view, DefaultView::List);
        assert_eq!(s.general.click_mode, ClickMode::Double);
        assert!(!s.general.show_hidden_default);
        assert_eq!(s.dnd.default_action_rule, DropActionRule::ByVolume);
        assert_eq!(s.dnd.spring_load_ms, 600);
        assert!(s.dnd.shelf_persist);
        assert!(s.ui.action_bar);
        assert!(s.ui.action_bar_labels);
        assert!(!s.ui.app_name_in_title);
        assert!(!s.ui.menu_bar);
        assert_eq!(s.validate(), Ok(()));
    }

    #[test]
    fn the_spring_load_delay_is_bounded_at_both_ends() {
        for (ms, ok) in [
            (199, false),
            (200, true),
            (2000, true),
            (2001, false),
            (0, false),
        ] {
            let mut s = Settings::default();
            s.dnd.spring_load_ms = ms;
            assert_eq!(s.validate().is_ok(), ok, "{ms} ms");
        }
        let mut s = Settings::default();
        s.dnd.spring_load_ms = 5;
        assert_eq!(
            s.validate().unwrap_err().to_string(),
            "dnd.springLoadMs must be between 200 and 2000"
        );
    }

    #[test]
    fn clamping_brings_a_stored_value_into_range() {
        let mut s = Settings::default();
        s.dnd.spring_load_ms = 90_000;
        assert_eq!(s.clone().clamped().dnd.spring_load_ms, SPRING_LOAD_MAX_MS);
        s.dnd.spring_load_ms = 0;
        assert_eq!(s.clamped().dnd.spring_load_ms, SPRING_LOAD_MIN_MS);
    }

    #[test]
    fn the_wire_form_is_camel_case_and_missing_fields_take_their_defaults() {
        let json = serde_json::to_value(Settings::default()).unwrap();
        assert_eq!(json["general"]["showHiddenDefault"], false);
        assert_eq!(json["general"]["startup"], "restoreSession");
        assert_eq!(json["dnd"]["defaultActionRule"], "byVolume");
        // A document from an older build that lacked the `dnd` section still loads.
        let partial: Settings =
            serde_json::from_str(r#"{"general":{"defaultView":"grid"}}"#).unwrap();
        assert_eq!(partial.general.default_view, DefaultView::Grid);
        assert_eq!(partial.general.click_mode, ClickMode::Double);
        assert_eq!(partial.dnd, DndSettings::default());
        // So does one from before the `ui` section: the Action bar is on, with labels.
        assert_eq!(partial.ui, UiSettings::default());
        let ui = serde_json::to_value(UiSettings::default()).unwrap();
        assert_eq!(ui["actionBar"], true);
        assert_eq!(ui["actionBarLabels"], true);
    }

    #[test]
    fn the_action_bar_choices_round_trip_and_default_one_by_one() {
        let hidden: Settings = serde_json::from_str(r#"{"ui":{"actionBar":false}}"#).unwrap();
        assert!(!hidden.ui.action_bar);
        assert!(hidden.ui.action_bar_labels);
        let icons_only = Settings {
            ui: UiSettings {
                action_bar: true,
                action_bar_labels: false,
                app_name_in_title: true,
                menu_bar: true,
                git_column: false,
                storage_class_column: true,
                column_widths: ListColumnWidths {
                    size: Some(120),
                    ..ListColumnWidths::default()
                },
            },
            ..Settings::default()
        };
        let text = serde_json::to_string(&icons_only).unwrap();
        assert_eq!(serde_json::from_str::<Settings>(&text).unwrap(), icons_only);
        assert_eq!(icons_only.validate(), Ok(()));
    }

    #[test]
    fn the_git_column_defaults_on_and_follows_the_header_menu() {
        let old: Settings = serde_json::from_str(r#"{"ui":{"actionBar":false}}"#).unwrap();
        assert!(old.ui.git_column);
        let off: Settings = serde_json::from_str(r#"{"ui":{"gitColumn":false}}"#).unwrap();
        assert!(!off.ui.git_column);
        assert_eq!(
            serde_json::to_value(Settings::default()).unwrap()["ui"]["gitColumn"],
            true
        );
    }

    #[test]
    fn the_storage_class_column_defaults_off_and_follows_the_header_menu() {
        let old: Settings = serde_json::from_str(r#"{"ui":{"gitColumn":false}}"#).unwrap();
        assert!(!old.ui.storage_class_column);
        let on: Settings = serde_json::from_str(r#"{"ui":{"storageClassColumn":true}}"#).unwrap();
        assert!(on.ui.storage_class_column);
        assert_eq!(
            serde_json::to_value(Settings::default()).unwrap()["ui"]["storageClassColumn"],
            false
        );
    }

    #[test]
    fn column_widths_default_to_the_columns_own_and_are_read_one_by_one() {
        let old: Settings = serde_json::from_str(r#"{"ui":{"gitColumn":false}}"#).unwrap();
        assert_eq!(old.ui.column_widths, ListColumnWidths::default());
        let one: Settings =
            serde_json::from_str(r#"{"ui":{"columnWidths":{"storageClass":150}}}"#).unwrap();
        assert_eq!(one.ui.column_widths.storage_class, Some(150));
        assert_eq!(one.ui.column_widths.size, None);
        let json = serde_json::to_value(one).unwrap();
        assert_eq!(json["ui"]["columnWidths"]["storageClass"], 150);
        assert!(json["ui"]["columnWidths"]["modified"].is_null());
    }

    #[test]
    fn a_column_width_is_bounded_at_both_ends() {
        let with = |width: u16| {
            let mut s = Settings::default();
            s.ui.column_widths.original = Some(width);
            s
        };
        assert_eq!(with(COLUMN_WIDTH_MIN).validate(), Ok(()));
        assert_eq!(with(COLUMN_WIDTH_MAX).validate(), Ok(()));
        for width in [0, COLUMN_WIDTH_MIN - 1, COLUMN_WIDTH_MAX + 1] {
            assert_eq!(
                with(width).validate(),
                Err(SettingsError::OutOfRange {
                    field: "ui.columnWidths.original",
                    min: COLUMN_WIDTH_MIN.into(),
                    max: COLUMN_WIDTH_MAX.into(),
                })
            );
        }
        let repaired = with(u16::MAX).clamped();
        assert_eq!(repaired.ui.column_widths.original, Some(COLUMN_WIDTH_MAX));
        assert_eq!(
            with(1).clamped().ui.column_widths.original,
            Some(COLUMN_WIDTH_MIN)
        );
        assert_eq!(with(1).clamped().ui.column_widths.size, None);
    }

    #[test]
    fn the_menu_bar_defaults_off_and_round_trips() {
        let partial: Settings = serde_json::from_str(r#"{"ui":{"actionBar":false}}"#).unwrap();
        assert!(!partial.ui.menu_bar);
        let on: Settings = serde_json::from_str(r#"{"ui":{"menuBar":true}}"#).unwrap();
        assert!(on.ui.menu_bar);
        assert_eq!(serde_json::to_value(on).unwrap()["ui"]["menuBar"], true);
    }

    #[test]
    fn matching_the_system_colours_is_off_by_default_and_round_trips() {
        assert!(!Settings::default().appearance.match_system_colours);
        let old: Settings = serde_json::from_str(r#"{"appearance":{"mode":"dark"}}"#).unwrap();
        assert!(!old.appearance.match_system_colours);
        let on: Settings =
            serde_json::from_str(r#"{"appearance":{"matchSystemColours":true}}"#).unwrap();
        assert!(on.appearance.match_system_colours);
        let json = serde_json::to_value(&on).unwrap();
        assert_eq!(json["appearance"]["matchSystemColours"], true);
    }

    #[test]
    fn an_appearance_document_without_the_icon_theme_gets_the_waypoint_set_in_liminal() {
        let old: Settings =
            serde_json::from_str(r#"{"appearance":{"mode":"dark","iconStyle":"bold"}}"#).unwrap();
        assert_eq!(old.appearance.icon_style, IconStyle::Bold);
        assert_eq!(old.appearance.icon_theme, IconTheme::Waypoint);
        assert_eq!(old.appearance.folder_colour, FolderColour::Liminal);
        let json = serde_json::to_value(Settings::default()).unwrap();
        assert_eq!(json["appearance"]["iconTheme"], "waypoint");
        assert_eq!(json["appearance"]["folderColour"], "liminal");
    }

    #[test]
    fn the_icon_theme_and_folder_colour_round_trip_in_their_wire_form() {
        let doc = r#"{"appearance":{"iconTheme":"portage","folderColour":"windows11"}}"#;
        let s: Settings = serde_json::from_str(doc).unwrap();
        assert_eq!(s.appearance.icon_theme, IconTheme::Portage);
        assert_eq!(s.appearance.folder_colour, FolderColour::Windows11);
        assert_eq!(s.validate(), Ok(()));
        let json = serde_json::to_value(&s).unwrap();
        assert_eq!(json["appearance"]["iconTheme"], "portage");
        assert_eq!(json["appearance"]["folderColour"], "windows11");
        let system: Settings =
            serde_json::from_str(r#"{"appearance":{"iconTheme":"system"}}"#).unwrap();
        assert_eq!(system.appearance.icon_theme, IconTheme::System);
        assert_eq!(system.validate(), Ok(()));
        for colour in [
            "liminal",
            "gnome",
            "cinnamon",
            "kde",
            "windows11",
            "red",
            "pink",
            "orange",
            "purple",
            "rainbow",
        ] {
            let text = format!(r#"{{"appearance":{{"folderColour":"{colour}"}}}}"#);
            assert!(serde_json::from_str::<Settings>(&text).is_ok(), "{colour}");
        }
    }

    #[test]
    fn an_unknown_icon_theme_or_folder_colour_is_refused() {
        assert!(
            serde_json::from_str::<Settings>(r#"{"appearance":{"iconTheme":"neon"}}"#).is_err()
        );
        assert!(
            serde_json::from_str::<Settings>(r#"{"appearance":{"folderColour":"teal"}}"#).is_err()
        );
    }

    #[test]
    fn an_unknown_choice_is_refused_rather_than_guessed() {
        assert!(serde_json::from_str::<Settings>(r#"{"general":{"startup":"nowhere"}}"#).is_err());
    }

    #[test]
    fn the_milestone_five_defaults_are_the_documented_ones() {
        let s = Settings::default();
        assert_eq!(s.appearance.mode, ColourMode::System);
        assert_eq!(s.appearance.accent, AccentChoice::Ember);
        assert_eq!(s.appearance.density, Density::Comfortable);
        assert!(!s.transparency.enabled);
        assert_eq!(s.transparency.opacity, 82);
        assert_eq!(s.transparency.blur, BlurLevel::Off);
        assert!(s.transparency.solid_when_unfocused);
        assert_eq!(s.accessibility.text_size, 100);
        assert_eq!(s.accessibility.touch_mode, TouchMode::Auto);
        assert_eq!(s.accessibility.high_contrast, OsPreference::Follow);
        assert_eq!(s.locale.language, "system");
        assert_eq!(s.previews.max_file_mb, 50);
        assert!(s.previews.measure_home_on_open);
        // The Git overlay is on until it is turned off (D161).
        assert!(s.general.git_decorations);
        // Every integration is off until it is enabled (D118).
        assert!(!s.integrations.notifications);
        assert!(s.integrations.notification_actions);
        assert!(!s.integrations.launcher_progress);
        assert!(!s.integrations.prevent_sleep);
        assert!(!s.integrations.default_file_manager);
        assert!(!s.integrations.remember_volume_passphrases);
        assert!(!s.integrations.global_shortcut_enabled);
        assert_eq!(s.integrations.global_shortcut, None);
        assert_eq!(s.validate(), Ok(()));
    }

    #[test]
    fn the_new_ranges_are_bounded_at_both_ends() {
        let edit = |change: &dyn Fn(&mut Settings)| {
            let mut s = Settings::default();
            change(&mut s);
            s.validate()
        };
        assert!(edit(&|s| s.transparency.opacity = 39).is_err());
        assert!(edit(&|s| s.transparency.opacity = 40).is_ok());
        assert!(edit(&|s| s.transparency.opacity = 101).is_err());
        let setters: [fn(&mut Settings, u8); 3] = [
            |s, v| s.transparency.rows_opacity = v,
            |s, v| s.transparency.sidebar_opacity = v,
            |s, v| s.transparency.content_opacity = v,
        ];
        for set in setters {
            assert!(edit(&|s| set(s, 39)).is_err());
            assert!(edit(&|s| set(s, 40)).is_ok());
            assert!(edit(&|s| set(s, 100)).is_ok());
            assert!(edit(&|s| set(s, 101)).is_err());
        }
        assert_eq!(
            edit(&|s| s.transparency.sidebar_opacity = 10)
                .unwrap_err()
                .to_string(),
            "transparency.sidebarOpacity must be between 40 and 100"
        );
        assert!(edit(&|s| s.transparency.menu_opacity = 59).is_err());
        assert!(edit(&|s| s.transparency.menu_opacity = 60).is_ok());
        assert!(edit(&|s| s.previews.max_file_mb = 0).is_err());
        assert!(edit(&|s| s.previews.max_file_mb = 2048).is_ok());
        assert!(edit(&|s| s.previews.max_file_mb = 2049).is_err());
        assert_eq!(
            edit(&|s| s.transparency.opacity = 10)
                .unwrap_err()
                .to_string(),
            "transparency.opacity must be between 40 and 100"
        );
    }

    #[test]
    fn text_size_accent_language_and_shortcut_must_be_valid() {
        let edit = |change: &dyn Fn(&mut Settings)| {
            let mut s = Settings::default();
            change(&mut s);
            s.validate()
        };
        for size in TEXT_SIZES {
            assert!(edit(&|s| s.accessibility.text_size = size).is_ok());
        }
        assert!(edit(&|s| s.accessibility.text_size = 110).is_err());
        assert!(edit(&|s| s.appearance.accent = AccentChoice::Custom {
            hex: "#f97316".into()
        })
        .is_ok());
        for bad in ["f97316", "#f9731", "#f97316a", "#gggggg", ""] {
            assert!(
                edit(&|s| s.appearance.accent = AccentChoice::Custom { hex: bad.into() }).is_err(),
                "{bad}"
            );
        }
        assert!(edit(&|s| s.locale.language = "fr-CA".into()).is_ok());
        assert!(edit(&|s| s.locale.language = "de-DE".into()).is_err());
        // The pseudo-locales are for developer builds, which is what the tests are.
        assert_eq!(
            edit(&|s| s.locale.language = "ar-XB".into()).is_ok(),
            cfg!(debug_assertions)
        );
        assert!(edit(&|s| s.integrations.global_shortcut = Some("Ctrl+Alt+W".into())).is_ok());
        assert!(edit(&|s| s.integrations.global_shortcut = Some("  ".into())).is_err());
        assert!(edit(&|s| s.integrations.global_shortcut = Some("x".repeat(65))).is_err());
        // A bare key would take that key from every application.
        assert!(edit(&|s| s.integrations.global_shortcut = Some("W".into())).is_err());
    }

    #[test]
    fn clamping_repairs_a_hand_edited_document() {
        let mut s = Settings::default();
        s.transparency.opacity = 3;
        s.transparency.menu_opacity = 250;
        s.transparency.rows_opacity = 0;
        s.transparency.sidebar_opacity = 12;
        s.transparency.content_opacity = 255;
        s.previews.max_file_mb = 0;
        s.accessibility.text_size = 120;
        s.appearance.accent = AccentChoice::Custom {
            hex: "orange".into(),
        };
        s.locale.language = "xx".into();
        s.integrations.global_shortcut = Some(String::new());
        let fixed = s.clamped();
        assert_eq!(fixed.transparency.opacity, OPACITY_MIN);
        assert_eq!(fixed.transparency.menu_opacity, OPACITY_MAX);
        assert_eq!(fixed.transparency.rows_opacity, OPACITY_MIN);
        assert_eq!(fixed.transparency.sidebar_opacity, OPACITY_MIN);
        assert_eq!(fixed.transparency.content_opacity, OPACITY_MAX);
        assert_eq!(fixed.previews.max_file_mb, PREVIEW_MAX_MB_MIN);
        assert_eq!(fixed.accessibility.text_size, 115);
        assert_eq!(fixed.appearance.accent, AccentChoice::Ember);
        assert_eq!(fixed.locale.language, "system");
        assert_eq!(fixed.integrations.global_shortcut, None);
        assert_eq!(fixed.validate(), Ok(()));
    }

    #[test]
    fn a_document_from_before_milestone_five_still_loads_with_the_new_sections_at_their_defaults() {
        let old: Settings = serde_json::from_str(
            r#"{"general":{"defaultView":"grid"},"dnd":{"springLoadMs":900}}"#,
        )
        .unwrap();
        assert_eq!(old.dnd.spring_load_ms, 900);
        assert_eq!(old.appearance, AppearanceSettings::default());
        assert_eq!(old.transparency, TransparencySettings::default());
        assert_eq!(old.accessibility, AccessibilitySettings::default());
        assert_eq!(old.locale, LocaleSettings::default());
        assert_eq!(old.previews, PreviewSettings::default());
        assert_eq!(old.integrations, IntegrationSettings::default());
        assert_eq!(old.experimental, ExperimentalSettings::default());
    }

    #[test]
    fn every_remote_protocol_is_off_until_it_is_turned_on() {
        let defaults = Settings::default().experimental;
        assert!(!defaults.sftp && !defaults.smb && !defaults.webdav && !defaults.s3);
        assert!(!defaults.native_context_menus);
        let json = serde_json::to_value(Settings::default()).unwrap();
        assert_eq!(
            json["experimental"],
            serde_json::json!({
                "sftp": false,
                "smb": false,
                "webdav": false,
                "s3": false,
                "nativeContextMenus": false
            })
        );
    }

    #[test]
    fn a_settings_file_from_before_native_menus_keeps_them_off() {
        let old: Settings =
            serde_json::from_str(r#"{"experimental":{"sftp":true,"smb":false}}"#).unwrap();
        assert!(old.experimental.sftp);
        assert!(!old.experimental.native_context_menus);
        let on: Settings =
            serde_json::from_str(r#"{"experimental":{"nativeContextMenus":true}}"#).unwrap();
        assert!(on.experimental.native_context_menus && !on.experimental.sftp);
        assert_eq!(on.validate(), Ok(()));
    }

    #[test]
    fn each_protocol_switch_is_read_on_its_own() {
        let one: Settings = serde_json::from_str(r#"{"experimental":{"smb":true}}"#).unwrap();
        assert_eq!(
            one.experimental,
            ExperimentalSettings {
                smb: true,
                ..ExperimentalSettings::default()
            }
        );
        assert_eq!(one.validate(), Ok(()));
        // A switch from a later version, or one written as something else, never stops the app.
        let odd: Settings =
            serde_json::from_str(r#"{"experimental":{"sftp":true,"gopher":true}}"#).unwrap();
        assert!(odd.experimental.sftp);
        assert_eq!(odd.clone().clamped(), odd);
    }

    #[test]
    fn a_general_document_without_the_git_switch_has_it_on() {
        let old: Settings =
            serde_json::from_str(r#"{"general":{"showHiddenDefault":true}}"#).unwrap();
        assert!(old.general.show_hidden_default);
        assert!(old.general.git_decorations);
        let off: Settings =
            serde_json::from_str(r#"{"general":{"gitDecorations":false}}"#).unwrap();
        assert!(!off.general.git_decorations);
        assert_eq!(off.validate(), Ok(()));
        let json = serde_json::to_value(Settings::default()).unwrap();
        assert_eq!(json["general"]["gitDecorations"], true);
    }

    #[test]
    fn an_integrations_document_without_the_passphrase_switch_has_it_off() {
        let old: Settings =
            serde_json::from_str(r#"{"integrations":{"notifications":true}}"#).unwrap();
        assert!(old.integrations.notifications);
        assert!(!old.integrations.remember_volume_passphrases);
        let on: Settings =
            serde_json::from_str(r#"{"integrations":{"rememberVolumePassphrases":true}}"#).unwrap();
        assert!(on.integrations.remember_volume_passphrases);
        assert_eq!(on.validate(), Ok(()));
        let json = serde_json::to_value(Settings::default()).unwrap();
        assert_eq!(json["integrations"]["rememberVolumePassphrases"], false);
    }

    #[test]
    fn an_integrations_document_without_the_notification_buttons_switch_has_it_on() {
        let old: Settings =
            serde_json::from_str(r#"{"integrations":{"notifications":true,"preventSleep":true}}"#)
                .unwrap();
        assert!(old.integrations.notifications);
        assert!(old.integrations.prevent_sleep);
        assert!(old.integrations.notification_actions);
        assert!(!old.integrations.launcher_progress);
        let off: Settings =
            serde_json::from_str(r#"{"integrations":{"notificationActions":false}}"#).unwrap();
        assert!(!off.integrations.notification_actions);
        assert_eq!(off.validate(), Ok(()));
        let json = serde_json::to_value(Settings::default()).unwrap();
        assert_eq!(json["integrations"]["notificationActions"], true);
    }

    #[test]
    fn a_transparency_document_without_the_region_opacities_gets_todays_look() {
        let old: Settings =
            serde_json::from_str(r#"{"transparency":{"enabled":true,"opacity":70,"menus":true}}"#)
                .unwrap();
        assert!(old.transparency.enabled);
        assert_eq!(old.transparency.opacity, 70);
        assert_eq!(old.transparency.rows_opacity, 90);
        assert_eq!(old.transparency.sidebar_opacity, 94);
        assert_eq!(old.transparency.content_opacity, 98);
        assert_eq!(old.validate(), Ok(()));
    }

    #[test]
    fn a_stored_low_blur_reads_as_the_one_blur_and_is_written_back_as_high() {
        let low: Settings = serde_json::from_str(r#"{"transparency":{"blur":"low"}}"#).unwrap();
        assert_eq!(low.transparency.blur, BlurLevel::High);
        let high: Settings = serde_json::from_str(r#"{"transparency":{"blur":"high"}}"#).unwrap();
        assert_eq!(high.transparency.blur, BlurLevel::High);
        let off: Settings = serde_json::from_str(r#"{"transparency":{"blur":"off"}}"#).unwrap();
        assert_eq!(off.transparency.blur, BlurLevel::Off);
        assert_eq!(
            serde_json::to_value(low.transparency).unwrap()["blur"],
            "high"
        );
        assert!(serde_json::from_str::<Settings>(r#"{"transparency":{"blur":"medium"}}"#).is_err());
    }

    #[test]
    fn the_new_wire_forms_are_camel_case_and_the_accent_is_tagged() {
        let json = serde_json::to_value(Settings::default()).unwrap();
        assert_eq!(json["appearance"]["themeSource"], "liminal");
        assert_eq!(json["appearance"]["accent"]["kind"], "ember");
        assert_eq!(json["transparency"]["solidWhenUnfocused"], true);
        assert_eq!(json["transparency"]["opacity"], 82);
        assert_eq!(json["transparency"]["rowsOpacity"], 90);
        assert_eq!(json["transparency"]["sidebarOpacity"], 94);
        assert_eq!(json["transparency"]["contentOpacity"], 98);
        assert_eq!(json["accessibility"]["highContrast"], "follow");
        assert_eq!(json["previews"]["measureHomeOnOpen"], true);
        assert_eq!(
            json["integrations"]["globalShortcut"],
            serde_json::Value::Null
        );
        assert_eq!(json["integrations"]["defaultFileManager"], false);
        assert_eq!(json["integrations"]["globalShortcutEnabled"], false);
        let custom: Settings = serde_json::from_str(
            r##"{"appearance":{"accent":{"kind":"custom","hex":"#112233"}}}"##,
        )
        .unwrap();
        assert_eq!(
            custom.appearance.accent,
            AccentChoice::Custom {
                hex: "#112233".into()
            }
        );
        assert!(serde_json::from_str::<Settings>(r#"{"appearance":{"density":"huge"}}"#).is_err());
    }
}
