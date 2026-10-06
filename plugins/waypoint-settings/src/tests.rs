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
    set(&app, second.clone()).unwrap();
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
    store(&app).on_change(move |settings| sink.lock().unwrap().push(settings.clone()));
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

#[test]
fn a_ui_change_keeps_every_other_setting_that_is_in_force() {
    let app = app_with(Arc::new(MemoryStorage::default()));
    let heard = hear(&app, "main-1");
    // The Settings window changes the default view; a main window that has not heard of it yet
    // then hides the Action bar.
    store(&app).set(grid()).unwrap();
    let after = store(&app)
        .update_ui(|ui| ui.action_bar = false)
        .expect("a ui change is accepted");
    assert_eq!(after.revision, 2);
    assert!(!after.settings.ui.action_bar);
    assert!(
        after.settings.ui.action_bar_labels,
        "a field not named stays"
    );
    assert_eq!(after.settings.general.default_view, DefaultView::Grid);
    assert_eq!(store(&app).snapshot(), after);
    assert_eq!(heard.lock().unwrap().last().cloned(), Some(after));
}

#[test]
fn a_ui_change_that_changes_nothing_makes_no_revision() {
    let app = app_with(Arc::new(MemoryStorage::default()));
    let before = store(&app).snapshot();
    assert_eq!(store(&app).update_ui(|_| {}).unwrap(), before);
}

// Export and import

use std::path::{Path, PathBuf};

use serde_json::Value;
use waypoint_settings::{
    change_groups, differing_paths, BundleError, BundleKind, ConfigFile, ExportReceipt, FilePlan,
    ImportPreview, MAX_TOTAL_BYTES,
};

use crate::{FilePicker, Transfers};

/// A picker that answers with what the test says, and remembers what it was asked.
#[derive(Default)]
struct Picker {
    save_to: Mutex<Option<PathBuf>>,
    open: Mutex<Option<PathBuf>>,
    asked: Mutex<Vec<String>>,
}

impl FilePicker for Picker {
    fn save(&self, window: &str, suggested_name: &str, kind: BundleKind) -> Option<PathBuf> {
        self.asked.lock().unwrap().push(format!(
            "save {window} {suggested_name} {}",
            kind.extension()
        ));
        self.save_to.lock().unwrap().clone()
    }
    fn open(&self, window: &str) -> Option<PathBuf> {
        self.asked.lock().unwrap().push(format!("open {window}"));
        self.open.lock().unwrap().clone()
    }
}

/// A storage that remembers each save and each time the replaced document was kept.
#[derive(Default)]
struct Recording {
    inner: MemoryStorage,
    log: Mutex<Vec<&'static str>>,
}

impl SettingsStorage for Recording {
    fn load(&self) -> Result<Option<SettingsDocument>, StorageError> {
        self.inner.load()
    }
    fn save(&self, document: &SettingsDocument) -> Result<(), StorageError> {
        self.log.lock().unwrap().push("save");
        self.inner.save(document)
    }
    fn keep_as_previous(&self) {
        self.log.lock().unwrap().push("keep");
    }
}

fn transfers(app: &App) -> tauri::State<'_, Transfers> {
    app.state::<Transfers>()
}

fn with_picker(app: &App) -> Arc<Picker> {
    let picker = Arc::new(Picker::default());
    transfers(app).set_picker(picker.clone());
    picker
}

fn export(app: &App, picker: &Picker, to: &Path) -> Result<Option<ExportReceipt>, Error> {
    *picker.save_to.lock().unwrap() = Some(to.to_path_buf());
    tauri::async_runtime::block_on(commands::export_settings(window(app), transfers(app), -240))
}

fn plan(app: &App, picker: &Picker, file: &Path) -> Result<Option<ImportPreview>, Error> {
    *picker.open.lock().unwrap() = Some(file.to_path_buf());
    tauri::async_runtime::block_on(commands::plan_settings_import(window(app), transfers(app)))
}

fn apply(app: &App, plan_id: u64) -> Result<SettingsSnapshot, Error> {
    tauri::async_runtime::block_on(commands::apply_settings_import(
        window(app),
        store(app),
        transfers(app),
        plan_id,
    ))
}

fn kind_of(error: &Error) -> String {
    serde_json::to_value(error).unwrap()["kind"]
        .as_str()
        .unwrap()
        .to_owned()
}

