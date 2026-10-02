// Linux (GTK 3 on Wayland and X11): outbound drags from a GTK drag source, lossless inbound URIs, modifiers and the GTK file clipboard
//
// The webview is a GTK widget, so a drag out of the window is a plain GTK drag with the webview as its source, and GDK carries the Wayland drag serial itself. Everything here must run on the main thread. The behaviours these functions rely on were observed on GNOME's Mutter, on Wayland and on XWayland (see the README).
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    ffi::{c_char, c_int, c_uint, c_void},
    rc::Rc,
};

use gdk::prelude::*;
use gtk::{
    glib::{self, translate::ToGlibPtr},
    prelude::*,
};
use log::{debug, warn};
use tauri::{AppHandle, Emitter, Runtime, Webview, WebviewWindow};

use crate::{
    error::{Error, Result},
    inbound::{modifiers_from_gdk_mask, PositionUnit},
    models::{ClipboardFiles, DisplayServer, Modifiers, CLIPBOARD_CHANGED_EVENT},
    outbound::{classify, Actions, Begun, DragRequest, Failure, Finisher},
    platform::InboundExtras,
    uri,
};

/// Wry passes GTK's coordinates, which are webview CSS pixels, through unscaled.
pub const POSITION_UNIT: PositionUnit = PositionUnit::Logical;

const URI_LIST: &str = "text/uri-list";
const GNOME_COPIED_FILES: &str = "x-special/gnome-copied-files";
/// The `info` of the offered targets, so `drag-data-get` knows which one was asked for.
const INFO_URI_LIST: u32 = 0;
const INFO_GNOME: u32 = 1;

/// Maps a GDK display's type name to the display server.
pub fn classify_display(type_name: &str) -> DisplayServer {
    if type_name.contains("Wayland") {
        DisplayServer::Wayland
    } else if type_name.contains("X11") {
        DisplayServer::X11
    } else {
        DisplayServer::None
    }
}

/// Must run on the main thread. GDK aborts if it is used before GTK is initialised, as in a unit test with no display.
pub fn display_server() -> DisplayServer {
    if !gtk::is_initialized_main_thread() {
        return DisplayServer::None;
    }
    gdk::Display::default()
        .map(|display| classify_display(display.type_().name()))
        .unwrap_or(DisplayServer::None)
}

pub fn unavailable_reason() -> String {
    if !gtk::is_initialized_main_thread() {
        return "no display is available: GTK is not running".to_string();
    }
    match gdk::Display::default() {
        None => "no display is available".to_string(),
        Some(display) => format!(
            "the display backend `{}` is not supported; only Wayland and X11 are",
            display.type_().name()
        ),
    }
}

fn seat_pointer() -> Option<gdk::Device> {
    gdk::Display::default()?.default_seat()?.pointer()
}

/// Finds the `WebKitWebView` inside a GTK widget.
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

fn web_view<R: Runtime>(window: &WebviewWindow<R>) -> Option<gtk::Widget> {
    let gtk_window = window.gtk_window().ok()?;
    find_web_view(gtk_window.upcast_ref::<gtk::Widget>())
}

/// Whether the primary mouse button is down, from the pointer's state over the webview.
pub fn primary_button_down<R: Runtime>(window: &WebviewWindow<R>) -> bool {
    if !gtk::is_initialized_main_thread() {
        return false;
    }
    let read = || -> Option<bool> {
        let pointer = seat_pointer()?;
        let gdk_window = web_view(window)?.window()?;
        let (_, _, _, mask) = gdk_window.device_position(&pointer);
        Some(mask.contains(gdk::ModifierType::BUTTON1_MASK))
    };
    read().unwrap_or(false)
}

