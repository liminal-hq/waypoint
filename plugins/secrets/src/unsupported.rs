// Reports every feature unavailable on systems with no keyring support
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![cfg_attr(target_os = "linux", allow(dead_code))]

use crate::backend::{Backend, BoxFuture};
use crate::error::{Result, SecretsError};
use crate::models::{Flavour, PluginStatus, Reason, Secret, SecretId};

const MESSAGE: &str = "this system has no keyring support";

pub struct Platform;

impl Platform {
    pub fn new(_namespace: String) -> Self {
        Platform
    }
}

impl Backend for Platform {
    fn status(&self) -> BoxFuture<'_, PluginStatus> {
        Box::pin(async {
            PluginStatus::all_unavailable(
                Flavour::Unsupported,
                Reason::UnsupportedPlatform,
                MESSAGE,
            )
        })
    }

    fn store(
        &self,
        _id: SecretId,
        _label: Option<String>,
        _secret: Secret,
    ) -> BoxFuture<'_, Result<()>> {
        Box::pin(async { Err(SecretsError::Unsupported) })
    }

    fn fetch(&self, _id: SecretId) -> BoxFuture<'_, Result<Option<Secret>>> {
        Box::pin(async { Err(SecretsError::Unsupported) })
    }

    fn exists(&self, _id: SecretId) -> BoxFuture<'_, Result<bool>> {
        Box::pin(async { Err(SecretsError::Unsupported) })
    }

    fn delete(&self, _id: SecretId) -> BoxFuture<'_, Result<bool>> {
        Box::pin(async { Err(SecretsError::Unsupported) })
    }

    fn delete_account(&self, _service: String, _account: String) -> BoxFuture<'_, Result<usize>> {
        Box::pin(async { Err(SecretsError::Unsupported) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{SecretKind, FEATURES};

    #[tokio::test]
    async fn every_feature_is_unavailable_with_a_reason() {
        let status = Platform::new("x".into()).status().await;
        assert!(!status.available);
        assert_eq!(status.flavour, Flavour::Unsupported);
        let names: Vec<&str> = status.features.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, FEATURES);
        assert!(status.features.iter().all(|f| !f.available
            && f.reason == Some(Reason::UnsupportedPlatform)
            && f.message.is_some()));
    }

    #[tokio::test]
    async fn every_action_is_unsupported() {
        let platform = Platform::new("x".into());
        let id = SecretId::new("s", "a", SecretKind::Password);
        assert_eq!(
            platform.fetch(id.clone()).await,
            Err(SecretsError::Unsupported)
        );
        assert_eq!(platform.delete(id).await, Err(SecretsError::Unsupported));
    }
}