/// Another configuration file, kept in memory (the operations settings are one in the app).
struct Extra {
    value: Mutex<Value>,
    fail: Mutex<bool>,
}

impl Extra {
    fn new(value: Value) -> Arc<Self> {
        Arc::new(Self {
            value: Mutex::new(value),
            fail: Mutex::new(false),
        })
    }
}

impl ConfigFile for Extra {
    fn id(&self) -> &str {
        "ops"
    }
    fn export(&self) -> Result<Value, String> {
        Ok(self.value.lock().unwrap().clone())
    }
    fn plan(&self, incoming: &Value) -> Result<FilePlan, BundleError> {
        let now = self.value.lock().unwrap().clone();
        Ok(FilePlan {
            document: incoming.clone(),
            changes: change_groups("ops", &differing_paths(&now, incoming), None),
            warnings: Vec::new(),
        })
    }
    fn apply(&self, document: &Value) -> Result<(), String> {
        if *self.fail.lock().unwrap() {
            return Err("no room".to_owned());
        }
        *self.value.lock().unwrap() = document.clone();
        Ok(())
    }
}

#[test]
fn export_asks_where_then_writes_one_json_for_the_one_file() {
    let app = app_with(Arc::new(MemoryStorage::default()));
    transfers(&app).unregister("folder-views");
    set(&app, grid()).unwrap();
    let picker = with_picker(&app);
    let dir = tempfile::tempdir().unwrap();
    let receipt = export(&app, &picker, &dir.path().join("mine.json"))
        .unwrap()
        .unwrap();
    assert_eq!(receipt.kind, BundleKind::Json);
    assert_eq!(receipt.files, ["settings"]);
    assert_eq!(receipt.path, dir.path().join("mine.json").to_string_lossy());
    let written: Value = serde_json::from_slice(&std::fs::read(&receipt.path).unwrap()).unwrap();
    assert_eq!(written["format"], "waypoint-settings");
    assert_eq!(
        written["files"]["settings"]["body"]["general"]["defaultView"],
        "grid"
    );
    let asked = picker.asked.lock().unwrap().clone();
    assert_eq!(asked.len(), 1);
    assert!(
        asked[0].starts_with("save settings waypoint-settings-20"),
        "{asked:?}"
    );
    assert!(asked[0].ends_with(".json json"));
    assert_eq!(
        std::fs::read_dir(dir.path()).unwrap().count(),
        1,
        "no temporary file is left"
    );
}

#[test]
fn export_of_several_files_is_a_zip_and_a_name_without_an_extension_gets_one() {
    let app = app_with(Arc::new(MemoryStorage::default()));
    transfers(&app).register(Extra::new(serde_json::json!({ "concurrency": 2 })));
    let picker = with_picker(&app);
    let dir = tempfile::tempdir().unwrap();
    let receipt = export(&app, &picker, &dir.path().join("mine"))
        .unwrap()
        .unwrap();
    assert_eq!(receipt.kind, BundleKind::Zip);
    assert_eq!(receipt.files, ["settings", "folder-views", "ops"]);
    assert!(receipt.path.ends_with("mine.zip"));
    assert!(std::fs::read(&receipt.path)
        .unwrap()
        .starts_with(b"PK\x03\x04"));
}

#[test]
fn a_closed_dialog_exports_and_imports_nothing() {
    let app = app_with(Arc::new(MemoryStorage::default()));
    let picker = with_picker(&app);
    let dir = tempfile::tempdir().unwrap();
    *picker.save_to.lock().unwrap() = None;
    let answer =
        tauri::async_runtime::block_on(commands::export_settings(window(&app), transfers(&app), 0))
            .unwrap();
    assert!(answer.is_none());
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    *picker.open.lock().unwrap() = None;
    let planned = tauri::async_runtime::block_on(commands::plan_settings_import(
        window(&app),
        transfers(&app),
    ))
    .unwrap();
    assert!(planned.is_none());
}

#[test]
fn without_a_dialog_export_and_import_say_so() {
    let app = app_with(Arc::new(MemoryStorage::default()));
    let refused =
        tauri::async_runtime::block_on(commands::export_settings(window(&app), transfers(&app), 0))
            .unwrap_err();
    assert_eq!(kind_of(&refused), "unavailable");
    let refused = tauri::async_runtime::block_on(commands::plan_settings_import(
        window(&app),
        transfers(&app),
    ))
    .unwrap_err();
    assert_eq!(kind_of(&refused), "unavailable");
}

