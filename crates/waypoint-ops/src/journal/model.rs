// The wire types of the undo journal: entries and their inverse steps, fingerprints, the document
// that is saved, and the report that start-up recovery hands to the app.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use waypoint_protocol::Location;

use crate::model::{JobId, JobKind, JobRequest, ResumePoint};
use crate::traits::TrashReceipt;

/// The document format this build writes and reads.
pub const JOURNAL_VERSION: u32 = 1;

/// Names a journal entry. Unlike a `JobId` it survives a restart, so an `Undo` job refers to one of
/// these and never to the job that made the entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct JournalId(#[ts(type = "number")] pub u64);

/// What an entry looked like when the job finished, so an undo can tell that someone changed it
/// since and refuse rather than destroy newer work.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct Fingerprint {
    pub is_dir: bool,
    /// A file's size in bytes.
    #[ts(type = "number | null")]
    pub size: Option<u64>,
    /// A file's modification time. A folder's own time is not compared: it moves whenever a child
    /// is added or removed, including by this journal's own later undos.
    #[ts(type = "number | null")]
    pub modified_ms: Option<i64>,
    /// How many entries a folder holds, at every depth.
    #[ts(type = "number | null")]
    pub entry_count: Option<u64>,
    /// A hash over the names, kinds and sizes (and file times) below a folder, or over a link's
    /// text: any change inside the tree shows up in it. Hexadecimal.
    pub digest: Option<String>,
}

/// One thing an undo does. They are stored in the order the job did the originals, and applied in
/// reverse. The set grows with the operations, so match with a catch-all arm.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
#[non_exhaustive]
pub enum InverseStep {
    /// Renames the entry at `from` back to the name at `to`.
    Rename { from: Location, to: Location },
    /// Moves the entry at `from` back to `to`, which may be in another folder.
    MoveBack { from: Location, to: Location },
    /// Removes an entry the job made, when it is still exactly what the job left. `fingerprint` is
    /// `None` only when it could not be taken, and the undo is then refused.
    RemoveCreated {
        location: Location,
        fingerprint: Option<Fingerprint>,
    },
    /// Puts back an entry a move across volumes took away: copies the entry at `from` (what the
    /// move left at its destination) to `to`, and removes `from` once the copy is in place. It is
    /// refused unless `from` is still exactly what `fingerprint` says and `to` is free, and `from`
    /// is checked again after the copy, so nothing is overwritten and nothing newer is lost.
    CopyBack {
        from: Location,
        to: Location,
        fingerprint: Option<Fingerprint>,
    },
    /// Makes a folder a move removed after emptying it into another, with the time and mode it
    /// had, so the entries that go back have somewhere to be.
    CreateDir {
        location: Location,
        #[ts(type = "number | null")]
        modified_ms: Option<i64>,
        #[ts(type = "number | null")]
        mode: Option<u32>,
    },
    /// Puts a trashed item back where it was.
    RestoreTrashed { receipt: TrashReceipt },
    /// Removes a folder, which must be empty, apart from entries that earlier steps of the same
    /// undo take out of it.
    RemoveEmptyDir { location: Location },
}

impl InverseStep {
    /// The entry the step acts on, for refusals and progress.
    pub fn subject(&self) -> Location {
        match self {
            InverseStep::Rename { from, .. } | InverseStep::MoveBack { from, .. } => from.clone(),
            InverseStep::RemoveCreated { location, .. }
            | InverseStep::RemoveEmptyDir { location }
            | InverseStep::CreateDir { location, .. } => location.clone(),
            InverseStep::CopyBack { from, .. } => from.clone(),
            InverseStep::RestoreTrashed { receipt } => receipt.original.clone(),
        }
    }
}

/// Why an undo found the world changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum StaleReason {
    /// The entry the step acts on is gone, or so is the folder it goes back into.
    Missing,
    /// The entry is not what the job left.
    Changed,
    /// The name the entry would go back to is taken.
    NameTaken,
    /// The item is no longer in the Trash.
    TrashEmptied,
    /// There was no fingerprint to check the entry against.
    Unverified,
}

/// Whether an entry's effects are in place.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum EntryState {
    Applied,
    Undone,
}

/// Enough to do the job again: the request with its sources resolved to locations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct ForwardSpec {
    pub request: JobRequest,
}

/// One undoable thing the user did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct JournalEntry {
    pub id: JournalId,
    #[ts(type = "number")]
    pub at_ms: i64,
    /// A title for people: "Move 3 items to Trash".
    pub label: String,
    pub kind: JobKind,
    /// The job of the run that made the entry. Job ids restart with the app, so this is for
    /// reference only.
    pub job: JobId,
    /// What an undo does, in the order the job did the originals. After a stopped undo it holds
    /// only the steps still to do.
    pub inverse: Vec<InverseStep>,
    pub forward: ForwardSpec,
    pub state: EntryState,
    /// Which undo this was, counting up over the whole journal; zero while applied. The redo of
    /// the highest is the one a redo shortcut does.
    #[ts(type = "number")]
    pub undone_order: u64,
    /// An undo stopped part way: some of the inverse steps are done and the rest remain. Undoing
    /// again finishes it; it cannot be redone until then.
    pub partly_undone: bool,
}

