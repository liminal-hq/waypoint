// Drags a real GTK3 toplevel with the pointer on Wayland, through `xdg-toplevel-drag-v1` on GTK's own connection
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Wayland gives an application no cursor position and no window positions, so an app cannot move
//! its own windows while a drag runs. `xdg-toplevel-drag-v1` lets the compositor do it: the app
//! starts a `wl_data_device` drag and attaches one of its toplevels to it, and the compositor
//! moves that window with the pointer, leaves it where it is dropped and snaps it back on cancel.
//!
//! GTK3 does not expose what the protocol needs, so this crate works with what it can reach:
//!
//! - the `wl_display` and a `wl_surface` come from `gdk_wayland_*` (see [`ffi`]);
//! - the `xdg_toplevel` and GTK's `wl_data_device` come from a small C interposer on
//!   `wl_proxy_marshal_flags` (`shim.c`) that the **final executable** has to export: link it with
//!   `-rdynamic` and `-Wl,--undefined=wl_proxy_marshal_flags` (a library cannot add link arguments
//!   downstream; in Cargo that is `cargo:rustc-link-arg-bins` in the app's `build.rs`);
//! - the pointer-button serial `start_drag` needs comes from a `wl_pointer` of this crate's own on
//!   a separate event queue of the shared connection ([`Tracker`]).
//!
//! GTK's own `wl_data_device` is reused for the drag: a second device on the seat makes some
//! compositors (Mutter) send drops to the wrong one.
//!
//! How a drag ends is told from the order of the data source's events: `dnd_drop_performed` then
//! `cancelled` is a release over no target (the window stays where it was dropped), a bare
//! `cancelled` is Escape or the compositor aborting (it snaps back). That reading is verified on
//! Mutter; see [`Event::Cancelled`] for the assumption about other compositors.
//!
//! Everything must run on the thread that runs GTK's main loop. Where GDK is not on Wayland, the
//! interposer is not exported, or the compositor has no `xdg_toplevel_drag_manager_v1`, [`probe`]
//! says why and nothing else is used. On other operating systems the crate is an empty stub.

#[cfg(target_os = "linux")]
mod event;
#[cfg(target_os = "linux")]
pub mod ffi;
#[cfg(target_os = "linux")]
mod machine;
#[cfg(target_os = "linux")]
mod tracker;

#[cfg(target_os = "linux")]
pub use event::{Event, Unavailable};
#[cfg(target_os = "linux")]
pub use tracker::{probe, DragRequest, StartError, Tracker};

/// Whether this build can ever work: Linux only.
pub const fn is_supported() -> bool {
    cfg!(target_os = "linux")
}