#[test]
fn a_round_trip_through_a_file_restores_the_settings_after_a_reset() {
    let app = app_with(Arc::new(MemoryStorage::default()));
    transfers(&app).unregister("folder-views");
    let mut mine = grid();
    mine.dnd.spring_load_ms = 1100;
    set(&app, mine.clone()).unwrap();
    let picker = with_picker(&app);
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("backup.json");
    export(&app, &picker, &file).unwrap();
    set(&app, Settings::default()).unwrap();

    let preview = plan(&app, &picker, &file).unwrap().unwrap();
    assert_eq!(preview.plan.kind, BundleKind::Json);
    assert_eq!(preview.plan.files, ["settings"]);
    let groups: Vec<(&str, u32)> = preview
        .plan
        .changes
        .iter()
        .map(|c| (c.group.as_str(), c.count))
        .collect();
    assert_eq!(groups, [("dnd", 1), ("general", 1)]);
    assert_eq!(
        store(&app).get(),
        Settings::default(),
        "planning changed nothing"
    );

    let answer = apply(&app, preview.plan_id).unwrap();
    assert_eq!(answer.settings, mine);
    assert_eq!(store(&app).get(), mine);
}

#[test]
fn applying_is_one_revision_one_event_and_keeps_the_replaced_document_as_previous() {
    let storage = Arc::new(Recording::default());
    let app = app_with(storage.clone());
    let picker = with_picker(&app);
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("backup.json");
    set(&app, grid()).unwrap();
    export(&app, &picker, &file).unwrap();
    set(&app, Settings::default()).unwrap();
    let before = store(&app).snapshot().revision;
    let heard = hear(&app, "main-1");
    storage.log.lock().unwrap().clear();

    let preview = plan(&app, &picker, &file).unwrap().unwrap();
    assert!(
        storage.log.lock().unwrap().is_empty(),
        "the plan saved nothing"
    );
    apply(&app, preview.plan_id).unwrap();

    assert_eq!(
        *storage.log.lock().unwrap(),
        ["keep", "save"],
        "kept, then saved, once"
    );
    let heard = heard.lock().unwrap();
    assert_eq!(heard.len(), 1, "one event");
    assert_eq!(heard[0].revision, before + 1);
    assert_eq!(heard[0].settings, grid());
}

#[test]
fn importing_what_is_in_force_keeps_nothing_and_says_nothing() {
    let storage = Arc::new(Recording::default());
    let app = app_with(storage.clone());
    let picker = with_picker(&app);
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("same.json");
    export(&app, &picker, &file).unwrap();
    let heard = hear(&app, "main-1");
    let preview = plan(&app, &picker, &file).unwrap().unwrap();
    assert!(preview.plan.changes.is_empty());
    apply(&app, preview.plan_id).unwrap();
    assert!(storage.log.lock().unwrap().is_empty());
    assert!(heard.lock().unwrap().is_empty());
}

#[test]
fn a_plan_is_replaced_by_the_next_one_and_spent_when_used() {
    let app = app_with(Arc::new(MemoryStorage::default()));
    let picker = with_picker(&app);
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a.json");
    set(&app, grid()).unwrap();
    export(&app, &picker, &file).unwrap();
    set(&app, Settings::default()).unwrap();
    let first = plan(&app, &picker, &file).unwrap().unwrap();
    let second = plan(&app, &picker, &file).unwrap().unwrap();
    assert_ne!(first.plan_id, second.plan_id);
    for stale in [first.plan_id, second.plan_id + 1, 0] {
        assert_eq!(kind_of(&apply(&app, stale).unwrap_err()), "stale");
    }
    // The wrong numbers left the real plan alone, and using it spends it.
    apply(&app, second.plan_id).unwrap();
    assert_eq!(kind_of(&apply(&app, second.plan_id).unwrap_err()), "stale");
}

#[test]
fn apply_checks_the_kept_file_again_against_what_is_in_force_now() {
    let app = app_with(Arc::new(MemoryStorage::default()));
    let picker = with_picker(&app);
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a.json");
    set(&app, grid()).unwrap();
    export(&app, &picker, &file).unwrap();
    set(&app, Settings::default()).unwrap();
    let preview = plan(&app, &picker, &file).unwrap().unwrap();
    // Another window changes something between the plan and the apply; the file's settings win
    // whole, because that is what the person confirmed replacing theirs with.
    let mut other = Settings::default();
    other.dnd.spring_load_ms = 700;
    set(&app, other).unwrap();
    let answer = apply(&app, preview.plan_id).unwrap();
    assert_eq!(answer.settings, grid());
}

