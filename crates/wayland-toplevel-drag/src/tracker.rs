// Tracks the pointer-button serial on GTK's connection and runs an `xdg-toplevel-drag-v1` drag on request
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::io::Write;

use thiserror::Error;
use wayland_client::{
    event_created_child,
    globals::{registry_queue_init, GlobalListContents},
    protocol::{
        wl_data_device::{self, WlDataDevice},
        wl_data_device_manager::{DndAction, WlDataDeviceManager},
        wl_data_offer::WlDataOffer,
        wl_data_source::{self, WlDataSource},
        wl_pointer::{self, WlPointer},
        wl_registry::WlRegistry,
        wl_seat::{self, WlSeat},
        wl_surface::WlSurface,
    },
    Connection, Dispatch, EventQueue, Proxy, QueueHandle, WEnum,
};
use wayland_protocols::xdg::{
    shell::client::xdg_toplevel::XdgToplevel,
    toplevel_drag::v1::client::{
        xdg_toplevel_drag_manager_v1::XdgToplevelDragManagerV1,
        xdg_toplevel_drag_v1::XdgToplevelDragV1,
    },
};

use crate::{
    event::{Event, Unavailable},
    ffi::{self, RawDisplay, RawProxy},
    machine::{Input, Machine},
};

const TOPLEVEL_DRAG: &str = "xdg_toplevel_drag_manager_v1";
const SEAT: &str = "wl_seat";
const DATA_DEVICE_MANAGER: &str = "wl_data_device_manager";

/// What a global list has to hold for a drag to be possible, as the interface names.
fn check_globals<'a>(interfaces: impl Iterator<Item = &'a str> + Clone) -> Result<(), Unavailable> {
    let has = |name: &str| interfaces.clone().any(|interface| interface == name);
    if !has(TOPLEVEL_DRAG) {
        return Err(Unavailable::NoToplevelDrag);
    }
    if !has(SEAT) || !has(DATA_DEVICE_MANAGER) {
        return Err(Unavailable::NoSeatOrDataDevice);
    }
    Ok(())
}

/// Whether a drag can run on this connection: the compositor's registry has the protocol, and the
/// interposer is active and has seen GTK's `wl_data_device`. Does a registry round trip, so call
/// it from the main thread once GTK has made its windows.
pub fn probe(display: RawDisplay) -> Result<(), Unavailable> {
    struct Probe;
    impl Dispatch<WlRegistry, GlobalListContents> for Probe {
        fn event(
            _: &mut Self,
            _: &WlRegistry,
            _: <WlRegistry as Proxy>::Event,
            _: &GlobalListContents,
            _: &Connection,
            _: &QueueHandle<Self>,
        ) {
        }
    }
    // SAFETY: the caller of `probe` hands over GDK's live display (see `ffi`).
    let conn = unsafe { ffi::shared_connection(display) };
    let (globals, _queue) = registry_queue_init::<Probe>(&conn)
        .map_err(|error| Unavailable::Connection(error.to_string()))?;
    let list = globals.contents().clone_list();
    check_globals(list.iter().map(|global| global.interface.as_str()))?;
    if !ffi::interposer_active() || ffi::gtk_data_device().is_none() {
        return Err(Unavailable::NoInterposer);
    }
    match ffi::interposer_overflows() {
        0 => Ok(()),
        lost => Err(Unavailable::InterposerOverflow(lost)),
    }
}

/// Why a drag did not start.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum StartError {
    #[error("a drag is already running")]
    AlreadyActive,
    #[error("no pointer button is held, so there is no press to start a drag from")]
    NoButtonHeld,
    #[error("a Wayland object could not be reached: {0}")]
    Object(&'static str),
    #[error("the Wayland connection failed: {0}")]
    Connection(String),
}

