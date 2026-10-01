// Plugin-level tests: the store and its commands run against a mock Tauri app with real windows
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::json;
use tauri::test::{mock_builder, mock_context, noop_assets, MockRuntime};
use tauri::{Listener, Manager, WebviewUrl, WebviewWindowBuilder};
use waypoint_settings::{
    DefaultView, DropActionRule, MemoryStorage, Settings, SettingsDocument, SettingsSnapshot,
    SettingsStorage, StorageError,
};

use crate::{commands, init, Error, SettingsStore, EVENT};

type App = tauri::App<MockRuntime>;

fn app_with(storage: Arc<dyn SettingsStorage>) -> App {
    let app = mock_builder()
        .plugin(init(storage))
        .build(mock_context(noop_assets()))
        .expect("the mock app builds");
    for label in ["main-1", "main-2", "settings"] {
        WebviewWindowBuilder::new(&app, label, WebviewUrl::App("index.html".into()))
            .build()
            .expect("the mock window opens");
    }
    app
}

fn store(app: &App) -> tauri::State<'_, SettingsStore<MockRuntime>> {
    app.state::<SettingsStore<MockRuntime>>()
}

/// Collects the snapshots a window hears.
fn hear(app: &App, label: &str) -> Arc<Mutex<Vec<SettingsSnapshot>>> {
    let heard = Arc::new(Mutex::new(Vec::new()));
    let sink = heard.clone();
    app.get_webview_window(label)
        .unwrap()
        .listen(EVENT, move |event| {
            sink.lock()
                .unwrap()
                .push(serde_json::from_str(event.payload()).expect("a snapshot"));
        });
    heard
}

fn grid() -> Settings {
    let mut settings = Settings::default();
    settings.general.default_view = DefaultView::Grid;
    settings
}

#[test]
fn a_fresh_app_starts_at_the_defaults_and_revision_zero() {
    let app = app_with(Arc::new(MemoryStorage::default()));
    assert_eq!(
        store(&app).snapshot(),
        SettingsSnapshot {
            revision: 0,
            settings: Settings::default()
        }
    );
}

#[test]
fn saved_settings_load_and_an_out_of_range_stored_value_is_brought_into_range() {
    let storage = MemoryStorage::default();
    let mut body = grid();
    body.dnd.spring_load_ms = 90_000;
    storage.save(&SettingsDocument::new(body)).unwrap();
    let app = app_with(Arc::new(storage));
    let loaded = store(&app).get();
    assert_eq!(loaded.general.default_view, DefaultView::Grid);
    assert_eq!(loaded.dnd.spring_load_ms, 2000);
}

fn window(app: &App) -> tauri::WebviewWindow<MockRuntime> {
    app.get_webview_window("settings").unwrap()
}

fn set(app: &App, settings: Settings) -> Result<SettingsSnapshot, Error> {
    tauri::async_runtime::block_on(commands::set_settings(window(app), store(app), settings))
}

#[test]
fn set_saves_bumps_the_revision_and_returns_what_is_in_force() {
    let storage = Arc::new(MemoryStorage::default());
    let app = app_with(storage.clone());
    let answer = set(&app, grid()).unwrap();
    assert_eq!(answer.revision, 1);
    assert_eq!(answer.settings, grid());
    assert_eq!(storage.saved(), Some(SettingsDocument::new(grid())));
    let read =
        tauri::async_runtime::block_on(commands::get_settings(window(&app), store(&app))).unwrap();
    assert_eq!(read, answer);
}

#[test]
fn every_window_hears_each_change_with_a_rising_revision() {
    let app = app_with(Arc::new(MemoryStorage::default()));
    let heard: Vec<_> = ["main-1", "main-2", "settings"]
        .iter()
        .map(|label| hear(&app, label))
        .collect();
    set(&app, grid()).unwrap();
    let mut second = grid();
    second.dnd.default_action_rule = DropActionRule::AlwaysCopy;
    set(&app, second).unwrap();
    for window in heard {
        let revisions: Vec<u64> = window.lock().unwrap().iter().map(|s| s.revision).collect();
        assert_eq!(revisions, vec![1, 2]);
        assert_eq!(window.lock().unwrap()[1].settings, second);
    }
}