#[test]
fn a_file_with_a_value_a_normal_change_would_refuse_is_refused_with_its_field() {
    let app = app_with(Arc::new(MemoryStorage::default()));
    let picker = with_picker(&app);
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("bad.json");
    let mut value = serde_json::to_value(SettingsDocument::new(Settings::default())).unwrap();
    value["body"]["dnd"]["springLoadMs"] = serde_json::json!(5);
    let bytes = waypoint_settings::export_bundle(
        &[waypoint_settings::ExportFile {
            id: "settings".into(),
            document: value,
        }],
        &waypoint_settings::ExportMeta {
            app_version: "0".into(),
            exported_at_unix: 0,
            local_offset_minutes: 0,
        },
    )
    .unwrap()
    .bytes;
    std::fs::write(&file, bytes).unwrap();
    let refused = plan(&app, &picker, &file).unwrap_err();
    let wire = serde_json::to_value(&refused).unwrap();
    assert_eq!(wire["kind"], "transfer");
    assert_eq!(wire["reason"], "invalid");
    assert!(wire["message"]
        .as_str()
        .unwrap()
        .contains("dnd.springLoadMs"));
}

#[test]
fn files_that_cannot_be_imported_are_refused_plainly() {
    let app = app_with(Arc::new(MemoryStorage::default()));
    let picker = with_picker(&app);
    let dir = tempfile::tempdir().unwrap();
    let reason = |bytes: &[u8]| {
        let file = dir.path().join("f");
        std::fs::write(&file, bytes).unwrap();
        let wire = serde_json::to_value(plan(&app, &picker, &file).unwrap_err()).unwrap();
        assert_eq!(wire["kind"], "transfer", "{wire}");
        wire["reason"].as_str().unwrap().to_owned()
    };
    assert_eq!(reason(b"just some text"), "not-a-bundle");
    assert_eq!(
        reason(b"{\"format\": \"waypoint-settings\", \"ver"),
        "corrupt"
    );
    assert_eq!(reason(b"PK\x03\x04 and then nothing useful"), "corrupt");
    assert_eq!(
        reason(br#"{"format":"waypoint-settings","version":9,"files":{}}"#),
        "newer-format"
    );
    assert_eq!(reason(&vec![b' '; MAX_TOTAL_BYTES + 1]), "too-large");
    // A folder is not a file, and a file that is not there is an I/O error.
    assert_eq!(
        kind_of(&plan(&app, &picker, dir.path()).unwrap_err()),
        "transfer"
    );
    assert_eq!(
        kind_of(&plan(&app, &picker, &dir.path().join("gone")).unwrap_err()),
        "io"
    );
    assert_eq!(store(&app).get(), Settings::default());
}

#[test]
fn a_failure_part_way_through_several_files_changes_none_of_them() {
    let app = app_with(Arc::new(MemoryStorage::default()));
    let ops = Extra::new(serde_json::json!({ "concurrency": 2 }));
    transfers(&app).register(ops.clone());
    let picker = with_picker(&app);
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("all.zip");
    set(&app, grid()).unwrap();
    *ops.value.lock().unwrap() = serde_json::json!({ "concurrency": 6 });
    export(&app, &picker, &file).unwrap();
    set(&app, Settings::default()).unwrap();
    *ops.value.lock().unwrap() = serde_json::json!({ "concurrency": 2 });

    let preview = plan(&app, &picker, &file).unwrap().unwrap();
    assert_eq!(preview.plan.kind, BundleKind::Zip);
    assert_eq!(preview.plan.files, ["settings", "folder-views", "ops"]);
    *ops.fail.lock().unwrap() = true;
    let refused = apply(&app, preview.plan_id).unwrap_err();
    assert_eq!(kind_of(&refused), "apply");
    assert_eq!(
        store(&app).get(),
        Settings::default(),
        "the settings were put back"
    );
    assert_eq!(
        *ops.value.lock().unwrap(),
        serde_json::json!({ "concurrency": 2 })
    );

    // With the disk back, the same file imports both.
    *ops.fail.lock().unwrap() = false;
    let preview = plan(&app, &picker, &file).unwrap().unwrap();
    apply(&app, preview.plan_id).unwrap();
    assert_eq!(store(&app).get(), grid());
    assert_eq!(
        *ops.value.lock().unwrap(),
        serde_json::json!({ "concurrency": 6 })
    );
}

// Remembered folder views

use waypoint_session::ViewMode;
use waypoint_settings::{
    FolderViewPatch, FolderViewsChanged, FolderViewsDocument, FolderViewsStorage, MemoryFolderViews,
};

use crate::{FolderViewsStore, FOLDER_VIEWS_EVENT};

fn app_with_views(views: Arc<dyn FolderViewsStorage>) -> App {
    let app = mock_builder()
        .plugin(crate::init_with_folder_views(
            |_| Arc::new(MemoryStorage::default()),
            move |_| views,
        ))
        .build(mock_context(noop_assets()))
        .expect("the mock app builds");
    for label in ["main-1", "main-2", "settings"] {
        WebviewWindowBuilder::new(&app, label, WebviewUrl::App("index.html".into()))
            .build()
            .expect("the mock window opens");
    }
    app
}

fn views(app: &App) -> tauri::State<'_, FolderViewsStore<MockRuntime>> {
    app.state::<FolderViewsStore<MockRuntime>>()
}

