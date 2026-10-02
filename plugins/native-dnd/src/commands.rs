// Implements IPC commands exposed by the native-dnd plugin
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use tauri::{AppHandle, Runtime, WebviewWindow};

use crate::{
    error::Error,
    models::{ClipboardFiles, PluginStatus, StartDragReport, StartDragRequest},
    NativeDndExt,
};

/// Reports which native drag and drop features work on this system, and why the others do not.
#[tauri::command]
pub async fn get_status<R: Runtime>(app: AppHandle<R>) -> Result<PluginStatus, Error> {
    Ok(app.native_dnd().status().await)
}

/// Starts an outbound drag of `uris` from the calling window, which must be in the middle of a press of the primary mouse button; rejects with `buttonNotPressed` otherwise and with `alreadyActive` while another drag runs. On Linux the command resolves as soon as the drag has started and `native-dnd://drag-ended` reports its end; on Windows it resolves when the drag has finished.
#[tauri::command]
pub async fn start_drag<R: Runtime>(
    app: AppHandle<R>,
    window: WebviewWindow<R>,
    request: StartDragRequest,
) -> Result<StartDragReport, Error> {
    app.native_dnd().start_drag(&window, request).await
}

/// Puts `uris` on the system clipboard as files to copy or, with `cut`, to move. On Wayland the compositor accepts it only shortly after a key press or click in the app, so call it from the handler of that action.
#[tauri::command]
pub async fn set_files<R: Runtime>(app: AppHandle<R>, files: ClipboardFiles) -> Result<(), Error> {
    app.native_dnd().set_files(files).await
}

/// The files on the system clipboard, or null when it holds none.
#[tauri::command]
pub async fn get_files<R: Runtime>(app: AppHandle<R>) -> Result<Option<ClipboardFiles>, Error> {
    app.native_dnd().get_files().await
}
