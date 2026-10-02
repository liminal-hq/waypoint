// Where the journal is saved: the storage trait the app implements, and the callback through which
// the journal asks for a save.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The app implements `JournalStorage` over its key-value store the way it does for the session: two
// generations (`journal` and `previous`, rotated on the first save of each run), `load` trying the
// latest and then the one before, and a copy of an unreadable file kept aside. This crate defines
// the seam only and does no I/O.

use thiserror::Error;

use super::model::JournalDocument;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum StorageError {
    #[error("storage failed: {0}")]
    Io(String),
}

/// What a load found.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Loaded {
    /// The saved document, or `None` when nothing was saved yet or nothing could be read.
    pub document: Option<JournalDocument>,
    /// The latest generation was unreadable and `document` is the one before it.
    pub from_previous: bool,
    /// Every generation that existed was unreadable (so `document` is `None`).
    pub unreadable: Option<String>,
    /// Where the unreadable file was copied to, when it was.
    pub set_aside: Option<String>,
}

pub trait JournalStorage: Send + Sync {
    /// Reads the journal. An error is a failure of the storage itself; a file that cannot be read
    /// as a document is reported in `Loaded`, never as an error.
    fn load(&self) -> Result<Loaded, StorageError>;

    /// Writes the document, rotating the earlier generation into `previous` on the first save of
    /// each run.
    fn save(&self, document: &JournalDocument) -> Result<(), StorageError>;

    /// Copies the stored file aside so it can be looked at, and says where. Called when a document
    /// reads but is of a version this build cannot use.
    fn set_aside(&self) -> Option<String> {
        None
    }
}

/// How the journal asks to be saved. The plugin debounces: it notes the request and calls
/// `Journal::flush` a moment later. The one save that is not a request is the write-ahead record,
/// which `Journal::begin` writes before the job starts.
pub trait SaveRequest: Send + Sync {
    fn save_requested(&self);
}

/// Ignores every request (the caller flushes when it likes).
#[derive(Debug, Clone, Copy, Default)]
pub struct NoSaveRequests;

impl SaveRequest for NoSaveRequests {
    fn save_requested(&self) {}
}
