// Exercises the plugin through Tauri's mock runtime over a fake application directory
//
// The mock runtime has no capability file, so the IPC layer would refuse plugin commands; these tests call the same `MimeApps` methods the commands delegate to, and check the JSON the commands would send. Nothing here starts an application, reads the real `mimeapps.list` or changes a default: every launch and every change is recorded by the fake.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![cfg(target_os = "linux")]

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde_json::json;
use tauri::test::{mock_builder, mock_context, noop_assets, MockRuntime};
use tauri::App;
use tauri_plugin_mime_apps::mimeapps::XdgEnv;
use tauri_plugin_mime_apps::{
    AppDirectory, AppEntry, DirectoryBackend, Flavour, MimeAppsError, MimeAppsExt, PluginStatus,
    Reason, FEATURES, FEATURE_CHOOSER, FEATURE_HANDLERS,
};

#[derive(Default)]
struct Record {
    launched: Vec<(String, Vec<String>)>,
    defaults_set: Vec<(String, String)>,
    failure: Option<MimeAppsError>,
}

#[derive(Clone)]
struct FakeDirectory {
    apps: Vec<AppEntry>,
    defaults: HashMap<String, String>,
    record: Arc<Mutex<Record>>,
}

fn app(id: &str, name: &str) -> AppEntry {
    AppEntry {
        id: id.into(),
        name: name.into(),
        icon: Some("icon".into()),
        exec: Some(format!("{name} %U")),
        hidden: false,
    }
}

impl FakeDirectory {
    fn find(&self, ids: &[&str]) -> Vec<AppEntry> {
        ids.iter()
            .filter_map(|id| self.apps.iter().find(|app| app.id == *id).cloned())
            .collect()
    }
}

impl AppDirectory for FakeDirectory {
    fn guess_type(&self, name: &str, _head: &[u8]) -> String {
        match name.rsplit_once('.').map(|(_, extension)| extension) {
            Some("png") => "image/png".into(),
            Some("txt") => "text/plain".into(),
            _ => "application/octet-stream".into(),
        }
    }

    fn describe_type(&self, mime: &str) -> (Option<String>, Option<String>) {
        match mime {
            "image/png" => (Some("PNG image".into()), Some("image-png".into())),
            _ => (None, None),
        }
    }

    fn all_apps(&self) -> Vec<AppEntry> {
        self.apps.clone()
    }

    fn all_for_type(&self, mime: &str) -> Vec<AppEntry> {
        match mime {
            "image/png" => self.find(&["gimp.desktop", "eog.desktop", "viewer.desktop"]),
            _ => Vec::new(),
        }
    }

    fn recommended_for_type(&self, mime: &str) -> Vec<AppEntry> {
        match mime {
            "image/png" => self.find(&["eog.desktop", "gimp.desktop"]),
            _ => Vec::new(),
        }
    }

    fn fallback_for_type(&self, _mime: &str) -> Vec<AppEntry> {
        Vec::new()
    }

    fn default_for_type(&self, mime: &str) -> Option<AppEntry> {
        let id = self.defaults.get(mime)?;
        self.find(&[id]).into_iter().next()
    }

    fn app(&self, id: &str) -> Option<AppEntry> {
        self.find(&[id]).into_iter().next()
    }

    fn launch(&self, id: &str, uris: &[String]) -> Result<(), MimeAppsError> {
        let mut record = self.record.lock().unwrap();
        if let Some(error) = record.failure.take() {
            return Err(error);
        }
        record.launched.push((id.to_string(), uris.to_vec()));
        Ok(())
    }

    fn set_default(&self, mime: &str, id: &str) -> Result<(), MimeAppsError> {
        self.record
            .lock()
            .unwrap()
            .defaults_set
            .push((mime.to_string(), id.to_string()));
        Ok(())
    }

    fn icon_png(&self, _id: &str, _size: u32) -> Option<Vec<u8>> {
        None
    }
}

struct Fixture {
    _tmp: tempfile::TempDir,
    config: std::path::PathBuf,
    record: Arc<Mutex<Record>>,
    app: App<MockRuntime>,
}

