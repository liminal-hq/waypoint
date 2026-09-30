// Waypoint's virtual file system: the `Provider` trait, the local provider, and listings that hold
// the sorted, filtered index in Rust and serve ranges of it to the frontend.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod cancel;
mod error;
mod icon;
mod index;
mod listing;
mod local;
mod model;
mod order;
mod places;
mod provider;
mod watch;

pub use cancel::CancelToken;
pub use error::from_io;
pub use icon::group_for;
pub use listing::{EventSink, Listing, ListingOptions, WatchState};
pub use local::LocalProvider;
pub use model::*;
pub use order::natural_key;
pub use places::*;
pub use provider::*;
pub use watch::{WatchMode, WatchOptions};