fn hear_views(app: &App, label: &str) -> Arc<Mutex<Vec<FolderViewsChanged>>> {
    let heard = Arc::new(Mutex::new(Vec::new()));
    let sink = heard.clone();
    app.get_webview_window(label)
        .unwrap()
        .listen(FOLDER_VIEWS_EVENT, move |event| {
            sink.lock()
                .unwrap()
                .push(serde_json::from_str(event.payload()).expect("a change"));
        });
    heard
}

fn grid_view() -> FolderViewPatch {
    FolderViewPatch {
        mode: Some(ViewMode::Grid),
        sort: None,
        ..Default::default()
    }
}

#[test]
fn a_remembered_folder_is_saved_announced_to_every_window_and_read_back() {
    let storage = Arc::new(MemoryFolderViews::default());
    let app = app_with_views(storage.clone());
    let one = hear_views(&app, "main-1");
    let two = hear_views(&app, "main-2");

    let revision = views(&app).remember("file:///a", grid_view()).unwrap();
    assert_eq!(revision, 1);
    for heard in [&one, &two] {
        let heard = heard.lock().unwrap();
        assert_eq!(heard.len(), 1);
        assert_eq!(heard[0].revision, 1);
        assert_eq!(heard[0].changes[0].key, "file:///a");
    }
    assert_eq!(storage.saved().unwrap().folders.len(), 1);
    let snapshot =
        tauri::async_runtime::block_on(commands::get_folder_views(window(&app), views(&app)))
            .unwrap();
    assert_eq!(snapshot.revision, 1);
    assert_eq!(snapshot.folders[0].view.mode, Some(ViewMode::Grid));
}

#[test]
fn a_folders_column_widths_are_saved_announced_and_dropped_with_the_last_choice() {
    use waypoint_settings::ListColumnWidths;

    let storage = Arc::new(MemoryFolderViews::default());
    let app = app_with_views(storage.clone());
    let heard = hear_views(&app, "main-2");
    let widths = |size| FolderViewPatch {
        column_widths: Some(ListColumnWidths {
            size,
            ..Default::default()
        }),
        ..Default::default()
    };

    assert_eq!(
        views(&app)
            .remember("file:///a", widths(Some(120)))
            .unwrap(),
        1
    );
    let saved = storage.saved().unwrap();
    assert_eq!(saved.folders[0].view.column_widths.unwrap().size, Some(120));

    // Every column back to its own forgets the folder, as nothing else is remembered for it.
    assert_eq!(views(&app).remember("file:///a", widths(None)).unwrap(), 2);
    assert!(storage.saved().unwrap().folders.is_empty());
    let heard = heard.lock().unwrap();
    assert_eq!(heard.len(), 2);
    assert_eq!(heard[1].changes[0].view, None);

    // A width outside the bound is refused and changes nothing.
    assert!(views(&app).remember("file:///a", widths(Some(5))).is_err());
}

