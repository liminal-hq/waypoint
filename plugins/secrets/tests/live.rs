// Round trip against a real Secret Service, skipped with a message unless asked for and available
//
// Run it under a private session bus with an unlocked throwaway keyring, so it never touches a person's own:
//
//   dbus-run-session -- bash -c 'echo "" | gnome-keyring-daemon --unlock --components=secrets >/dev/null; SECRETS_LIVE_TEST=1 cargo test -p tauri-plugin-secrets --test live -- --nocapture'
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![cfg(target_os = "linux")]

use tauri_plugin_secrets::{Secret, SecretId, SecretKind, Secrets, FEATURE_STORE};

/// Items filed under a namespace of their own, so nothing else in the keyring is read or deleted.
const NAMESPACE: &str = "ca.liminalhq.secrets-live-test";

#[tokio::test]
async fn a_secret_round_trips_through_the_system_keyring() {
    if std::env::var_os("SECRETS_LIVE_TEST").is_none() {
        println!(
            "skipped: set SECRETS_LIVE_TEST=1 (under a private session bus) to use a real keyring"
        );
        return;
    }
    let secrets = Secrets::new(tauri_plugin_secrets::system_backend(NAMESPACE));
    let status = secrets.get_status().await;
    if !status.has(FEATURE_STORE) {
        println!(
            "skipped: no Secret Service is available here ({:?}: {})",
            status.reason,
            status.message.unwrap_or_default()
        );
        return;
    }
    println!("running against {:?}", status.flavour);

    let password = SecretId::new("live", "account", SecretKind::Password);
    let passphrase = SecretId::new("live", "account", SecretKind::Passphrase);
    let other = SecretId::new("live", "other", SecretKind::Password);
    secrets.delete_account("live", "account").await.unwrap();
    secrets.delete_account("live", "other").await.unwrap();

    assert!(!secrets.exists(&password).await.unwrap());
    assert!(secrets.fetch(&password).await.unwrap().is_none());

    secrets
        .store(
            &password,
            Some("Live test".into()),
            Secret::from_text("one".into()),
        )
        .await
        .unwrap();
    secrets
        .store(&passphrase, None, Secret::from_text("two".into()))
        .await
        .unwrap();
    secrets
        .store(&other, None, Secret::from_bytes(vec![0xff, 0x00, 0xfe]))
        .await
        .unwrap();

    assert!(secrets.exists(&password).await.unwrap());
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
        Some("two")
    );
    assert_eq!(
        secrets.fetch(&other).await.unwrap().unwrap().expose_bytes(),
        &[0xff, 0x00, 0xfe]
    );

    // Storing again replaces; it does not add a second item.
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

    assert!(secrets.delete(&password).await.unwrap());
    assert!(!secrets.delete(&password).await.unwrap());
    assert!(secrets.exists(&passphrase).await.unwrap());
    // Every kind of one account goes, and another account stays.
    assert_eq!(secrets.delete_account("live", "account").await.unwrap(), 1);
    assert!(secrets.exists(&other).await.unwrap());
    assert!(secrets.delete(&other).await.unwrap());
}
