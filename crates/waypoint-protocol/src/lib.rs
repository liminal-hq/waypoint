// Shared types crossing the Rust <-> JS boundary. `ts-rs` generates the
// matching TypeScript interfaces into `packages/protocol/src/generated/`
// from this crate — never hand-maintain a parallel TS copy of these shapes.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod entry_id;
mod location;
mod remote;
mod status;
mod vfs_error;
mod window;

pub use entry_id::*;
pub use location::*;
pub use remote::*;
pub use status::*;
pub use vfs_error::*;
pub use window::*;
