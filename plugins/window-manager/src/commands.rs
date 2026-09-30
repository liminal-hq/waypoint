// Implements IPC commands exposed by the window manager plugin
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use tauri::{Runtime, WebviewWindow};

use crate::{
    error::Error,
    models::{PluginStatus, WindowCapabilities, WindowPosition},
    platform,
};

#[tauri::command]
pub async fn get_status<R: Runtime>(window: WebviewWindow<R>) -> Result<PluginStatus, Error> {
    Ok(platform::capabilities(&window).await.status())
}

#[tauri::command]
pub async fn get_capabilities<R: Runtime>(
    window: WebviewWindow<R>,
) -> Result<WindowCapabilities, Error> {
    Ok(platform::capabilities(&window).await)
}

/// Asks the compositor to show its window menu for the invoking window; false if unsupported or refused.
#[tauri::command]
pub async fn show_system_window_menu<R: Runtime>(
    window: WebviewWindow<R>,
    position: WindowPosition,
) -> Result<bool, Error> {
    Ok(platform::show_system_window_menu(&window, position).await)
}

/// Whether the window manager is keeping the invoking window above others, read from the window manager rather than echoed from the last request. `None` where it cannot be observed, such as under Wayland.
#[tauri::command]
pub async fn get_always_on_top<R: Runtime>(
    window: WebviewWindow<R>,
) -> Result<Option<bool>, Error> {
    Ok(platform::always_on_top(&window).await)
}