fn fixture() -> Fixture {
    let tmp = tempfile::tempdir().unwrap();
    let config = tmp.path().join("config");
    std::fs::create_dir_all(&config).unwrap();
    let record = Arc::new(Mutex::new(Record::default()));
    let directory = FakeDirectory {
        apps: vec![
            app("eog.desktop", "Image Viewer"),
            app("gimp.desktop", "GIMP"),
            app("viewer.desktop", "Viewer"),
            app("kate.desktop", "Kate"),
        ],
        defaults: HashMap::from([("image/png".to_string(), "eog.desktop".to_string())]),
        record: Arc::clone(&record),
    };
    // The injected directory stands in for the user's `mimeapps.list`; the real one is never read.
    let xdg = XdgEnv {
        config_home: config.clone(),
        config_dirs: Vec::new(),
        data_home: tmp.path().join("share"),
        data_dirs: Vec::new(),
        desktops: Vec::new(),
    };
    let backend = DirectoryBackend::new(directory, Some(xdg), || {
        let mut status = PluginStatus::build(
            Flavour::Gio,
            FEATURES
                .iter()
                .map(|name| {
                    if *name == FEATURE_CHOOSER {
                        tauri_plugin_mime_apps::FeatureStatus::unavailable(
                            name,
                            Reason::NoSystemChooser,
                            "no chooser",
                        )
                    } else {
                        tauri_plugin_mime_apps::FeatureStatus::available(name)
                    }
                })
                .collect(),
        );
        status.association_files = Vec::new();
        status
    });
    let app = mock_builder()
        .plugin(tauri_plugin_mime_apps::init_with(Arc::new(backend)))
        .build(mock_context(noop_assets()))
        .expect("the plugin should initialise");
    Fixture {
        _tmp: tmp,
        config,
        record,
        app,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn the_status_lists_every_feature_with_typed_reasons_and_the_files_read() {
    let fx = fixture();
    std::fs::write(fx.config.join("mimeapps.list"), "[Default Applications]\n").unwrap();
    let status = fx.app.mime_apps().get_status().await;
    assert!(status.available);
    let names: Vec<&str> = status.features.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, FEATURES);
    assert!(status.has(FEATURE_HANDLERS));
    assert_eq!(status.reason, Some(Reason::NoSystemChooser));
    assert_eq!(
        status.association_files,
        [fx.config.join("mimeapps.list").display().to_string()]
    );
    let json = serde_json::to_value(&status).unwrap();
    assert_eq!(json["flavour"], "gio");
    assert_eq!(json["features"][5]["name"], "chooser");
    assert_eq!(json["features"][5]["reason"], "no-system-chooser");
}

#[tokio::test(flavor = "multi_thread")]
async fn type_info_and_handlers_serialise_as_the_front_end_reads_them() {
    let fx = fixture();
    let info = fx
        .app
        .mime_apps()
        .type_info("file:///home/u/My%20Pics/cat.png", false)
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(&info).unwrap(),
        json!({ "mime": "image/png", "description": "PNG image", "icon": "image-png" })
    );

    let handlers = fx
        .app
        .mime_apps()
        .handlers(&["/home/u/cat.png".to_string()])
        .await
        .unwrap();
    let json = serde_json::to_value(&handlers).unwrap();
    assert_eq!(json["mime"], "image/png");
    assert_eq!(json["mixed"], false);
    assert_eq!(json["default"]["id"], "eog.desktop");
    assert_eq!(json["default"]["execHint"], "Image Viewer %U");
    assert_eq!(json["recommended"][0]["id"], "gimp.desktop");
    // What is registered for the type is not repeated; the rest of the installed applications follow by name.
    assert_eq!(json["others"][0]["id"], "kate.desktop");
    assert_eq!(json["others"][1]["id"], "viewer.desktop");
}

#[tokio::test(flavor = "multi_thread")]
async fn opening_records_the_launch_and_starts_nothing_else() {
    let fx = fixture();
    let mime_apps = fx.app.mime_apps();
    mime_apps
        .open_with(&["/home/u/a.png".into(), "/home/u/b.png".into()], "gimp")
        .await
        .unwrap();
    mime_apps
        .open_default(&["/home/u/c.png".into()])
        .await
        .unwrap();
    let record = fx.record.lock().unwrap();
    assert_eq!(
        record.launched,
        [
            (
                "gimp.desktop".to_string(),
                vec![
                    "file:///home/u/a.png".to_string(),
                    "file:///home/u/b.png".to_string()
                ]
            ),
            (
                "eog.desktop".to_string(),
                vec!["file:///home/u/c.png".to_string()]
            ),
        ]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn errors_are_typed_and_serialise_with_a_kind() {
    let fx = fixture();
    let mime_apps = fx.app.mime_apps();

    let no_handler = mime_apps
        .open_default(&["/home/u/notes.txt".into()])
        .await
        .unwrap_err();
    assert_eq!(
        serde_json::to_value(&no_handler).unwrap(),
        json!({ "kind": "noHandler", "mime": "text/plain" })
    );
    assert_eq!(
        mime_apps.open_with(&["/a.png".into()], "missing").await,
        Err(MimeAppsError::AppNotFound)
    );
    assert_eq!(mime_apps.handlers(&[]).await, Err(MimeAppsError::Empty));
    assert_eq!(
        mime_apps.type_info("relative/cat.png", false).await,
        Err(MimeAppsError::InvalidUri {
            uri: "relative/cat.png".into()
        })
    );
    fx.record.lock().unwrap().failure = Some(MimeAppsError::failed("exec failed"));
    assert_eq!(
        mime_apps.open_with(&["/a.png".into()], "gimp").await,
        Err(MimeAppsError::failed("exec failed"))
    );
    assert!(fx.record.lock().unwrap().launched.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn set_default_is_passed_through_and_the_system_pages_are_unsupported() {
    let fx = fixture();
    let mime_apps = fx.app.mime_apps();
    mime_apps.set_default("image/png", "gimp").await.unwrap();
    assert_eq!(
        fx.record.lock().unwrap().defaults_set,
        [("image/png".to_string(), "gimp.desktop".to_string())]
    );
    assert_eq!(
        mime_apps.choose(&["/a.png".into()], Some("main")).await,
        Err(MimeAppsError::Unsupported)
    );
    assert_eq!(
        mime_apps.open_default_apps_settings().await,
        Err(MimeAppsError::Unsupported)
    );
}
