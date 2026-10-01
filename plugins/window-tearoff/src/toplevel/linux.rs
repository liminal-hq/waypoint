// Runs a toplevel drag on GTK's Wayland connection and lets the app's windows take its payload
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::{cell::RefCell, ffi::c_void, sync::Arc, time::Duration};

use gdk::prelude::*;
use gtk::{
    glib::{self, translate::ToGlibPtr},
    prelude::*,
};
use log::{debug, info, warn};
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager, Runtime};
use wayland_toplevel_drag::{
    ffi::{self, RawDisplay, RawProxy},
    DragRequest, Event, StartError, Tracker, Unavailable,
};

use super::{emit_ended, Finish, State};
use crate::{
    models::{
        ToplevelBeginReport, ToplevelBeginState, TAB_DROPPED_EVENT, TOPLEVEL_DRAG_STARTED_EVENT,
    },
    session::Tearoff,
    status::{Platform, ToplevelProbe},
};

/// Set to anything but `0` or the empty string to turn the real-window drag off, so the caller's fallback can be tried on a compositor that has it.
pub const DISABLE_ENV: &str = "WINDOW_TEAROFF_DISABLE_TOPLEVEL_DRAG";

/// How often the tracker's queue is dispatched while idle (it only has to keep the button press fresh and the queue short) and during a drag.
const IDLE_PUMP: Duration = Duration::from_millis(50);
const DRAG_PUMP: Duration = Duration::from_millis(4);

/// The `info` value of the drop targets this plugin adds; WebKit's own (and wry's uri list) use others.
const TARGET_INFO: u32 = 0x7467;

thread_local! {
    /// The tracker lives on the main thread, where GTK does.
    static TRACKER: RefCell<Option<Tracker>> = const { RefCell::new(None) };
}

fn disabled() -> bool {
    std::env::var_os(DISABLE_ENV).is_some_and(|value| !value.is_empty() && value != "0")
}

fn map_unavailable(error: &Unavailable) -> ToplevelProbe {
    match error {
        Unavailable::NoToplevelDrag => ToplevelProbe::NoProtocol,
        Unavailable::NoSeatOrDataDevice => ToplevelProbe::NoSeat,
        Unavailable::NoInterposer => ToplevelProbe::NoInterposer,
        Unavailable::Connection(_) | Unavailable::InterposerOverflow(_) => ToplevelProbe::Failed,
    }
}

/// GDK's `wl_display`. Only called once the display is known to be a Wayland one.
fn gdk_wl_display() -> Option<RawDisplay> {
    let display = gdk::Display::default()?;
    let raw: *mut gdk::ffi::GdkDisplay = display.to_glib_none().0;
    // SAFETY: `display` is GDK's live default display and the caller has checked it is the Wayland backend.
    unsafe { ffi::wl_display_of_gdk(raw.cast::<c_void>()) }
}

/// The `wl_surface` of a GTK window that has been realised.
fn surface_of(window: &gtk::ApplicationWindow) -> Option<RawProxy> {
    let gdk_window = window.window()?;
    let raw: *mut gdk::ffi::GdkWindow = gdk_window.to_glib_none().0;
    // SAFETY: `gdk_window` is a live, realised window of the Wayland backend (the display was probed as Wayland).
    unsafe { ffi::wl_surface_of_gdk_window(raw.cast::<c_void>()) }
}

/// Dispatches the tracker's queue; true while a drag is running. Skipped (and idle) if the tracker is busy further up the stack.
fn pump() -> bool {
    TRACKER.with(|cell| {
        let Ok(mut cell) = cell.try_borrow_mut() else {
            return true;
        };
        match cell.as_mut() {
            Some(tracker) => {
                if let Err(error) = tracker.pump() {
                    warn!("window-tearoff: the Wayland queue failed: {error}");
                }
                tracker.is_dragging()
            }
            None => false,
        }
    })
}