/// The modifier keys held now: the keymap's state together with the seat pointer's, which GDK keeps in step on both Wayland and X11.
pub fn modifiers_now() -> Modifiers {
    if !gtk::is_initialized_main_thread() {
        return Modifiers::default();
    }
    let mut mask = 0u32;
    if let Some(keymap) =
        gdk::Display::default().and_then(|display| gdk::Keymap::for_display(&display))
    {
        mask |= keymap.modifier_state();
    }
    if let (Some(pointer), Some(root)) = (
        seat_pointer(),
        gdk::Screen::default().and_then(|screen| screen.root_window()),
    ) {
        let (_, _, _, state) = root.device_position(&pointer);
        mask |= state.bits();
    }
    modifiers_from_gdk_mask(mask)
}

// ---- Inbound ------------------------------------------------------------------------------

/// What the second drag handlers record for a window while a drag is over it.
#[derive(Default)]
struct DragInfo {
    raw_uris: Option<Vec<String>>,
    source_is_ours: bool,
    /// The modifiers at the drag's last motion. Events reach the plugin a moment after GTK's signals, so reading the keys then could miss a key released in between.
    modifiers: Option<Modifiers>,
}

thread_local! {
    /// Per window label. GTK signal handlers and the plugin's event hook both run on the main thread.
    static DRAGS: RefCell<HashMap<String, DragInfo>> = RefCell::new(HashMap::new());
}

/// What was recorded for `label`; `consume` forgets it, as a drop or a leave ends the drag.
pub fn inbound_extras(label: &str, consume: bool) -> InboundExtras {
    DRAGS.with(|drags| {
        let mut drags = drags.borrow_mut();
        let extras = drags.get(label).map(|info| InboundExtras {
            raw_uris: info.raw_uris.clone(),
            source_is_ours: info.source_is_ours,
            modifiers: info.modifiers,
        });
        if consume {
            drags.remove(label);
        }
        extras.unwrap_or_default()
    })
}

/// Adds the plugin's handlers next to wry's on the window's webview: a second `drag-data-received` that reads the raw `text/uri-list` (wry decodes it into lossy paths), and a `drag-motion` that notes whether the drag began in this process. They add to wry's handlers and change nothing for it, and a drag of any other type, such as a tab, passes untouched.
pub fn on_webview_ready<R: Runtime>(webview: &Webview<R>) {
    if !gtk::is_initialized_main_thread() {
        return;
    }
    let window = webview.window();
    // Only the window's own content webview takes file drops.
    if webview.label() != window.label() {
        return;
    }
    let Ok(gtk_window) = window.gtk_window() else {
        return;
    };
    let Some(view) = find_web_view(gtk_window.upcast_ref::<gtk::Widget>()) else {
        debug!("native-dnd: no webview in `{}` to watch", window.label());
        return;
    };
    let label = window.label().to_string();
    view.connect_drag_data_received({
        let label = label.clone();
        move |_, context, _, _, data, _, _| {
            if data.target().name() != URI_LIST {
                return;
            }
            let body = data.data();
            if body.is_empty() {
                return;
            }
            let uris = uri::parse_uri_list(&body);
            DRAGS.with(|drags| {
                let mut drags = drags.borrow_mut();
                let info = drags.entry(label.clone()).or_default();
                info.raw_uris = Some(uris);
                info.source_is_ours |= context.drag_get_source_widget().is_some();
                info.modifiers = Some(modifiers_now());
            });
        }
    });
    view.connect_drag_motion({
        let label = label.clone();
        move |_, context, _, _, _| {
            let targets: Vec<String> = context
                .list_targets()
                .iter()
                .map(|atom| atom.name().to_string())
                .collect();
            note_source(&label, context.drag_get_source_widget().is_some(), &targets);
            // Not handled here: wry's handler and WebKit's own decide whether the drag is accepted.
            false
        }
    });
    // GTK also emits this just before a drop, but the drop's `drag-data-received` records the flag again.
    view.connect_drag_leave(move |_, _, _| clear_source(&label));
}

