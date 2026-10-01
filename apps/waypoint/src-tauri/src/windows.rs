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

/// How far a new window sits down and to the right of the one it was opened from, in logical pixels.
pub const CASCADE_STEP: f64 = 30.0;

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

/// How far a window landed from where it was asked to go: the shift between the size and the inner
/// origin that were requested and the ones read back.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Correction {
    /// The size to ask for instead, or `None` when the window landed at the size it was given.
    pub size: Option<(u32, u32)>,
    /// The position to ask for instead, or `None` when the window landed where it was put.
    pub position: Option<(i32, i32)>,
}

/// Works out how to correct a window that did not land where it was asked to.
///
/// A frameless window with a shadow can be given a different inner size than the one `set_size`
/// asked for (Windows adds its caption height to it) and an inner origin that is not the one
/// `set_position` was given (the position is the outer origin, an invisible border above and to the
/// left of the content). The geometry the store keeps is the inner size and origin, so without a
/// correction a restored window grows and shifts by that much on every run. Asking for the wanted
/// value minus the error lands it right, once.
pub fn correction(
    wanted_size: (u32, u32),
    wanted_position: Option<(i32, i32)>,
    landed_size: Option<(u32, u32)>,
    landed_position: Option<(i32, i32)>,
) -> Correction {
    let shifted = |wanted: i64, landed: i64| wanted + (wanted - landed);
    let size = landed_size
        .filter(|landed| *landed != wanted_size)
        .map(|landed| {
            let axis = |w: u32, l: u32| {
                shifted(i64::from(w), i64::from(l)).clamp(1, i64::from(u32::MAX)) as u32
            };
            (axis(wanted_size.0, landed.0), axis(wanted_size.1, landed.1))
        });
    let position = match (wanted_position, landed_position) {
        (Some(wanted), Some(landed)) if wanted != landed => {
            let axis = |w: i32, l: i32| {
                shifted(i64::from(w), i64::from(l)).clamp(i64::from(i32::MIN), i64::from(i32::MAX))
                    as i32
            };
            Some((axis(wanted.0, landed.0), axis(wanted.1, landed.1)))
        }
        _ => None,
    };
    Correction { size, position }
}

/// What `place` needs of a window, so a test can stand in for the real one.
pub trait Placeable {
    fn set_size(&self, size: (u32, u32));
    fn set_position(&self, position: (i32, i32));
    fn inner_size(&self) -> Option<(u32, u32)>;
    fn inner_position(&self) -> Option<(i32, i32)>;
}

impl<R: Runtime> Placeable for WebviewWindow<R> {
    fn set_size(&self, size: (u32, u32)) {
        let _ = WebviewWindow::set_size(self, PhysicalSize::new(size.0, size.1));
    }
    fn set_position(&self, position: (i32, i32)) {
        let _ = WebviewWindow::set_position(self, PhysicalPosition::new(position.0, position.1));
    }
    fn inner_size(&self) -> Option<(u32, u32)> {
        WebviewWindow::inner_size(self)
            .ok()
            .map(|s| (s.width, s.height))
    }
    fn inner_position(&self) -> Option<(i32, i32)> {
        WebviewWindow::inner_position(self).ok().map(|p| (p.x, p.y))
    }
}

/// Gives `window` the inner `size`. With `correct` set it then reads the size back and asks again
/// for the difference, once. Only Windows sets it: elsewhere a read straight after a resize can
/// still be the old value (X11 and Wayland apply a resize asynchronously), and the correction would
/// then be wrong.
pub fn place_size(window: &impl Placeable, size: (u32, u32), correct: bool) {
    window.set_size(size);
    if !correct {
        return;
    }
    if let Some(corrected) = correction(size, None, window.inner_size(), None).size {
        window.set_size(corrected);
    }
}

/// Gives `window` the inner origin `position`; with `correct` set, corrects it once as `place_size`
/// does. Call it after the size is final, because resizing can move the origin.
pub fn place_position(window: &impl Placeable, position: (i32, i32), correct: bool) {
    window.set_position(position);
    if !correct {
        return;
    }
    if let Some(corrected) =
        correction((0, 0), Some(position), None, window.inner_position()).position
    {
        window.set_position(corrected);
    }
}

/// Sizes, then positions, `window` the way `build_main_window` does (it shows the window between
/// the two): `place_size` and `place_position` together.
#[cfg(test)]
fn place(window: &impl Placeable, size: (u32, u32), position: Option<(i32, i32)>, correct: bool) {
    place_size(window, size, correct);
    if let Some(at) = position {
        place_position(window, at, correct);
    }
}