/// Checks that a real-window drag can run here and sets it up: the registry has the protocol, the interposer is active, and the tracker is bound and being pumped. Must run on the main thread.
pub fn probe<R: Runtime>(_app: &AppHandle<R>, platform: Platform) -> ToplevelProbe {
    if platform != Platform::Wayland {
        return ToplevelProbe::NotWayland;
    }
    if disabled() {
        return ToplevelProbe::Disabled;
    }
    let Some(display) = gdk_wl_display() else {
        return ToplevelProbe::Failed;
    };
    if let Err(error) = wayland_toplevel_drag::probe(display) {
        info!("window-tearoff: no toplevel drag: {error}");
        return map_unavailable(&error);
    }
    let made = TRACKER.with(|cell| {
        if cell.borrow().is_some() {
            return Ok(());
        }
        let tracker = Tracker::new(display)?;
        *cell.borrow_mut() = Some(tracker);
        Ok(())
    });
    match made {
        Ok(()) => {
            glib::timeout_add_local(IDLE_PUMP, || {
                pump();
                glib::ControlFlow::Continue
            });
            ToplevelProbe::Available
        }
        Err(error) => {
            info!("window-tearoff: no toplevel drag: {error}");
            map_unavailable(&error)
        }
    }
}

/// Starts the drag of `window` from the press in `source`. Must run on the main thread.
pub fn begin<R: Runtime>(
    app: &AppHandle<R>,
    state: &Arc<State>,
    source: &str,
    window: &str,
    payload: &Value,
    grab: (i32, i32),
    mime: &str,
) -> ToplevelBeginReport {
    let failed = |reason: String| ToplevelBeginReport {
        state: ToplevelBeginState::Failed,
        reason: Some(reason),
    };
    let (Some(source_window), Some(dragged)) = (
        app.get_webview_window(source),
        app.get_webview_window(window),
    ) else {
        return failed("a window of the drag no longer exists".into());
    };
    let (Ok(source_gtk), Ok(dragged_gtk)) = (source_window.gtk_window(), dragged.gtk_window())
    else {
        return failed("a window of the drag has no GTK window".into());
    };
    let started = match state.begin(source, window, payload) {
        Ok(started) => started,
        Err(report) => return report,
    };
    let bytes = match serde_json::to_vec(payload) {
        Ok(bytes) => bytes,
        Err(error) => {
            state.abandon();
            return failed(format!("the payload is not JSON: {error}"));
        }
    };

    // The window is created hidden. GDK makes its `xdg_toplevel` when it is shown, and the drag has to attach it before the window is mapped, so it is shown through GTK now, not through Tauri's queued `show`.
    dragged_gtk.show_all();
    let request = (|| {
        Some(DragRequest {
            origin_surface: surface_of(&source_gtk)?,
            toplevel: ffi::toplevel_for_surface(surface_of(&dragged_gtk)?)?,
            data_device: ffi::gtk_data_device()?,
            offset: grab,
            mime: mime.to_string(),
            payload: bytes,
        })
    })();
    let Some(request) = request else {
        state.abandon();
        return failed("GTK's Wayland objects could not be found".into());
    };

    let sink_app = app.clone();
    let sink_state = state.clone();
    let source_label = source.to_string();
    let sink = move |event: Event| match event {
        Event::Started => {
            if let Err(error) =
                sink_app.emit_to(source_label.as_str(), TOPLEVEL_DRAG_STARTED_EVENT, &started)
            {
                warn!("window-tearoff: cannot tell the page the drag started: {error}");
            }
        }
        Event::Target { .. } | Event::DropPerformed => {}
        Event::Finished => end(&sink_app, &sink_state, Finish::Finished),
        Event::Cancelled { after_drop } => {
            end(&sink_app, &sink_state, Finish::Cancelled { after_drop })
        }
        Event::Failed(reason) => end(&sink_app, &sink_state, Finish::Failed(reason)),
    };
    let result = TRACKER.with(|cell| match cell.borrow_mut().as_mut() {
        Some(tracker) => tracker.start(request, sink),
        None => Err(StartError::Connection("the tracker is not running".into())),
    });
    match result {
        Ok(()) => {
            glib::timeout_add_local(DRAG_PUMP, || {
                if pump() {
                    glib::ControlFlow::Continue
                } else {
                    glib::ControlFlow::Break
                }
            });
            ToplevelBeginReport {
                state: ToplevelBeginState::Started,
                reason: None,
            }
        }
        Err(error) => {
            state.abandon();
            failed(error.to_string())
        }
    }
}

/// Ends the running drag as a cancel; the dragged window snaps back. Must run on the main thread.
pub fn cancel() {
    TRACKER.with(|cell| {
        if let Ok(mut cell) = cell.try_borrow_mut() {
            if let Some(tracker) = cell.as_mut() {
                tracker.cancel();
            }
        }
    });
}

