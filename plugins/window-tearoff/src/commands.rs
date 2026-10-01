// Implements IPC commands exposed by the window tear-off plugin
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use serde_json::Value;
use tauri::{AppHandle, Runtime, State, WebviewWindow};

use crate::{
    error::Error,
    models::{
        BeginReport, DropReport, Hit, Outcome, PluginStatus, Point, Region, Size,
        ToplevelBeginReport, ToplevelDragEnded,
    },
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

/// Drags the window labelled `window_label` with the pointer through the compositor (`toplevel_drag`), from the press in the calling window; the window follows the pointer outside every window, stays where it is dropped and snaps back on cancel. The window may be the caller itself, or another one created hidden for the drag. `payload` is opaque JSON that a window it is dropped on receives as `payload-dropped`. `grab_offset` is where the pointer holds the window, in logical pixels from its top-left. The caller gets `toplevel-drag-started` once the compositor has taken the drag, then no pointer events until the drag ends; `toplevel-drag-ended` (also sent to the dragged window) says how it ended.
#[tauri::command]
pub async fn begin_toplevel_drag<R: Runtime>(
    app: AppHandle<R>,
    window: WebviewWindow<R>,
    state: State<'_, Tearoff>,
    payload: Value,
    window_label: String,
    grab_offset: Point,
) -> Result<ToplevelBeginReport, Error> {
    Ok(state
        .begin_toplevel_drag(&app, &window, window_label, payload, grab_offset)
        .await)
}

/// Cancels the toplevel drag in progress, if any; it ends as `cancelled`.
#[tauri::command]
pub async fn end_toplevel_drag<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, Tearoff>,
) -> Result<(), Error> {
    state.end_toplevel_drag(&app).await;
    Ok(())
}

/// How the toplevel drag that moved the calling window ended, if it ended and the window has not read it. For a page that was still loading when the drag ended.
#[tauri::command]
pub fn take_toplevel_drag_result<R: Runtime>(
    window: WebviewWindow<R>,
    state: State<'_, Tearoff>,
) -> Result<Option<ToplevelDragEnded>, Error> {
    Ok(state.take_toplevel_result(window.label()))
}
