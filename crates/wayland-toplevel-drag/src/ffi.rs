// The one module with `unsafe`: the C interposer's lookups, GDK's Wayland accessors and foreign Wayland objects
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! # Safety
//!
//! Everything unsafe in the crate is here, so that the rest is safe Rust.
//!
//! - The interposer functions (`wtd_*`, `shim.c`) take and return raw `wl_proxy` pointers owned by
//!   GDK's Wayland connection. They only compare and hand out pointers, never dereference them.
//! - The GDK accessors are looked up with `dlsym(RTLD_DEFAULT, ..)` instead of linked, so the crate
//!   needs no GTK and a process without GDK (or with GDK on X11) simply gets `None`. Calling them
//!   requires a live `GdkDisplay` or `GdkWindow` of the Wayland backend, which the caller vouches
//!   for in the `unsafe` functions below; `GDK` itself checks the type and warns otherwise.
//! - A pointer handed to `foreign` must be a live `wl_proxy` of the named interface on the
//!   connection `wl_display` belongs to. wayland-rs checks the interface name against the proxy and
//!   returns an error rather than a wrong-typed object.
//!
//! Every pointer is only valid while GDK keeps the object alive, and all of them must be used on
//! the thread that runs GTK's main loop.

use std::ffi::{c_char, c_void, CStr};
use std::ptr::NonNull;

use wayland_backend::client::{Backend, ObjectId};
use wayland_client::{Connection, Proxy};

extern "C" {
    fn wtd_interposer_hits() -> libc::c_ulong;
    fn wtd_interposer_overflows() -> libc::c_ulong;
    fn wtd_find_xdg_toplevel(wl_surface: *mut c_void) -> *mut c_void;
    fn wtd_find_data_device() -> *mut c_void;
}

/// A pointer to a Wayland object (`wl_proxy`) owned by GDK's connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawProxy(NonNull<c_void>);

impl RawProxy {
    /// `None` for a null pointer.
    ///
    /// # Safety
    /// A non-null `ptr` must be a live `wl_proxy` of GDK's Wayland connection, and must stay alive
    /// for as long as the value is used.
    pub unsafe fn from_ptr(ptr: *mut c_void) -> Option<Self> {
        NonNull::new(ptr).map(Self)
    }

    pub fn as_ptr(self) -> *mut c_void {
        self.0.as_ptr()
    }
}

/// A `wl_display` pointer: the connection GDK made.
pub type RawDisplay = RawProxy;

/// Looks up `name` in the running process, `None` if it is not loaded.
fn symbol(name: &CStr) -> Option<NonNull<c_void>> {
    // SAFETY: `dlsym` with the default handle only reads the loaded objects' symbol tables.
    NonNull::new(unsafe { libc::dlsym(libc::RTLD_DEFAULT, name.as_ptr() as *const c_char) })
}

/// The `wl_display` of a `GdkWaylandDisplay`.
///
/// # Safety
/// `gdk_display` must be a live `GdkDisplay*` whose backend is Wayland (check its type name first:
/// GDK refuses any other, with a critical warning).
pub unsafe fn wl_display_of_gdk(gdk_display: *mut c_void) -> Option<RawDisplay> {
    let get = symbol(c"gdk_wayland_display_get_wl_display")?;
    // SAFETY: the symbol has this signature in libgdk-3; `gdk_display` is vouched for by the caller.
    let get: unsafe extern "C" fn(*mut c_void) -> *mut c_void = std::mem::transmute(get.as_ptr());
    RawProxy::from_ptr(get(gdk_display))
}

/// The `wl_surface` of a realised `GdkWindow` of the Wayland backend.
///
/// # Safety
/// `gdk_window` must be a live, realised `GdkWindow*` of the Wayland backend.
pub unsafe fn wl_surface_of_gdk_window(gdk_window: *mut c_void) -> Option<RawProxy> {
    let get = symbol(c"gdk_wayland_window_get_wl_surface")?;
    // SAFETY: as for `wl_display_of_gdk`.
    let get: unsafe extern "C" fn(*mut c_void) -> *mut c_void = std::mem::transmute(get.as_ptr());
    RawProxy::from_ptr(get(gdk_window))
}

/// True once the interposer has seen GDK make a Wayland request, which proves it is the
/// `wl_proxy_marshal_flags` GDK calls (the executable exports it).
pub fn interposer_active() -> bool {
    // SAFETY: a counter read under the interposer's own lock.
    unsafe { wtd_interposer_hits() > 0 }
}

/// How many of GDK's proxies the interposer could not record (it grows as windows open, so only
/// running out of memory counts), which leaves those windows undraggable.
pub fn interposer_overflows() -> u64 {
    // SAFETY: a counter read under the interposer's own lock.
    unsafe { wtd_interposer_overflows() as u64 }
}

/// GTK's `wl_data_device`, as the interposer saw GDK create it.
pub fn gtk_data_device() -> Option<RawProxy> {
    // SAFETY: only returns a pointer the interposer recorded; it is not dereferenced here.
    NonNull::new(unsafe { wtd_find_data_device() }).map(RawProxy)
}

/// The `xdg_toplevel` GDK made for `wl_surface`, once the window has been shown (GDK creates it
/// when it maps the window, not when it realises it).
pub fn toplevel_for_surface(wl_surface: RawProxy) -> Option<RawProxy> {
    // SAFETY: as for `gtk_data_device`; the surface pointer is only compared.
    NonNull::new(unsafe { wtd_find_xdg_toplevel(wl_surface.as_ptr()) }).map(RawProxy)
}

/// A connection that shares GDK's `wl_display` and does not close it.
///
/// # Safety
/// `display` must be a live `wl_display*` that outlives the connection, with GDK's own queue on it
/// being read from the same thread.
pub unsafe fn shared_connection(display: RawDisplay) -> Connection {
    Connection::from_backend(Backend::from_foreign_display(display.as_ptr().cast()))
}

/// A typed handle to a Wayland object another library created, to send requests on.
///
/// # Safety
/// `proxy` must be a live `wl_proxy` on `conn`'s display.
pub unsafe fn foreign<I: Proxy>(conn: &Connection, proxy: RawProxy) -> Option<I> {
    let id = ObjectId::from_ptr(I::interface(), proxy.as_ptr().cast()).ok()?;
    I::from_id(conn, id).ok()
}
