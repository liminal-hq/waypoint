// Defines the serialisable status of the elevate plugin: whether starting an administrator helper works here, and why not
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Which implementation is behind the plugin on this system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub enum Flavour {
    /// `pkexec`, so the prompt is polkit's own.
    Polkit,
    /// No elevation on this system (yet).
    Unsupported,
}

/// The one feature the plugin has: starting the helper.
pub const FEATURE_ELEVATE: &str = "elevate";

/// Whether one feature of the plugin works on this system.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct FeatureStatus {
    /// Always `elevate` for now.
    pub name: String,
    pub available: bool,
    /// Why the feature is unavailable; absent when it works. Plain text for the person, and never a path.
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
    pub flavour: Flavour,
    pub features: Vec<FeatureStatus>,
}

impl PluginStatus {
    /// A status with the single `elevate` feature available.
    pub fn available(flavour: Flavour) -> Self {
        Self::build(flavour, vec![FeatureStatus::available(FEATURE_ELEVATE)])
    }

    /// A status with the single `elevate` feature unavailable, for `reason`.
    pub fn unavailable(flavour: Flavour, reason: impl Into<String>) -> Self {
        Self::build(
            flavour,
            vec![FeatureStatus::unavailable(FEATURE_ELEVATE, reason)],
        )
    }

    /// Builds a status from its features; the reason is the first unavailable feature's when none is available.
    pub fn build(flavour: Flavour, features: Vec<FeatureStatus>) -> Self {
        let available = features.iter().any(|feature| feature.available);
        let reason = if available {
            None
        } else {
            features.iter().find_map(|feature| feature.reason.clone())
        };
        PluginStatus {
            available,
            reason,
            flavour,
            features,
        }
    }
}

impl FeatureStatus {
    pub fn available(name: &str) -> Self {
        FeatureStatus {
            name: name.to_string(),
            available: true,
            reason: None,
        }
    }

    pub fn unavailable(name: &str, reason: impl Into<String>) -> Self {
        FeatureStatus {
            name: name.to_string(),
            available: false,
            reason: Some(reason.into()),
        }
    }
}
