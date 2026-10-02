// The Shelf window: the one window the Shelf lives in while it is undocked, made by the session and raised or hidden from the pages
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The session plugin decides when there is a Shelf window (`SetShelfUndocked`) and has
// `TauriWindowFactory::create_shelf` build it here; closing it docks the Shelf again. The window is
// frameless and transparent like the others, so the shared chrome draws its title bar, and it is
// placed from the geometry the store kept. Showing and hiding it is the pages' (Ctrl+B and the
// status bar button) and is not a session change: a hidden Shelf window is still undocked.

use tauri::{
    AppHandle, Emitter, Manager, Runtime, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
};
use tauri_plugin_waypoint_session::WindowError;
use waypoint_protocol::SHELF_LABEL;
use waypoint_session::ShelfWindow;

use crate::windows::{
    after_show, fit_geometry, monitors, place_position, place_size, position_applies, Fit,
};

/// Sent to every window when the Shelf window is shown or hidden, with a `bool` payload.
pub const VISIBLE_EVENT: &str = "waypoint://shelf-window-visible";

/// What the window opens at when the store has no geometry for it, in logical pixels.
const SIZE: (f64, f64) = (760.0, 260.0);
const MIN_SIZE: (f64, f64) = (360.0, 170.0);

/// Builds the Shelf window from what the store kept: its place and size, and whether it stays above
/// other windows. The window is shown once placed, like a main window.
pub fn build<R: Runtime>(
    app: &AppHandle<R>,
    shelf: &ShelfWindow,
) -> Result<WebviewWindow<R>, WindowError> {
    let fail = |e: tauri::Error| WindowError::Failed(e.to_string());
    let window = WebviewWindowBuilder::new(app, SHELF_LABEL, WebviewUrl::App("index.html".into()))
        .title("Waypoint — Shelf")
        .inner_size(SIZE.0, SIZE.1)
        .min_inner_size(MIN_SIZE.0, MIN_SIZE.1)
        .resizable(true)
        .decorations(false)
        .transparent(true)
        .shadow(true)
        .always_on_top(shelf.on_top)
        .visible(false)
        .build()
        .map_err(fail)?;
    let mut fit: Option<Fit> = None;
    if let Some(g) = &shelf.geometry {
        let applied = fit_geometry(g, &monitors(app), position_applies());
        place_size(&window, applied.size, cfg!(windows));
        fit = Some(applied);
    }
    window.show().map_err(fail)?;
    match after_show(fit.as_ref()) {
        crate::windows::AfterShow::At(at) => place_position(&window, at, cfg!(windows)),
        crate::windows::AfterShow::Centre => {
            let _ = window.center();
        }
        crate::windows::AfterShow::Leave => {}
    }
    let _ = window.set_focus();
    Ok(window)
}

fn visible<R: Runtime>(app: &AppHandle<R>) -> bool {
    app.get_webview_window(SHELF_LABEL)
        .is_some_and(|w| w.is_visible().unwrap_or(false) && !w.is_minimized().unwrap_or(false))
}

fn announce<R: Runtime>(app: &AppHandle<R>, shown: bool) {
    if let Err(e) = app.emit(VISIBLE_EVENT, shown) {
        log::debug!("could not tell the windows the Shelf window is {shown}: {e}");
    }
}

/// Brings the Shelf window to the front (un-minimising and showing it). Without one it does
/// nothing, and says so.
pub fn raise<R: Runtime>(app: &AppHandle<R>) -> Result<bool, String> {
    let Some(window) = app.get_webview_window(SHELF_LABEL) else {
        return Ok(false);
    };
    let _ = window.unminimize();
    window.show().map_err(|e| e.to_string())?;
    let _ = window.set_focus();
    announce(app, true);
    Ok(true)
}

/// Hides the Shelf window; the Shelf stays undocked, and the main windows keep no dock.
pub fn hide<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(SHELF_LABEL) {
        window.hide().map_err(|e| e.to_string())?;
        announce(app, false);
    }
    Ok(())
}

/// Raises the Shelf window, or hides it when it is on screen and focused: the Ctrl+B and status bar
/// toggle. Returns whether the window is shown afterwards.
pub fn toggle<R: Runtime>(app: &AppHandle<R>) -> Result<bool, String> {
    let Some(window) = app.get_webview_window(SHELF_LABEL) else {
        return Ok(false);
    };
    if visible(app) && window.is_focused().unwrap_or(false) {
        hide(app)?;
        return Ok(false);
    }
    raise(app)
}

/// Raises the Shelf window; `false` when the Shelf is docked and there is none. Async so a window
/// operation never runs on the main thread's event handling.
#[tauri::command]
pub async fn raise_shelf_window(app: AppHandle) -> Result<bool, String> {
    raise(&app)
}

/// Raises or hides the Shelf window; see `toggle`.
#[tauri::command]
pub async fn toggle_shelf_window(app: AppHandle) -> Result<bool, String> {
    toggle(&app)
}

/// Hides the Shelf window.
#[tauri::command]
pub async fn hide_shelf_window(app: AppHandle) -> Result<(), String> {
    hide(&app)
}

/// Whether the Shelf window is on screen, for a page that starts after it was shown or hidden.
#[tauri::command]
pub async fn shelf_window_visible(app: AppHandle) -> Result<bool, String> {
    Ok(visible(&app))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tauri::test::{mock_builder, mock_context, noop_assets};

    fn app() -> tauri::App<tauri::test::MockRuntime> {
        mock_builder()
            .build(mock_context(noop_assets()))
            .expect("the mock app builds")
    }

    #[test]
    fn the_label_is_the_one_the_front_end_routes_to_the_shelf_screen() {
        assert_eq!(
            waypoint_protocol::WindowKind::from_label(SHELF_LABEL),
            Some(waypoint_protocol::WindowKind::Shelf)
        );
    }

    #[test]
    fn building_gives_one_window_with_the_label() {
        let app = app();
        let window = build(app.handle(), &ShelfWindow::default()).expect("the window opens");
        assert_eq!(window.label(), SHELF_LABEL);
        assert!(app.get_webview_window(SHELF_LABEL).is_some());
    }

    #[test]
    fn with_no_shelf_window_there_is_nothing_to_raise_or_toggle() {
        let app = app();
        assert_eq!(raise(app.handle()), Ok(false));
        assert_eq!(toggle(app.handle()), Ok(false));
        assert!(!visible(app.handle()));
    }

    #[test]
    fn hiding_leaves_the_window_in_place_and_raising_finds_it_again() {
        // The mock runtime does not track visibility, so only the window's survival is checked.
        let app = app();
        build(app.handle(), &ShelfWindow::default()).unwrap();
        hide(app.handle()).unwrap();
        assert!(app.get_webview_window(SHELF_LABEL).is_some());
        assert_eq!(raise(app.handle()), Ok(true));
    }
}
