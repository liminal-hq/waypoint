// Implements IPC commands exposed by the system appearance plugin
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use tauri::{AppHandle, Runtime, State};

use crate::{
    appearance::{models::AppearancePreferences, service::AppearanceService},
    error::Error,
    models::{PluginStatus, TitlebarSnapshot},
    palette::{models::Palette, service::PaletteService},
    service::Service,
};

#[tauri::command]
pub async fn get_status<R: Runtime>(
    app: AppHandle<R>,
    service: State<'_, Service>,
    appearance: State<'_, AppearanceService>,
    palette: State<'_, PaletteService>,
) -> Result<PluginStatus, Error> {
    let titlebar = service.refresh(&app).await.snapshot.status;
    let appearance = appearance.refresh(&app).await.status;
    let palette = palette.refresh(&app).await.status;
    Ok(titlebar.with_appearance(appearance).with_palette(palette))
}

#[tauri::command]
pub async fn get_titlebar_preferences<R: Runtime>(
    app: AppHandle<R>,
    service: State<'_, Service>,
) -> Result<TitlebarSnapshot, Error> {
    Ok(service.refresh(&app).await.titlebar())
}

#[tauri::command]
pub async fn get_appearance<R: Runtime>(
    app: AppHandle<R>,
    service: State<'_, AppearanceService>,
) -> Result<AppearancePreferences, Error> {
    Ok(service.refresh(&app).await.preferences)
}

#[tauri::command]
pub async fn get_palette<R: Runtime>(
    app: AppHandle<R>,
    service: State<'_, PaletteService>,
) -> Result<Palette, Error> {
    Ok(service.refresh(&app).await)
}
