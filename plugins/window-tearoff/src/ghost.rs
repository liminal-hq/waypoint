// Creates the one shared ghost window, makes it click-through safely, and probes what this system can do with it
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::time::Duration;

use log::{debug, info, warn};
use tauri::{
    AppHandle, Manager, PhysicalPosition, Runtime, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
};

use crate::{main_thread, models::Options, platform, status, status::Probes, Error};

/// Where the realise-then-click-through sequence parks the ghost, well off any screen.
const PARKING: (i32, i32) = (-5000, -5000);
/// The two positions the probe asks for; far enough apart to tell a move from a rounding error.
const PROBE_POSITIONS: [(i32, i32); 2] = [(64, 64), (104, 84)];
const PROBE_POLL: Duration = Duration::from_millis(50);
const PROBE_POLLS: u32 = 10;

/// Builds the ghost window: hidden, transparent, undecorated, resizable, always on top, non-focusable, off the taskbar, shadowless, and with no file-drop handler.
///
/// It must be resizable: a non-resizable window on Linux ignores `set_size`. It must be built hidden and made click-through only after `realise`.
pub fn build<R: Runtime>(app: &AppHandle<R>, options: &Options) -> Result<WebviewWindow<R>, Error> {
    let (width, height) = options.ghost_size;
    WebviewWindowBuilder::new(
        app,
        &options.ghost_label,
        WebviewUrl::App(options.ghost_url.clone().into()),
    )
    .title("")
    .inner_size(width, height)
    .visible(false)
    .decorations(false)
    .transparent(true)
    .resizable(true)
    .always_on_top(true)
    .focusable(false)
    .skip_taskbar(true)
    .shadow(false)
    .disable_drag_drop_handler()
    .build()
    .map_err(|error| Error::Internal(format!("cannot create the ghost window: {error}")))
}

/// Builds the ghost and makes it click-through. Must run on the main thread, with the event loop running.
pub fn create<R: Runtime>(app: &AppHandle<R>, options: &Options) {
    match build(app, options) {
        Ok(ghost) => realise(&ghost),
        Err(error) => warn!("window-tearoff: {error}"),
    }
}

/// Shows the ghost without taking activation or keyboard focus from the window being dragged from. Must run on the main thread.
pub fn show<R: Runtime>(ghost: &WebviewWindow<R>) {
    if platform::platform().needs_show_without_activating() {
        platform::show_without_activating(ghost);
    } else {
        let _ = ghost.show();
    }
}

/// Makes the ghost click-through. Must run on the main thread.
///
/// `set_ignore_cursor_events` on a window that was never shown aborts the process on Linux (`tao` unwraps the unrealised GTK window), so the ghost is shown once off-screen first. The calls are queued in order on the event loop, so they need no sleeps, and click-through is in place before the first real `show`.
pub fn realise<R: Runtime>(ghost: &WebviewWindow<R>) {
    let _ = ghost.set_position(PhysicalPosition::new(PARKING.0, PARKING.1));
    show(ghost);
    if let Err(error) = ghost.set_ignore_cursor_events(true) {
        warn!("window-tearoff: cannot make the ghost click-through: {error}");
    }
    let _ = ghost.hide();
    debug!("window-tearoff: the ghost is realised and click-through");
}

/// Shows the ghost, then positions it: on X11 a position set before `show` is overridden when the window maps. Must run on the main thread.
pub fn show_at<R: Runtime>(
    ghost: &WebviewWindow<R>,
    size: Option<(f64, f64)>,
    position: (i32, i32),
) {
    if let Some((width, height)) = size {
        let _ = ghost.set_size(tauri::LogicalSize::new(width, height));
    }
    show(ghost);
    let _ = ghost.set_position(PhysicalPosition::new(position.0, position.1));
}

fn inner_position<R: Runtime>(window: &WebviewWindow<R>) -> Option<(i32, i32)> {
    window.inner_position().ok().map(|p| (p.x, p.y))
}

/// Sets the ghost's position, then polls until its read-back equals `expected` (relative to `base` when given) or the budget runs out.
async fn settle<R: Runtime>(
    app: &AppHandle<R>,
    label: &str,
    request: (i32, i32),
    done: impl Fn((i32, i32)) -> bool + Copy + Send + 'static,
) -> Option<(i32, i32)> {
    let owned = label.to_string();
    main_thread::run(app, {
        let owned = owned.clone();
        let app = app.clone();
        move || {
            if let Some(ghost) = app.get_webview_window(&owned) {
                let _ = ghost.set_position(PhysicalPosition::new(request.0, request.1));
            }
        }
    })
    .await?;
    let mut last = None;
    for _ in 0..PROBE_POLLS {
        tokio::time::sleep(PROBE_POLL).await;
        let read = main_thread::run(app, {
            let owned = owned.clone();
            let app = app.clone();
            move || {
                app.get_webview_window(&owned)
                    .and_then(|ghost| inner_position(&ghost))
            }
        })
        .await
        .flatten();
        last = read;
        if read.is_some_and(done) {
            break;
        }
    }
    last
}

/// Finds out what this system can do, by asking it: set the ghost's position twice and read it back, and read the other windows' positions. Runs the ghost through a brief, empty, transparent, click-through show. Wayland answers `(0, 0)` to every position read, so only a real read-back counts.
pub async fn probe<R: Runtime>(app: &AppHandle<R>, ghost_label: &str) -> Probes {
    let label = ghost_label.to_string();
    let setup = main_thread::run(app, {
        let app = app.clone();
        move || {
            let platform = platform::platform();
            let ghost = app.get_webview_window(&label);
            if let Some(ghost) = ghost.as_ref().filter(|_| !platform.trusts_probes()) {
                show(ghost);
            }
            (platform, ghost.is_some())
        }
    })
    .await;
    let Some((platform, ghost_created)) = setup else {
        return Probes {
            platform: status::Platform::Unsupported,
            ghost_created: false,
            window_position: false,
            hit_test: false,
        };
    };
    if platform.trusts_probes() {
        return Probes::trusted(platform, ghost_created);
    }
    if !ghost_created {
        return Probes {
            platform,
            ghost_created,
            window_position: false,
            hit_test: false,
        };
    }

    let [first, second] = PROBE_POSITIONS;
    let first_read = settle(app, ghost_label, first, move |read| read == first).await;
    let window_position = match first_read {
        // Positions may carry a window manager offset, so the second read is judged against the first.
        Some(base) if base != (0, 0) || first == (0, 0) => {
            let delta = (second.0 - first.0, second.1 - first.1);
            let second_read = settle(app, ghost_label, second, move |read| {
                (read.0 - base.0, read.1 - base.1) == delta
            })
            .await;
            second_read
                .map(|second_read| {
                    status::position_probe_passes(PROBE_POSITIONS, [base, second_read])
                })
                .unwrap_or(false)
        }
        _ => false,
    };

    let label = ghost_label.to_string();
    let others = main_thread::run(app, {
        let app = app.clone();
        move || {
            if let Some(ghost) = app.get_webview_window(&label) {
                let _ = ghost.hide();
            }
            app.webview_windows()
                .iter()
                .filter(|(window_label, _)| **window_label != label)
                .filter_map(|(_, window)| inner_position(window))
                .collect::<Vec<_>>()
        }
    })
    .await
    .unwrap_or_default();
    let hit_test = status::hit_test_probe(window_position, &others);

    let probes = Probes {
        platform,
        ghost_created,
        window_position,
        hit_test,
    };
    info!("window-tearoff: probed {probes:?}");
    probes
}