/// Ends the implicit pointer grab GDK still holds for the press that began the drag.
///
/// The compositor owns the pointer for the whole drag and takes the button release, so GDK never sees it: it goes on believing the button is held, and keeps routing every pointer event to the window that was pressed (a window made after the drag, or the one that was dragged, would get none). Ungrabbing the pointer ends the grab GDK holds. Must run on the main thread.
#[allow(deprecated)]
fn release_pointer_grab() {
    if let Some(pointer) = gdk::Display::default()
        .and_then(|display| display.default_seat())
        .and_then(|seat| seat.pointer())
    {
        let raw: *mut gdk::ffi::GdkDevice = pointer.to_glib_none().0;
        // SAFETY: `pointer` is a live `GdkDevice`, and this runs on the main thread.
        unsafe { gdk::ffi::gdk_device_ungrab(raw, gdk::ffi::GDK_CURRENT_TIME as u32) };
    }
}

fn end<R: Runtime>(app: &AppHandle<R>, state: &State, how: Finish) {
    if let Some(ended) = state.finish(how) {
        release_pointer_grab();
        debug!("window-tearoff: toplevel drag ended: {ended:?}");
        emit_ended(app, &ended);
    }
}

/// Finds the `WebKitWebView` inside a GTK window.
fn find_web_view(widget: &gtk::Widget) -> Option<gtk::Widget> {
    if widget.type_().name() == "WebKitWebView" {
        return Some(widget.clone());
    }
    widget
        .downcast_ref::<gtk::Container>()?
        .children()
        .iter()
        .find_map(find_web_view)
}

/// Lets the window's webview take a drop of `mime`. WebKitGTK refuses a drag whose type it does not know, so the type is added to its drop targets and the drop is accepted and read here; the payload goes to the window's page as `TAB_DROPPED_EVENT`. Types WebKit knows (files, text) are left to it. Must run on the main thread.
pub fn install_drop_target<R: Runtime>(window: &tauri::Window<R>, mime: &str) {
    // GTK is not running in a unit test with a mock runtime, which has no GTK window to give.
    if !gtk::is_initialized_main_thread() {
        return;
    }
    let Ok(gtk_window) = window.gtk_window() else {
        return;
    };
    let Some(view) = find_web_view(gtk_window.upcast_ref::<gtk::Widget>()) else {
        debug!(
            "window-tearoff: no webview in `{}` to take drops",
            window.label()
        );
        return;
    };
    let atom = gdk::Atom::intern(mime);
    match view.drag_dest_get_target_list() {
        Some(targets) => targets.add(&atom, 0, TARGET_INFO),
        None => {
            let targets = gtk::TargetList::new(&[]);
            targets.add(&atom, 0, TARGET_INFO);
            view.drag_dest_set_target_list(Some(&targets));
        }
    }
    let ours = move |context: &gdk::DragContext| context.list_targets().contains(&atom);
    view.connect_drag_motion({
        move |_, context, _, _, time| {
            if !ours(context) {
                return false;
            }
            context.drag_status(gdk::DragAction::MOVE, time);
            true
        }
    });
    view.connect_drag_drop({
        move |view, context, _, _, time| {
            if !ours(context) {
                return false;
            }
            view.drag_get_data(context, &atom, time);
            true
        }
    });
    let app = window.app_handle().clone();
    let label = window.label().to_string();
    view.connect_drag_data_received(move |_, context, _, _, data, info, time| {
        if info != TARGET_INFO || data.target() != atom {
            return;
        }
        match serde_json::from_slice::<Value>(&data.data()) {
            Ok(payload) => {
                let routed = app
                    .try_state::<Tearoff>()
                    .and_then(|tearoff| tearoff.toplevel().payload_dropped(&label, payload));
                if let Some(dropped) = routed {
                    if let Err(error) = app.emit_to(label.as_str(), TAB_DROPPED_EVENT, &dropped) {
                        warn!("window-tearoff: cannot hand the payload to `{label}`: {error}");
                    }
                }
                context.drag_finish(true, false, time);
            }
            Err(error) => {
                warn!("window-tearoff: a dropped payload is not JSON: {error}");
                context.drag_finish(false, false, time);
            }
        }
    });
}
