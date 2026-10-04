// The status overlay: what changed in a working tree, kept fresh without ever blocking a listing.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod compute;
mod model;

pub use compute::{compute, StatusError, StatusOptions, UntrackedMode};
pub use model::{Change, EntryStatus, FolderBadge, NameStatus, RepoStatus, StatusCounts};
