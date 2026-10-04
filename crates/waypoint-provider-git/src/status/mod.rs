// The status overlay: what changed in a working tree, kept fresh without ever blocking a listing.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod compute;
mod marks;
mod model;
mod service;
mod summary;
mod throttle;
mod tracker;

pub use compute::{compute, StatusError, StatusOptions, UntrackedMode};
pub use marks::{folder_marks, mark};
pub use model::{Change, EntryStatus, FolderBadge, NameStatus, RepoStatus, StatusCounts};
pub use service::{StatusService, Subscription};
pub use summary::{summarize, AheadBehind, HeadState, InProgress, RepoSummary, AHEAD_BEHIND_CAP};
pub use throttle::Throttle;
pub use tracker::{rel_bytes, Snapshot, TrackEvent, TrackSink, Tracker, TrackerOptions};
