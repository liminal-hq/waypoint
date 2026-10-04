// Tauri command handlers exposed to the webview
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use tauri::{command, AppHandle, Runtime};

use crate::{
    error::SecretsError,
    models::{PluginStatus, Secret, SecretId},
    SecretsExt,
};

#[command]
pub(crate) async fn get_status<R: Runtime>(app: AppHandle<R>) -> PluginStatus {
    app.secrets().get_status().await
}

/// Stores a secret. The value crosses the IPC boundary here, once, and is moved into a buffer that is zeroed when dropped.
#[command]
pub(crate) async fn store<R: Runtime>(
    app: AppHandle<R>,
    id: SecretId,
    secret: String,
    label: Option<String>,
) -> Result<(), SecretsError> {
    app.secrets()
        .store(&id, label, Secret::from_text(secret))
        .await
}

/// Reads a secret back into the webview. Not in the plugin's default permission set.
#[command]
pub(crate) async fn fetch<R: Runtime>(
    app: AppHandle<R>,
    id: SecretId,
) -> Result<Option<String>, SecretsError> {
    match app.secrets().fetch(&id).await? {
        None => Ok(None),
        Some(secret) => match secret.expose_text() {
            Some(text) => Ok(Some(text.to_string())),
            None => Err(SecretsError::Invalid {
                field: "secret (not text)".into(),
            }),
        },
    }
}

#[command]
pub(crate) async fn exists<R: Runtime>(
    app: AppHandle<R>,
    id: SecretId,
) -> Result<bool, SecretsError> {
    app.secrets().exists(&id).await
}

#[command]
pub(crate) async fn delete<R: Runtime>(
    app: AppHandle<R>,
    id: SecretId,
) -> Result<bool, SecretsError> {
    app.secrets().delete(&id).await
}

#[command]
pub(crate) async fn delete_account<R: Runtime>(
    app: AppHandle<R>,
    service: String,
    account: String,
) -> Result<u32, SecretsError> {
    Ok(app.secrets().delete_account(&service, &account).await? as u32)
}
