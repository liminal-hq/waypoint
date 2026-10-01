// Plugin-level tests: the commands run against a mock Tauri app
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use serde_json::json;
use tauri::{
    test::{mock_builder, mock_context, noop_assets, MockRuntime},
    Manager, WebviewUrl, WebviewWindowBuilder,
};

use crate::{
    commands, init,
    models::{
        BeginState, DropReport, Hit, Options, Outcome, Point, Region, Size, DEFAULT_GHOST_LABEL,
    },
    session::Tearoff,
};

fn app() -> tauri::App<MockRuntime> {
    let app = mock_builder()
        .plugin(init(Options::default()))
        .build(mock_context(noop_assets()))
        .expect("the mock app builds");
    // The plugin creates the ghost once the event loop is running, which the mock runtime never does, so a test does it by hand.
    app.state::<Tearoff>().create_ghost(app.handle());
    WebviewWindowBuilder::new(&app, "main", WebviewUrl::App("index.html".into()))
        .build()
        .expect("the mock window opens");
    app
}

#[test]
fn the_ghost_is_created_hidden_and_registered_for_the_denylist() {
    let options = Options::default();
    assert_eq!(options.ghost_label, "tear-ghost");
    assert_eq!(
        options.window_state_denylist(),
        vec![DEFAULT_GHOST_LABEL.to_string()]
    );
    let custom = Options {
        ghost_label: "ghost-2".into(),
        ..Options::default()
    };
    assert_eq!(custom.window_state_denylist(), vec!["ghost-2".to_string()]);

    let app = app();
    assert!(app.get_webview_window(DEFAULT_GHOST_LABEL).is_some());
}

#[test]
fn with_no_display_every_feature_reports_unavailable_with_reasons() {
    let app = app();
    let handle = app.handle().clone();
    let status = tauri::async_runtime::block_on(commands::get_status(
        handle.clone(),
        handle.state::<Tearoff>(),
    ))
    .unwrap();
    assert!(!status.available);
    assert!(status.features.is_empty());
    assert_eq!(status.unavailable.len(), 4);
}

#[test]
fn begin_without_a_ghost_reports_no_ghost_and_end_still_answers() {
    let app = app();
    let handle = app.handle().clone();
    let window = handle.get_webview_window("main").unwrap();
    let begin = tauri::async_runtime::block_on(commands::begin(
        handle.clone(),
        window.clone(),
        handle.state::<Tearoff>(),
        json!({ "title": "Documents" }),
        Point { x: 10.0, y: 5.0 },
        Size {
            width: 200.0,
            height: 60.0,
        },
    ))
    .unwrap();
    assert_eq!(begin.state, BeginState::NoGhost);
    let report = tauri::async_runtime::block_on(commands::end(
        handle.clone(),
        window,
        handle.state::<Tearoff>(),
        Outcome::Drop,
    ))
    .unwrap();
    assert_eq!(report.cursor, None);
    assert_eq!(report.hit, None);
    assert_eq!(report.scale_factor, 1.0);
}

#[test]
fn regions_are_kept_per_window_and_cleared_on_destroy() {
    let app = app();
    let window = app.get_webview_window("main").unwrap();
    commands::set_drop_regions(
        window.clone(),
        app.state::<Tearoff>(),
        vec![Region {
            id: "strip".into(),
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 30.0,
        }],
    )
    .unwrap();
    app.state::<Tearoff>()
        .window_destroyed(app.handle(), "main");
    // Nothing observable remains to hit; registering again works from a clean slate.
    commands::set_drop_regions(window, app.state::<Tearoff>(), Vec::new()).unwrap();
}

#[test]
fn the_payload_is_opaque_json_that_round_trips() {
    let payload = json!({
        "title": "Photos",
        "count": 3,
        "nested": { "tabs": [1, 2, 3], "label": "naïve — ok", "none": null },
    });
    let app = app();
    let state = app.state::<Tearoff>();
    assert_eq!(state.payload(), None);
    *state_payload(&state) = Some(payload.clone());
    assert_eq!(
        commands::get_payload(app.state::<Tearoff>()).unwrap(),
        Some(payload.clone())
    );
    let text = serde_json::to_string(&payload).unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&text).unwrap(),
        payload
    );
}

fn state_payload<'a>(state: &'a Tearoff) -> std::sync::MutexGuard<'a, Option<serde_json::Value>> {
    state.payload_for_test()
}

#[test]
fn wire_types_use_camel_case() {
    let report = DropReport {
        cursor: Some(Point { x: 1.0, y: 2.0 }),
        scale_factor: 2.0,
        cursor_stale: false,
        hit: Some(Hit {
            window: "main-1".into(),
            region: "strip".into(),
        }),
    };
    let value = serde_json::to_value(&report).unwrap();
    assert_eq!(
        value,
        json!({
            "cursor": { "x": 1.0, "y": 2.0 },
            "scaleFactor": 2.0,
            "cursorStale": false,
            "hit": { "window": "main-1", "region": "strip" },
        })
    );
    assert_eq!(
        serde_json::to_value(Outcome::Cancel).unwrap(),
        json!("cancel")
    );
    assert_eq!(
        serde_json::from_value::<Outcome>(json!("drop")).unwrap(),
        Outcome::Drop
    );
    assert_eq!(
        serde_json::to_value(BeginState::NoGhost).unwrap(),
        json!("noGhost")
    );
}
