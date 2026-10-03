// The plugin's logic over a backend: check a request against what the system can do, then hand it over
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::sync::Arc;

use tauri::{AppHandle, Manager, Runtime, Window};

use crate::backend::Backend;
use crate::error::{Result, WindowEffectsError};
use crate::models::{
    EffectKind, Effects, Flavour, Insets, PluginStatus, Reason, FEATURE_SHADOW_INSET,
};
use crate::request::{feature_for, validate_insets, validate_region};
use crate::status::status_for;

/// The plugin's state: the backend every request goes through.
pub struct WindowEffects<R: Runtime> {
    app: AppHandle<R>,
    backend: Arc<dyn Backend<R>>,
}

impl<R: Runtime> WindowEffects<R> {
    pub fn new(app: AppHandle<R>, backend: Arc<dyn Backend<R>>) -> Self {
        WindowEffects { app, backend }
    }

    /// What works on this system now.
    pub async fn get_status(&self) -> PluginStatus {
        status_for(&self.backend.environment(&self.app).await)
    }

    fn window(&self, label: &str) -> Result<Window<R>> {
        self.app
            .get_webview_window(label)
            .map(|webview_window| webview_window.as_ref().window())
            .ok_or_else(|| WindowEffectsError::WindowNotFound {
                label: label.to_string(),
            })
    }

    fn unsupported(status: &PluginStatus, feature: &str) -> WindowEffectsError {
        match status.feature(feature) {
            Some(entry) => WindowEffectsError::Unsupported {
                reason: entry.reason.unwrap_or(Reason::UnsupportedPlatform),
                message: entry.message.clone().unwrap_or_default(),
            },
            None => WindowEffectsError::Unsupported {
                reason: Reason::UnsupportedPlatform,
                message: format!("{feature} is not a feature of this plugin"),
            },
        }
    }

    /// Puts the effect behind the window with that label. The caller applies it again when the theme changes.
    pub async fn apply(&self, label: &str, effects: Effects) -> Result<()> {
        let window = self.window(label)?;
        let env = self.backend.environment(&self.app).await;
        let status = status_for(&env);
        let Some(feature) = feature_for(effects.kind) else {
            return self.clear_checked(window, env, &status).await;
        };
        if !status.has(feature) {
            return Err(Self::unsupported(&status, feature));
        }
        if let (EffectKind::Blur, Some(region)) = (effects.kind, effects.region.as_deref()) {
            let (width, height) = self.backend.logical_size(&window)?;
            validate_region(region, width, height)?;
        }
        self.backend.apply(window, env, effects).await
    }

    /// Takes the window's effect away.
    pub async fn clear(&self, label: &str) -> Result<()> {
        let window = self.window(label)?;
        let env = self.backend.environment(&self.app).await;
        let status = status_for(&env);
        self.clear_checked(window, env, &status).await
    }

    async fn clear_checked(
        &self,
        window: Window<R>,
        env: crate::status::Environment,
        status: &PluginStatus,
    ) -> Result<()> {
        if status.flavour == Flavour::Unsupported {
            return Err(Self::unsupported(status, FEATURE_SHADOW_INSET));
        }
        self.backend.clear(window, env).await
    }

    /// Tells the compositor how much of the window is shadow or invisible border.
    pub async fn set_shadow_inset(&self, label: &str, insets: Insets) -> Result<()> {
        let window = self.window(label)?;
        let status = self.get_status().await;
        if !status.has(FEATURE_SHADOW_INSET) {
            return Err(Self::unsupported(&status, FEATURE_SHADOW_INSET));
        }
        let (width, height) = self.backend.logical_size(&window)?;
        validate_insets(insets, width, height)?;
        self.backend.set_shadow_inset(window, insets).await
    }
}
