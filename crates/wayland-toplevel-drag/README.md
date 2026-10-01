# wayland-toplevel-drag

Drags a real GTK3 window with the pointer on Wayland, through the `xdg-toplevel-drag-v1` protocol, on GTK's own Wayland connection and `wl_data_device`. The compositor moves the window with the pointer for the whole drag (outside every window of the app too), leaves it where it is dropped, and snaps it back when the drag is cancelled. A drop on another window of the same app can carry a payload under a MIME type of your choosing.

The crate knows nothing about what is dragged: it has no Tauri and no application imports, so it can be used by any GTK3 program or move into a shared plugins workspace. It is Linux-only; elsewhere it builds as an empty stub (`is_supported()` is `false`).

## Why a crate for this

Wayland gives an application no cursor position and no window positions, so an app cannot follow its own pointer with a window. The protocol lets the compositor do it, but GTK3 does not expose what the protocol needs:

| Needed                                       | GTK3                                 | Where this crate gets it                                                         |
| -------------------------------------------- | ------------------------------------ | -------------------------------------------------------------------------------- |
| `wl_display`                                 | `gdk_wayland_display_get_wl_display` | the same call, looked up with `dlsym` so GTK is not linked                       |
| `wl_surface` of a window                     | `gdk_wayland_window_get_wl_surface`  | the same                                                                         |
| `xdg_toplevel` of a window                   | not exposed                          | a C interposer on `wl_proxy_marshal_flags` (`shim.c`)                            |
| GTK's `wl_data_device`                       | not exposed                          | the same interposer                                                              |
| the pointer-button serial `start_drag` needs | not exposed                          | a `wl_pointer` of the crate's own, on a separate event queue of GTK's connection |

GTK's own `wl_data_device` has to be reused: a second device on the seat makes some compositors (Mutter) send drops to the wrong one.

## Linking the interposer

`shim.c` defines `wl_proxy_marshal_flags` (the one call GDK's inlined protocol stubs use), forwards each call to libwayland and notes which proxies it created. For its definition to win over libwayland's the **final executable** has to export it. A library crate cannot add link arguments downstream, so put this in the application's `build.rs`:

```rust
fn main() {
    println!("cargo:rustc-link-arg-bins=-rdynamic");
    println!("cargo:rustc-link-arg-bins=-Wl,--undefined=wl_proxy_marshal_flags");
}
```

Without them nothing breaks: `probe` reports `Unavailable::NoInterposer` and the caller falls back. The interposer changes no request, and records nothing where GDK is not on Wayland.

## Use

Everything runs on the thread that runs GTK's main loop.

```rust
use wayland_toplevel_drag::{ffi, probe, DragRequest, Event, Tracker};

// After GTK has made its windows. `display` is `ffi::wl_display_of_gdk(gdk_display)`.
probe(display)?;                       // the registry has the protocol, the interposer is active
let mut tracker = Tracker::new(display)?;

// Dispatch the tracker's queue regularly: every few ms during a drag, every few tens of ms otherwise.
tracker.pump()?;

// A window created hidden, then shown through GTK so GDK makes its `xdg_toplevel` but maps nothing yet.
gtk_window.show_all();
tracker.start(
    DragRequest {
        origin_surface,                 // the wl_surface of the window the pointer is pressed in
        toplevel: ffi::toplevel_for_surface(window_surface).unwrap(),
        data_device: ffi::gtk_data_device().unwrap(),
        offset: (40, 20),               // where the pointer holds the window
        mime: "application/x-example".into(),
        payload: b"{}".to_vec(),        // what a target that accepts the type receives
    },
    |event| match event {
        Event::Started => {}
        Event::Target { accepted } => {}
        Event::DropPerformed => {}
        Event::Finished => {}                       // a target took the payload
        Event::Cancelled { after_drop: true } => {} // released over nothing: the window stays put
        Event::Cancelled { after_drop: false } => {} // Escape: the window snapped back
        Event::Failed(reason) => {}
    },
)?;
```

`start` reads the button press after a dispatch of the queue: the page's own press may be milliseconds old and still waiting there. It fails with `StartError::NoButtonHeld` when no button is down.

### Requirements the protocol and GTK impose

- Show the window to be dragged with GTK directly (`show_all` on the main thread), not through a toolkit wrapper that queues the call, before looking up its `xdg_toplevel`: GDK creates it when the window is shown.
- An end of drag can be reported twice (`cancelled` can follow `dnd_finished`). The crate reports exactly one of `Finished`, `Cancelled` and `Failed`.
- The page in the pressed window gets no pointer events once the compositor owns the drag (only `gotpointercapture` and `blur` come before), so tell it the drag started and ended and have it reset its pointer state.
- A WebKitGTK webview rejects a drop of a type it does not know unless the type is added to its drop targets and `drag-motion`, `drag-drop` and `drag-data-received` accept it; see `tauri-plugin-window-tearoff` for a worked example.

## Tests

The event ordering is a pure state machine (`machine.rs`) and the registry check is a pure function, both unit tested. The protocol itself needs a compositor: it was exercised on headless Mutter and on a live GNOME Shell session with a standalone GTK3 window and a Tauri window (see `docs/architecture/milestone-0-spikes.md` and `docs/tauri-tear-off.md` in the Waypoint repository).
