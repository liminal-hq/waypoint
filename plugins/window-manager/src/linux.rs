// Asks the Wayland or X11 compositor for its window menu through GDK
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use gdk::glib::translate::{ToGlibPtr, ToGlibPtrMut};
use gdk::prelude::*;
use gtk::prelude::*;
use log::{info, warn};
use tauri::{Emitter, Runtime, WebviewWindow, Window};

use crate::{
    main_thread,
    models::{Session, WindowCapabilities, WindowPosition, ALWAYS_ON_TOP_CHANGED_EVENT},
    session,
};

/// Detects the session, preferring the backend GDK actually opened over the environment.
///
/// GDK objects may only be touched on the GTK main thread after GTK is initialised; anywhere else, or before GTK has a display, this falls back to the environment rule. A live display of an unrecognised type stays `Unknown` whatever the environment says.
pub fn detect_session() -> Session {
    if gtk::is_initialized_main_thread() {
        if let Some(display) = gdk::Display::default() {
            return session::from_live_display_type(display.type_().name());
        }
    }
    session::from_process_env()
}

/// Wayland has no client-side "keep above" (the compositor's window menu offers it), so only X11 reports `always_on_top`; a backend that is neither reports nothing.
pub fn capabilities_for(session: Session) -> WindowCapabilities {
    match session {
        Session::Wayland => WindowCapabilities::new(Session::Wayland, false, true),
        Session::X11 => WindowCapabilities::new(Session::X11, true, true),
        // A backend GDK opened that is neither (such as Broadway) offers nothing: `getAlwaysOnTop()` can only be answered on X11.
        other => WindowCapabilities::new(other, false, false),
    }
}

pub async fn capabilities<R: Runtime>(window: &WebviewWindow<R>) -> WindowCapabilities {
    let detected = main_thread::run(window, detect_session)
        .await
        .unwrap_or_else(session::from_process_env);
    capabilities_for(detected)
}

/// Whether the X11 window manager lists `_NET_WM_STATE_ABOVE` for the window, read straight from the `_NET_WM_STATE` property.
///
/// GDK's own `ABOVE` window state only echoes `gdk_window_set_keep_above` and never follows the window manager, so it cannot see a change made from the window manager's menu. A window with no `_NET_WM_STATE` property has no state set, so a missing property reads as false. Must run on the GTK main thread, on X11.
fn read_above(gdk_window: &gdk::Window) -> bool {
    let property = gdk::Atom::intern("_NET_WM_STATE");
    let atom_type = gdk::Atom::intern("ATOM");
    let Some((_, format, data)) = gdk::property_get(gdk_window, &property, &atom_type, 0, 1024, 0)
    else {
        return false;
    };
    if format != 32 {
        return false;
    }
    // Asking for type `ATOM` makes GDK translate each X atom in the list into its own atom value,
    // delivered as an array of C `long`s, so the state is compared against a GDK atom too.
    lists_atom(&data, gdk::Atom::intern("_NET_WM_STATE_ABOVE").value())
}

/// Whether `atom` is among the native-endian machine words in a format-32 property's data.
fn lists_atom(data: &[u8], atom: usize) -> bool {
    const WORD: usize = std::mem::size_of::<usize>();
    data.as_chunks::<WORD>()
        .0
        .iter()
        .any(|word| usize::from_ne_bytes(*word) == atom)
}

/// X11's window manager is the authority on `_NET_WM_STATE_ABOVE`. Wayland has no such state to read, so this is `None` there.
pub async fn always_on_top<R: Runtime>(window: &WebviewWindow<R>) -> Option<bool> {
    let target = window.clone();
    main_thread::run(window, move || {
        if detect_session() != Session::X11 {
            return None;
        }
        let gdk_window = target.gtk_window().ok()?.window()?;
        Some(read_above(&gdk_window))
    })
    .await
    .flatten()
}

/// Emits `ALWAYS_ON_TOP_CHANGED_EVENT` to the window whenever the X11 window manager changes `_NET_WM_STATE_ABOVE`, whether the request came from the app or from the window manager's own menu.
pub fn watch_always_on_top<R: Runtime>(window: &Window<R>) {
    let target = window.clone();
    let scheduled = window.run_on_main_thread(move || {
        if detect_session() != Session::X11 {
            return;
        }
        let Ok(gtk_window) = target.gtk_window() else {
            warn!(
                "always on top watcher: no GTK window for label={}",
                target.label()
            );
            return;
        };
        // GTK only delivers property changes to widgets that ask for them.
        gtk_window.add_events(gdk::EventMask::PROPERTY_CHANGE_MASK);
        let state_atom = gdk::Atom::intern("_NET_WM_STATE");
        let last = std::cell::Cell::new(None::<bool>);
        let emitter = target.clone();
        gtk_window.connect_property_notify_event(move |widget, event| {
            if event.atom() == state_atom {
                if let Some(gdk_window) = widget.window() {
                    let above = read_above(&gdk_window);
                    if last.replace(Some(above)) != Some(above) {
                        info!(
                            "always on top changed for label={}: {above}",
                            emitter.label()
                        );
                        if let Err(error) =
                            emitter.emit_to(emitter.label(), ALWAYS_ON_TOP_CHANGED_EVENT, above)
                        {
                            warn!(
                                "could not emit the always on top change for label={}: {error}",
                                emitter.label()
                            );
                        }
                    }
                }
            }
            gdk::glib::Propagation::Proceed
        });
    });
    if let Err(error) = scheduled {
        warn!(
            "always on top watcher was not installed for label={}: {error}",
            window.label()
        );
    }
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
            WindowCapabilities::new(Session::Unknown, false, false)
        );
    }

    #[test]
    fn finds_an_atom_in_property_data() {
        let words: Vec<u8> = [89usize, 115]
            .iter()
            .flat_map(|word| word.to_ne_bytes())
            .collect();
        assert!(lists_atom(&words, 89));
        assert!(lists_atom(&words, 115));
        assert!(!lists_atom(&words, 271));
        assert!(!lists_atom(&[], 89));
        // A trailing partial word is ignored rather than misread.
        assert!(!lists_atom(&words[..words.len() - 1], 115));
    }

    /// Run on a real desktop: `cargo test live_capabilities -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn live_capabilities() {
        let capabilities = capabilities_for(detect_session());
        println!("{}", serde_json::to_string(&capabilities).unwrap());
    }
}
