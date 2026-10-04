// Exercises the plugin through Tauri's mock runtime over the in-memory keyring: the managed state, the typed failures and the JSON the commands send
//
// The mock runtime has no capability file, so the IPC layer would refuse plugin commands; these tests call the same `Secrets` methods the commands delegate to. Nothing here touches a real keyring.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::sync::Arc;

use serde_json::json;
use tauri::test::{mock_builder, mock_context, noop_assets, MockRuntime};
use tauri::App;
use tauri_plugin_secrets::{
    MemoryBackend, PluginStatus, Reason, Secret, SecretId, SecretKind, SecretsError, SecretsExt,
};

fn app(backend: Arc<MemoryBackend>) -> App<MockRuntime> {
    mock_builder()
        .plugin(tauri_plugin_secrets::init_with(backend))
        .build(mock_context(noop_assets()))
        .expect("the app builds")
}

#[tokio::test]
async fn the_plugin_manages_one_secrets_service_over_the_backend() {
    let backend = Arc::new(MemoryBackend::new());
    let app = app(backend.clone());
    let id = SecretId::new("sftp", "home", SecretKind::Password);
    app.secrets()
        .store(
            &id,
            Some("Home server".into()),
            Secret::from_text("pw".into()),
        )
        .await
        .unwrap();
    assert_eq!(backend.label_of(&id), Some("Home server".into()));
    let got = app.secrets().fetch(&id).await.unwrap().unwrap();
    assert_eq!(got.expose_text(), Some("pw"));
    assert!(app.secrets().get_status().await.available);
}

#[tokio::test]
async fn a_missing_keyring_is_a_typed_failure_and_a_status_with_a_reason() {
    let backend = Arc::new(MemoryBackend::new());
    backend.fail_with(Some(SecretsError::NoKeyring));
    let app = app(backend);
    let id = SecretId::new("sftp", "home", SecretKind::Password);
    let error = app
        .secrets()
        .store(&id, None, Secret::from_text("pw".into()))
        .await
        .unwrap_err();
    assert_eq!(error, SecretsError::NoKeyring);
    assert_eq!(
        serde_json::to_value(&error).unwrap(),
        json!({ "kind": "noKeyring" })
    );
    let status: PluginStatus = app.secrets().get_status().await;
    assert!(!status.available);
    assert_eq!(status.reason, Some(Reason::NoKeyring));
}

#[test]
fn ids_and_status_serialise_in_camel_case_and_a_secret_is_not_serialisable() {
    let id = SecretId::new("volume", "uuid", SecretKind::Passphrase);
    assert_eq!(
        serde_json::to_value(&id).unwrap(),
        json!({ "service": "volume", "account": "uuid", "kind": "passphrase" })
    );
    // `Secret` has no `Serialize` impl: this would not compile if it had one and a test wrote it out.
    let secret = Secret::from_text("hunter2".into());
    assert!(!format!("{secret:?}").contains("hunter2"));
}
