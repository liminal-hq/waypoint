// The seam between the plugin's logic and the operating system: what a platform backend has to provide
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::future::Future;
use std::pin::Pin;

use tauri::{AppHandle, Runtime, Window};

use crate::error::{Result, WindowEffectsError};
use crate::models::{Effects, Insets};
use crate::status::Environment;

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// A platform's window effects. The real ones are in `linux.rs` and `windows.rs`; tests inject a fake. The plugin has already checked that the request is one the environment can do and that its region fits the window, so a backend only has to do it.
pub trait Backend<R: Runtime>: Send + Sync + 'static {
    /// What the system is: probed when a status is wanted rather than once, because the compositor can change under the app.
    fn environment(&self, app: &AppHandle<R>) -> BoxFuture<'_, Environment>;

    /// The window's size in logical pixels, which regions and insets are in. The default asks Tauri; a test overrides it, because the mock runtime's windows have no size.
    fn logical_size(&self, window: &Window<R>) -> Result<(i64, i64)> {
        let failed = |error: tauri::Error| WindowEffectsError::failed(error.to_string());
        let physical = window.inner_size().map_err(failed)?;
        let scale = window.scale_factor().map_err(failed)?;
        let logical = physical.to_logical::<f64>(scale);
        Ok((logical.width.round() as i64, logical.height.round() as i64))
    }

    /// Puts the effect behind the window, replacing any it had. `env` is the environment the request was checked against.
    fn apply(
        &self,
        window: Window<R>,
        env: Environment,
        effects: Effects,
    ) -> BoxFuture<'_, Result<()>>;

    /// Takes the window's effect away. Does nothing for a window that has none.
    fn clear(&self, window: Window<R>, env: Environment) -> BoxFuture<'_, Result<()>>;

    fn set_shadow_inset(&self, window: Window<R>, insets: Insets) -> BoxFuture<'_, Result<()>>;
}
