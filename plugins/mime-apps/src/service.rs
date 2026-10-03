// The Rust API of the plugin: the `MimeApps` handle behind `app.mime_apps()`
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::sync::Arc;

#[cfg(windows)]
use tauri::Manager;
use tauri::{AppHandle, Runtime};

use crate::backend::{Backend, ParentWindow};
use crate::error::{MimeAppsError, Result};
use crate::models::{Handlers, PluginStatus, TypeInfo};
use crate::scheme::IconCache;
use crate::target::{self, Target};
use crate::typeicons::TypeIconCache;

/// File types and the applications that open them. Every call runs the blocking system query off the async runtime, and nothing here starts an application except `open_with`, `open_default` and `choose`.
pub struct MimeApps<R: Runtime> {
    app: AppHandle<R>,
    backend: Arc<dyn Backend>,
    icons: Arc<IconCache>,
    type_icons: Arc<TypeIconCache>,
}

impl<R: Runtime> MimeApps<R> {
    pub(crate) fn new(app: AppHandle<R>, backend: Arc<dyn Backend>) -> Self {
        MimeApps {
            app,
            backend,
            icons: Arc::default(),
            type_icons: Arc::default(),
        }
    }

    /// What the scheme handler needs: the backend and the picture cache.
    pub(crate) fn scheme_parts(&self) -> (Arc<dyn Backend>, Arc<IconCache>) {
        (Arc::clone(&self.backend), Arc::clone(&self.icons))
    }

    /// What the `typeicon://` scheme handler needs: the backend and the picture cache.
    pub(crate) fn type_icon_parts(&self) -> (Arc<dyn Backend>, Arc<TypeIconCache>) {
        (Arc::clone(&self.backend), Arc::clone(&self.type_icons))
    }

    /// Forgets every file and folder icon made so far and what the platform kept for them. Call it when the system's icon theme changes (or its files do), then ask for the icons again.
    pub fn refresh_type_icons(&self) {
        self.type_icons.clear();
        self.backend.refresh_type_icons();
    }

    async fn run<T: Send + 'static>(
        &self,
        job: impl FnOnce(&dyn Backend) -> T + Send + 'static,
    ) -> Result<T> {
        let backend = Arc::clone(&self.backend);
        tauri::async_runtime::spawn_blocking(move || job(backend.as_ref()))
            .await
            .map_err(|error| MimeAppsError::failed(error.to_string()))
    }

    /// What works on this system, and why anything does not.
    pub async fn get_status(&self) -> PluginStatus {
        self.run(|backend| backend.status())
            .await
            .unwrap_or_else(|error| {
                PluginStatus::all_unavailable(
                    crate::models::Flavour::Unsupported,
                    crate::models::Reason::NotImplemented,
                    &error.to_string(),
                )
            })
    }

    /// The type of one location, from its name; with `sniff` also from the start of a local file.
    pub async fn type_info(&self, uri: &str, sniff: bool) -> Result<TypeInfo> {
        let target = target::parse(uri)?;
        self.run(move |backend| backend.type_info(&target, sniff))
            .await?
    }

    /// The applications for the locations' type.
    pub async fn handlers(&self, uris: &[String]) -> Result<Handlers> {
        let targets = parse_all(uris)?;
        self.run(move |backend| backend.handlers(&targets)).await?
    }

    /// Opens the locations in the application with this id.
    pub async fn open_with(&self, uris: &[String], app_id: &str) -> Result<()> {
        let targets = parse_all(uris)?;
        let app_id = app_id.to_string();
        self.run(move |backend| backend.open_with(&targets, &app_id))
            .await?
    }

    /// Opens each location in its default application.
    pub async fn open_default(&self, uris: &[String]) -> Result<()> {
        let targets = parse_all(uris)?;
        self.run(move |backend| backend.open_default(&targets))
            .await?
    }

    /// Asks the system's chooser, as a dialog of the window labelled `parent_label`, and opens the locations in the application picked. `Cancelled` when the person closes it; `Unsupported` when the system has none.
    pub async fn choose(&self, uris: &[String], parent_label: Option<&str>) -> Result<()> {
        let targets = parse_all(uris)?;
        let parent = parent_label.and_then(|label| self.window_handle(label));
        self.run(move |backend| backend.choose(&targets, parent))
            .await?
    }

    /// Makes an application the default for a type.
    pub async fn set_default(&self, mime: &str, app_id: &str) -> Result<()> {
        let (mime, app_id) = (mime.to_string(), app_id.to_string());
        self.run(move |backend| backend.set_default(&mime, &app_id))
            .await?
    }

    /// Opens the system's page for default applications, where there is one.
    pub async fn open_default_apps_settings(&self) -> Result<()> {
        self.run(|backend| backend.open_default_apps_settings())
            .await?
    }

    #[cfg(windows)]
    fn window_handle(&self, label: &str) -> ParentWindow {
        self.app
            .get_webview_window(label)
            .and_then(|window| window.hwnd().ok())
            .map(|hwnd| hwnd.0 as isize)
    }

    #[cfg(not(windows))]
    fn window_handle(&self, label: &str) -> ParentWindow {
        let _ = (&self.app, label);
        None
    }
}

fn parse_all(uris: &[String]) -> Result<Vec<Target>> {
    if uris.is_empty() {
        return Err(MimeAppsError::Empty);
    }
    uris.iter().map(|uri| target::parse(uri)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_to_act_on_is_an_error_not_an_empty_success() {
        assert_eq!(parse_all(&[]), Err(MimeAppsError::Empty));
    }

    #[test]
    fn one_bad_location_fails_the_whole_call_before_anything_starts() {
        let uris = vec!["/a/b.txt".to_string(), "relative.txt".to_string()];
        assert_eq!(
            parse_all(&uris),
            Err(MimeAppsError::InvalidUri {
                uri: "relative.txt".into()
            })
        );
    }
}
