// Creates the main windows from the session, applies and captures their geometry
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Every main window is created here, never from `tauri.conf.json` (A40): `setup` creates the
// windows the restored session holds, and the session plugin calls `TauriWindowFactory` for the
// ones its commands make. The options are the ones the static `main-1` window had: frameless and
// transparent, `shadow: true` (the OS draws the shadow and rounded corners where it will, the page
// draws its own on Linux, D89), the minimum size, and the drag-and-drop handler off (A36).

use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tauri::{
    AppHandle, Manager, PhysicalPosition, PhysicalSize, Runtime, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder, WindowEvent,
};
use tauri_plugin_waypoint_session::{Sessions, WindowError, WindowFactory};
use waypoint_session::{Command, Geometry};

/// The size a window opens at when the session has no geometry for it, in logical pixels.
const DEFAULT_SIZE: (f64, f64) = (1100.0, 720.0);
const MIN_SIZE: (f64, f64) = (640.0, 400.0);

/// How long a burst of moves and resizes settles before the geometry is saved.
const GEOMETRY_DELAY: Duration = Duration::from_millis(250);

/// A monitor's area in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Area {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Area {
    fn contains(&self, x: i64, y: i64) -> bool {
        x >= self.x as i64
            && y >= self.y as i64
            && x < self.x as i64 + self.width as i64
            && y < self.y as i64 + self.height as i64
    }
}

/// What to do with a saved geometry on this machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fit {
    /// Physical inner size, shrunk to fit the monitor it lands on.
    pub size: (u32, u32),
    /// Where to put the window, or `None` to let the system (or `center()`) place it.
    pub position: Option<(i32, i32)>,
    pub maximised: bool,
}

/// Fits `geometry` to the monitors that exist now. A position is kept only when the window's title
/// area (a point just inside its top left) is on a monitor, and never when the platform ignores
/// positions (`position_applies` false: Wayland); the size is capped at the monitor it lands on.
pub fn fit_geometry(geometry: &Geometry, monitors: &[Area], position_applies: bool) -> Fit {
    let anchor = match (geometry.x, geometry.y) {
        (Some(x), Some(y)) if position_applies => Some((x, y)),
        _ => None,
    };
    let placed = anchor.and_then(|(x, y)| {
        monitors
            .iter()
            .find(|m| m.contains(x as i64 + 40, y as i64 + 16))
            .map(|m| ((x, y), m))
    });
    let monitor = placed.map(|(_, m)| m).or_else(|| monitors.first());
    let size = match monitor {
        Some(m) => (geometry.width.min(m.width), geometry.height.min(m.height)),
        None => (geometry.width, geometry.height),
    };
    Fit {
        size,
        position: placed.map(|(at, _)| at),
        maximised: geometry.maximised,
    }
}

/// Wayland compositors place windows themselves, so a window's position can be neither read nor set.
pub fn position_applies() -> bool {
    !(cfg!(target_os = "linux")
        && std::env::var_os("WAYLAND_DISPLAY").is_some()
        && std::env::var("GDK_BACKEND").map_or(true, |b| !b.starts_with("x11")))
}

fn monitors<R: Runtime>(app: &AppHandle<R>) -> Vec<Area> {
    app.available_monitors()
        .unwrap_or_default()
        .iter()
        .map(|m| Area {
            x: m.position().x,
            y: m.position().y,
            width: m.size().width,
            height: m.size().height,
        })
        .collect()
}

/// Builds a main window with `label`, placed from `geometry` when there is one.
pub fn build_main_window<R: Runtime>(
    app: &AppHandle<R>,
    label: &str,
    geometry: Option<&Geometry>,
) -> Result<WebviewWindow<R>, WindowError> {
    let fail = |e: tauri::Error| WindowError::Failed(e.to_string());
    // Hidden until the geometry is applied, so the window never shows at the wrong size.
    let window = WebviewWindowBuilder::new(app, label, WebviewUrl::App("index.html".into()))
        .title("Waypoint")
        .inner_size(DEFAULT_SIZE.0, DEFAULT_SIZE.1)
        .min_inner_size(MIN_SIZE.0, MIN_SIZE.1)
        .resizable(true)
        .decorations(false)
        .transparent(true)
        .shadow(true)
        .disable_drag_drop_handler()
        .visible(false)
        .build()
        .map_err(fail)?;
    match geometry {
        Some(g) => {
            let fit = fit_geometry(g, &monitors(app), position_applies());
            let _ = window.set_size(PhysicalSize::new(fit.size.0, fit.size.1));
            match fit.position {
                Some((x, y)) => {
                    let _ = window.set_position(PhysicalPosition::new(x, y));
                }
                None => {
                    let _ = window.center();
                }
            }
            if fit.maximised {
                let _ = window.maximize();
            }
        }
        None => {
            let _ = window.center();
        }
    }
    window.show().map_err(fail)?;
    Ok(window)
}

/// The session plugin's window factory.
pub struct TauriWindowFactory;

impl<R: Runtime> WindowFactory<R> for TauriWindowFactory {
    fn create(
        &self,
        app: &AppHandle<R>,
        label: &str,
        geometry: Option<&Geometry>,
    ) -> Result<(), WindowError> {
        build_main_window(app, label, geometry).map(drop)
    }
}

/// Sends a window's geometry to the store, at most once per `GEOMETRY_DELAY` per window.
#[derive(Default, Clone)]
pub struct GeometryCapture {
    pending: Arc<Mutex<HashSet<String>>>,
}

