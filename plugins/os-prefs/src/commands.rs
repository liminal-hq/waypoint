// Tauri command handlers exposed to the webview
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use tauri::{command, AppHandle, Runtime};

use crate::{
    error::Result,
    models::{AnimatorDurationScaleResponse, PluginStatus, TimeFormat},
    OsPrefsExt,
};

#[command]
pub(crate) async fn get_status<R: Runtime>(app: AppHandle<R>) -> Result<PluginStatus> {
    app.os_prefs().status().await
}

#[command]
pub(crate) async fn get_time_format<R: Runtime>(app: AppHandle<R>) -> Result<TimeFormat> {
    app.os_prefs().get_time_format().await
}

#[command]
pub(crate) async fn get_animator_duration_scale<R: Runtime>(
    app: AppHandle<R>,
) -> Result<AnimatorDurationScaleResponse> {
    app.os_prefs().get_animator_duration_scale().await
}

#[command]
pub(crate) async fn open_notification_settings<R: Runtime>(app: AppHandle<R>) -> Result<()> {
    app.os_prefs().open_notification_settings().await
}