/// Where a window opened from `source` goes: the same size, `CASCADE_STEP` logical pixels (`scale`
/// physical per logical) down and to the right, on the monitor the source is on. When that would
/// push the window past the monitor's right or bottom edge the cascade starts again from the
/// monitor's top left corner, so a run of new windows never walks off the screen. A source with no
/// position (Wayland) gives a window with none, which the compositor places.
pub fn cascade(source: &Geometry, scale: f64, monitors: &[Area]) -> Geometry {
    let step = (CASCADE_STEP * scale).round() as i32;
    let (Some(x), Some(y)) = (source.x, source.y) else {
        return Geometry {
            x: None,
            y: None,
            maximised: false,
            ..*source
        };
    };
    let Some(monitor) = monitors
        .iter()
        .find(|m| m.contains(x as i64 + 40, y as i64 + 16))
    else {
        return Geometry {
            x: None,
            y: None,
            maximised: false,
            ..*source
        };
    };
    let fits = |at: (i32, i32)| {
        at.0 as i64 + source.width as i64 <= monitor.x as i64 + monitor.width as i64
            && at.1 as i64 + source.height as i64 <= monitor.y as i64 + monitor.height as i64
    };
    let next = (x + step, y + step);
    let at = if fits(next) {
        next
    } else {
        (monitor.x + step, monitor.y + step)
    };
    Geometry {
        x: Some(at.0),
        y: Some(at.1),
        maximised: false,
        ..*source
    }
}

