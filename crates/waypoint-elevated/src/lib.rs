// Waypoint's elevated provider: the wire protocol, the privileged helper's serve loop and the
// client that reaches the helper over a byte stream. It knows nothing of how the helper is started.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod client;
mod connection;
pub mod frame;
mod os_name;
mod outbox;
mod paths;
pub mod policy;
mod server;
mod streams;
mod sync;
#[cfg(any(test, feature = "testing"))]
pub mod testing;
pub mod wire;

/// The one line the helper writes to its error stream, before it serves anything, once it is running. The program that starts the helper waits for it, because the system's prompt can keep the helper from starting for as long as the person likes. This is a wire contract with the launching plugin, which is configured with the same text and never imports this crate.
pub const READY_LINE: &str = "waypoint-elevate-helper ready";

pub use client::{ElevatedProvider, Launcher, Transport};
pub use frame::{read_frame, write_frame, Frame, FrameError, MAX_CHUNK, MAX_FRAME};
pub use os_name::{WireError, WireOs};
pub use server::{serve, Fault, ServeConfig, ServeEnd};
