// Joins the operations settings to the settings export, and gives it the system's file dialogs
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The settings plugin exports and imports whatever configuration files the app registers with it.
// The settings document is its own; the operations settings are the operations plugin's and the
// saved connections the file system plugin's, so this is the one place they meet (plugins never
// call each other). To put another configuration
// file in the export, implement `ConfigFile` over its owner here and register it in `wire`.
//
// The file dialogs are `tauri-plugin-dialog`'s, used from Rust only: no capability grants the
// plugin's own commands to any window, so a page cannot open a dialog or learn a path through it.

use std::path::PathBuf;
use std::sync::Arc;

use serde_json::Value;
use tauri::{AppHandle, Manager, Wry};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_waypoint_ops::Ops;
use tauri_plugin_waypoint_settings::{FilePicker, Transfers};
use waypoint_ops::OpsSettings;
use waypoint_settings::{
    differing_paths, unknown_paths, BundleError, BundleKind, ChangeGroup, ConfigFile, FilePlan,
    ImportWarning,
};

/// The id the operations settings are exported under.
const OPS_FILE_ID: &str = "ops";

/// The operations settings (concurrency, verification, undo depth, Trash sweep), owned by the
/// operations plugin and changed only through it.
struct OpsConfigFile(AppHandle<Wry>);

impl OpsConfigFile {
    fn ops(&self) -> Result<tauri::State<'_, Ops<Wry>>, String> {
        self.0
            .try_state::<Ops<Wry>>()
            .ok_or_else(|| "the operations plugin is not running".to_owned())
    }
}

impl ConfigFile for OpsConfigFile {
    fn id(&self) -> &str {
        OPS_FILE_ID
    }

    fn export(&self) -> Result<Value, String> {
        serde_json::to_value(self.ops()?.settings()).map_err(|e| e.to_string())
    }

    fn plan(&self, incoming: &Value) -> Result<FilePlan, BundleError> {
        let invalid = |message: String| BundleError::Invalid {
            file: OPS_FILE_ID.to_owned(),
            message,
        };
        let settings: OpsSettings =
            serde_json::from_value(incoming.clone()).map_err(|e| invalid(e.to_string()))?;
        Ops::<Wry>::validate_settings(&settings).map_err(|e| invalid(e.to_string()))?;
        let document = serde_json::to_value(settings).map_err(|e| invalid(e.to_string()))?;
        let now = self.export().map_err(invalid)?;
        let changed = differing_paths(&now, &document).len();
        let unknown = unknown_paths(incoming, &document);
        Ok(FilePlan {
            document,
            // The operations settings are one group on the page, however many values differ.
            changes: if changed == 0 {
                Vec::new()
            } else {
                vec![ChangeGroup {
                    file: OPS_FILE_ID.to_owned(),
                    group: OPS_FILE_ID.to_owned(),
                    count: changed as u32,
                }]
            },
            warnings: if unknown.is_empty() {
                Vec::new()
            } else {
                vec![ImportWarning::UnknownKeys {
                    file: OPS_FILE_ID.to_owned(),
                    keys: unknown,
                }]
            },
        })
    }

    fn apply(&self, document: &Value) -> Result<(), String> {
        let settings: OpsSettings =
            serde_json::from_value(document.clone()).map_err(|e| e.to_string())?;
        self.ops()?
            .set_settings(settings)
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}

/// The saved connections and recent servers, owned by the file system plugin. They hold no secret
/// (A81), so the export carries them whole; a remembered password stays in the keyring.
struct ConnectionsConfigFile(AppHandle<Wry>);

impl ConnectionsConfigFile {
    fn hub(&self) -> Result<Arc<waypoint_connections::ConnectionsHub>, String> {
        self.0
            .try_state::<tauri_plugin_waypoint_vfs::Vfs>()
            .and_then(|vfs| vfs.connections().cloned())
            .ok_or_else(|| "the saved connections are not kept here".to_owned())
    }
}

impl ConfigFile for ConnectionsConfigFile {
    fn id(&self) -> &str {
        waypoint_connections::CONNECTIONS_FILE_ID
    }

    fn export(&self) -> Result<Value, String> {
        serde_json::to_value(self.hub()?.export()).map_err(|e| e.to_string())
    }

    fn plan(&self, incoming: &Value) -> Result<FilePlan, BundleError> {
        let hub = self.hub().map_err(|message| BundleError::Invalid {
            file: waypoint_connections::CONNECTIONS_FILE_ID.to_owned(),
            message,
        })?;
        waypoint_connections::plan_connections(&hub.probe(), incoming)
    }

    fn apply(&self, document: &Value) -> Result<(), String> {
        let document: waypoint_connections::ConnectionsDocument =
            serde_json::from_value(document.clone()).map_err(|e| e.to_string())?;
        self.hub()?.replace_all(document).map_err(|e| e.to_string())
    }
}

/// The system's save and open dialogs, parented to the window that asked.
struct DialogPicker(AppHandle<Wry>);

impl FilePicker for DialogPicker {
    fn save(&self, window: &str, suggested_name: &str, kind: BundleKind) -> Option<PathBuf> {
        let mut dialog = self
            .0
            .dialog()
            .file()
            .set_file_name(suggested_name)
            .add_filter("Waypoint settings", &[kind.extension()]);
        if let Some(parent) = self.0.get_webview_window(window) {
            dialog = dialog.set_parent(&parent);
        }
        dialog.blocking_save_file()?.into_path().ok()
    }

    fn open(&self, window: &str) -> Option<PathBuf> {
        let mut dialog = self
            .0
            .dialog()
            .file()
            .add_filter("Waypoint settings", &["json", "zip"]);
        if let Some(parent) = self.0.get_webview_window(window) {
            dialog = dialog.set_parent(&parent);
        }
        dialog.blocking_pick_file()?.into_path().ok()
    }
}

/// Registers the operations settings and the saved connections with the export and gives it the dialogs. Call after the
/// plugins are set up.
pub fn wire(app: &AppHandle<Wry>) {
    let Some(transfers) = app.try_state::<Transfers>() else {
        return;
    };
    transfers.register(Arc::new(OpsConfigFile(app.clone())));
    transfers.register(Arc::new(ConnectionsConfigFile(app.clone())));
    transfers.set_picker(Arc::new(DialogPicker(app.clone())));
}
