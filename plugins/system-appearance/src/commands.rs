// Implements IPC commands exposed by the system appearance plugin
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use tauri::{AppHandle, Runtime, State};

use crate::{
    error::Error,
    models::{PluginStatus, TitlebarSnapshot},
    service::Service,
};

#[tauri::command]
pub async fn get_status<R: Runtime>(
    app: AppHandle<R>,
    service: State<'_, Service>,
) -> Result<PluginStatus, Error> {
    Ok(service.refresh(&app).await.snapshot.status)
}

#[tauri::command]
pub async fn get_titlebar_preferences<R: Runtime>(
    app: AppHandle<R>,
    service: State<'_, Service>,
) -> Result<TitlebarSnapshot, Error> {
    Ok(service.refresh(&app).await.titlebar())
}
