// Plugin-level tests: the commands and the handle run against a mock Tauri app
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use tauri::{
    test::{mock_builder, mock_context, noop_assets, MockRuntime},
    Listener, Manager, WebviewUrl, WebviewWindowBuilder,
};

use crate::{
    models::{ClipboardFiles, DisplayServer, DragAction, ErrorKind, StartDragRequest, ENTER_EVENT},
    NativeDndExt,
};

fn app() -> tauri::App<MockRuntime> {
    let app = mock_builder()
        .plugin(crate::init())
        .build(mock_context(noop_assets()))
        .expect("the mock app builds");
    WebviewWindowBuilder::new(&app, "main", WebviewUrl::App("index.html".into()))
        .build()
        .expect("the mock window opens");
    app
}

fn request() -> StartDragRequest {
    StartDragRequest {
        uris: vec!["file:///t/a".into()],
        actions: vec![DragAction::Copy],
        icon: None,
    }
}

#[test]
fn the_handle_is_managed_and_nothing_is_dragging() {
    let app = app();
    assert!(!app.native_dnd().outbound_active());
}

// Windows always has a display, so its commands do not fail the way a headless Linux's do.
#[test]
#[cfg_attr(windows, ignore = "Windows always has a display")]
fn with_no_display_everything_is_unavailable_with_a_reason() {
    let app = app();
    let status = tauri::async_runtime::block_on(app.native_dnd().status());
    assert!(!status.available);
    assert_eq!(status.display_server, DisplayServer::None);
    assert!(status.reason.is_some());
    assert!(!status.features.outbound.available);
    assert!(status.features.clipboard.reason.is_some());
}

#[test]
#[cfg_attr(windows, ignore = "Windows always has a display")]
fn with_no_display_the_commands_return_unsupported() {
    let app = app();
    let window = app.get_webview_window("main").unwrap();
    let dnd = app.native_dnd();
    let drag = tauri::async_runtime::block_on(dnd.start_drag(&window, request()));
    assert_eq!(drag.unwrap_err().kind(), ErrorKind::Unsupported);
    let set = tauri::async_runtime::block_on(dnd.set_files(ClipboardFiles {
        uris: vec!["file:///t/a".into()],
        cut: false,
    }));
    assert_eq!(set.unwrap_err().kind(), ErrorKind::Unsupported);
    let got = tauri::async_runtime::block_on(dnd.get_files());
    assert_eq!(got.unwrap_err().kind(), ErrorKind::Unsupported);
}

#[test]
fn a_bad_clipboard_request_is_invalid() {
    let app = app();
    let dnd = app.native_dnd();
    let empty = tauri::async_runtime::block_on(dnd.set_files(ClipboardFiles {
        uris: vec![],
        cut: true,
    }));
    assert_eq!(empty.unwrap_err().kind(), ErrorKind::Invalid);
    let web = tauri::async_runtime::block_on(dnd.set_files(ClipboardFiles {
        uris: vec!["https://example.com/a".into()],
        cut: false,
    }));
    assert_eq!(web.unwrap_err().kind(), ErrorKind::Invalid);
}

#[test]
fn errors_reach_the_page_as_a_kind_and_a_message() {
    let json = serde_json::to_value(crate::Error::ButtonNotPressed).unwrap();
    assert_eq!(json["kind"], "buttonNotPressed");
    assert!(json["message"].as_str().unwrap().contains("button"));
    let json = serde_json::to_value(crate::Error::AlreadyActive).unwrap();
    assert_eq!(json["kind"], "alreadyActive");
    let json = serde_json::to_value(crate::Error::KeysHeld).unwrap();
    assert_eq!(json["kind"], "keysHeld");
    assert_eq!(json["message"], crate::outbound::KEY_AFTER_PRESS);
    let json = serde_json::to_value(crate::Error::Unsupported("why".into())).unwrap();
    assert_eq!(json["kind"], "unsupported");
}

#[test]
fn a_drag_drop_event_reaches_the_window_normalised() {
    let app = app();
    let (tx, rx) = std::sync::mpsc::channel();
    let window = app.get_webview_window("main").unwrap();
    window.listen(ENTER_EVENT, move |event| {
        let _ = tx.send(event.payload().to_string());
    });
    let handle = app.handle().clone();
    crate::on_drag_drop(
        &handle,
        "main",
        &tauri::DragDropEvent::Enter {
            paths: vec![std::path::PathBuf::from(if cfg!(windows) {
                r"C:\t\a b.txt"
            } else {
                "/t/a b.txt"
            })],
            position: tauri::PhysicalPosition::new(10.0, 20.0),
        },
    );
    let payload: serde_json::Value = serde_json::from_str(
        &rx.recv_timeout(std::time::Duration::from_secs(5))
            .expect("an enter event"),
    )
    .unwrap();
    assert_eq!(payload["window"], "main");
    assert_eq!(payload["modifiers"]["ctrl"], false);
    assert!(payload["action"].is_null());
    let uri = payload["uris"][0].as_str().unwrap();
    assert!(
        uri.starts_with("file:///") && uri.contains("a%20b.txt"),
        "{uri}"
    );
}