/// Records that the drag over `label` began in this process, but only for a drag of files: a text, link or image drag from our own webview raises no `Enter` from wry, so nothing would consume a flag set for it and the next outside drop would arrive flagged as a self-drop. Returns whether it was recorded.
fn note_source(label: &str, source_is_ours: bool, targets: &[String]) -> bool {
    let files = targets.iter().any(|name| name == URI_LIST);
    if !(source_is_ours && files) {
        return false;
    }
    DRAGS.with(|drags| {
        drags
            .borrow_mut()
            .entry(label.to_string())
            .or_default()
            .source_is_ours = true;
    });
    true
}

/// Forgets that the drag over `label` began here; the rest of what was recorded stays for the drop.
fn clear_source(label: &str) {
    DRAGS.with(|drags| {
        if let Some(info) = drags.borrow_mut().get_mut(label) {
            info.source_is_ours = false;
        }
    });
}

// ---- Outbound -----------------------------------------------------------------------------

fn gdk_actions(actions: Actions) -> gdk::DragAction {
    let mut out = gdk::DragAction::empty();
    if actions.copy {
        out |= gdk::DragAction::COPY;
    }
    if actions.r#move {
        out |= gdk::DragAction::MOVE;
    }
    if actions.link {
        out |= gdk::DragAction::LINK;
    }
    out
}

fn actions_from_gdk(action: gdk::DragAction) -> Actions {
    Actions {
        copy: action.contains(gdk::DragAction::COPY),
        r#move: action.contains(gdk::DragAction::MOVE),
        link: action.contains(gdk::DragAction::LINK),
    }
}

/// WebKit is left believing the primary button is still down, because GTK took the release that ended the drag: the page then gets a `pointerup` with no `pointerdown` on the next click and a second drag silently fails to start. A synthetic release delivered to the webview fixes that.
fn release_primary_button(view: &gtk::Widget) {
    let (Some(window), Some(pointer)) = (view.window(), seat_pointer()) else {
        return;
    };
    let (_, x, y, _) = window.device_position_double(&pointer);
    let mut event = gdk::Event::new(gdk::EventType::ButtonRelease);
    // SAFETY: `gdk_event_new` allocated a `GdkEventButton` for this event type, and `gdk_event_free` unreferences the window once, so it is given its own reference.
    unsafe {
        let raw = ToGlibPtr::<*mut gdk::ffi::GdkEvent>::to_glib_none(&event).0
            as *mut gdk::ffi::GdkEventButton;
        (*raw).window = window.to_glib_full();
        (*raw).send_event = 1;
        (*raw).time = gtk::current_event_time();
        (*raw).x = x;
        (*raw).y = y;
        (*raw).state = gdk::ModifierType::BUTTON1_MASK.bits();
        (*raw).button = 1;
    }
    event.set_device(Some(&pointer));
    view.event(&event);
}

fn set_drag_icon(context: &gdk::DragContext, png: &[u8]) {
    let loader = match gdk_pixbuf::PixbufLoader::with_type("png") {
        Ok(loader) => loader,
        Err(error) => {
            warn!("native-dnd: cannot load the drag image: {error}");
            return;
        }
    };
    let loaded = loader.write(png).and_then(|()| loader.close());
    match (loaded, loader.pixbuf()) {
        (Ok(()), Some(pixbuf)) => {
            context.drag_set_icon_pixbuf(&pixbuf, pixbuf.width() / 2, pixbuf.height() / 2)
        }
        (Err(error), _) => warn!("native-dnd: the drag image is not a PNG: {error}"),
        (Ok(()), None) => warn!("native-dnd: the drag image has no pixels"),
    }
}

/// What the source's signal handlers share.
struct Running {
    failure: RefCell<Option<Failure>>,
    deleted: Cell<bool>,
    last_action: Cell<gdk::DragAction>,
    handlers: RefCell<Vec<glib::SignalHandlerId>>,
    context_handlers: RefCell<Vec<glib::SignalHandlerId>>,
}

