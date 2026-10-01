// Defines serialisable models for the os-prefs IPC payloads and the change event
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Where the reported time format came from.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub enum TimeFormatSource {
    /// GNOME's `clock-format` setting, read through xdg-desktop-portal.
    GnomePortal,
    /// GNOME's `clock-format` setting, read through the `gsettings` tool when the portal did not answer.
    GnomeGsettings,
    /// Cinnamon's `clock-use-24h` setting, read through the `gsettings` tool.
    CinnamonGsettings,
    /// The time format of the process locale (`LC_TIME`), which is what the desktop applies on KDE, Xfce, MATE and other desktops without a portal-exposed clock setting.
    Locale,
    /// The user's Region settings on Windows (`GetLocaleInfoEx` on the user default locale).
    WindowsUserLocale,
    /// The Windows system default locale's time format, used when the user's own Region settings could not be read.
    WindowsFallback,
    /// The hour pattern macOS generates for the current locale, which follows the 24-hour toggle in System Settings.
    MacosDateTemplate,
    /// The hour pattern iOS generates for the current locale.
    Ios,
    /// Android's `DateFormat.is24HourFormat`.
    Android,
    /// Nothing could be read: the value is a guess and a consumer should prefer its own inference.
    Default,
}

/// The user's 12/24-hour clock preference.
///
/// This is the payload of `get_time_format` and of the `os-prefs://time-format-changed` event. The
/// `is24Hour` field is the original wire shape of the mobile plugin; `source` is additive and is
/// filled in by the Rust side on every platform.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct TimeFormat {
    #[serde(rename = "is24Hour")]
    #[ts(rename = "is24Hour")]
    pub is_24_hour: bool,
    pub source: TimeFormatSource,
}

/// Android's Developer Options "Animator duration scale"; always 1 off Android.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct AnimatorDurationScaleResponse {
    pub scale: f32,
}

/// Whether one feature of the plugin works on this system.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct FeatureStatus {
    /// One of `timeFormat`, `timeFormatWatch`, `animatorDurationScale` or `notificationSettings`.
    pub name: String,
    pub available: bool,
    /// Why the feature is unavailable or degraded; absent when it works fully.
    pub reason: Option<String>,
}

/// What the plugin can do on the running system.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct PluginStatus {
    /// True when at least one feature is available.
    pub available: bool,
    /// Why nothing is available; absent otherwise.
    pub reason: Option<String>,
    pub features: Vec<FeatureStatus>,
    /// Where the current time format comes from; absent when it has not been read.
    pub time_format_source: Option<TimeFormatSource>,
}

impl FeatureStatus {
    pub fn available(name: &str) -> Self {
        Self {
            name: name.to_string(),
            available: true,
            reason: None,
        }
    }

    /// A feature that works, but not from the best source; `reason` says what is missing.
    pub fn degraded(name: &str, reason: impl Into<String>) -> Self {
        Self {
            name: name.to_string(),
            available: true,
            reason: Some(reason.into()),
        }
    }

    pub fn unavailable(name: &str, reason: impl Into<String>) -> Self {
        Self {
            name: name.to_string(),
            available: false,
            reason: Some(reason.into()),
        }
    }
}

impl PluginStatus {
    pub fn new(features: Vec<FeatureStatus>, time_format_source: Option<TimeFormatSource>) -> Self {
        let available = features.iter().any(|feature| feature.available);
        let reason = (!available).then(|| "no feature is available on this platform".to_string());
        Self {
            available,
            reason,
            features,
            time_format_source,
        }
    }
}
