// Shared types crossing the Rust <-> JS boundary. `ts-rs` generates the
// matching TypeScript interfaces into `apps/waypoint/src/domain/protocol/generated/`
// from this crate — never hand-maintain a parallel TS copy of these shapes.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod status;
mod window;

pub use status::*;
pub use window::*;
