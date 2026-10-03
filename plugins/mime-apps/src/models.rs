// Defines the serialisable models of the mime-apps plugin: types, applications, handler lists and the status
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The content type of directories, as the shared MIME database names it. It is also what `type_info` reports for a URI that ends in a slash or names a directory.
pub const DIRECTORY_TYPE: &str = "inode/directory";

/// What the system knows about a type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct TypeInfo {
    /// The content type: a MIME type on Linux (`image/png`), the registered content type or, failing that, the extension (`.png`) on Windows.
    pub mime: String,
    /// A phrase for people ("PNG image"). Falls back to the type itself.
    pub description: String,
    /// The name of the type's icon in the icon theme, when it has one.
    pub icon: Option<String>,
}

/// One application that can open a type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct App {
    /// A stable handle: the desktop-file id on Linux (`org.gnome.eog.desktop`), the handler's name on Windows. Pass it to `openWith`, `setDefault` and `appIconUrl`; its form is otherwise opaque.
    pub id: String,
    /// The name to show.
    pub name: String,
    /// The name of the application's icon in the icon theme, when it has one. The picture itself comes from the `appicon://` scheme, by `id`.
    pub icon: Option<String>,
    /// The command line the application is started with, for a tooltip. Never run by the plugin itself.
    pub exec_hint: Option<String>,
}

/// The applications for one type, or for several types at once.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct Handlers {
    /// The type every URI has. When the URIs have different types, this is the first one and `mixed` is set; the lists then hold only the applications that open every type.
    pub mime: String,
    pub mixed: bool,
    /// What opens the type when nothing is chosen. Not repeated in `recommended`.
    pub default: Option<App>,
    /// Applications registered for the type, in the system's order.
    pub recommended: Vec<App>,
    /// Applications that can open it without being registered for it, by name.
    pub others: Vec<App>,
}

/// Which implementation is behind the plugin on this system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub enum Flavour {
    /// gio's `AppInfo`, over `mimeapps.list` and the desktop files.
    Gio,
    /// The OpenURI portal of a Flatpak sandbox, which cannot list or choose an application.
    Portal,
    /// The shell's association handlers.
    Windows,
    /// No support on this system.
    Unsupported,
}

/// Why a feature is unavailable. A code the front end can branch on; `message` beside it is a sentence for people.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub enum Reason {
    /// A Flatpak sandbox sees neither the host's applications nor its defaults.
    FlatpakSandbox,
    /// The system offers no chooser of its own; the front end draws a list from `handlers`.
    NoSystemChooser,
    /// The system does not allow an application to change the default silently (Windows).
    ManagedBySystem,
    /// There is no display or icon theme to draw from.
    NoDisplay,
    /// The system has no icon theme to draw file and folder icons from.
    NoIconTheme,
    /// The plugin does not do this on this operating system.
    NotImplemented,
    /// This operating system has no support in the plugin.
    UnsupportedPlatform,
}

pub const FEATURE_TYPE_INFO: &str = "typeInfo";
pub const FEATURE_HANDLERS: &str = "handlers";
pub const FEATURE_OPEN_WITH: &str = "openWith";
pub const FEATURE_OPEN_DEFAULT: &str = "openDefault";
pub const FEATURE_SET_DEFAULT: &str = "setDefault";
pub const FEATURE_CHOOSER: &str = "chooser";
pub const FEATURE_APP_ICONS: &str = "appIcons";
pub const FEATURE_TYPE_ICONS: &str = "typeIcons";
pub const FEATURE_FOLDER_ICONS: &str = "folderIcons";

/// Every feature, in the order `get_status` lists them.
pub const FEATURES: [&str; 9] = [
    FEATURE_TYPE_INFO,
    FEATURE_HANDLERS,
    FEATURE_OPEN_WITH,
    FEATURE_OPEN_DEFAULT,
    FEATURE_SET_DEFAULT,
    FEATURE_CHOOSER,
    FEATURE_APP_ICONS,
    FEATURE_TYPE_ICONS,
    FEATURE_FOLDER_ICONS,
];

/// Whether one feature of the plugin works on this system.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct FeatureStatus {
    /// One of `typeInfo`, `handlers`, `openWith`, `openDefault`, `setDefault`, `chooser`, `appIcons`, `typeIcons` or `folderIcons`.
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
    /// Why the first unavailable feature is; absent when everything works.
    pub reason: Option<Reason>,
    pub message: Option<String>,
    pub flavour: Flavour,
    pub features: Vec<FeatureStatus>,
    /// The `mimeapps.list` files that exist, from the one that wins to the one that loses (Linux only; empty elsewhere). Read-only: it explains where a default comes from.
    pub association_files: Vec<String>,
}

impl PluginStatus {
    /// Builds a status from the features. The top-level reason is the first unavailable feature's.
    pub fn build(flavour: Flavour, features: Vec<FeatureStatus>) -> Self {
        let available = features.iter().any(|feature| feature.available);
        let first = features.iter().find(|feature| !feature.available);
        PluginStatus {
            available,
            reason: first.and_then(|feature| feature.reason),
            message: first.and_then(|feature| feature.message.clone()),
            flavour,
            features,
            association_files: Vec::new(),
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

    pub fn with_association_files(mut self, files: Vec<String>) -> Self {
        self.association_files = files;
        self
    }

    /// Whether the named feature is available.
    pub fn has(&self, name: &str) -> bool {
        self.features
            .iter()
            .any(|feature| feature.name == name && feature.available)
    }
}
