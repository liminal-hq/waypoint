// Reports every feature unavailable on systems with no volume support
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![cfg_attr(any(target_os = "linux", target_os = "windows"), allow(dead_code))]

use crate::backend::{Backend, BoxFuture, Notify};
use crate::error::{Result, VolumesError};
use crate::models::{Flavour, Passphrase, PluginStatus, Reason, Volume};

const MESSAGE: &str = "this system has no volume support";

pub struct Platform;

impl Platform {
    pub fn new() -> Self {
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

    fn volumes(&self) -> BoxFuture<'_, Result<Vec<Volume>>> {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn mount(&self, _id: String) -> BoxFuture<'_, Result<String>> {
        Box::pin(async { Err(VolumesError::Unsupported) })
    }

    fn unmount(&self, _id: String) -> BoxFuture<'_, Result<()>> {
        Box::pin(async { Err(VolumesError::Unsupported) })
    }

    fn eject(&self, _id: String) -> BoxFuture<'_, Result<()>> {
        Box::pin(async { Err(VolumesError::Unsupported) })
    }

    fn unlock(&self, _id: String, _passphrase: Passphrase) -> BoxFuture<'_, Result<String>> {
        Box::pin(async { Err(VolumesError::Unsupported) })
    }

    fn watch(&self, _notify: Notify) -> BoxFuture<'_, Result<()>> {
        Box::pin(async { Err(VolumesError::Unsupported) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::FEATURES;

    #[tokio::test]
    async fn every_feature_is_unavailable_with_a_reason() {
        let status = Platform::new().status().await;
        assert!(!status.available);
        assert_eq!(status.flavour, Flavour::Unsupported);
        assert_eq!(status.reason, Some(Reason::UnsupportedPlatform));
        let names: Vec<&str> = status.features.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, FEATURES);
        assert!(status.features.iter().all(|f| !f.available
            && f.reason == Some(Reason::UnsupportedPlatform)
            && f.message.is_some()));
    }

    #[tokio::test]
    async fn the_list_is_empty_and_every_action_is_unsupported() {
        let platform = Platform::new();
        assert_eq!(platform.volumes().await, Ok(Vec::new()));
        assert_eq!(
            platform.mount("x".into()).await,
            Err(VolumesError::Unsupported)
        );
        assert_eq!(
            platform.unmount("x".into()).await,
            Err(VolumesError::Unsupported)
        );
        assert_eq!(
            platform.eject("x".into()).await,
            Err(VolumesError::Unsupported)
        );
        assert_eq!(
            platform.unlock("x".into(), Passphrase("p".into())).await,
            Err(VolumesError::Unsupported)
        );
    }
}