/// What to drag and what it carries. The pointers are GDK's (see [`ffi`]).
pub struct DragRequest {
    /// The `wl_surface` the pointer is pressed on.
    pub origin_surface: RawProxy,
    /// The `xdg_toplevel` to move with the pointer; it must exist but not be mapped yet.
    pub toplevel: RawProxy,
    /// GTK's `wl_data_device` (never a second one).
    pub data_device: RawProxy,
    /// Where the pointer holds the window, in surface-local coordinates from its top left.
    pub offset: (i32, i32),
    /// The one MIME type offered.
    pub mime: String,
    /// What a target receives for it.
    pub payload: Vec<u8>,
}

struct ActiveDrag {
    source: WlDataSource,
    toplevel_drag: XdgToplevelDragV1,
    machine: Machine,
    mime: String,
    payload: Vec<u8>,
    sink: Box<dyn FnMut(Event)>,
}

struct State {
    /// Held so the seat stays bound for the pointer's life.
    _seat: WlSeat,
    pointer: Option<WlPointer>,
    /// The serial of the press that is still held.
    pressed: Option<(u32, u32)>,
    manager: WlDataDeviceManager,
    toplevel_drag_manager: XdgToplevelDragManagerV1,
    drag: Option<ActiveDrag>,
}

impl State {
    /// Reports `events` to the drag's sink, and puts the protocol objects away once it has ended.
    fn deliver(&mut self, events: Vec<Event>) {
        let Some(drag) = self.drag.as_mut() else {
            return;
        };
        for event in events {
            (drag.sink)(event);
        }
        if drag.machine.is_ended() {
            self.finish();
        }
    }

    fn finish(&mut self) {
        if let Some(drag) = self.drag.take() {
            // The drag has ended, so the protocol object can go; the source after it.
            drag.toplevel_drag.destroy();
            drag.source.destroy();
        }
    }

    fn feed(&mut self, input: Input) {
        let Some(drag) = self.drag.as_mut() else {
            return;
        };
        let events = drag.machine.feed(input);
        self.deliver(events);
    }
}

/// Follows the pointer's buttons on GTK's connection, so that a drag can be started from the press
/// that is happening, and runs the drag.
///
/// It owns an event queue of the shared connection, which has to be dispatched: call [`pump`]
/// regularly (every few milliseconds while a drag runs, every few tens of milliseconds otherwise,
/// from the GTK main thread), or the queue grows with every pointer motion.
///
/// [`pump`]: Tracker::pump
pub struct Tracker {
    conn: Connection,
    queue: EventQueue<State>,
    state: State,
}

impl Tracker {
    /// Binds the seat, its pointer and the managers on a new queue of GDK's connection.
    pub fn new(display: RawDisplay) -> Result<Self, Unavailable> {
        // SAFETY: the caller hands over GDK's live display (see `ffi`).
        let conn = unsafe { ffi::shared_connection(display) };
        let (globals, mut queue) = registry_queue_init::<State>(&conn)
            .map_err(|error| Unavailable::Connection(error.to_string()))?;
        let list = globals.contents().clone_list();
        check_globals(list.iter().map(|global| global.interface.as_str()))?;
        let qh = queue.handle();
        let bind_failed =
            |error: wayland_client::globals::BindError| Unavailable::Connection(error.to_string());
        let mut state = State {
            _seat: globals.bind(&qh, 1..=7, ()).map_err(bind_failed)?,
            pointer: None,
            pressed: None,
            manager: globals.bind(&qh, 1..=3, ()).map_err(bind_failed)?,
            toplevel_drag_manager: globals.bind(&qh, 1..=1, ()).map_err(bind_failed)?,
            drag: None,
        };
        // The seat reports its capabilities, and the pointer is made when it has one, now or when one is plugged in (a headless compositor has none until a remote desktop session adds a virtual one). A drag cannot start until it exists: `start` reports no button held.
        queue
            .roundtrip(&mut state)
            .map_err(|error| Unavailable::Connection(error.to_string()))?;
        Ok(Self { conn, queue, state })
    }

    /// Handles the events GDK's reads have queued for this tracker: button presses and the drag's.
    pub fn pump(&mut self) -> Result<(), String> {
        self.queue
            .dispatch_pending(&mut self.state)
            .map(drop)
            .map_err(|error| error.to_string())?;
        self.conn.flush().map_err(|error| error.to_string())
    }

