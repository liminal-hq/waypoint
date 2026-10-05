// Remembers where the Settings, Properties and Operations windows were left, through `tauri-plugin-window-state`
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use tauri::plugin::TauriPlugin;
use tauri::{Runtime, WebviewWindow};
use tauri_plugin_window_state::{StateFlags, WindowExt};

use crate::ops_window::OPS_LABEL;
use crate::properties_window::LABEL_PREFIX as PROPERTIES_PREFIX;
use crate::settings_window::SETTINGS_LABEL;

/// The one key every Properties window shares, so the remembered place is the same however many
/// have been opened and the state file does not grow with the label counter.
const PROPERTIES_KEY: &str = "properties";

/// What is remembered: the outer position, and nothing else. The size is left out because the
/// plugin restores it with `set_size`, which on Windows gives a frameless window with a shadow more
/// than it asked for (the caption height, see `windows::correction`), so a remembered size would
/// grow on every open; the position is read and set as the same outer origin and does not drift.
/// Not visibility (these windows are built hidden and shown by the code that opens them),
/// decorations (they are frameless), fullscreen or maximised, which a dialog has no use for.
fn flags() -> StateFlags {
    StateFlags::POSITION
}

/// The key a window's state is stored under: every `properties-{n}` shares one, any other label is
/// its own.
fn state_key(label: &str) -> &str {
    if label.starts_with(PROPERTIES_PREFIX) {
        PROPERTIES_KEY
    } else {
        label
    }
}

/// Whether the plugin manages the window with this state key. Only the three dialogs, which nothing
/// else places: the main windows and the Shelf keep their geometry in the session, and the
/// tear-off ghost must stay hidden and unsaved.
fn is_managed(key: &str) -> bool {
    key == SETTINGS_LABEL || key == PROPERTIES_KEY || key == OPS_LABEL
}

/// Applies the remembered place again once the window is shown, on the platforms that need it.
/// The plugin restores a window as it is created, hidden, which is enough on Windows and is all
/// that happens there. On X11 a position set while the window is hidden is lost when it maps, so it
/// is set once more now that it is on screen.
///
/// That second restore runs on the main thread, never on the thread that built the window: the
/// plugin's restore holds its cache lock while it waits for the main thread to move the window,
/// and its own move handler (which `show()` sets off) takes that lock on the main thread, so a
/// restore from any other thread can deadlock the whole application.
#[cfg(not(windows))]
pub fn restore_after_show<R: Runtime>(window: &WebviewWindow<R>) {
    use tauri::Manager;
    let window = window.clone();
    let app = window.app_handle().clone();
    let _ = app.run_on_main_thread(move || {
        if let Err(error) = window.restore_state(flags()) {
            log::warn!("could not restore the place of {}: {error}", window.label());
        }
    });
}

/// Windows needs nothing after `show()`: the plugin's restore at creation already placed the window.
#[cfg(windows)]
pub fn restore_after_show<R: Runtime>(_window: &WebviewWindow<R>) {}

/// The plugin, restoring a dialog window as it is created and saving it as the app exits.
/// `denylist` carries the labels another plugin needs left alone (the tear-off ghost).
pub fn plugin<R: Runtime>(denylist: &[String]) -> TauriPlugin<R> {
    let denylist: Vec<&str> = denylist.iter().map(String::as_str).collect();
    tauri_plugin_window_state::Builder::new()
        .with_state_flags(flags())
        .with_denylist(&denylist)
        .with_filter(is_managed)
        .map_label(state_key)
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;
    use waypoint_protocol::{WindowKind, SHELF_LABEL};

    #[test]
    fn every_properties_window_shares_one_key() {
        let cases = [
            ("properties-1", "properties"),
            ("properties-2", "properties"),
            ("properties-4096", "properties"),
            ("settings", "settings"),
            ("main-1", "main-1"),
            ("shelf", "shelf"),
            ("ops", "ops"),
            ("tear-ghost", "tear-ghost"),
        ];
        for (label, key) in cases {
            assert_eq!(state_key(label), key, "{label}");
        }
    }

    #[test]
    fn only_the_three_dialogs_are_managed() {
        let cases = [
            ("settings", true),
            ("ops", true),
            ("properties-1", true),
            ("properties-12", true),
            ("main-1", false),
            ("main-7", false),
            (SHELF_LABEL, false),
            ("tear-ghost", false),
            ("mystery", false),
            ("", false),
        ];
        for (label, managed) in cases {
            assert_eq!(is_managed(state_key(label)), managed, "{label}");
        }
    }

    #[test]
    fn the_managed_labels_are_the_dialog_kinds() {
        assert_eq!(WindowKind::from_label(OPS_LABEL), Some(WindowKind::Ops));
        assert_eq!(
            WindowKind::from_label(SETTINGS_LABEL),
            Some(WindowKind::Settings)
        );
        assert_eq!(
            WindowKind::from_label(&format!("{PROPERTIES_PREFIX}3")),
            Some(WindowKind::Properties)
        );
    }

    #[test]
    fn only_the_position_is_remembered_and_nothing_that_shows_or_frames_a_window() {
        let flags = flags();
        assert!(flags.contains(StateFlags::POSITION));
        assert!(!flags.intersects(
            StateFlags::SIZE
                | StateFlags::VISIBLE
                | StateFlags::DECORATIONS
                | StateFlags::FULLSCREEN
                | StateFlags::MAXIMIZED
        ));
    }
}
