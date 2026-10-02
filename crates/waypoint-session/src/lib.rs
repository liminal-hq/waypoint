// Waypoint's session store: every window's tabs, groups, pairs and view as one pure reducer.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// A `Store` holds all windows under one global revision and one writer. `Store::dispatch` applies
// a `Command` to a copy and returns the granular events (each tagged with its window) the change
// made; a command that changes nothing makes no event and keeps the revision. `Store::to_document`
// and `Store::from_document` carry the store through a versioned `Document`, and `SessionStorage`
// is the seam the app implements over its key-value store. `Session` and `apply` are the
// milestone 2 single-window view over the same reducer.
//
// Most-recently-used rule: `mru` lists the tabs that were explicitly activated (by `Activate`
// changing the active tab, or by `Reopen`), newest first, and is trimmed as tabs close. Opening,
// moving and closing do not record the tab they activate. When the active tab closes, the first
// live `mru` entry becomes active, and when `mru` has none the neighbour rule of milestone 2
// applies (the tab that takes its place, else the one before it).

mod diff;
mod document;
mod layout;
mod model;
mod reducer;
mod session;
mod store;

pub use document::{Document, DocumentError, SessionStorage, StorageError, DOCUMENT_VERSION};
pub use model::*;
pub use reducer::{apply, Command, GroupSort, MoveTo, MoveWhat, SessionError};
pub use session::Session;
pub use store::{Outcome, Store, StorePolicy, CLOSED_LIMIT, SHELF_LIMIT};