    /// Whether a drag is running.
    pub fn is_dragging(&self) -> bool {
        self.state.drag.is_some()
    }

    /// Starts a drag from the pointer press that is held now, attaches `request.toplevel` to it and
    /// reports its events to `sink` (called from [`pump`](Tracker::pump), never re-entrantly into
    /// the tracker). `Started` is reported before this returns.
    ///
    /// The press is read after a dispatch, because the page's own press may be only a few
    /// milliseconds old and still waiting in the queue.
    pub fn start(
        &mut self,
        request: DragRequest,
        mut sink: impl FnMut(Event) + 'static,
    ) -> Result<(), StartError> {
        self.pump().map_err(StartError::Connection)?;
        if self.state.drag.is_some() {
            return Err(StartError::AlreadyActive);
        }
        let (serial, _) = self.state.pressed.ok_or(StartError::NoButtonHeld)?;
        // SAFETY: the request's pointers are GDK's live objects, per `DragRequest`'s contract.
        let (device, origin, toplevel) = unsafe {
            (
                ffi::foreign::<WlDataDevice>(&self.conn, request.data_device)
                    .ok_or(StartError::Object("wl_data_device"))?,
                ffi::foreign::<WlSurface>(&self.conn, request.origin_surface)
                    .ok_or(StartError::Object("wl_surface"))?,
                ffi::foreign::<XdgToplevel>(&self.conn, request.toplevel)
                    .ok_or(StartError::Object("xdg_toplevel"))?,
            )
        };
        let qh = self.queue.handle();
        let source = self.state.manager.create_data_source(&qh, ());
        source.offer(request.mime.clone());
        source.set_actions(DndAction::Move);
        let toplevel_drag =
            self.state
                .toplevel_drag_manager
                .get_xdg_toplevel_drag(&source, &qh, ());
        device.start_drag(Some(&source), &origin, None, serial);
        toplevel_drag.attach(&toplevel, request.offset.0, request.offset.1);
        let (machine, started) = Machine::start();
        sink(started);
        self.state.drag = Some(ActiveDrag {
            source,
            toplevel_drag,
            machine,
            mime: request.mime,
            payload: request.payload,
            sink: Box::new(sink),
        });
        self.conn
            .flush()
            .map_err(|error| StartError::Connection(error.to_string()))
    }

    /// Ends the running drag as a cancel (the window snaps back). Does nothing once it has ended.
    pub fn cancel(&mut self) {
        let Some(drag) = self.state.drag.as_mut() else {
            return;
        };
        let events = drag.machine.abort(Event::Cancelled { after_drop: false });
        self.state.deliver(events);
        let _ = self.conn.flush();
    }
}

impl Drop for Tracker {
    fn drop(&mut self) {
        self.state.finish();
        if let Some(pointer) = self.state.pointer.take() {
            if pointer.version() >= 3 {
                pointer.release();
            }
        }
        let _ = self.conn.flush();
    }
}

