// Window kinds, matching the webview labels the frontend routes on.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The kind of window a webview label denotes. Labels are `main-{n}`,
/// `settings`, `properties-{id}`, `ops` and `tear-ghost`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum WindowKind {
    Main,
    Settings,
    Properties,
    Ops,
    TearGhost,
}

impl WindowKind {
    /// Classifies a webview label, or `None` if it is not a known window.
    pub fn from_label(label: &str) -> Option<Self> {
        match label {
            "settings" => Some(Self::Settings),
            "ops" => Some(Self::Ops),
            "tear-ghost" => Some(Self::TearGhost),
            _ if label.starts_with("main-") => Some(Self::Main),
            _ if label.starts_with("properties-") => Some(Self::Properties),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_known_labels() {
        assert_eq!(WindowKind::from_label("main-1"), Some(WindowKind::Main));
        assert_eq!(
            WindowKind::from_label("settings"),
            Some(WindowKind::Settings)
        );
        assert_eq!(
            WindowKind::from_label("properties-42"),
            Some(WindowKind::Properties)
        );
        assert_eq!(
            WindowKind::from_label("tear-ghost"),
            Some(WindowKind::TearGhost)
        );
    }

    #[test]
    fn rejects_unknown_labels() {
        assert_eq!(WindowKind::from_label("mystery"), None);
    }
}
