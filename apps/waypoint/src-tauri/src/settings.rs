// Wires the settings plugin to its file, and carries what the settings decide into the session
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The document is saved in `settings.json` under the key `settings`, beside the operations
// plugin's `ops` key in the same file (each key has exactly one writer). The settings decide two
// things before any window exists: whether the last session is restored or the app opens at Home,
// and which view a new window starts with. Both are read here, so the plugins stay unaware of
// each other.

use std::sync::Arc;

use serde_json::Value;
use tauri::{AppHandle, Manager, Wry};
use tauri_plugin_waypoint_session::Sessions;
use tauri_plugin_waypoint_settings::SettingsStore;
use waypoint_session::{ViewMode, ViewPrefs};
use waypoint_settings::{
    DefaultView, KeyValue, MemoryStorage, Persistence, Settings, SettingsStorage, StartupMode,
};

use crate::ops::SETTINGS_FILE;
use crate::storage::FileKeyValue;

/// The settings file's key-value view: the app's own file adapter under the crate's trait.
struct SettingsFile(FileKeyValue<Wry>);

impl KeyValue for SettingsFile {
    fn get(&self, key: &str) -> Option<Value> {
        crate::storage::KeyValue::get(&self.0, key)
    }

    fn set(&self, key: &str, value: Value) {
        crate::storage::KeyValue::set(&self.0, key, value);
    }

    fn save(&self) -> Result<(), String> {
        crate::storage::KeyValue::save(&self.0)
    }

    fn set_aside(&self) -> Option<String> {
        crate::storage::KeyValue::set_aside(&self.0)
    }

    fn was_unreadable(&self) -> bool {
        crate::storage::KeyValue::was_unreadable(&self.0)
    }
}

/// The settings plugin's storage, made once the app exists. A file that cannot be opened leaves the
/// settings in memory for this run, with a warning: they must never stop the app starting.
pub fn storage(app: &AppHandle<Wry>) -> Arc<dyn SettingsStorage> {
    match FileKeyValue::open_file(app, SETTINGS_FILE) {
        Ok(kv) => Arc::new(Persistence::new(SettingsFile(kv))),
        Err(e) => {
            log::warn!("could not open the settings file, settings will not be saved: {e}");
            Arc::new(MemoryStorage::default())
        }
    }
}

/// The view a new window starts with under `settings`.
pub fn view_for(settings: &Settings) -> ViewPrefs {
    ViewPrefs {
        mode: match settings.general.default_view {
            DefaultView::List => ViewMode::List,
            DefaultView::Grid => ViewMode::Grid,
        },
        show_hidden: settings.general.show_hidden_default,
        ..ViewPrefs::default()
    }
}

/// Whether the last session is brought back at start-up under `settings`.
pub fn restores_session(settings: &Settings) -> bool {
    settings.general.startup == StartupMode::RestoreSession
}

/// The settings in force, or the defaults when the plugin is not set up (a test app).
pub fn current(app: &AppHandle<Wry>) -> Settings {
    app.try_state::<SettingsStore<Wry>>()
        .map(|store| store.get())
        .unwrap_or_default()
}

