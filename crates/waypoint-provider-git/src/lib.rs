// Waypoint's Git provider: revisions browsed as read-only folders, and the status of working trees.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod errors;
mod names;
mod provider;
mod repository;
mod status;

pub use provider::{GitEntryInfo, GitProvider, ObjectKind};
pub use repository::{find_repository, repositories_in};
pub use status::*;