impl Dispatch<WlRegistry, GlobalListContents> for State {
    fn event(
        _: &mut Self,
        _: &WlRegistry,
        _: <WlRegistry as Proxy>::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

macro_rules! ignore_events {
    ($($interface:ty),*) => {$(
        impl Dispatch<$interface, ()> for State {
            fn event(
                _: &mut Self,
                _: &$interface,
                _: <$interface as Proxy>::Event,
                _: &(),
                _: &Connection,
                _: &QueueHandle<Self>,
            ) {
            }
        }
    )*};
}
ignore_events!(
    WlDataDeviceManager,
    WlDataOffer,
    XdgToplevelDragManagerV1,
    XdgToplevelDragV1
);

// Only ever sent requests (GTK's device receives its events on GTK's queue); this satisfies the
// bound on the proxy type.
impl Dispatch<WlDataDevice, ()> for State {
    fn event(
        _: &mut Self,
        _: &WlDataDevice,
        _: wl_data_device::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
    event_created_child!(State, WlDataDevice, [wl_data_device::EVT_DATA_OFFER_OPCODE => (WlDataOffer, ())]);
}

impl Dispatch<WlSeat, ()> for State {
    fn event(
        state: &mut Self,
        seat: &WlSeat,
        event: wl_seat::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_seat::Event::Capabilities {
            capabilities: WEnum::Value(capabilities),
        } = event
        {
            let has_pointer = capabilities.contains(wl_seat::Capability::Pointer);
            if has_pointer && state.pointer.is_none() {
                state.pointer = Some(seat.get_pointer(qh, ()));
            } else if !has_pointer {
                // The pointer went away (unplugged, or a remote desktop session ended): its object is dead, and a new one is made when a pointer comes back.
                if let Some(pointer) = state.pointer.take() {
                    if pointer.version() >= 3 {
                        pointer.release();
                    }
                }
                state.pressed = None;
            }
        }
    }
}

impl Dispatch<WlPointer, ()> for State {
    fn event(
        state: &mut Self,
        _: &WlPointer,
        event: wl_pointer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_pointer::Event::Button {
            serial,
            button,
            state: WEnum::Value(button_state),
            ..
        } = event
        {
            match button_state {
                wl_pointer::ButtonState::Pressed => state.pressed = Some((serial, button)),
                wl_pointer::ButtonState::Released
                    if state.pressed.is_some_and(|(_, held)| held == button) =>
                {
                    state.pressed = None;
                }
                _ => {}
            }
        }
    }
}

impl Dispatch<WlDataSource, ()> for State {
    fn event(
        state: &mut Self,
        _: &WlDataSource,
        event: wl_data_source::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_data_source::Event::Send { mime_type, fd } => {
                if let Some(drag) = state.drag.as_ref().filter(|drag| drag.mime == mime_type) {
                    let mut file = std::fs::File::from(fd);
                    let _ = file.write_all(&drag.payload);
                }
            }
            wl_data_source::Event::Target { mime_type } => state.feed(Input::Target(mime_type)),
            wl_data_source::Event::DndDropPerformed => state.feed(Input::DropPerformed),
            wl_data_source::Event::DndFinished => state.feed(Input::Finished),
            wl_data_source::Event::Cancelled => state.feed(Input::Cancelled),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all() -> Vec<&'static str> {
        vec!["wl_compositor", SEAT, DATA_DEVICE_MANAGER, TOPLEVEL_DRAG]
    }

    #[test]
    fn a_full_registry_passes() {
        assert_eq!(check_globals(all().into_iter()), Ok(()));
    }

    #[test]
    fn a_registry_without_the_protocol_says_so() {
        let globals = all().into_iter().filter(|name| *name != TOPLEVEL_DRAG);
        assert_eq!(check_globals(globals), Err(Unavailable::NoToplevelDrag));
    }

    #[test]
    fn a_registry_without_a_seat_or_data_device_says_so() {
        let globals = all().into_iter().filter(|name| *name != SEAT);
        assert_eq!(check_globals(globals), Err(Unavailable::NoSeatOrDataDevice));
        let globals = all()
            .into_iter()
            .filter(|name| *name != DATA_DEVICE_MANAGER);
        assert_eq!(check_globals(globals), Err(Unavailable::NoSeatOrDataDevice));
    }

    #[test]
    fn an_empty_registry_lacks_the_protocol_first() {
        assert_eq!(
            check_globals(std::iter::empty()),
            Err(Unavailable::NoToplevelDrag)
        );
    }

    #[test]
    fn without_gdk_nothing_is_recorded() {
        // A test binary neither exports nor links the interposer's symbol, and has no GTK.
        assert!(!ffi::interposer_active());
        assert!(ffi::gtk_data_device().is_none());
    }

    #[test]
    fn gdk_accessors_are_absent_without_gdk() {
        // SAFETY: a null display is only passed where the lookup fails first (no libgdk loaded).
        let found = unsafe { ffi::wl_display_of_gdk(std::ptr::null_mut()) };
        assert!(found.is_none());
    }
}
