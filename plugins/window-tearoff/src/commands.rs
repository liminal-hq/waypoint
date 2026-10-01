// Implements IPC commands exposed by the window tear-off plugin
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use serde_json::Value;
use tauri::{AppHandle, Runtime, State, WebviewWindow};

use crate::{
    error::Error,
    models::{BeginReport, DropReport, Hit, Outcome, PluginStatus, Point, Region, Size},
    session::Tearoff,
};

/// Reports which tear-off features work on this system, and why the others do not. The first call probes the windowing system, which can take up to a second; call it at startup.
#[tauri::command]
pub async fn get_status<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, Tearoff>,
) -> Result<PluginStatus, Error> {
    Ok(state.status(&app).await)
}

/// Starts a drag: shows the ghost under the cursor with `payload` and follows the cursor until `end`. `grab_offset` is where inside the dragged thing the user grabbed it, in logical pixels from its top-left, and `size` is the ghost's logical size. Returns `noGhost` where no ghost can be shown; render a preview in the page instead.
#[tauri::command]
pub async fn begin<R: Runtime>(
    app: AppHandle<R>,
    window: WebviewWindow<R>,
    state: State<'_, Tearoff>,
    payload: Value,
    grab_offset: Point,
    size: Size,
) -> Result<BeginReport, Error> {
    Ok(state.begin(&app, &window, payload, grab_offset, size).await)
}

/// Replaces the ghost's payload while a drag is in progress.
#[tauri::command]
pub fn update<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, Tearoff>,
    payload: Value,
) -> Result<(), Error> {
    state.update(&app, payload);
    Ok(())
}

/// Ends the drag, hides the ghost, and reports the cursor and the registered region it was over.
#[tauri::command]
pub async fn end<R: Runtime>(
    app: AppHandle<R>,
    window: WebviewWindow<R>,
    state: State<'_, Tearoff>,
    outcome: Outcome,
) -> Result<DropReport, Error> {
    Ok(state.end(&app, &window, outcome).await)
}

/// Registers the calling window's drop regions (logical pixels from the content's top-left), replacing its earlier ones. They are forgotten when the window is destroyed.
#[tauri::command]
pub fn set_drop_regions<R: Runtime>(
    window: WebviewWindow<R>,
    state: State<'_, Tearoff>,
    regions: Vec<Region>,
) -> Result<(), Error> {
    state.set_regions(window.label(), regions);
    Ok(())
}

/// The native cursor in physical pixels, or null where the system does not report a usable one.
#[tauri::command]
pub async fn get_cursor<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, Tearoff>,
) -> Result<Option<Point>, Error> {
    Ok(state.cursor(&app).await)
}

/// The payload of the drag in progress, so a ghost page that loaded after `begin` can still draw it.
#[tauri::command]
pub fn get_payload(state: State<'_, Tearoff>) -> Result<Option<Value>, Error> {
    Ok(state.payload())
}

/// The registered region under the cursor right now, without ending the drag, so a caller can say what a release would do. Null where the system reports no usable cursor or cannot hit-test.
#[tauri::command]
pub async fn hit_test<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, Tearoff>,
) -> Result<Option<Hit>, Error> {
    Ok(state.peek_hit(&app).await)
}
