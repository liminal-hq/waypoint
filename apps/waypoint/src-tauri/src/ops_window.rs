// The Operations window: the one window that lists the queue, opened from the status bar popover
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use tauri::{AppHandle, Manager, Runtime, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::dialog_window_state;

/// The label `WindowKind::Ops` routes to `OpsScreen`.
pub const OPS_LABEL: &str = "ops";

const SIZE: (f64, f64) = (460.0, 560.0);
const MIN_SIZE: (f64, f64) = (360.0, 300.0);

/// Opens the Operations window, or brings the one that is open to the front. There is only one.
pub fn open<R: Runtime>(app: &AppHandle<R>) -> Result<WebviewWindow<R>, String> {
    if let Some(window) = app.get_webview_window(OPS_LABEL) {
        // A minimised or hidden window would otherwise stay out of sight after the click.
        let _ = window.unminimize();
        window.show().map_err(|e| e.to_string())?;
        window.set_focus().map_err(|e| e.to_string())?;
        return Ok(window);
    }
    // The same frameless, transparent window the main windows are, so the shared chrome draws it.
    let window = WebviewWindowBuilder::new(app, OPS_LABEL, WebviewUrl::App("index.html".into()))
        .title("Operations")
        .inner_size(SIZE.0, SIZE.1)
        .min_inner_size(MIN_SIZE.0, MIN_SIZE.1)
        .resizable(true)
        .decorations(false)
        .transparent(true)
        .shadow(true)
        .visible(false)
        .center()
        .build()
        .map_err(|e| e.to_string())?;
    window.show().map_err(|e| e.to_string())?;
    dialog_window_state::restore_after_show(&window);
    let _ = window.set_focus();
    Ok(window)
}

/// Opens (or focuses) the Operations window. Async so the window is made off the main thread, which
/// `WebviewWindowBuilder` requires on Windows.
#[tauri::command]
pub async fn open_ops_window(app: AppHandle) -> Result<(), String> {
    open(&app).map(drop)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tauri::test::{mock_builder, mock_context, noop_assets};

    #[test]
    fn the_label_is_the_one_the_front_end_routes_to_the_operations_screen() {
        assert_eq!(
            waypoint_protocol::WindowKind::from_label(OPS_LABEL),
            Some(waypoint_protocol::WindowKind::Ops)
        );
    }

    #[test]
    fn opening_twice_gives_one_window() {
        let app = mock_builder()
            .plugin(dialog_window_state::plugin(&[]))
            .build(mock_context(noop_assets()))
            .expect("the mock app builds");
        let first = open(app.handle()).expect("the window opens");
        let second = open(app.handle()).expect("the window is found");
        assert_eq!(first.label(), second.label());
        assert_eq!(app.webview_windows().len(), 1);
    }
}
