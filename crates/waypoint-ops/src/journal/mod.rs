// The undo journal (A52): what each undoable job did and how to reverse it, kept globally, capped,
// saved through an injected storage with a write-ahead record, and recovered at start-up.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// `model` holds the wire types, `engine` the `Journal`, `record` the translation from a finished
// job to an entry and from a plan to a write-ahead record, `apply` undo and redo as jobs,
// `fingerprint` the check that an entry is unchanged, `storage` the seams the app implements, and
// `recover` the start-up pass. The crate does no I/O here except through the storage and the
// providers it is given.

mod apply;
mod engine;
mod fingerprint;
mod model;
mod record;
mod recover;
mod storage;

pub use apply::{
    check_steps, fingerprint_steps, prepare, Prepared, RedoPlan, UndoFailure, UndoPlan, UndoReport,
};
pub use engine::{Journal, JournalDeps, Recorded};
pub use fingerprint::{fingerprint, same as same_fingerprint, verify as verify_fingerprint};
pub use model::*;
pub use recover::{recover, Recovery};
pub use storage::{JournalStorage, Loaded, NoSaveRequests, SaveRequest, StorageError};
