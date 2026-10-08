// Tauri command handlers exposed to the webview: the status only, never a launch
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use tauri::{command, AppHandle, Runtime};

use crate::{models::PluginStatus, ElevateExt};

#[command]
pub(crate) fn get_status<R: Runtime>(app: AppHandle<R>) -> PluginStatus {
    app.elevate().status()
}
