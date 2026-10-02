// The one module with `unsafe`: GDK's Wayland accessors and a typed handle to a Wayland object GDK owns
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! # Safety
//!
//! GTK3 does not expose the `wl_display` and `wl_surface` it owns, so this module reaches them the
//! way GDK documents for Wayland-specific code, and everything unsafe in the Wayland path is here.
//!
//! - The GDK accessors are looked up with `dlsym(RTLD_DEFAULT, ..)` instead of linked, so a process
//!   without GDK (or with GDK on X11) simply gets `None`. Calling them requires a live `GdkDisplay`
//!   or `GdkWindow` of the Wayland backend, which the caller vouches for in the `unsafe` functions
//!   below; GDK itself checks the type and warns otherwise.
//! - A pointer handed to `foreign` must be a live `wl_proxy` of the named interface on the
//!   connection the display belongs to. wayland-rs checks the interface name against the proxy and
//!   returns an error rather than a wrong-typed object.
//!
//! Every pointer is only valid while GDK keeps the object alive, and all of them must be used on the
//! thread that runs GTK's main loop.

use std::ffi::{c_char, c_void, CStr};
use std::ptr::NonNull;

use wayland_backend::client::{Backend, ObjectId};
use wayland_client::{Connection, Proxy};

/// A pointer to a Wayland object (`wl_proxy`) owned by GDK's connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawProxy(NonNull<c_void>);

impl RawProxy {
    pub fn as_ptr(self) -> *mut c_void {
        self.0.as_ptr()
    }
}

/// Looks up `name` in the running process, `None` if it is not loaded.
fn symbol(name: &CStr) -> Option<NonNull<c_void>> {
    // SAFETY: `dlsym` with the default handle only reads the loaded objects' symbol tables.
    NonNull::new(unsafe { libc::dlsym(libc::RTLD_DEFAULT, name.as_ptr() as *const c_char) })
}

/// The `wl_display` of a `GdkWaylandDisplay`.
///
/// # Safety
/// `gdk_display` must be a live `GdkDisplay*` whose backend is Wayland (check its type name first).
pub unsafe fn wl_display_of_gdk(gdk_display: *mut c_void) -> Option<RawProxy> {
    let get = symbol(c"gdk_wayland_display_get_wl_display")?;
    // SAFETY: the symbol has this signature in libgdk-3; `gdk_display` is vouched for by the caller.
    let get: unsafe extern "C" fn(*mut c_void) -> *mut c_void = std::mem::transmute(get.as_ptr());
    NonNull::new(get(gdk_display)).map(RawProxy)
}

/// The `wl_surface` of a realised `GdkWindow` of the Wayland backend.
///
/// # Safety
/// `gdk_window` must be a live, realised `GdkWindow*` of the Wayland backend.
pub unsafe fn wl_surface_of_gdk_window(gdk_window: *mut c_void) -> Option<RawProxy> {
    let get = symbol(c"gdk_wayland_window_get_wl_surface")?;
    // SAFETY: as for `wl_display_of_gdk`.
    let get: unsafe extern "C" fn(*mut c_void) -> *mut c_void = std::mem::transmute(get.as_ptr());
    NonNull::new(get(gdk_window)).map(RawProxy)
}

/// A connection that shares GDK's `wl_display` and does not close it.
///
/// # Safety
/// `display` must be a live `wl_display*` that outlives the connection, with GDK's own queue on it
/// being read from the same thread.
pub unsafe fn shared_connection(display: RawProxy) -> Connection {
    Connection::from_backend(Backend::from_foreign_display(display.as_ptr().cast()))
}

/// A typed handle to a Wayland object GDK created, to send requests on.
///
/// # Safety
/// `proxy` must be a live `wl_proxy` on `conn`'s display.
pub unsafe fn foreign<I: Proxy>(conn: &Connection, proxy: RawProxy) -> Option<I> {
    let id = ObjectId::from_ptr(I::interface(), proxy.as_ptr().cast()).ok()?;
    I::from_id(conn, id).ok()
}