/// The position to store for a window: the inner origin that `set_position` will be given again,
/// and nothing where the platform does not report one (Wayland returns (0, 0) for every window).
pub fn captured_position(
    position_applies: bool,
    inner: Option<(i32, i32)>,
) -> (Option<i32>, Option<i32>) {
    match inner {
        Some((x, y)) if position_applies => (Some(x), Some(y)),
        _ => (None, None),
    }
}

fn read_geometry<R: Runtime>(
    window: &WebviewWindow<R>,
    previous: Option<Geometry>,
) -> Option<Geometry> {
    let maximised = window.is_maximized().ok()?;
    let size = window.inner_size().ok()?;
    let applies = position_applies();
    if maximised {
        // A maximised window keeps the size and place it will go back to.
        return Some(match previous {
            Some(p) => Geometry {
                maximised: true,
                ..p
            },
            None => Geometry {
                x: None,
                y: None,
                width: size.width,
                height: size.height,
                maximised: true,
            },
        });
    }
    if window.is_minimized().unwrap_or(false) {
        return None;
    }
    // `set_position` places the inner (client) origin of these frameless windows, so the inner
    // position is the one that restores to the same place; `outer_position` sits a shadow inset
    // above it on X11 and the window would climb by that much on every run.
    let inner = window.inner_position().ok().map(|p| (p.x, p.y));
    let (x, y) = captured_position(applies, inner);
    Some(Geometry {
        x,
        y,
        width: size.width,
        height: size.height,
        maximised: false,
    })
}

impl GeometryCapture {
    /// Call for every window event of a main window.
    pub fn on_event<R: Runtime>(&self, window: &tauri::Window<R>, event: &WindowEvent) {
        if !matches!(event, WindowEvent::Moved(_) | WindowEvent::Resized(_)) {
            return;
        }
        let label = window.label().to_string();
        if !self
            .pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(label.clone())
        {
            return;
        }
        let app = window.app_handle().clone();
        let pending = Arc::clone(&self.pending);
        std::thread::spawn(move || {
            std::thread::sleep(GEOMETRY_DELAY);
            pending
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .remove(&label);
            let Some(window) = app.get_webview_window(&label) else {
                return;
            };
            let sessions = app.state::<Sessions<R>>();
            let previous = sessions.with_store(|s| s.window(&label).and_then(|w| w.geometry));
            if let Some(geometry) = read_geometry(&window, previous) {
                if previous != Some(geometry) {
                    // A window the store no longer holds (closing) is not an error worth more
                    // than a debug line.
                    if let Err(e) = sessions.run(&app, &label, Command::SetGeometry { geometry }) {
                        log::debug!("geometry of `{label}` not saved: {e}");
                    }
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LEFT: Area = Area {
        x: 0,
        y: 0,
        width: 1920,
        height: 1080,
    };
    const RIGHT: Area = Area {
        x: 1920,
        y: 0,
        width: 1280,
        height: 1024,
    };

    fn geometry(x: Option<i32>, y: Option<i32>, width: u32, height: u32) -> Geometry {
        Geometry {
            x,
            y,
            width,
            height,
            maximised: false,
        }
    }

    #[test]
    fn a_position_on_a_monitor_is_kept() {
        let fit = fit_geometry(
            &geometry(Some(2000), Some(100), 1000, 700),
            &[LEFT, RIGHT],
            true,
        );
        assert_eq!(fit.position, Some((2000, 100)));
        assert_eq!(fit.size, (1000, 700));
    }

    #[test]
    fn a_position_on_a_monitor_that_is_gone_falls_back_to_centring() {
        let fit = fit_geometry(&geometry(Some(2000), Some(100), 1000, 700), &[LEFT], true);
        assert_eq!(fit.position, None);
    }

    #[test]
    fn a_window_wider_than_its_monitor_is_shrunk() {
        let fit = fit_geometry(
            &geometry(Some(2000), Some(0), 3000, 2000),
            &[LEFT, RIGHT],
            true,
        );
        assert_eq!(fit.size, (1280, 1024));
        let no_position = fit_geometry(&geometry(None, None, 3000, 500), &[LEFT, RIGHT], true);
        assert_eq!(no_position.size, (1920, 500));
    }

    #[test]
    fn the_captured_position_is_the_inner_origin_and_restores_unchanged() {
        let (x, y) = captured_position(true, Some((120, 80)));
        assert_eq!((x, y), (Some(120), Some(80)));
        let g = geometry(x, y, 1200, 800);
        let fit = fit_geometry(&g, &[LEFT], true);
        assert_eq!(fit.position, Some((120, 80)), "a second run must not drift");
    }

    #[test]
    fn no_position_is_captured_where_the_platform_has_none() {
        assert_eq!(captured_position(false, Some((0, 0))), (None, None));
        assert_eq!(captured_position(true, None), (None, None));
    }

    #[test]
    fn wayland_keeps_the_size_and_drops_the_position() {
        let fit = fit_geometry(&geometry(Some(10), Some(10), 900, 600), &[LEFT], false);
        assert_eq!(fit.position, None);
        assert_eq!(fit.size, (900, 600));
    }

    #[test]
    fn maximised_carries_through_and_no_monitors_leaves_the_size_alone() {
        let mut g = geometry(None, None, 900, 600);
        g.maximised = true;
        let fit = fit_geometry(&g, &[], true);
        assert!(fit.maximised);
        assert_eq!(fit.size, (900, 600));
    }
}
