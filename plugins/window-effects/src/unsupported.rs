// Reports every feature unavailable on systems with no window effects
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![cfg_attr(any(target_os = "linux", target_os = "windows"), allow(dead_code))]

use tauri::{AppHandle, Runtime, Window};

use crate::backend::{Backend, BoxFuture};
use crate::error::{Result, WindowEffectsError};
use crate::models::{Effects, Insets, Reason};
use crate::status::Environment;

pub struct Platform;

impl Platform {
    pub fn new() -> Self {
        Platform
    }
}

fn unsupported() -> WindowEffectsError {
    WindowEffectsError::Unsupported {
        reason: Reason::UnsupportedPlatform,
        message: "this system has no window effects".to_string(),
    }
}

impl<R: Runtime> Backend<R> for Platform {
    fn environment(&self, _app: &AppHandle<R>) -> BoxFuture<'_, Environment> {
        Box::pin(async { Environment::unsupported() })
    }

    fn apply(
        &self,
        _window: Window<R>,
        _env: Environment,
        _effects: Effects,
    ) -> BoxFuture<'_, Result<()>> {
        Box::pin(async { Err(unsupported()) })
    }

    fn clear(&self, _window: Window<R>, _env: Environment) -> BoxFuture<'_, Result<()>> {
        Box::pin(async { Err(unsupported()) })
    }

    fn set_shadow_inset(&self, _window: Window<R>, _insets: Insets) -> BoxFuture<'_, Result<()>> {
        Box::pin(async { Err(unsupported()) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Flavour;
    use crate::status::status_for;

    #[test]
    fn the_status_of_an_unsupported_system_has_every_feature_unavailable() {
        let status = status_for(&Environment::unsupported());
        assert_eq!(status.flavour, Flavour::Unsupported);
        assert!(status.features.iter().all(|feature| !feature.available));
    }

    #[tokio::test]
    async fn every_request_is_a_typed_unsupported_error() {
        let app = tauri::test::mock_app();
        let window = tauri::WebviewWindowBuilder::new(&app, "w", Default::default())
            .build()
            .unwrap()
            .as_ref()
            .window();
        let platform = Platform::new();
        let effects = Effects {
            kind: crate::models::EffectKind::Blur,
            dark: false,
            region: None,
        };
        let error = platform
            .apply(window.clone(), Environment::unsupported(), effects)
            .await
            .unwrap_err();
        assert!(matches!(
            error,
            WindowEffectsError::Unsupported {
                reason: Reason::UnsupportedPlatform,
                ..
            }
        ));
        assert!(platform
            .clear(window.clone(), Environment::unsupported())
            .await
            .is_err());
        let insets = Insets {
            top: 1,
            right: 1,
            bottom: 1,
            left: 1,
        };
        assert!(platform.set_shadow_inset(window, insets).await.is_err());
    }
}
