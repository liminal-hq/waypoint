// Registers the volumes plugin: list drives, mounts and network shares with their free space, and mount, unmount, eject and unlock them, on Linux and Windows
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod debounce;
mod error;
mod holders;
pub mod models;
pub mod mountinfo;
pub mod space;
pub mod udisks;
mod winmap;

pub use error::{Result, VolumesError};
pub use models::*;