/// Starts a GTK drag from the webview of `window`. Must run on the main thread with the primary button down (checked by the caller).
pub fn begin_drag<R: Runtime>(
    _app: &AppHandle<R>,
    window: &WebviewWindow<R>,
    _id: u32,
    request: &DragRequest,
    finisher: Finisher,
) -> Result<Begun> {
    let view = web_view(window)
        .ok_or_else(|| Error::Failed("the window has no webview to drag from".into()))?;
    let cut_only = request.actions.r#move && !request.actions.copy && !request.actions.link;
    let list = uri::format_uri_list(&request.uris).into_bytes();
    let gnome = uri::format_gnome_copied_files(&ClipboardFiles {
        uris: request.uris.clone(),
        cut: cut_only,
    })
    .into_bytes();

    let targets = gtk::TargetList::new(&[]);
    targets.add(&gdk::Atom::intern(URI_LIST), 0, INFO_URI_LIST);
    targets.add(&gdk::Atom::intern(GNOME_COPIED_FILES), 0, INFO_GNOME);
    let context = view
        .drag_begin_with_coordinates(&targets, gdk_actions(request.actions), 1, None, -1, -1)
        .ok_or_else(|| Error::Failed("GTK did not start the drag".into()))?;
    if let Some(png) = &request.icon {
        set_drag_icon(&context, png);
    }

    let running = Rc::new(Running {
        failure: RefCell::new(None),
        deleted: Cell::new(false),
        last_action: Cell::new(gdk::DragAction::empty()),
        handlers: RefCell::new(Vec::new()),
        context_handlers: RefCell::new(Vec::new()),
    });

    let data_get = view.connect_drag_data_get({
        let context = context.clone();
        move |_, asked, selection, info, _time| {
            if asked != &context {
                return;
            }
            let body = match info {
                INFO_URI_LIST => &list,
                INFO_GNOME => &gnome,
                _ => return,
            };
            // `time` is 0 on Wayland, so nothing here depends on it.
            selection.set(&selection.target(), 8, body);
        }
    });
    let data_delete = view.connect_drag_data_delete({
        let (context, running) = (context.clone(), running.clone());
        move |_, asked| {
            if asked == &context {
                running.deleted.set(true);
            }
        }
    });
    let failed = view.connect_drag_failed({
        let (context, running) = (context.clone(), running.clone());
        move |_, asked, result| {
            if asked == &context {
                *running.failure.borrow_mut() = match result {
                    gtk::DragResult::UserCancelled => Some(Failure::UserCancelled),
                    // Wayland ends an abandoned drag and a drop on nothing with a plain error.
                    gtk::DragResult::NoTarget | gtk::DragResult::Error => Some(Failure::NoTarget),
                    gtk::DragResult::Success => None,
                    other => Some(Failure::Other(format!("the drag failed: {other:?}"))),
                };
            }
            // Not handled: GTK plays its failure animation and still ends the drag.
            glib::Propagation::Proceed
        }
    });
    let end = view.connect_drag_end({
        let (context, running) = (context.clone(), running.clone());
        move |view, ended| {
            if ended != &context {
                return;
            }
            for id in running.handlers.borrow_mut().drain(..) {
                view.disconnect(id);
            }
            for id in running.context_handlers.borrow_mut().drain(..) {
                context.disconnect(id);
            }
            release_primary_button(view.upcast_ref());
            let mut selected = actions_from_gdk(context.selected_action());
            if selected.is_empty() {
                selected = actions_from_gdk(running.last_action.get());
            }
            let (outcome, reason) = classify(
                selected,
                running.failure.borrow().as_ref(),
                running.deleted.get(),
            );
            finisher(outcome, reason);
        }
    });
    running
        .handlers
        .borrow_mut()
        .extend([data_get, data_delete, failed, end]);
    let changed = context.connect_action_changed({
        let running = running.clone();
        move |_, action| {
            if !action.is_empty() {
                running.last_action.set(action);
            }
        }
    });
    running.context_handlers.borrow_mut().push(changed);
    Ok(Begun::Running)
}

// ---- Clipboard ----------------------------------------------------------------------------

/// What a clipboard offer holds, kept alive by GTK until another owner replaces it.
struct Held {
    gnome: Vec<u8>,
    list: Vec<u8>,
}

