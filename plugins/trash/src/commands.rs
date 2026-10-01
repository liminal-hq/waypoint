// Tauri command handlers exposed to the webview
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::PathBuf;

use tauri::{command, AppHandle, Runtime};

use crate::{
    error::TrashError,
    models::{EmptyReport, PluginStatus, RestoreTarget, TrashOutcome, TrashReceipt, TrashedItem},
    TrashExt,
};

#[command]
pub(crate) fn get_status<R: Runtime>(app: AppHandle<R>) -> PluginStatus {
    app.trash().get_status()
}

#[command]
pub(crate) async fn trash<R: Runtime>(app: AppHandle<R>, paths: Vec<PathBuf>) -> Vec<TrashOutcome> {
    app.trash()
        .trash(paths)
        .await
        .into_iter()
        .map(TrashOutcome::from)
        .collect()
}

#[command]
pub(crate) async fn list<R: Runtime>(app: AppHandle<R>) -> Result<Vec<TrashedItem>, TrashError> {
    app.trash().list().await
}

#[command]
pub(crate) async fn restore<R: Runtime>(
    app: AppHandle<R>,
    receipt: TrashReceipt,
    target: RestoreTarget,
) -> Result<TrashReceipt, TrashError> {
    app.trash().restore(&receipt, target).await
}

#[command]
pub(crate) async fn delete<R: Runtime>(
    app: AppHandle<R>,
    receipt: TrashReceipt,
) -> Result<(), TrashError> {
    app.trash().delete(&receipt).await
}

#[command]
pub(crate) async fn empty<R: Runtime>(
    app: AppHandle<R>,
    older_than_days: Option<u32>,
) -> Result<EmptyReport, TrashError> {
    app.trash().empty(older_than_days).await
}
