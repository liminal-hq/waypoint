// What a drag reports, and why the protocol can be unavailable
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use thiserror::Error;

/// What happened to a drag, in order. Exactly one of `Finished`, `Cancelled` and `Failed` ends it,
/// however many end-of-drag requests the compositor sends.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// The compositor has taken the drag: the attached window now follows the pointer.
    Started,
    /// The surface under the pointer accepted the offered type (`true`) or stopped accepting it.
    Target { accepted: bool },
    /// The button was released over a target that accepted the type.
    DropPerformed,
    /// The target took the payload: the drag succeeded and the attached window stays where it is.
    Finished,
    /// The drag did not complete. `after_drop` is true when the button was released first, with no
    /// target to take the payload (a drop on the bare desktop: the attached window stays where
    /// it was dropped). It is false for a cancel, such as Escape: the window snaps back.
    Cancelled { after_drop: bool },
    /// The drag could not run.
    Failed(String),
}

/// Why a drag cannot be started on this system.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum Unavailable {
    #[error("the compositor does not offer xdg_toplevel_drag_manager_v1")]
    NoToplevelDrag,
    #[error("the compositor offers no wl_seat or wl_data_device_manager")]
    NoSeatOrDataDevice,
    #[error("the Wayland connection could not be shared: {0}")]
    Connection(String),
    #[error("the proxy interposer is not installed in this executable (link with `-rdynamic` and `-Wl,--undefined=wl_proxy_marshal_flags`) or GTK has made no data device yet")]
    NoInterposer,
}