unsafe extern "C" fn clipboard_get(
    _clipboard: *mut gtk::ffi::GtkClipboard,
    selection: *mut gtk::ffi::GtkSelectionData,
    info: c_uint,
    data: glib::ffi::gpointer,
) {
    // SAFETY: `data` is the `Held` leaked by `set_files` and freed only by `clipboard_clear`.
    let held = unsafe { &*(data as *const Held) };
    let body = if info == INFO_GNOME {
        &held.gnome
    } else {
        &held.list
    };
    // SAFETY: `selection` is live for the call and `body` outlives it; GTK copies the bytes.
    unsafe {
        let target = gtk::ffi::gtk_selection_data_get_target(selection);
        gtk::ffi::gtk_selection_data_set(selection, target, 8, body.as_ptr(), body.len() as c_int);
    }
}

unsafe extern "C" fn clipboard_clear(
    _clipboard: *mut gtk::ffi::GtkClipboard,
    data: glib::ffi::gpointer,
) {
    // SAFETY: GTK calls this once, when the offer is replaced or the clipboard is cleared; `data` came from `Box::into_raw`.
    drop(unsafe { Box::from_raw(data as *mut Held) });
}

/// Puts `files` on the clipboard as `x-special/gnome-copied-files` and `text/uri-list`. Must run on the main thread. On Wayland the compositor ignores the offer unless the app has had input recently, so call it from the handler of a key press or click.
pub fn set_files(files: &ClipboardFiles) -> Result<()> {
    if !gtk::is_initialized_main_thread() {
        return Err(Error::Unsupported(unavailable_reason()));
    }
    let held = Box::new(Held {
        gnome: uri::format_gnome_copied_files(files).into_bytes(),
        list: uri::format_uri_list(&files.uris).into_bytes(),
    });
    let mut entries = [
        gtk::ffi::GtkTargetEntry {
            target: c"x-special/gnome-copied-files".as_ptr() as *mut c_char,
            flags: 0,
            info: INFO_GNOME,
        },
        gtk::ffi::GtkTargetEntry {
            target: c"text/uri-list".as_ptr() as *mut c_char,
            flags: 0,
            info: INFO_URI_LIST,
        },
    ];
    let clipboard = gtk::Clipboard::get(&gdk::SELECTION_CLIPBOARD);
    let raw = Box::into_raw(held);
    // SAFETY: the entries point at static C strings; GTK copies the target list; `raw` is freed by `clipboard_clear` when GTK later replaces or drops the offer; if the call itself fails, the code below frees the box instead, so it is never freed twice.
    let set = unsafe {
        gtk::ffi::gtk_clipboard_set_with_data(
            clipboard.to_glib_none().0,
            entries.as_mut_ptr(),
            entries.len() as c_uint,
            Some(clipboard_get),
            Some(clipboard_clear),
            raw as *mut c_void,
        )
    };
    if set == glib::ffi::GFALSE {
        // GTK did not take the offer and will not call `clipboard_clear`.
        // SAFETY: `raw` is still the box leaked above.
        drop(unsafe { Box::from_raw(raw) });
        return Err(Error::Failed(
            "GTK did not accept the clipboard offer".into(),
        ));
    }
    Ok(())
}

/// The files on the clipboard: `x-special/gnome-copied-files` where offered, else a `text/uri-list` (read as a copy). Must run on the main thread; reading another application's offer runs a nested main loop until it answers.
pub fn get_files() -> Result<Option<ClipboardFiles>> {
    if !gtk::is_initialized_main_thread() {
        return Err(Error::Unsupported(unavailable_reason()));
    }
    let clipboard = gtk::Clipboard::get(&gdk::SELECTION_CLIPBOARD);
    if let Some(data) = clipboard.wait_for_contents(&gdk::Atom::intern(GNOME_COPIED_FILES)) {
        if let Some(files) = uri::parse_gnome_copied_files(&data.data()) {
            return Ok(Some(files));
        }
    }
    if let Some(data) = clipboard.wait_for_contents(&gdk::Atom::intern(URI_LIST)) {
        let uris = uri::parse_uri_list(&data.data());
        if !uris.is_empty() {
            return Ok(Some(ClipboardFiles { uris, cut: false }));
        }
    }
    Ok(None)
}

