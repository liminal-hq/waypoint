// The Linux backend: GTK for transparency and the shadow inset, Wayland protocols and X11 properties for blur
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::cell::RefCell;
use std::ffi::c_void;

use gdk::glib::translate::ToGlibPtr;
use gdk::prelude::*;
use gtk::prelude::*;
use log::{info, warn};
use tauri::{AppHandle, Manager, Runtime, Window};
use tokio::sync::oneshot;

use crate::backend::{Backend, BoxFuture};
use crate::error::{Result, WindowEffectsError};
use crate::models::{EffectKind, Effects, Insets};
use crate::request::x11_blur_property;
use crate::status::{blur_path, BlurPath, Desktop, Environment, SessionType};
use crate::{ffi, wayland, x11};

thread_local! {
    /// The Wayland session lives on the main thread, where GTK does.
    static WAYLAND: RefCell<Option<wayland::Session>> = const { RefCell::new(None) };
}

pub struct Platform;

impl Platform {
    pub fn new() -> Self {
        Platform
    }
}

/// Runs `job` on the main thread, the one that owns GTK, and awaits its result.
async fn on_main<R: Runtime, T: Send + 'static>(
    app: &AppHandle<R>,
    job: impl FnOnce() -> T + Send + 'static,
) -> Result<T> {
    let (tx, rx) = oneshot::channel();
    app.run_on_main_thread(move || {
        let _ = tx.send(job());
    })
    .map_err(|error| WindowEffectsError::failed(error.to_string()))?;
    rx.await
        .map_err(|_| WindowEffectsError::failed("the event loop is gone"))
}

/// Maps a GDK display's type name to the display server. XWayland counts as X11: the app is an X11 client there.
pub fn classify_display(type_name: &str) -> SessionType {
    if type_name.contains("Wayland") {
        SessionType::Wayland
    } else if type_name.contains("X11") {
        SessionType::X11
    } else {
        SessionType::Other
    }
}

fn wayland_display(display: &gdk::Display) -> Option<ffi::RawProxy> {
    let raw: *mut gdk::ffi::GdkDisplay = display.to_glib_none().0;
    // SAFETY: `display` is GDK's live default display, and the caller has checked it is the Wayland backend.
    unsafe { ffi::wl_display_of_gdk(raw.cast::<c_void>()) }
}

fn wayland_surface(window: &gtk::ApplicationWindow) -> Option<ffi::RawProxy> {
    let gdk_window = window.window()?;
    let raw: *mut gdk::ffi::GdkWindow = gdk_window.to_glib_none().0;
    // SAFETY: `gdk_window` is a live, realised window of the Wayland backend (the display was probed as Wayland).
    unsafe { ffi::wl_surface_of_gdk_window(raw.cast::<c_void>()) }
}

/// Runs `job` on the Wayland session, connecting on first use.
fn with_wayland<T>(
    display: &gdk::Display,
    job: impl FnOnce(&mut wayland::Session) -> std::result::Result<T, String>,
) -> std::result::Result<T, String> {
    WAYLAND.with(|cell| {
        let mut cell = cell
            .try_borrow_mut()
            .map_err(|_| "the Wayland session is busy".to_string())?;
        if cell.is_none() {
            let raw = wayland_display(display).ok_or("GDK has no Wayland display")?;
            // SAFETY: the raw display is GDK's own, and this runs on the GTK main thread.
            *cell = Some(unsafe { wayland::Session::new(raw)? });
        }
        match cell.as_mut() {
            Some(session) => job(session),
            None => Err("no Wayland session".to_string()),
        }
    })
}

/// Reads the session and compositor facts. Must run on the main thread.
pub fn probe_environment() -> Environment {
    // GDK aborts if it is used before GTK is initialised, as in a unit test with no display.
    if !gtk::is_initialized_main_thread() {
        return Environment::unsupported();
    }
    let Some(display) = gdk::Display::default() else {
        return Environment::unsupported();
    };
    let session_type = classify_display(display.type_().name());
    let desktop = Desktop::from_xdg(&std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default());
    let mut env = Environment {
        session_type,
        desktop,
        has_composite: false,
        wayland_globals: Vec::new(),
        build_number: None,
    };
    match session_type {
        SessionType::Wayland => {
            // A Wayland compositor always composites.
            env.has_composite = true;
            match with_wayland(&display, |session| session.globals()) {
                Ok(globals) => env.wayland_globals = globals,
                Err(error) => info!("window-effects: could not read the Wayland registry: {error}"),
            }
        }
        SessionType::X11 => {
            env.has_composite = gdk::Screen::default().is_some_and(|screen| screen.is_composited());
        }
        _ => {}
    }
    env
}

fn no_window() -> WindowEffectsError {
    WindowEffectsError::failed(
        "the window is not shown yet, so it has no surface to put an effect on",
    )
}