/// An entry as a menu or the palette lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct JournalEntrySummary {
    pub id: JournalId,
    pub label: String,
    #[ts(type = "number")]
    pub at_ms: i64,
    pub undoable: bool,
    pub redoable: bool,
    /// An undo of this entry stopped part way: undoing it again finishes the rest, and it cannot be redone until then.
    pub partly_undone: bool,
}

/// What the app needs to label Undo and Redo, at one revision.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct JournalSnapshot {
    /// The journal's own revision, which counts its changes and carries on across restarts.
    #[ts(type = "number")]
    pub revision: u64,
    /// What Undo would undo: the newest applied entry.
    pub undo: Option<JournalEntrySummary>,
    /// What Redo would redo: the entry undone most recently.
    pub redo: Option<JournalEntrySummary>,
}

/// A rename a job was in the middle of, so recovery can put a case-only rename's file back under
/// its name when the job was interrupted between its two steps.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct RenamePair {
    pub from: Location,
    pub to: Location,
}

/// The write-ahead record of a job that has begun and not yet been committed or aborted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct PendingRecord {
    pub job: JobId,
    #[ts(type = "number")]
    pub at_ms: i64,
    pub kind: JobKind,
    pub label: String,
    /// The entries the job planned to act on, for the notice.
    pub items: Vec<Location>,
    /// The folders the job writes partial files into, which are the only places recovery looks.
    pub folders: Vec<Location>,
    pub renames: Vec<RenamePair>,
}

/// A job held by a schedule that has not started: what the journal keeps so that it is made again
/// after a restart (D157). The request carries the schedule, the options as they are now and the
/// sources as locations (a selection was resolved when the job was submitted).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct ScheduledRecord {
    /// The job this run knew it as; job ids restart with the app, so this is for reference only.
    pub job: JobId,
    pub request: JobRequest,
}

/// A transfer that stopped on a lost connection with partial files on a server that a later run
/// can continue (D165). It is kept until it is resumed, discarded or dismissed, across restarts:
/// start-up recovery leaves its partial files alone and offers it, and never resumes it by itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct ResumableRecord {
    /// The job that stopped; ids restart with the app, so after a restart it only names the record.
    pub job: JobId,
    /// What the job did, for the notice ("Copy 3 items to NAS").
    pub label: String,
    #[ts(type = "number")]
    pub at_ms: i64,
    /// The request to run again, its sources as locations.
    pub request: JobRequest,
    /// The partial files kept, one for each file that stopped part way.
    pub points: Vec<ResumePoint>,
}

/// Everything the journal keeps. Entries are oldest first.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct JournalBody {
    #[ts(type = "number")]
    pub revision: u64,
    #[ts(type = "number")]
    pub next_id: u64,
    #[ts(type = "number")]
    pub undo_counter: u64,
    pub entries: Vec<JournalEntry>,
    pub pending: Vec<PendingRecord>,
    /// Jobs waiting for their schedule.
    #[serde(default)]
    pub scheduled: Vec<ScheduledRecord>,
    /// Transfers that can continue the partial files a lost connection left (D165).
    #[serde(default)]
    pub resumable: Vec<ResumableRecord>,
}

/// The journal as a file: a version so a later build can migrate, and the body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct JournalDocument {
    pub version: u32,
    pub body: JournalBody,
}

impl JournalDocument {
    pub fn new(body: JournalBody) -> Self {
        Self {
            version: JOURNAL_VERSION,
            body,
        }
    }
}

/// A job that was running when the app stopped, and what recovery did about it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct InterruptedJob {
    pub job: JobId,
    pub kind: JobKind,
    pub label: String,
    #[ts(type = "number")]
    pub at_ms: i64,
    /// What the job planned to act on.
    pub planned: Vec<Location>,
    /// Partial files recovery removed.
    pub removed: Vec<Location>,
    /// Files a case-only rename had set aside, put back under their names.
    pub restored: Vec<Location>,
    /// Partial files recovery could not remove (or put back).
    pub left: Vec<Location>,
}

/// Why a stored journal was not used.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum DiscardReason {
    /// Neither the stored journal nor the one before it could be read.
    Corrupt {
        why: String,
    },
    UnsupportedVersion {
        found: u32,
        supported: u32,
    },
    /// The storage itself failed.
    StorageFailed {
        why: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct Discarded {
    pub reason: DiscardReason,
    /// Where the unreadable file was copied to, when it was.
    pub set_aside: Option<String>,
}

/// What start-up recovery found and did. The app turns it into the notice "An operation was
/// interrupted"; nothing is ever resumed.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct RecoveryReport {
    pub interrupted: Vec<InterruptedJob>,
    pub discarded: Option<Discarded>,
    /// The latest copy was unreadable and the one before it was used.
    pub from_previous: bool,
    /// Repairs made to a document that read but was inconsistent.
    pub repairs: Vec<String>,
    /// Transfers that stopped on a lost connection and can continue where they were (D165), which
    /// the notice offers to resume; nothing resumes by itself.
    #[serde(default)]
    pub resumable: Vec<ResumableRecord>,
}

impl RecoveryReport {
    /// Whether there is anything to tell the person.
    pub fn needs_notice(&self) -> bool {
        !self.interrupted.is_empty()
            || self.discarded.is_some()
            || self.from_previous
            || !self.resumable.is_empty()
    }
}
