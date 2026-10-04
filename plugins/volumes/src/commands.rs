// Tauri command handlers exposed to the webview
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use tauri::{command, AppHandle, Runtime};

use crate::{
    error::VolumesError,
    models::{Passphrase, PluginStatus, Unlocked, Volume},
    VolumesExt,
};

#[command]
pub(crate) async fn get_status<R: Runtime>(app: AppHandle<R>) -> PluginStatus {
    app.volumes().get_status().await
}

#[command]
pub(crate) async fn list<R: Runtime>(
    app: AppHandle<R>,
    measure: Option<bool>,
) -> Result<Vec<Volume>, VolumesError> {
    app.volumes().list(measure.unwrap_or(false)).await
}

#[command]
pub(crate) async fn refresh_space<R: Runtime>(
    app: AppHandle<R>,
    id: String,
) -> Result<Volume, VolumesError> {
    app.volumes().refresh_space(&id).await
}

#[command]
pub(crate) async fn mount<R: Runtime>(
    app: AppHandle<R>,
    id: String,
) -> Result<String, VolumesError> {
    app.volumes().mount(&id).await
}

#[command]
pub(crate) async fn unmount<R: Runtime>(app: AppHandle<R>, id: String) -> Result<(), VolumesError> {
    app.volumes().unmount(&id).await
}

#[command]
pub(crate) async fn eject<R: Runtime>(app: AppHandle<R>, id: String) -> Result<(), VolumesError> {
    app.volumes().eject(&id).await
}

#[command]
pub(crate) async fn unlock<R: Runtime>(
    app: AppHandle<R>,
    id: String,
    passphrase: String,
    remember: Option<bool>,
) -> Result<Unlocked, VolumesError> {
    app.volumes()
        .unlock_and_remember(&id, Passphrase(passphrase), remember.unwrap_or(false))
        .await
}

#[command]
pub(crate) async fn forget<R: Runtime>(
    app: AppHandle<R>,
    id: String,
) -> Result<bool, VolumesError> {
    app.volumes().forget(&id).await
}
