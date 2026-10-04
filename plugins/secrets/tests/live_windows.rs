// Round trip against the real Credential Manager, for a Windows machine: ignored by default
//
// Run it on Windows with `cargo test -p tauri-plugin-secrets --test live_windows -- --ignored --nocapture`. It files its credentials under a test namespace and deletes them again; while it runs they show in Credential Manager under Windows Credentials.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![cfg(target_os = "windows")]

use tauri_plugin_secrets::{Secret, SecretId, SecretKind, Secrets, SecretsError, FEATURE_STORE};

const NAMESPACE: &str = "ca.liminalhq.secrets-live-test";

#[tokio::test]
#[ignore = "uses the real Credential Manager"]
async fn a_secret_round_trips_through_credential_manager() {
    let secrets = Secrets::new(tauri_plugin_secrets::system_backend(NAMESPACE));
    assert!(secrets.get_status().await.has(FEATURE_STORE));

    let password = SecretId::new("live", "account/with/slashes", SecretKind::Password);
    let passphrase = SecretId::new("live", "account/with/slashes", SecretKind::Passphrase);
    let other = SecretId::new("live", "other", SecretKind::Token);
    secrets
        .delete_account("live", "account/with/slashes")
        .await
        .unwrap();
    secrets.delete_account("live", "other").await.unwrap();

    assert!(!secrets.exists(&password).await.unwrap());
    secrets
        .store(
            &password,
            Some("Live test".into()),
            Secret::from_text("one".into()),
        )
        .await
        .unwrap();
    secrets
        .store(&passphrase, None, Secret::from_text("two ünïcode".into()))
        .await
        .unwrap();
    secrets
        .store(&other, None, Secret::from_bytes(vec![0xff, 0x00, 0xfe]))
        .await
        .unwrap();

    assert_eq!(
        secrets
            .fetch(&password)
            .await
            .unwrap()
            .unwrap()
            .expose_text(),
        Some("one")
    );
    assert_eq!(
        secrets
            .fetch(&passphrase)
            .await
            .unwrap()
            .unwrap()
            .expose_text(),
        Some("two ünïcode")
    );
    assert_eq!(
        secrets.fetch(&other).await.unwrap().unwrap().expose_bytes(),
        &[0xff, 0x00, 0xfe]
    );

    // Storing again replaces.
    secrets
        .store(&password, None, Secret::from_text("three".into()))
        .await
        .unwrap();
    assert_eq!(
        secrets
            .fetch(&password)
            .await
            .unwrap()
            .unwrap()
            .expose_text(),
        Some("three")
    );

    // A secret longer than Credential Manager keeps is refused, not cut.
    let long = Secret::from_bytes(vec![b'x'; 2561]);
    assert!(matches!(
        secrets.store(&password, None, long).await,
        Err(SecretsError::Invalid { .. })
    ));
    assert_eq!(
        secrets
            .fetch(&password)
            .await
            .unwrap()
            .unwrap()
            .expose_text(),
        Some("three")
    );

    assert!(secrets.delete(&password).await.unwrap());
    assert!(!secrets.delete(&password).await.unwrap());
    assert_eq!(
        secrets
            .delete_account("live", "account/with/slashes")
            .await
            .unwrap(),
        1
    );
    assert!(secrets.exists(&other).await.unwrap());
    assert!(secrets.delete(&other).await.unwrap());
}