#[test]
fn remembering_the_same_thing_again_is_no_revision_event_or_save() {
    let storage = Arc::new(MemoryFolderViews::default());
    let app = app_with_views(storage.clone());
    views(&app).remember("file:///a", grid_view()).unwrap();
    let heard = hear_views(&app, "main-1");
    assert_eq!(views(&app).remember("file:///a", grid_view()).unwrap(), 1);
    assert_eq!(views(&app).forget("file:///elsewhere").unwrap(), 1);
    assert!(heard.lock().unwrap().is_empty());
}

#[test]
fn the_commands_remember_and_reset_a_folder() {
    let app = app_with_views(Arc::new(MemoryFolderViews::default()));
    let heard = hear_views(&app, "main-1");
    tauri::async_runtime::block_on(commands::remember_folder_view(
        window(&app),
        views(&app),
        "file:///a".into(),
        grid_view(),
    ))
    .unwrap();
    let revision = tauri::async_runtime::block_on(commands::reset_folder_view(
        window(&app),
        views(&app),
        "file:///a".into(),
    ))
    .unwrap();
    assert_eq!(revision, 2);
    let heard = heard.lock().unwrap();
    assert_eq!(heard.len(), 2);
    assert_eq!(heard[1].changes[0].view, None);
    assert!(views(&app).snapshot().folders.is_empty());
}

#[test]
fn a_location_that_cannot_be_one_is_refused_as_invalid_and_changes_nothing() {
    let app = app_with_views(Arc::new(MemoryFolderViews::default()));
    let refused = views(&app).remember("", grid_view()).unwrap_err();
    assert_eq!(kind_of(&refused), "invalid");
    assert_eq!(views(&app).snapshot().revision, 0);
}

struct FailingViews;

impl FolderViewsStorage for FailingViews {
    fn load(&self) -> Result<Option<FolderViewsDocument>, StorageError> {
        Ok(None)
    }
    fn save(&self, _: &FolderViewsDocument) -> Result<(), StorageError> {
        Err(StorageError::Io("disk full".into()))
    }
}

#[test]
fn a_save_that_fails_changes_nothing_and_says_nothing() {
    let app = app_with_views(Arc::new(FailingViews));
    let heard = hear_views(&app, "main-1");
    let refused = views(&app).remember("file:///a", grid_view()).unwrap_err();
    assert_eq!(kind_of(&refused), "storage");
    assert_eq!(views(&app).snapshot().revision, 0);
    assert!(views(&app).snapshot().folders.is_empty());
    assert!(heard.lock().unwrap().is_empty());
}

#[test]
fn what_was_saved_is_there_when_the_app_starts_again() {
    let storage = Arc::new(MemoryFolderViews::default());
    let first = app_with_views(storage.clone());
    views(&first).remember("file:///a", grid_view()).unwrap();
    drop(first);
    let second = app_with_views(storage);
    let snapshot = views(&second).snapshot();
    assert_eq!(
        snapshot.revision, 0,
        "a revision is per run, like the settings'"
    );
    assert_eq!(snapshot.folders[0].key, "file:///a");
}

#[test]
fn the_folder_views_travel_in_the_settings_export_and_come_back_through_an_import() {
    let app = app_with_views(Arc::new(MemoryFolderViews::default()));
    views(&app).remember("file:///a", grid_view()).unwrap();
    views(&app).remember("file:///b", grid_view()).unwrap();
    let picker = with_picker(&app);
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("backup.zip");
    let receipt = export(&app, &picker, &file).unwrap().unwrap();
    assert_eq!(receipt.files, ["settings", "folder-views"]);

    views(&app).forget("file:///a").unwrap();
    views(&app)
        .remember(
            "file:///c",
            FolderViewPatch {
                mode: Some(ViewMode::List),
                sort: None,
                ..Default::default()
            },
        )
        .unwrap();
    let preview = plan(&app, &picker, &file).unwrap().unwrap();
    let folders = preview
        .plan
        .changes
        .iter()
        .find(|c| c.file == "folder-views")
        .expect("the folders are a group of the plan");
    assert_eq!((folders.group.as_str(), folders.count), ("folders", 2));

    let heard = hear_views(&app, "main-2");
    apply(&app, preview.plan_id).unwrap();
    let keys: Vec<String> = views(&app)
        .snapshot()
        .folders
        .into_iter()
        .map(|e| e.key)
        .collect();
    assert_eq!(keys, ["file:///a", "file:///b"]);
    let heard = heard.lock().unwrap();
    assert_eq!(heard.len(), 1, "an import is one change, one event");
}
