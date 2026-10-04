// The secrets service: validates what the app or the webview asks for and delegates to the keyring backend
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::sync::Arc;

use crate::backend::Backend;
use crate::error::{Result, SecretsError};
use crate::models::{PluginStatus, Secret, SecretId};

/// The plugin's Rust API, cheap to clone. The app wraps it in whatever trait its own crates define (plugins never call each other), and tests build one over [`crate::MemoryBackend`] with [`Secrets::new`].
#[derive(Clone)]
pub struct Secrets {
    backend: Arc<dyn Backend>,
}

impl Secrets {
    pub fn new(backend: Arc<dyn Backend>) -> Self {
        Secrets { backend }
    }

    /// What works on this system; never prompts.
    pub async fn get_status(&self) -> PluginStatus {
        self.backend.status().await
    }

    /// Stores a secret, replacing one with the same id. `label` is what the keyring's own manager (Seahorse, KWalletManager, Windows Credentials) shows; without one it shows the service and kind.
    pub async fn store(&self, id: &SecretId, label: Option<String>, secret: Secret) -> Result<()> {
        id.validate()?;
        if secret.is_empty() {
            return Err(SecretsError::Invalid {
                field: "secret".into(),
            });
        }
        log::debug!("storing a {} secret for {}", id.kind.as_str(), id.service);
        self.backend.store(id.clone(), label, secret).await
    }

    /// The secret, or `None` when there is none.
    pub async fn fetch(&self, id: &SecretId) -> Result<Option<Secret>> {
        id.validate()?;
        log::debug!("fetching a {} secret for {}", id.kind.as_str(), id.service);
        self.backend.fetch(id.clone()).await
    }

    /// Whether a secret exists, without reading it.
    pub async fn exists(&self, id: &SecretId) -> Result<bool> {
        id.validate()?;
        self.backend.exists(id.clone()).await
    }

    /// Deletes a secret; true when there was one.
    pub async fn delete(&self, id: &SecretId) -> Result<bool> {
        id.validate()?;
        log::debug!("deleting a {} secret for {}", id.kind.as_str(), id.service);
        self.backend.delete(id.clone()).await
    }

    /// Deletes every kind of secret of one account and returns how many there were.
    pub async fn delete_account(&self, service: &str, account: &str) -> Result<usize> {
        SecretsError::check_name("service", service)?;
        SecretsError::check_name("account", account)?;
        log::debug!("deleting every secret for {service}");
        self.backend
            .delete_account(service.to_string(), account.to_string())
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::MemoryBackend;
    use crate::models::SecretKind;

    fn secrets() -> (Secrets, Arc<MemoryBackend>) {
        let backend = Arc::new(MemoryBackend::new());
        (Secrets::new(backend.clone()), backend)
    }

    #[tokio::test]
    async fn invalid_names_and_empty_secrets_never_reach_the_backend() {
        let (secrets, backend) = secrets();
        let bad = SecretId::new("", "a", SecretKind::Password);
        assert!(matches!(
            secrets
                .store(&bad, None, Secret::from_text("x".into()))
                .await,
            Err(SecretsError::Invalid { .. })
        ));
        let good = SecretId::new("s", "a", SecretKind::Password);
        assert!(matches!(
            secrets.store(&good, None, Secret::from_text(String::new())).await,
            Err(SecretsError::Invalid { field }) if field == "secret"
        ));
        assert!(matches!(
            secrets.delete_account("s", "").await,
            Err(SecretsError::Invalid { .. })
        ));
        assert!(backend.is_empty());
    }

    #[tokio::test]
    async fn a_stored_secret_is_fetched_tested_and_deleted() {
        let (secrets, _) = secrets();
        let id = SecretId::new("volume", "uuid-1", SecretKind::Passphrase);
        assert!(!secrets.exists(&id).await.unwrap());
        secrets
            .store(&id, None, Secret::from_text("pw".into()))
            .await
            .unwrap();
        assert!(secrets.exists(&id).await.unwrap());
        assert_eq!(
            secrets.fetch(&id).await.unwrap().unwrap().expose_text(),
            Some("pw")
        );
        assert!(secrets.delete(&id).await.unwrap());
        assert_eq!(secrets.fetch(&id).await.unwrap().map(|s| s.len()), None);
    }
}
