// A missing session bus is the typed `NoKeyring` failure, never a hang or a panic
//
// This is its own test binary because it changes the process's environment.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![cfg(target_os = "linux")]

use tauri_plugin_secrets::{Reason, SecretId, SecretKind, Secrets, SecretsError};

#[tokio::test]
async fn without_a_session_bus_the_failure_is_no_keyring() {
    // Pointing the bus address at nothing must give the typed reason, never a hang or a panic.
    std::env::set_var(
        "DBUS_SESSION_BUS_ADDRESS",
        "unix:path=/nonexistent/secrets-test-bus",
    );
    let secrets = Secrets::new(tauri_plugin_secrets::system_backend("org.example.test"));
    let status = secrets.get_status().await;
    assert!(!status.available);
    assert_eq!(
        status.reason,
        Some(Reason::NoKeyring),
        "{:?}",
        status.message
    );
    let id = SecretId::new("live", "account", SecretKind::Password);
    assert_eq!(
        secrets.fetch(&id).await.unwrap_err(),
        SecretsError::NoKeyring
    );
}