/// Tells every window when the clipboard's owner changes. Runs on the main thread when the event loop is ready.
pub fn on_ready<R: Runtime>(app: &AppHandle<R>) {
    if !gtk::is_initialized_main_thread() {
        return;
    }
    let app = app.clone();
    gtk::Clipboard::get(&gdk::SELECTION_CLIPBOARD).connect_local(
        "owner-change",
        false,
        move |_| {
            if let Err(error) = app.emit(CLIPBOARD_CHANGED_EVENT, ()) {
                warn!("native-dnd: cannot send the clipboard change: {error}");
            }
            None
        },
    );
}

pub fn on_exit<R: Runtime>(_app: &AppHandle<R>) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_types_map_to_servers() {
        assert_eq!(
            classify_display("GdkWaylandDisplay"),
            DisplayServer::Wayland
        );
        assert_eq!(classify_display("GdkX11Display"), DisplayServer::X11);
        assert_eq!(classify_display("GdkBroadwayDisplay"), DisplayServer::None);
        assert_eq!(classify_display(""), DisplayServer::None);
    }

    #[test]
    fn without_gtk_nothing_is_available_and_nothing_aborts() {
        if gtk::is_initialized_main_thread() {
            return;
        }
        assert_eq!(display_server(), DisplayServer::None);
        assert!(unavailable_reason().contains("GTK is not running"));
        assert_eq!(modifiers_now(), Modifiers::default());
        assert_eq!(
            set_files(&ClipboardFiles {
                uris: vec![],
                cut: false
            })
            .unwrap_err()
            .kind(),
            crate::models::ErrorKind::Unsupported
        );
        assert!(get_files().is_err());
    }

    #[test]
    fn the_inbound_record_is_per_window_and_consumed_once() {
        DRAGS.with(|drags| {
            drags.borrow_mut().insert(
                "a".into(),
                DragInfo {
                    raw_uris: Some(vec!["file:///x".into()]),
                    source_is_ours: true,
                    modifiers: Some(Modifiers {
                        ctrl: true,
                        ..Modifiers::default()
                    }),
                },
            );
        });
        assert!(inbound_extras("b", false).raw_uris.is_none());
        let kept = inbound_extras("a", false);
        assert!(kept.source_is_ours);
        assert_eq!(kept.modifiers.map(|m| m.ctrl), Some(true));
        assert_eq!(
            kept.raw_uris.as_deref(),
            Some(&["file:///x".to_string()][..])
        );
        assert!(inbound_extras("a", true).raw_uris.is_some());
        assert!(inbound_extras("a", false).raw_uris.is_none());
    }

    #[test]
    fn only_a_file_drag_from_this_process_is_marked_as_ours() {
        let text = vec!["text/plain".to_string(), "UTF8_STRING".to_string()];
        let files = vec![URI_LIST.to_string(), "text/plain".to_string()];
        assert!(!note_source("src-text", true, &text));
        assert!(!inbound_extras("src-text", false).source_is_ours);
        assert!(!note_source("src-ext", false, &files));
        assert!(!inbound_extras("src-ext", false).source_is_ours);
        assert!(note_source("src-files", true, &files));
        assert!(inbound_extras("src-files", false).source_is_ours);
        // Leaving clears the flag only; the rest stays for the drop.
        clear_source("src-files");
        assert!(!inbound_extras("src-files", true).source_is_ours);
    }

    #[test]
    fn gdk_actions_round_trip() {
        let actions = Actions {
            copy: true,
            r#move: false,
            link: true,
        };
        assert_eq!(actions_from_gdk(gdk_actions(actions)), actions);
        assert_eq!(gdk_actions(Actions::default()), gdk::DragAction::empty());
    }
}
