// An in-memory keyring for tests: nothing it holds leaves the process or touches the disk
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::HashMap;
use std::sync::Mutex;

use crate::backend::{Backend, BoxFuture};
use crate::error::{Result, SecretsError};
use crate::models::{Flavour, PluginStatus, Reason, Secret, SecretId};

/// A [`Backend`] over a map. [`MemoryBackend::fail_with`] makes every action fail the way a missing or locked keyring does, so callers can test their fallbacks.
#[derive(Default)]
pub struct MemoryBackend {
    items: Mutex<HashMap<SecretId, (Option<String>, Secret)>>,
    failure: Mutex<Option<SecretsError>>,
}

impl MemoryBackend {
    pub fn new() -> Self {
        Self::default()
    }

    /// Makes every action (and the status) fail with `error` until it is called with `None`.
    pub fn fail_with(&self, error: Option<SecretsError>) {
        *self.failure.lock().unwrap() = error;
    }

    /// How many secrets are held.
    pub fn len(&self) -> usize {
        self.items.lock().unwrap().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The label a secret was stored with.
    pub fn label_of(&self, id: &SecretId) -> Option<String> {
        self.items
            .lock()
            .unwrap()
            .get(id)
            .and_then(|(label, _)| label.clone())
    }

    fn check(&self) -> Result<()> {
        match self.failure.lock().unwrap().clone() {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}

impl Backend for MemoryBackend {
    fn status(&self) -> BoxFuture<'_, PluginStatus> {
        Box::pin(async move {
            match self.failure.lock().unwrap().clone() {
                None => PluginStatus::all_available(Flavour::Memory),
                Some(SecretsError::Locked | SecretsError::Dismissed) => {
                    PluginStatus::all_unavailable(
                        Flavour::Memory,
                        Reason::Locked,
                        "the keyring is locked",
                    )
                }
                Some(SecretsError::NoKeyring) => PluginStatus::all_unavailable(
                    Flavour::Memory,
                    Reason::NoKeyring,
                    "no keyring is running",
                ),
                Some(other) => PluginStatus::all_unavailable(
                    Flavour::Memory,
                    Reason::Failed,
                    &other.to_string(),
                ),
            }
        })
    }

    fn store(
        &self,
        id: SecretId,
        label: Option<String>,
        secret: Secret,
    ) -> BoxFuture<'_, Result<()>> {
        Box::pin(async move {
            self.check()?;
            self.items.lock().unwrap().insert(id, (label, secret));
            Ok(())
        })
    }

    fn fetch(&self, id: SecretId) -> BoxFuture<'_, Result<Option<Secret>>> {
        Box::pin(async move {
            self.check()?;
            Ok(self
                .items
                .lock()
                .unwrap()
                .get(&id)
                .map(|(_, secret)| secret.clone()))
        })
    }

    fn exists(&self, id: SecretId) -> BoxFuture<'_, Result<bool>> {
        Box::pin(async move {
            self.check()?;
            Ok(self.items.lock().unwrap().contains_key(&id))
        })
    }

    fn delete(&self, id: SecretId) -> BoxFuture<'_, Result<bool>> {
        Box::pin(async move {
            self.check()?;
            Ok(self.items.lock().unwrap().remove(&id).is_some())
        })
    }

    fn delete_account(&self, service: String, account: String) -> BoxFuture<'_, Result<usize>> {
        Box::pin(async move {
            self.check()?;
            let mut items = self.items.lock().unwrap();
            let before = items.len();
            items.retain(|id, _| !(id.service == service && id.account == account));
            Ok(before - items.len())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::SecretKind;

    fn id(kind: SecretKind) -> SecretId {
        SecretId::new("sftp", "home", kind)
    }

    #[tokio::test]
    async fn a_secret_round_trips_and_kinds_are_separate_identities() {
        let backend = MemoryBackend::new();
        backend
            .store(
                id(SecretKind::Password),
                Some("Home".into()),
                Secret::from_text("a".into()),
            )
            .await
            .unwrap();
        backend
            .store(
                id(SecretKind::Passphrase),
                None,
                Secret::from_text("b".into()),
            )
            .await
            .unwrap();
        let got = backend
            .fetch(id(SecretKind::Password))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(got.expose_text(), Some("a"));
        assert_eq!(
            backend.label_of(&id(SecretKind::Password)),
            Some("Home".into())
        );
        assert!(backend.exists(id(SecretKind::Passphrase)).await.unwrap());
        assert_eq!(backend.fetch(id(SecretKind::Token)).await, Ok(None));
    }

    #[tokio::test]
    async fn storing_again_replaces_and_delete_reports_whether_there_was_one() {
        let backend = MemoryBackend::new();
        let key = id(SecretKind::Password);
        backend
            .store(key.clone(), None, Secret::from_text("1".into()))
            .await
            .unwrap();
        backend
            .store(key.clone(), None, Secret::from_text("2".into()))
            .await
            .unwrap();
        assert_eq!(backend.len(), 1);
        assert_eq!(
            backend
                .fetch(key.clone())
                .await
                .unwrap()
                .unwrap()
                .expose_text(),
            Some("2")
        );
        assert_eq!(backend.delete(key.clone()).await, Ok(true));
        assert_eq!(backend.delete(key).await, Ok(false));
    }

    #[tokio::test]
    async fn deleting_an_account_removes_every_kind_and_only_that_account() {
        let backend = MemoryBackend::new();
        for kind in [SecretKind::Password, SecretKind::Passphrase] {
            backend
                .store(id(kind), None, Secret::from_text("x".into()))
                .await
                .unwrap();
        }
        backend
            .store(
                SecretId::new("sftp", "work", SecretKind::Password),
                None,
                Secret::from_text("y".into()),
            )
            .await
            .unwrap();
        assert_eq!(
            backend.delete_account("sftp".into(), "home".into()).await,
            Ok(2)
        );
        assert_eq!(backend.len(), 1);
    }

    #[tokio::test]
    async fn a_failure_reaches_every_action_and_the_status() {
        let backend = MemoryBackend::new();
        backend.fail_with(Some(SecretsError::Locked));
        assert_eq!(
            backend.fetch(id(SecretKind::Password)).await,
            Err(SecretsError::Locked)
        );
        let status = backend.status().await;
        assert!(!status.available);
        assert_eq!(status.reason, Some(Reason::Locked));
        backend.fail_with(None);
        assert!(backend.status().await.available);
    }
}
