// Waypoint's virtual file system: the `Provider` trait, the local provider, and listings that hold
// the sorted, filtered index in Rust and serve ranges of it to the frontend.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod cancel;
mod details;
mod error;
mod group;
mod icon;
mod index;
mod listing;
mod local;
#[cfg(any(test, feature = "testing"))]
mod memory;
mod model;
mod names;
mod navigation;
mod order;
mod places;
mod provider;
mod space;
#[cfg(unix)]
#[path = "sys_unix.rs"]
mod sys;
#[cfg(windows)]
#[path = "sys_windows.rs"]
mod sys;
mod trash;
mod watch;
mod write;

pub use cancel::CancelToken;
pub use details::*;
pub use error::{from_io, from_io_pair, InjectedError};
pub use icon::group_for;
pub use listing::{EventSink, Listing, ListingOptions, WatchState};
pub use local::LocalProvider;
#[cfg(any(test, feature = "testing"))]
pub use memory::{MemOp, MemoryProvider};
pub use model::*;
pub use names::{child_path, validate_name};
pub use navigation::{describe_location, parse_location};
pub use order::natural_key;
pub use places::*;
pub use provider::*;
pub use space::free_space;
#[cfg(any(test, feature = "testing"))]
pub use trash::MemoryTrashSource;
pub use trash::{
    TrashInfo, TrashProvider, TrashSource, TrashedItem, POLL_INTERVAL as TRASH_POLL_INTERVAL,
};
pub use watch::{WatchMode, WatchOptions};
pub use write::{FileTimes, Permissions, ReadStream, VolumeId, WriteOptions, WriteStream};
