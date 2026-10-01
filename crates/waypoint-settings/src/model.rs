// The settings document's types and the rules a value must satisfy
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use serde::{Deserialize, Serialize};
use thiserror::Error;
use ts_rs::TS;

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
}

impl Default for GeneralSettings {
    fn default() -> Self {
        Self {
            show_hidden_default: false,
            startup: StartupMode::RestoreSession,
            default_view: DefaultView::List,
            click_mode: ClickMode::Double,
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

/// Everything the Settings window edits that is not an operations setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct Settings {
    pub general: GeneralSettings,
    pub dnd: DndSettings,
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
}

impl Settings {
    /// Checks every value against its range.
    pub fn validate(&self) -> Result<(), SettingsError> {
        if !(SPRING_LOAD_MIN_MS..=SPRING_LOAD_MAX_MS).contains(&self.dnd.spring_load_ms) {
            return Err(SettingsError::OutOfRange {
                field: "dnd.springLoadMs",
                min: SPRING_LOAD_MIN_MS,
                max: SPRING_LOAD_MAX_MS,
            });
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
        self
    }
}

/// The settings with the revision that produced them, so a window can tell a newer state from an
/// older one that arrives late.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
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
        assert_eq!(s.clamped().dnd.spring_load_ms, SPRING_LOAD_MAX_MS);
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
    }

    #[test]
    fn an_unknown_choice_is_refused_rather_than_guessed() {
        assert!(serde_json::from_str::<Settings>(r#"{"general":{"startup":"nowhere"}}"#).is_err());
    }
}
