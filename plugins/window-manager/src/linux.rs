// Asks the Wayland or X11 compositor for its window menu through GDK
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use gdk::glib::translate::{ToGlibPtr, ToGlibPtrMut};
use gdk::prelude::*;
use gtk::prelude::*;
use log::{info, warn};
use tauri::{Runtime, WebviewWindow};

use crate::{
    main_thread,
    models::{Session, WindowCapabilities, WindowPosition},
    session,
};

/// Detects the session, preferring the backend GDK actually opened over the environment.
///
/// GDK objects may only be touched on the GTK main thread after GTK is initialised; anywhere else this falls back to the environment rule.
pub fn detect_session() -> Session {
    if gtk::is_initialized_main_thread() {
        if let Some(found) = gdk::Display::default()
            .and_then(|display| session::from_gdk_display_type(display.type_().name()))
        {
            return found;
        }
    }
    session::from_process_env()
}

/// Wayland has no client-side "keep above" (the compositor's window menu offers it), so only X11 reports `always_on_top`.
pub fn capabilities_for(session: Session) -> WindowCapabilities {
    match session {
        Session::Wayland => WindowCapabilities::new(Session::Wayland, false, true),
        Session::X11 => WindowCapabilities::new(Session::X11, true, true),
        other => WindowCapabilities::new(other, true, false),
    }
}

pub async fn capabilities<R: Runtime>(window: &WebviewWindow<R>) -> WindowCapabilities {
    let detected = main_thread::run(window, detect_session)
        .await
        .unwrap_or_else(session::from_process_env);
    capabilities_for(detected)
}

pub async fn show_system_window_menu<R: Runtime>(
    window: &WebviewWindow<R>,
    position: WindowPosition,
) -> bool {
    let target = window.clone();
    let shown = main_thread::run(window, move || show_menu(&target, position))
        .await
        .unwrap_or(false);
    info!(
        "system window menu for label={}: shown={shown}",
        window.label()
    );
    shown
}

/// Must run on the GTK main thread.
fn show_menu<R: Runtime>(window: &WebviewWindow<R>, position: WindowPosition) -> bool {
    let Ok(gtk_window) = window.gtk_window() else {
        warn!("the GTK window is unavailable");
        return false;
    };
    let Some(gdk_window) = gtk_window.window() else {
        warn!("the window is not realised yet");
        return false;
    };
    let Some(pointer) = gdk_window
        .display()
        .default_seat()
        .and_then(|seat| seat.pointer())
    else {
        warn!("no default seat pointer");
        return false;
    };

    // The webview fills the window, so window-relative CSS pixels equal GDK logical
    // coordinates on the toplevel surface (this includes any transparent margin the app
    // draws, as that is part of the webview).
    let (_, origin_x, origin_y) = gdk_window.origin();
    let mut event = gdk::Event::new(gdk::EventType::ButtonPress);
    let raw = ToGlibPtrMut::<*mut gdk::ffi::GdkEvent>::to_glib_none_mut(&mut event).0;
    // SAFETY: `raw` points at the GdkEvent owned by `event`, allocated by `gdk_event_new`
    // as a GdkEventButton (the `ButtonPress` type), so viewing it as one is valid. The
    // window and device fields start null and the event releases them with `g_object_unref`
    // when it is freed, so each is stored as a full reference (`to_glib_full`); we keep no
    // other owner of those references and never free them ourselves.
    unsafe {
        let button = &mut *(raw as *mut gdk::ffi::GdkEventButton);
        button.window = gdk_window.to_glib_full();
        button.device = pointer.to_glib_full();
        button.send_event = 0;
        button.time = gtk::current_event_time();
        button.button = 3;
        button.x = position.x;
        button.y = position.y;
        button.x_root = f64::from(origin_x) + position.x;
        button.y_root = f64::from(origin_y) + position.y;
    }

    let accepted = gdk_window.show_window_menu(&mut event);
    if !accepted {
        warn!("the compositor refused the window menu request");
    }
    accepted
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capabilities_table() {
        assert_eq!(
            capabilities_for(Session::Wayland),
            WindowCapabilities::new(Session::Wayland, false, true)
        );
        assert_eq!(
            capabilities_for(Session::X11),
            WindowCapabilities::new(Session::X11, true, true)
        );
        assert_eq!(
            capabilities_for(Session::Unknown),
            WindowCapabilities::new(Session::Unknown, true, false)
        );
    }

    /// Run on a real desktop: `cargo test live_capabilities -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn live_capabilities() {
        let capabilities = capabilities_for(detect_session());
        println!("{}", serde_json::to_string(&capabilities).unwrap());
    }
}
