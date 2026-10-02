// Tauri command handlers exposed to the webview
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use tauri::{command, AppHandle, Runtime};

use crate::{
    error::WindowEffectsError,
    models::{Effects, Insets, PluginStatus},
    WindowEffectsExt,
};

#[command]
pub(crate) async fn get_status<R: Runtime>(app: AppHandle<R>) -> PluginStatus {
    app.window_effects().get_status().await
}

#[command]
pub(crate) async fn apply<R: Runtime>(
    app: AppHandle<R>,
    label: String,
    effects: Effects,
) -> Result<(), WindowEffectsError> {
    app.window_effects().apply(&label, effects).await
}

#[command]
pub(crate) async fn clear<R: Runtime>(
    app: AppHandle<R>,
    label: String,
) -> Result<(), WindowEffectsError> {
    app.window_effects().clear(&label).await
}

#[command]
pub(crate) async fn set_shadow_inset<R: Runtime>(
    app: AppHandle<R>,
    label: String,
    insets: Insets,
) -> Result<(), WindowEffectsError> {
    app.window_effects().set_shadow_inset(&label, insets).await
}