#[test]
fn setting_what_is_already_in_force_is_quiet() {
    let storage = Arc::new(MemoryStorage::default());
    let app = app_with(storage.clone());
    let heard = hear(&app, "main-1");
    let answer = set(&app, Settings::default()).unwrap();
    assert_eq!(answer.revision, 0);
    assert!(heard.lock().unwrap().is_empty());
    assert_eq!(storage.saved(), None, "nothing was written");
}

#[test]
fn a_value_out_of_range_is_refused_with_its_field_and_range_and_changes_nothing() {
    let storage = Arc::new(MemoryStorage::default());
    let app = app_with(storage.clone());
    let heard = hear(&app, "main-1");
    let mut bad = grid();
    bad.dnd.spring_load_ms = 50;
    let refused = set(&app, bad).unwrap_err();
    assert!(matches!(refused, Error::Invalid(_)));
    assert_eq!(
        serde_json::to_value(&refused).unwrap(),
        json!({
            "kind": "invalid",
            "message": "dnd.springLoadMs must be between 200 and 2000",
            "field": "dnd.springLoadMs",
            "min": 200,
            "max": 2000,
        })
    );
    assert_eq!(store(&app).snapshot().revision, 0);
    assert_eq!(store(&app).get(), Settings::default());
    assert_eq!(storage.saved(), None);
    assert!(heard.lock().unwrap().is_empty());
}

/// A storage that cannot save.
struct Failing;

impl SettingsStorage for Failing {
    fn load(&self) -> Result<Option<SettingsDocument>, StorageError> {
        Ok(None)
    }
    fn save(&self, _: &SettingsDocument) -> Result<(), StorageError> {
        Err(StorageError::Io("the disk is full".into()))
    }
}

#[test]
fn a_save_that_fails_changes_nothing_and_says_so() {
    let app = app_with(Arc::new(Failing));
    let heard = hear(&app, "main-1");
    let refused = set(&app, grid()).unwrap_err();
    assert!(matches!(refused, Error::Storage(_)));
    let wire = serde_json::to_value(&refused).unwrap();
    assert_eq!(wire["kind"], "storage");
    assert!(wire["message"]
        .as_str()
        .unwrap()
        .contains("the disk is full"));
    assert_eq!(store(&app).get(), Settings::default());
    assert!(heard.lock().unwrap().is_empty());
}

/// A storage that cannot load.
struct Unreadable;

impl SettingsStorage for Unreadable {
    fn load(&self) -> Result<Option<SettingsDocument>, StorageError> {
        Err(StorageError::Io("no such file".into()))
    }
    fn save(&self, _: &SettingsDocument) -> Result<(), StorageError> {
        Ok(())
    }
}

#[test]
fn settings_that_cannot_be_read_start_the_app_at_the_defaults() {
    let app = app_with(Arc::new(Unreadable));
    assert_eq!(store(&app).get(), Settings::default());
}

#[test]
fn a_hook_hears_changes_but_not_refusals_or_no_ops() {
    let app = app_with(Arc::new(MemoryStorage::default()));
    let seen = Arc::new(Mutex::new(Vec::new()));
    let sink = seen.clone();
    store(&app).on_change(move |settings| sink.lock().unwrap().push(*settings));
    set(&app, Settings::default()).unwrap();
    let mut bad = grid();
    bad.dnd.spring_load_ms = 1;
    set(&app, bad).unwrap_err();
    set(&app, grid()).unwrap();
    assert_eq!(*seen.lock().unwrap(), vec![grid()]);
}

#[test]
fn a_hook_runs_after_the_new_settings_are_readable() {
    let app = app_with(Arc::new(MemoryStorage::default()));
    let handle = app.handle().clone();
    let consistent = Arc::new(AtomicBool::new(false));
    let flag = consistent.clone();
    store(&app).on_change(move |settings| {
        let now = handle.state::<SettingsStore<MockRuntime>>().get();
        flag.store(now == *settings, Ordering::SeqCst);
    });
    set(&app, grid()).unwrap();
    assert!(consistent.load(Ordering::SeqCst));
}

#[test]
fn the_status_command_reports_the_plugin_available() {
    let status = tauri::async_runtime::block_on(commands::get_status());
    assert!(status.available);
    assert_eq!(status.features, vec!["settings", "events"]);
}
