// The Settings window: one window for the application's settings, opened from a shortcut or a menu
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use tauri::{
    AppHandle, Emitter, Manager, Runtime, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
};

use crate::dialog_window_state;

/// The label `WindowKind::Settings` routes to `SettingsScreen`.
pub const SETTINGS_LABEL: &str = "settings";

/// Sent to the Settings window to show another page, with the page's id as the payload.
pub const SECTION_EVENT: &str = "waypoint://settings-section";

/// A page id the window can be asked to open: the ids are lower-case letters, so anything else is
/// dropped rather than put in a URL.
fn clean_section(section: Option<&str>) -> Option<&str> {
    section.filter(|id| {
        !id.is_empty() && id.len() <= 32 && id.bytes().all(|b| b.is_ascii_alphabetic())
    })
}

const SIZE: (f64, f64) = (820.0, 600.0);
const MIN_SIZE: (f64, f64) = (420.0, 360.0);

/// Opens the Settings window, or brings the one that is open to the front. There is only one.
/// With a `section` it shows that page: a new window is opened on it and an open one is told to
/// switch (the page ids are in `settingsSections.tsx`).
pub fn open<R: Runtime>(
    app: &AppHandle<R>,
    section: Option<&str>,
) -> Result<WebviewWindow<R>, String> {
    let section = clean_section(section);
    if let Some(window) = app.get_webview_window(SETTINGS_LABEL) {
        // A minimised or hidden window would otherwise stay out of sight after the request.
        let _ = window.unminimize();
        window.show().map_err(|e| e.to_string())?;
        window.set_focus().map_err(|e| e.to_string())?;
        if let Some(section) = section {
            window
                .emit(SECTION_EVENT, section)
                .map_err(|e| e.to_string())?;
        }
        return Ok(window);
    }
    // The same frameless, transparent window the main windows are, so the shared chrome draws it.
    let url = match section {
        Some(section) => format!("index.html?section={section}"),
        None => "index.html".to_owned(),
    };
    let window = WebviewWindowBuilder::new(app, SETTINGS_LABEL, WebviewUrl::App(url.into()))
        .title("Settings")
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

/// Opens (or focuses) the Settings window, on `section` when one is given. Async so the window is
/// made off the main thread, which `WebviewWindowBuilder` requires on Windows.
#[tauri::command]
pub async fn open_settings_window(app: AppHandle, section: Option<String>) -> Result<(), String> {
    open(&app, section.as_deref()).map(drop)
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
            .plugin(dialog_window_state::plugin(&[]))
            .build(mock_context(noop_assets()))
            .expect("the mock app builds");
        let first = open(app.handle(), None).expect("the window opens");
        let second = open(app.handle(), Some("experimental")).expect("the window is found");
        assert_eq!(first.label(), second.label());
        let settings: Vec<_> = app
            .webview_windows()
            .into_keys()
            .filter(|label| label == SETTINGS_LABEL)
            .collect();
        assert_eq!(settings.len(), 1);
    }

    #[test]
    fn only_a_plain_page_id_is_passed_on() {
        assert_eq!(clean_section(Some("experimental")), Some("experimental"));
        assert_eq!(clean_section(Some("a&b=c")), None);
        assert_eq!(clean_section(Some("")), None);
        assert_eq!(clean_section(None), None);
    }
}
