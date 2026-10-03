// Reads and watches the palette on Linux: KDE's kdeglobals on KDE, the GTK theme's named colours elsewhere, the portal's accent for what is left
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::{
    io::ErrorKind,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use gtk::prelude::*;
use tauri::{AppHandle, Runtime};
use tokio::sync::mpsc::UnboundedSender;

use super::{
    models::{PaletteColour, PaletteColours, PaletteSource},
    parse,
};
use crate::{
    appearance::{
        linux::{kdeglobals, portal},
        models::UnavailableReason,
    },
    linux::kwin,
    models::DesktopEnvironment,
    parse as titlebar_parse,
    service::Readiness,
};

/// Keeps the change watchers alive; dropping it stops them.
pub struct Watcher {
    _guards: Vec<Box<dyn Send>>,
    readiness: Option<Readiness>,
}

impl Watcher {
    /// Takes how the watcher reports that it is listening.
    pub fn take_readiness(&mut self) -> Readiness {
        self.readiness.take().unwrap_or(Readiness::Listening)
    }
}

fn current_desktop() -> DesktopEnvironment {
    titlebar_parse::desktop_environment(&std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default())
}

/// Reads the colours of the GTK theme on the main thread, where GTK must be used. A theme the
/// window system has not initialised GTK for (no display) is a failed read.
fn read_gtk_on_main_thread() -> PaletteColours {
    if !gtk::is_initialized_main_thread() {
        return PaletteColours::unavailable(
            UnavailableReason::ReadFailed,
            "GTK is not initialised, so the theme's colours cannot be asked",
        );
    }
    let label = gtk::Label::new(None);
    let context = label.style_context();
    parse::gtk_palette(|name| {
        context
            .lookup_color(name)
            .map(|colour| (colour.red(), colour.green(), colour.blue(), colour.alpha()))
    })
}

async fn read_gtk<R: Runtime>(app: &AppHandle<R>) -> PaletteColours {
    let failed =
        |detail: String| PaletteColours::unavailable(UnavailableReason::ReadFailed, &detail);
    let (reply, answer) = tokio::sync::oneshot::channel();
    if let Err(error) = app.run_on_main_thread(move || {
        let _ = reply.send(read_gtk_on_main_thread());
    }) {
        return failed(format!("the GTK main thread could not be reached: {error}"));
    }
    answer
        .await
        .unwrap_or_else(|_| failed("the GTK main thread did not answer".to_string()))
}

/// Reads the user's `kdeglobals`. A missing file means KDE's defaults are in effect, which this
/// plugin cannot know, so every colour is a miss.
fn read_kdeglobals() -> PaletteColours {
    let Some(path) = kwin::config_file(kdeglobals::FILE_NAME) else {
        return PaletteColours::unavailable(
            UnavailableReason::ReadFailed,
            "cannot locate the user config directory",
        );
    };
    match std::fs::read_to_string(&path) {
        Ok(text) => parse::kde_palette(&text),
        Err(error) if error.kind() == ErrorKind::NotFound => PaletteColours::unavailable(
            UnavailableReason::SourceMissing,
            &format!("{} does not exist", path.display()),
        ),
        Err(error) => PaletteColours::unavailable(
            UnavailableReason::ReadFailed,
            &format!("{}: {error}", path.display()),
        ),
    }
}

/// Fills the selection and the focus from the portal's accent colour when the theme gave none.
async fn fill_from_portal(colours: &mut PaletteColours) {
    let wanted = [PaletteColour::SelectionBackground, PaletteColour::Focus];
    if wanted
        .iter()
        .all(|colour| colours.entry(*colour).is_found())
    {
        return;
    }
    let reading = portal::read().await;
    if let Some(accent) = reading.values.accent {
        for colour in wanted {
            if !colours.entry(colour).is_found() {
                colours.set(colour, accent.clone(), PaletteSource::Portal);
            }
        }
    }
}

pub async fn read<R: Runtime>(app: &AppHandle<R>) -> PaletteColours {
    let mut colours = if current_desktop() == DesktopEnvironment::Kde {
        let mut kde = tauri::async_runtime::spawn_blocking(read_kdeglobals)
            .await
            .unwrap_or_else(|error| {
                PaletteColours::unavailable(UnavailableReason::ReadFailed, &error.to_string())
            });
        if !kde.is_usable() {
            // No colour sections (a fresh profile): the GTK theme is the next best answer.
            let gtk = read_gtk(app).await;
            if gtk.is_usable() {
                kde = gtk;
            }
        }
        kde
    } else {
        read_gtk(app).await
    };
    fill_from_portal(&mut colours).await;
    colours
}

/// Stops a GTK settings handler from notifying once the watcher is dropped.
struct Alive(Arc<AtomicBool>);

impl Drop for Alive {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

/// Notifies when GTK's theme name or dark preference changes; connected on the main thread.
fn watch_gtk_settings<R: Runtime>(
    app: &AppHandle<R>,
    changed: UnboundedSender<()>,
) -> Box<dyn Send> {
    let alive = Arc::new(AtomicBool::new(true));
    let flag = alive.clone();
    let connected = app.run_on_main_thread(move || {
        let Some(settings) = gtk::Settings::default() else {
            return;
        };
        for property in [
            "gtk-theme-name",
            "gtk-application-prefer-dark-theme",
            "gtk-color-scheme",
        ] {
            let changed = changed.clone();
            let flag = flag.clone();
            settings.connect_notify_local(Some(property), move |_, _| {
                if flag.load(Ordering::SeqCst) {
                    let _ = changed.send(());
                }
            });
        }
    });
    if let Err(error) = connected {
        log::warn!("cannot listen to GTK theme changes: {error}");
    }
    Box::new(Alive(alive))
}

pub fn watch<R: Runtime>(app: &AppHandle<R>, changed: UnboundedSender<()>) -> Watcher {
    // The portal announces colour-scheme and accent changes; GTK's settings announce a theme
    // switch on desktops that keep their theme elsewhere (Cinnamon, MATE, Xfce).
    let (mut guards, portal_ready) = portal::watch(changed.clone());
    guards.push(watch_gtk_settings(app, changed.clone()));
    if current_desktop() == DesktopEnvironment::Kde {
        // The file watcher is registered before `watch_file` returns.
        guards.extend(kwin::watch_file(kdeglobals::FILE_NAME, changed));
    }
    Watcher {
        _guards: guards,
        readiness: Some(Readiness::Signal(portal_ready)),
    }
}
