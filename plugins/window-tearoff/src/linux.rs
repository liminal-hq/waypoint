// Linux specifics: which display server is running, and whether a mouse button is held
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use gdk::prelude::*;

use crate::status::Platform;

/// Maps a GDK display's type name to the display server. XWayland counts as X11: the app is an X11 client there and sees X11 cursor semantics, which are live only while a button is held.
pub fn classify_display(type_name: &str) -> Platform {
    if type_name.contains("Wayland") {
        Platform::Wayland
    } else if type_name.contains("X11") {
        Platform::X11
    } else {
        Platform::Unsupported
    }
}

/// Must run on the main thread.
pub fn platform() -> Platform {
    // GDK aborts if it is used before GTK is initialised, as in a unit test with no display.
    if !gtk::is_initialized_main_thread() {
        return Platform::Unsupported;
    }
    gdk::Display::default()
        .map(|display| classify_display(display.type_().name()))
        .unwrap_or(Platform::Unsupported)
}

/// Whether any mouse button is held, read from the pointer's state on the root window. Must run on the main thread. Assumes held if it cannot be read, so a drag in progress is not treated as finished.
pub fn buttons_held() -> bool {
    if !gtk::is_initialized_main_thread() {
        return false;
    }
    let held = || -> Option<bool> {
        let pointer = gdk::Display::default()?.default_seat()?.pointer()?;
        let root = gdk::Screen::default()?.root_window()?;
        let (_, _, _, mask) = root.device_position(&pointer);
        Some(mask.intersects(
            gdk::ModifierType::BUTTON1_MASK
                | gdk::ModifierType::BUTTON2_MASK
                | gdk::ModifierType::BUTTON3_MASK,
        ))
    };
    held().unwrap_or(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_types_map_to_servers() {
        assert_eq!(classify_display("GdkWaylandDisplay"), Platform::Wayland);
        assert_eq!(classify_display("GdkX11Display"), Platform::X11);
        assert_eq!(
            classify_display("GdkBroadwayDisplay"),
            Platform::Unsupported
        );
        assert_eq!(classify_display(""), Platform::Unsupported);
    }
}
