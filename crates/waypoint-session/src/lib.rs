// Waypoint's session state. This slice is the minimal tab model of milestone 2: tabs, order, the
// active tab and per-tab history. Groups, pairs, pinning and persistence arrive in milestone 3.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod model;
mod reducer;
mod session;

pub use model::*;
pub use reducer::{apply, Command, SessionError};
pub use session::Session;