fn set_blur(window: &Window<impl Runtime>, env: &Environment, effects: &Effects) -> Result<()> {
    let gtk_window = window
        .gtk_window()
        .map_err(|error| WindowEffectsError::failed(error.to_string()))?;
    let display = gdk::Display::default().ok_or_else(no_window)?;
    let path = blur_path(env).ok_or_else(|| WindowEffectsError::failed("no blur path"))?;
    let region = effects.region.as_deref();
    match path {
        BlurPath::X11Property => {
            let gdk_window = gtk_window.window().ok_or_else(no_window)?;
            let xid = gdk_window
                .downcast_ref::<gdkx11::X11Window>()
                .ok_or_else(|| WindowEffectsError::failed("the window is not an X11 window"))?
                .xid();
            let scale = u32::try_from(gtk_window.scale_factor()).unwrap_or(1);
            let property = x11_blur_property(region, scale);
            x11::set_blur(&display.name(), xid as u32, Some(&property))
                .map_err(WindowEffectsError::failed)
        }
        BlurPath::ExtBackgroundEffect | BlurPath::KdeBlur => {
            let surface = wayland_surface(&gtk_window).ok_or_else(no_window)?;
            with_wayland(&display, |session| {
                // SAFETY: `surface` is the live `wl_surface` of this window, read on the main thread just now.
                unsafe { session.blur(window.label(), surface, path, region) }
            })
            .map_err(WindowEffectsError::failed)?;
            // The compositor applies the change on the surface's next commit.
            gtk_window.queue_draw();
            Ok(())
        }
        BlurPath::Dwm => Err(WindowEffectsError::failed("DWM is not on this system")),
    }
}

fn clear_blur(window: &Window<impl Runtime>, env: &Environment) -> Result<()> {
    let gtk_window = window
        .gtk_window()
        .map_err(|error| WindowEffectsError::failed(error.to_string()))?;
    let Some(display) = gdk::Display::default() else {
        return Ok(());
    };
    match env.session_type {
        SessionType::X11 => {
            let Some(gdk_window) = gtk_window.window() else {
                return Ok(());
            };
            let Some(x11_window) = gdk_window.downcast_ref::<gdkx11::X11Window>() else {
                return Ok(());
            };
            x11::set_blur(&display.name(), x11_window.xid() as u32, None)
                .map_err(WindowEffectsError::failed)
        }
        SessionType::Wayland => {
            // Nothing was ever applied if the session was never opened.
            let opened = WAYLAND.with(|cell| cell.borrow().is_some());
            if !opened {
                return Ok(());
            }
            let surface = wayland_surface(&gtk_window);
            with_wayland(&display, |session| {
                // SAFETY: `surface` is this window's live `wl_surface`.
                unsafe { session.unblur(window.label(), surface) }
            })
            .map_err(WindowEffectsError::failed)?;
            gtk_window.queue_draw();
            Ok(())
        }
        _ => Ok(()),
    }
}

impl<R: Runtime> Backend<R> for Platform {
    fn environment(&self, app: &AppHandle<R>) -> BoxFuture<'_, Environment> {
        let app = app.clone();
        Box::pin(async move {
            on_main(&app, probe_environment)
                .await
                .unwrap_or_else(|error| {
                    warn!("window-effects: could not probe the system: {error}");
                    Environment::unsupported()
                })
        })
    }

    fn apply(
        &self,
        window: Window<R>,
        env: Environment,
        effects: Effects,
    ) -> BoxFuture<'_, Result<()>> {
        Box::pin(async move {
            let target = window.clone();
            on_main(window.app_handle(), move || match effects.kind {
                EffectKind::Blur => set_blur(&target, &env, &effects),
                // The service has refused these where the system cannot do them.
                _ => Err(WindowEffectsError::failed("not an effect of this system")),
            })
            .await?
        })
    }

    fn clear(&self, window: Window<R>, env: Environment) -> BoxFuture<'_, Result<()>> {
        Box::pin(async move {
            let target = window.clone();
            on_main(window.app_handle(), move || clear_blur(&target, &env)).await?
        })
    }

    fn set_shadow_inset(&self, window: Window<R>, insets: Insets) -> BoxFuture<'_, Result<()>> {
        Box::pin(async move {
            let target = window.clone();
            on_main(window.app_handle(), move || {
                let gtk_window = target
                    .gtk_window()
                    .map_err(|error| WindowEffectsError::failed(error.to_string()))?;
                let gdk_window = gtk_window.window().ok_or_else(no_window)?;
                gdk_window.set_shadow_width(insets.left, insets.right, insets.top, insets.bottom);
                Ok(())
            })
            .await?
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_display_is_classified_by_its_type_name() {
        assert_eq!(classify_display("GdkWaylandDisplay"), SessionType::Wayland);
        assert_eq!(classify_display("GdkX11Display"), SessionType::X11);
        assert_eq!(classify_display("GdkBroadwayDisplay"), SessionType::Other);
    }
}
