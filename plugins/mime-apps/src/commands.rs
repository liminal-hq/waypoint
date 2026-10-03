// Tauri command handlers exposed to the webview
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use tauri::{command, AppHandle, Runtime};

use crate::{
    error::MimeAppsError,
    models::{Handlers, PluginStatus, TypeInfo},
    MimeAppsExt,
};

#[command]
pub(crate) async fn get_status<R: Runtime>(app: AppHandle<R>) -> PluginStatus {
    app.mime_apps().get_status().await
}

#[command]
pub(crate) async fn type_info<R: Runtime>(
    app: AppHandle<R>,
    uri: String,
    sniff: Option<bool>,
) -> Result<TypeInfo, MimeAppsError> {
    app.mime_apps()
        .type_info(&uri, sniff.unwrap_or(false))
        .await
}

#[command]
pub(crate) async fn handlers<R: Runtime>(
    app: AppHandle<R>,
    uris: Vec<String>,
) -> Result<Handlers, MimeAppsError> {
    app.mime_apps().handlers(&uris).await
}

#[command]
pub(crate) async fn open_with<R: Runtime>(
    app: AppHandle<R>,
    uris: Vec<String>,
    app_id: String,
) -> Result<(), MimeAppsError> {
    app.mime_apps().open_with(&uris, &app_id).await
}

#[command]
pub(crate) async fn open_default<R: Runtime>(
    app: AppHandle<R>,
    uris: Vec<String>,
) -> Result<(), MimeAppsError> {
    app.mime_apps().open_default(&uris).await
}

#[command]
pub(crate) async fn choose<R: Runtime>(
    app: AppHandle<R>,
    uris: Vec<String>,
    parent_label: Option<String>,
) -> Result<(), MimeAppsError> {
    app.mime_apps().choose(&uris, parent_label.as_deref()).await
}

#[command]
pub(crate) async fn set_default<R: Runtime>(
    app: AppHandle<R>,
    mime: String,
    app_id: String,
) -> Result<(), MimeAppsError> {
    app.mime_apps().set_default(&mime, &app_id).await
}

#[command]
pub(crate) async fn open_default_apps_settings<R: Runtime>(
    app: AppHandle<R>,
) -> Result<(), MimeAppsError> {
    app.mime_apps().open_default_apps_settings().await
}

#[command]
pub(crate) fn refresh_type_icons<R: Runtime>(app: AppHandle<R>) {
    app.mime_apps().refresh_type_icons();
}
