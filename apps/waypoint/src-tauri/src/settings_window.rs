// The Settings window: one window for the application's settings, opened from a shortcut or a menu
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use tauri::{AppHandle, Manager, Runtime, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

/// The label `WindowKind::Settings` routes to `SettingsScreen`.
pub const SETTINGS_LABEL: &str = "settings";

const SIZE: (f64, f64) = (820.0, 600.0);
const MIN_SIZE: (f64, f64) = (420.0, 360.0);

/// Opens the Settings window, or brings the one that is open to the front. There is only one.
pub fn open<R: Runtime>(app: &AppHandle<R>) -> Result<WebviewWindow<R>, String> {
    if let Some(window) = app.get_webview_window(SETTINGS_LABEL) {
        // A minimised or hidden window would otherwise stay out of sight after the request.
        let _ = window.unminimize();
        window.show().map_err(|e| e.to_string())?;
        window.set_focus().map_err(|e| e.to_string())?;
        return Ok(window);
    }
    // The same frameless, transparent window the main windows are, so the shared chrome draws it.
    let window =
        WebviewWindowBuilder::new(app, SETTINGS_LABEL, WebviewUrl::App("index.html".into()))
            .title("Waypoint — Settings")
            .inner_size(SIZE.0, SIZE.1)
            .min_inner_size(MIN_SIZE.0, MIN_SIZE.1)
            .resizable(true)
            .decorations(false)
            .transparent(true)
            .shadow(true)
            .visible(false)
            .build()
            .map_err(|e| e.to_string())?;
    window.show().map_err(|e| e.to_string())?;
    let _ = window.set_focus();
    Ok(window)
}

/// Opens (or focuses) the Settings window. Async so the window is made off the main thread, which
/// `WebviewWindowBuilder` requires on Windows.
#[tauri::command]
pub async fn open_settings_window(app: AppHandle) -> Result<(), String> {
    open(&app).map(drop)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tauri::test::{mock_builder, mock_context, noop_assets};

    #[test]
    fn the_label_is_the_one_the_front_end_routes_to_the_settings_screen() {
        assert_eq!(
            waypoint_protocol::WindowKind::from_label(SETTINGS_LABEL),
            Some(waypoint_protocol::WindowKind::Settings)
        );
    }

    #[test]
    fn opening_twice_gives_one_window() {
        let app = mock_builder()
            .build(mock_context(noop_assets()))
            .expect("the mock app builds");
        let first = open(app.handle()).expect("the window opens");
        let second = open(app.handle()).expect("the window is found");
        assert_eq!(first.label(), second.label());
        let settings: Vec<_> = app
            .webview_windows()
            .into_keys()
            .filter(|label| label == SETTINGS_LABEL)
            .collect();
        assert_eq!(settings.len(), 1);
    }
}
