// Waypoint's Git provider: revisions browsed as read-only folders, and the status of working trees.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod errors;
mod history;
mod names;
mod provider;
mod repository;
mod status;

pub use history::{
    diff_stat, history, CommitInfo, DiffStat, History, DIFF_FILE_BYTES, DIFF_FILE_CAP, SCAN_CAP,
};
pub use provider::{GitEntryInfo, GitProvider, ObjectKind};
pub use repository::{find_repository, repositories_in};
pub use status::*;
