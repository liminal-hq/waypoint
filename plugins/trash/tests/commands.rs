// Exercises the plugin through Tauri's mock runtime over a temporary freedesktop environment
//
// The mock runtime has no capability file, so the IPC layer would refuse plugin commands; these tests call the same `Trash` methods the commands delegate to, and check the JSON the commands would send. Nothing here touches the real `~/.local/share/Trash`.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![cfg(target_os = "linux")]

use std::path::PathBuf;

use serde_json::json;
use tauri::test::{mock_builder, mock_context, noop_assets, MockRuntime};
use tauri::App;
use tauri_plugin_trash::freedesktop::{system_clock, MountInfo, TrashEnv};
use tauri_plugin_trash::{
    Flavour, RestoreTarget, TrashError, TrashExt, TrashOutcome, FEATURE_EMPTY, FEATURE_EXPIRY,
    FEATURE_LIST, FEATURE_PER_VOLUME, FEATURE_RESTORE, FEATURE_TRASH,
};

struct Fixture {
    _tmp: tempfile::TempDir,
    root: PathBuf,
    app: App<MockRuntime>,
}

fn fixture() -> Fixture {
    let tmp = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(tmp.path()).unwrap();
    std::fs::create_dir_all(root.join("home")).unwrap();
    std::fs::create_dir_all(root.join("work")).unwrap();
    let env = TrashEnv {
        data_home: root.join("home/.local/share"),
        home_dir: root.join("home"),
        // The real user: the trash only trusts folders this user owns.
        // SAFETY: `geteuid` takes no arguments, cannot fail and has no side effects.
        uid: unsafe { libc::geteuid() },
        // The temporary directory is its own volume, on the same device as the home trash.
        mounts: vec![MountInfo {
            mount_point: root.clone(),
            device_id: 1,
            is_network: false,
            is_removable: false,
        }],
        now: system_clock(),
    };
    let app = mock_builder()
        .plugin(tauri_plugin_trash::init_with_env(env))
        .build(mock_context(noop_assets()))
        .expect("the plugin should initialise");
    Fixture {
        _tmp: tmp,
        root,
        app,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn the_status_lists_every_feature_and_the_flavour() {
    let fx = fixture();
    let status = fx.app.trash().get_status();
    assert!(status.available);
    assert_eq!(status.flavour, Flavour::Freedesktop);
    let names: Vec<&str> = status.features.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(
        names,
        [
            FEATURE_TRASH,
            FEATURE_LIST,
            FEATURE_RESTORE,
            FEATURE_EMPTY,
            FEATURE_EXPIRY,
            FEATURE_PER_VOLUME
        ]
    );
    assert!(status
        .features
        .iter()
        .all(|f| f.available && f.reason.is_none()));
    let json = serde_json::to_value(&status).unwrap();
    assert_eq!(json["flavour"], "freedesktop");
    assert_eq!(
        json["features"][0],
        json!({"name": "trash", "available": true, "reason": null})
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_batch_reports_one_outcome_per_path_and_the_rest_still_go() {
    let fx = fixture();
    let a = fx.root.join("work/a.txt");
    let b = fx.root.join("work/b.txt");
    std::fs::write(&a, "a").unwrap();
    std::fs::write(&b, "b").unwrap();
    let missing = fx.root.join("work/missing");
    let results = fx
        .app
        .trash()
        .trash(vec![
            a.clone(),
            missing,
            b.clone(),
            PathBuf::from("relative"),
        ])
        .await;
    assert_eq!(results.len(), 4);
    assert!(results[0].is_ok());
    assert_eq!(results[1], Err(TrashError::NotFound));
    assert!(results[2].is_ok());
    assert!(matches!(results[3], Err(TrashError::Io { .. })));
    assert!(!a.exists() && !b.exists());

    // What the command sends: tagged outcomes.
    let wire: Vec<TrashOutcome> = results.into_iter().map(TrashOutcome::from).collect();
    let json = serde_json::to_value(&wire).unwrap();
    assert_eq!(json[0]["status"], "trashed");
    assert_eq!(
        json[0]["receipt"]["originalPath"],
        a.to_string_lossy().as_ref()
    );
    assert_eq!(
        json[1],
        json!({"status": "failed", "error": {"kind": "notFound"}})
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn list_restore_delete_and_empty_work_through_the_handle() {
    let fx = fixture();
    let files: Vec<PathBuf> = ["one", "two", "three"]
        .iter()
        .map(|name| {
            let path = fx.root.join("work").join(name);
            std::fs::write(&path, name).unwrap();
            path
        })
        .collect();
    let receipts: Vec<_> = fx
        .app
        .trash()
        .trash(files.clone())
        .await
        .into_iter()
        .map(Result::unwrap)
        .collect();
    let listed = fx.app.trash().list().await.unwrap();
    assert_eq!(listed.len(), 3);
    let json = serde_json::to_value(&listed[0]).unwrap();
    assert!(json["receipt"]["trashId"].is_string());
    assert!(json["deletedAt"].is_i64());
    assert_eq!(json["isDir"], false);

    // Restore one; a second restore of it is not found; restoring onto a taken place says so.
    fx.app
        .trash()
        .restore(&receipts[0], RestoreTarget::Original)
        .await
        .unwrap();
    assert_eq!(std::fs::read_to_string(&files[0]).unwrap(), "one");
    assert_eq!(
        fx.app
            .trash()
            .restore(&receipts[0], RestoreTarget::Original)
            .await
            .unwrap_err(),
        TrashError::NotFound
    );
    std::fs::write(&files[1], "taken").unwrap();
    let error = fx
        .app
        .trash()
        .restore(&receipts[1], RestoreTarget::Original)
        .await
        .unwrap_err();
    assert_eq!(
        error,
        TrashError::OriginExists {
            path: files[1].clone()
        }
    );
    let wire = serde_json::to_value(&error).unwrap();
    assert_eq!(wire["kind"], "originExists");
    assert_eq!(wire["path"], files[1].to_string_lossy().as_ref());
    // The caller picks somewhere else.
    let elsewhere = fx.root.join("work/two-restored");
    fx.app
        .trash()
        .restore(
            &receipts[1],
            RestoreTarget::Path {
                path: elsewhere.clone(),
            },
        )
        .await
        .unwrap();
    assert_eq!(std::fs::read_to_string(elsewhere).unwrap(), "two");

    fx.app.trash().delete(&receipts[2]).await.unwrap();
    assert!(fx.app.trash().list().await.unwrap().is_empty());
    // Empty, with and without an age, on a trash with something in it.
    let again = fx
        .app
        .trash()
        .trash(vec![files[0].clone()])
        .await
        .remove(0)
        .unwrap();
    assert_eq!(fx.app.trash().empty(Some(7)).await.unwrap().removed, 0);
    let report = fx.app.trash().empty(None).await.unwrap();
    assert_eq!(report.removed, 1);
    assert_eq!(
        fx.app.trash().delete(&again).await.unwrap_err(),
        TrashError::NotFound
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn commands_take_and_return_the_json_the_guest_api_sends() {
    // The guest API sends `{ kind: 'original' }` / `{ kind: 'path', path }` and reads `{ kind: ... }` errors.
    let target: RestoreTarget = serde_json::from_value(json!({"kind": "original"})).unwrap();
    assert_eq!(target, RestoreTarget::Original);
    let target: RestoreTarget =
        serde_json::from_value(json!({"kind": "path", "path": "/x/y"})).unwrap();
    assert_eq!(
        target,
        RestoreTarget::Path {
            path: PathBuf::from("/x/y")
        }
    );
    for (error, expected) in [
        (TrashError::NotFound, json!({"kind": "notFound"})),
        (
            TrashError::PermissionDenied,
            json!({"kind": "permissionDenied"}),
        ),
        (
            TrashError::TrashUnavailable { reason: "r".into() },
            json!({"kind": "trashUnavailable", "reason": "r"}),
        ),
        (
            TrashError::OriginMissingParent { path: "/p".into() },
            json!({"kind": "originMissingParent", "path": "/p"}),
        ),
        (TrashError::Unsupported, json!({"kind": "unsupported"})),
        (
            TrashError::Io {
                message: "m".into(),
            },
            json!({"kind": "io", "message": "m"}),
        ),
        (
            TrashError::Refused {
                reason: "no".into(),
            },
            json!({"kind": "refused", "reason": "no"}),
        ),
    ] {
        assert_eq!(serde_json::to_value(&error).unwrap(), expected);
        assert_eq!(
            serde_json::from_value::<TrashError>(expected).unwrap(),
            error
        );
    }
}