/// Gives the session the view new windows start with now and after every change. Call before the
/// first window is made. The start-up choice is read when the app starts; changing it takes
/// effect at the next start.
pub fn wire(app: &AppHandle<Wry>) {
    let Some(sessions) = app.try_state::<Sessions<Wry>>() else {
        return;
    };
    sessions.set_new_window_view(view_for(&current(app)));
    if let Some(store) = app.try_state::<SettingsStore<Wry>>() {
        let handle = app.clone();
        store.on_change(move |settings| {
            handle
                .state::<Sessions<Wry>>()
                .set_new_window_view(view_for(settings));
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use waypoint_settings::{ClickMode, GeneralSettings};

    fn with(general: GeneralSettings) -> Settings {
        Settings {
            general,
            ..Settings::default()
        }
    }

    #[test]
    fn the_defaults_give_a_list_without_hidden_files_and_restore_the_session() {
        let defaults = Settings::default();
        assert_eq!(view_for(&defaults), ViewPrefs::default());
        assert!(restores_session(&defaults));
    }

    #[test]
    fn the_general_page_decides_the_view_and_the_start() {
        let settings = with(GeneralSettings {
            show_hidden_default: true,
            default_view: DefaultView::Grid,
            startup: StartupMode::Home,
            // Not a view choice: the browser reads it itself.
            click_mode: ClickMode::Single,
        });
        let view = view_for(&settings);
        assert_eq!(view.mode, ViewMode::Grid);
        assert!(view.show_hidden);
        assert_eq!(view.icon_size, ViewPrefs::default().icon_size);
        assert!(!restores_session(&settings));
    }

    /// Calls `command` (`plugin:name|command`) as the page of `window` would, through the IPC
    /// layer, so the app's real capability files decide.
    fn call(
        window: &tauri::WebviewWindow<tauri::test::MockRuntime>,
        command: &str,
        body: serde_json::Value,
    ) -> Result<tauri::ipc::InvokeResponseBody, Value> {
        tauri::test::get_ipc_response(
            window,
            tauri::webview::InvokeRequest {
                cmd: command.to_string(),
                callback: tauri::ipc::CallbackFn(0),
                error: tauri::ipc::CallbackFn(1),
                url: if cfg!(windows) {
                    "http://tauri.localhost"
                } else {
                    "tauri://localhost"
                }
                .parse()
                .unwrap(),
                body: tauri::ipc::InvokeBody::Json(body),
                headers: Default::default(),
                invoke_key: tauri::test::INVOKE_KEY.to_string(),
            },
        )
    }

    fn refused(window: &tauri::WebviewWindow<tauri::test::MockRuntime>, command: &str) -> bool {
        call(window, command, serde_json::json!({}))
            .err()
            .is_some_and(|e| e.to_string().contains("not allowed"))
    }

    #[test]
    fn the_settings_window_and_the_main_windows_use_the_settings_and_nobody_else_may() {
        use tauri::{WebviewUrl, WebviewWindowBuilder};
        let app = tauri::test::mock_builder()
            .plugin(tauri_plugin_waypoint_settings::init(Arc::new(
                MemoryStorage::default(),
            )))
            .build(tauri::generate_context!())
            .expect("the mock app builds with the app's own capabilities");
        let open = |label: &str| {
            WebviewWindowBuilder::new(&app, label, WebviewUrl::App("index.html".into()))
                .build()
                .expect("the mock window opens")
        };
        let body =
            serde_json::to_value(serde_json::json!({ "settings": Settings::default() })).unwrap();

        let editor = open("settings");
        assert!(call(
            &editor,
            "plugin:waypoint-settings|get_settings",
            serde_json::json!({})
        )
        .is_ok());
        assert!(call(
            &editor,
            "plugin:waypoint-settings|get_status",
            serde_json::json!({})
        )
        .is_ok());
        assert!(call(
            &editor,
            "plugin:waypoint-settings|set_settings",
            body.clone()
        )
        .is_ok());

        for label in ["main-1", "main-7"] {
            let window = open(label);
            for command in ["get_settings", "get_status"] {
                let answer = call(
                    &window,
                    &format!("plugin:waypoint-settings|{command}"),
                    serde_json::json!({}),
                );
                assert!(answer.is_ok(), "{label} may call {command}: {answer:?}");
            }
            // The Action bar's choices are saved from the main windows, and only those: the whole
            // document is the Settings window's, or a stale main window would turn its changes back.
            let ui = call(
                &window,
                "plugin:waypoint-settings|set_ui_settings",
                serde_json::json!({ "change": { "actionBar": false } }),
            );
            assert!(ui.is_ok(), "{label} may change the ui settings: {ui:?}");
            let whole = call(
                &window,
                "plugin:waypoint-settings|set_settings",
                body.clone(),
            )
            .expect_err("the whole document is refused");
            assert!(
                whole.to_string().contains("not allowed"),
                "{label} may not save the whole document: {whole}"
            );
        }

        for label in ["ops", "properties-1", "tear-ghost"] {
            let window = open(label);
            for command in [
                "get_settings",
                "get_status",
                "set_settings",
                "set_ui_settings",
            ] {
                assert!(
                    refused(&window, &format!("plugin:waypoint-settings|{command}")),
                    "{label} may not call {command}"
                );
            }
        }
    }
}