/// The geometry a window opened from `opener` should have: see `cascade`. `None` when `opener` is
/// gone or cannot be read, which leaves the new window at the default size and place.
fn cascaded<R: Runtime>(app: &AppHandle<R>, opener: &str) -> Option<Geometry> {
    let source = app.get_webview_window(opener)?;
    let scale = source.scale_factor().ok()?;
    let size = if source.is_maximized().unwrap_or(false) {
        // A maximised window has no size worth copying.
        PhysicalSize::new(
            (DEFAULT_SIZE.0 * scale).round() as u32,
            (DEFAULT_SIZE.1 * scale).round() as u32,
        )
    } else {
        source.inner_size().ok()?
    };
    let position = if position_applies() && !source.is_maximized().unwrap_or(false) {
        // The inner position is the one `set_position` takes for these frameless windows (X11).
        source.inner_position().ok()
    } else {
        None
    };
    Some(cascade(
        &Geometry {
            x: position.map(|p| p.x),
            y: position.map(|p| p.y),
            width: size.width,
            height: size.height,
            maximised: false,
        },
        scale,
        &monitors(app),
    ))
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
    // Showing comes before positioning: on X11 a position set while the window is hidden is
    // overridden when it is mapped (the milestone 3 multi-window spike).
    let mut fit = None;
    if let Some(g) = geometry {
        let applied = fit_geometry(g, &monitors(app), position_applies());
        place_size(&window, applied.size, cfg!(windows));
        if applied.maximised {
            let _ = window.maximize();
        }
        fit = Some(applied);
    }
    window.show().map_err(fail)?;
    match fit.and_then(|f| f.position) {
        Some(at) => place_position(&window, at, cfg!(windows)),
        None => {
            let _ = window.center();
        }
    }
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
        opener: Option<&str>,
    ) -> Result<(), WindowError> {
        let placed = match geometry {
            Some(_) => None,
            None => opener.and_then(|opener| cascaded(app, opener)),
        };
        build_main_window(app, label, geometry.or(placed.as_ref())).map(drop)
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
                    // than a debug line, and must not be registered again: that would save an
                    // empty ghost window.
                    if let Err(e) =
                        sessions.run_existing(&app, &label, Command::SetGeometry { geometry })
                    {
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

    /// A frameless, shadowed window as Windows 11 makes it: `set_size` adds a caption height to the
    /// inner size and `set_position` puts the outer origin, so the inner origin lands a border away.
    struct Framed {
        extra_height: u32,
        border: (i32, i32),
        inner_size: std::cell::Cell<(u32, u32)>,
        inner_position: std::cell::Cell<(i32, i32)>,
    }

    impl Framed {
        fn windows_11() -> Self {
            Self {
                extra_height: 50,
                border: (12, 2),
                inner_size: std::cell::Cell::new((1100, 720)),
                inner_position: std::cell::Cell::new((0, 0)),
            }
        }
        /// What the store would keep for this window.
        fn saved(&self) -> Geometry {
            let (width, height) = self.inner_size.get();
            let (x, y) = self.inner_position.get();
            geometry(Some(x), Some(y), width, height)
        }
    }

    impl Placeable for Framed {
        fn set_size(&self, size: (u32, u32)) {
            self.inner_size.set((size.0, size.1 + self.extra_height));
        }
        fn set_position(&self, position: (i32, i32)) {
            self.inner_position
                .set((position.0 + self.border.0, position.1 + self.border.1));
        }
        fn inner_size(&self) -> Option<(u32, u32)> {
            Some(self.inner_size.get())
        }
        fn inner_position(&self) -> Option<(i32, i32)> {
            Some(self.inner_position.get())
        }
    }

    /// Restores `saved` into a fresh window the way `build_main_window` does.
    fn restore(saved: &Geometry, correct: bool) -> Framed {
        let window = Framed::windows_11();
        let fit = fit_geometry(saved, &[LEFT], true);
        place(&window, fit.size, fit.position, correct);
        window
    }

    #[test]
    fn a_corrected_restore_is_a_fixed_point_through_save_and_restore() {
        let first = geometry(Some(300), Some(200), 1000, 686);
        let mut saved = first;
        for run in 0..4 {
            saved = restore(&saved, true).saved();
            assert_eq!(saved, first, "run {run} moved or resized the window");
        }
    }

    #[test]
    fn without_the_correction_every_restart_grows_and_shifts_the_window() {
        let mut saved = geometry(Some(300), Some(200), 1000, 686);
        for _ in 0..3 {
            saved = restore(&saved, false).saved();
        }
        // The Windows 11 evidence: +50 physical px of height and the origin a border away, per run.
        assert_eq!(saved.height, 686 + 3 * 50);
        assert_eq!((saved.x, saved.y), (Some(336), Some(206)));
    }

    #[test]
    fn a_window_that_lands_exactly_needs_no_correction() {
        assert_eq!(
            correction((900, 600), Some((10, 20)), Some((900, 600)), Some((10, 20))),
            Correction::default()
        );
        assert_eq!(
            correction((900, 600), None, None, None),
            Correction::default()
        );
    }

    #[test]
    fn the_correction_asks_for_the_wanted_value_minus_the_error() {
        let fix = correction(
            (900, 600),
            Some((100, 50)),
            Some((900, 650)),
            Some((112, 52)),
        );
        assert_eq!(fix.size, Some((900, 550)));
        assert_eq!(fix.position, Some((88, 48)));
        // A size error never asks for an empty window.
        let tiny = correction((10, 10), None, Some((10, 40)), None);
        assert_eq!(tiny.size, Some((10, 1)));
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

    #[test]
    fn a_new_window_cascades_down_and_right_by_the_scaled_step() {
        let source = geometry(Some(100), Some(80), 1000, 700);
        let next = cascade(&source, 1.0, &[LEFT]);
        assert_eq!((next.x, next.y), (Some(130), Some(110)));
        assert_eq!((next.width, next.height), (1000, 700));
        let scaled = cascade(&source, 2.0, &[LEFT]);
        assert_eq!((scaled.x, scaled.y), (Some(160), Some(140)));
    }

    #[test]
    fn a_cascade_that_would_leave_the_monitor_starts_again_at_its_corner() {
        // The second monitor starts at x = 1920; the window would end past its right edge.
        let source = geometry(Some(2000), Some(100), 1250, 700);
        let next = cascade(&source, 1.0, &[LEFT, RIGHT]);
        assert_eq!((next.x, next.y), (Some(1950), Some(30)));
        // Past the bottom edge as well.
        let tall = geometry(Some(10), Some(400), 800, 680);
        let next = cascade(&tall, 1.0, &[LEFT]);
        assert_eq!((next.x, next.y), (Some(30), Some(30)));
    }

    #[test]
    fn a_source_with_no_position_or_off_every_monitor_leaves_placement_to_the_system() {
        let wayland = cascade(&geometry(None, None, 900, 600), 1.0, &[LEFT]);
        assert_eq!((wayland.x, wayland.y), (None, None));
        assert_eq!((wayland.width, wayland.height), (900, 600));
        let lost = cascade(&geometry(Some(9000), Some(9000), 900, 600), 1.0, &[LEFT]);
        assert_eq!((lost.x, lost.y), (None, None));
    }

    #[test]
    fn a_cascaded_position_survives_fitting() {
        let next = cascade(&geometry(Some(100), Some(80), 1000, 700), 1.0, &[LEFT]);
        let fit = fit_geometry(&next, &[LEFT], true);
        assert_eq!(fit.position, Some((130, 110)));
    }
}
