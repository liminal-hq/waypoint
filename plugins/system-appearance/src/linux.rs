// Reads and watches titlebar preferences on Linux, choosing a source by desktop environment
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod cli;
mod gsettings;
mod kwin;
mod portal;
mod xfconf;

use tokio::sync::mpsc::UnboundedSender;

use crate::{
    models::{DesktopEnvironment, Snapshot, TitlebarPreferences},
    parse,
};

/// Keeps the change watchers alive; dropping it stops them and kills any child processes.
pub struct Watcher {
    _guards: Vec<Box<dyn Send>>,
    ready: Option<tokio::sync::oneshot::Receiver<()>>,
}

impl Watcher {
    /// Takes the signal that resolves once the watcher is listening; `None` when it listens at once.
    pub fn take_ready(&mut self) -> Option<tokio::sync::oneshot::Receiver<()>> {
        self.ready.take()
    }
}

fn current_desktop() -> DesktopEnvironment {
    parse::desktop_environment(&std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default())
}

/// Runs a blocking reader off the async executor.
async fn blocking<F>(reader: F) -> Result<TitlebarPreferences, String>
where
    F: FnOnce() -> Result<TitlebarPreferences, String> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(reader)
        .await
        .map_err(|error| error.to_string())?
}

pub async fn read() -> Snapshot {
    let desktop = current_desktop();
    let (result, feature) = match desktop {
        DesktopEnvironment::Kde => (blocking(kwin::read).await, "kwin-config"),
        DesktopEnvironment::Cinnamon => (
            blocking(move || gsettings::read(gsettings::CINNAMON_SCHEMA, desktop)).await,
            "gsettings",
        ),
        DesktopEnvironment::Mate => (
            blocking(move || gsettings::read(gsettings::MATE_SCHEMA, desktop)).await,
            "gsettings",
        ),
        DesktopEnvironment::Xfce => (blocking(xfconf::read).await, "xfconf"),
        // GNOME-family desktops and unknown ones both expose the GNOME schema through the portal.
        _ => (portal::read(desktop).await, "portal"),
    };
    match result {
        Ok(preferences) => Snapshot::from_source(preferences, feature),
        Err(reason) => {
            log::warn!("{feature} read failed: {reason}");
            Snapshot::unavailable(desktop, format!("{feature}: {reason}"))
        }
    }
}

pub fn watch(changed: UnboundedSender<()>) -> Watcher {
    let (guards, ready): (Vec<Box<dyn Send>>, _) = match current_desktop() {
        DesktopEnvironment::Kde => (kwin::watch(changed), None),
        DesktopEnvironment::Cinnamon => {
            (gsettings::watch(gsettings::CINNAMON_SCHEMA, changed), None)
        }
        DesktopEnvironment::Mate => (gsettings::watch(gsettings::MATE_SCHEMA, changed), None),
        DesktopEnvironment::Xfce => (xfconf::watch(changed), None),
        _ => {
            let (guards, ready) = portal::watch(changed);
            (guards, Some(ready))
        }
    };
    Watcher {
        _guards: guards,
        ready,
    }
}
