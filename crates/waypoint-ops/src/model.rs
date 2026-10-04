// The wire types of the operations engine: jobs and their states, requests, conflicts, errors, and
// the snapshots and events the frontend mirrors.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use serde::{Deserialize, Serialize};
use thiserror::Error;
use ts_rs::TS;

use waypoint_protocol::{Location, VfsError};
use waypoint_vfs::{ListingHandle, SelectionSpec};

use crate::journal::{JournalEntrySummary, JournalId, JournalSnapshot, StaleReason};
use crate::rename_rules::RenameSpec;
use crate::schedule::Schedule;

/// Names a job. Global to the store and never reused while the store lives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct JobId(#[ts(type = "number")] pub u64);

/// What a job does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum JobKind {
    CreateFolder,
    CreateFile,
    /// Renames one entry within its folder.
    Rename,
    /// Copies each source next to itself under a new name.
    Duplicate,
    Trash,
    /// Takes entries out of the Trash and puts them back where they were.
    Restore,
    /// Removes entries permanently. An item in the Trash is removed from the Trash.
    Delete,
    /// Removes everything in the Trash, or only what has been there `older_than_days` days or more.
    /// It has no sources.
    EmptyTrash {
        #[ts(type = "number | null")]
        older_than_days: Option<u32>,
    },
    Copy,
    Move,
    /// Makes a symbolic link in the destination to each source, which stays where it is.
    Link,
    /// Renames many entries by a stack of rules, all or none (`JobRequest::rename` holds the rules).
    BatchRename,
    /// Reverses the journal entry `of`.
    Undo {
        of: JournalId,
    },
    /// Applies again the journal entry `of` that an undo reversed.
    Redo {
        of: JournalId,
    },
}

/// Where a job's sources come from (A47).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum Sources {
    /// What the user has selected in a listing; the engine resolves it, so a selection of a
    /// hundred thousand files is a handle and a range, never a list of paths.
    Selection {
        handle: ListingHandle,
        spec: SelectionSpec,
    },
    /// Explicit locations.
    Locations { locations: Vec<Location> },
}

/// What to do when a name is already taken in the destination (A48). Nothing is overwritten
/// without one of these being chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum ConflictPolicy {
    Replace,
    Skip,
    /// Keep the existing entry and give the new one a free name (`name (2).ext`).
    KeepBoth,
    /// Fold a folder into the existing folder of the same name.
    MergeFolders,
    ReplaceIfNewer,
}

/// Per-job choices that override the settings.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct JobOptions {
    /// The answer to every conflict, given up front. `None` asks.
    pub conflict: Option<ConflictPolicy>,
    /// Whether to verify copies. `None` takes the setting.
    pub verify: Option<bool>,
    /// This job's own speed limit in bytes per second, on top of the global one; `None` is no
    /// limit of its own (D157).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(type = "number | null", optional)]
    pub speed_limit: Option<u64>,
    /// Where the job stands when a slot frees up; `None` is `Normal` (D157).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub priority: Option<JobPriority>,
    /// When the job may start; `None` starts it as soon as a slot is free (D157).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub schedule: Option<Schedule>,
}

/// How soon a queued job is taken when a slot frees up: the highest first, and in queue order
/// among equals. A running job is never stopped for a higher one.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, TS,
)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum JobPriority {
    Low,
    #[default]
    Normal,
    High,
}

impl JobOptions {
    /// The priority the job runs at.
    pub fn priority(&self) -> JobPriority {
        self.priority.unwrap_or_default()
    }
}

/// A request to do something: what the frontend sends and the queue keeps, so a failed job can be
/// retried.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct JobRequest {
    pub kind: JobKind,
    pub sources: Sources,
    /// The folder to copy or move into, or the folder to create in. Ignored by operations that
    /// work in place.
    pub destination: Option<Location>,
    /// The new name, for a create or a rename.
    pub name: Option<String>,
    pub options: JobOptions,
    /// The window label the request came from, which a selection handle belongs to.
    pub origin_window: String,
    /// The rules of a batch rename.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub rename: Option<RenameSpec>,
}

/// How far a job has got.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct Progress {
    #[ts(type = "number")]
    pub items_done: u64,
    #[ts(type = "number")]
    pub items_total: u64,
    #[ts(type = "number")]
    pub bytes_done: u64,
    #[ts(type = "number")]
    pub bytes_total: u64,
    /// What the job is working on, as a name for people.
    pub current: Option<String>,
    #[ts(type = "number")]
    pub speed_bps: u64,
    #[ts(type = "number | null")]
    pub eta_ms: Option<u64>,
}

impl Progress {
    /// How much of the job is done, from 0 to 1, by bytes when there are any and by items
    /// otherwise.
    pub fn fraction(&self) -> f64 {
        let (done, total) = if self.bytes_total > 0 {
            (self.bytes_done, self.bytes_total)
        } else {
            (self.items_done, self.items_total)
        };
        if total == 0 {
            0.0
        } else {
            (done as f64 / total as f64).min(1.0)
        }
    }
}

/// How a name that is taken clashes with the entry that wants it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum ConflictKind {
    FileOverFile,
    FolderOverFolder,
    FileOverFolder,
    FolderOverFile,
}

/// One name that is already taken in the destination.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct Conflict {
    /// The entry being copied or moved.
    pub source: Location,
    /// What is in the way: the entry that already has the name, or for a clash inside the batch
    /// the place the first of the clashing sources will go.
    pub existing: Location,
    pub name: String,
    pub kind: ConflictKind,
    /// Two sources of one request want the same name, so nothing exists yet.
    pub within_batch: bool,
    #[ts(type = "number | null")]
    pub source_size: Option<u64>,
    #[ts(type = "number | null")]
    pub existing_size: Option<u64>,
    #[ts(type = "number | null")]
    pub source_modified_ms: Option<i64>,
    #[ts(type = "number | null")]
    pub existing_modified_ms: Option<i64>,
}

/// The user's answer to a conflict, or to all of them (`conflict` `None`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct Resolution {
    /// The source the answer is for; `None` applies the policy to every remaining conflict.
    pub source: Option<Location>,
    pub policy: ConflictPolicy,
}

/// The user's answer to an error that stopped a job on one item (A48).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum Decision {
    Retry,
    /// Make the folders the item needs (a restore from the Trash whose original folder is gone),
    /// then try again. For any other error it is `Retry`.
    CreateParents,
    Skip,
    /// Skip this item and every later one that fails the same kind of way (the same `OpsError`
    /// variant); a different kind of failure asks again.
    SkipAll,
    Cancel,
}

/// Why an operation failed, as the UI shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, Error)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum OpsError {
    #[error("{} was not found", .location.display)]
    NotFound { location: Location },
    #[error("permission denied for {}", .location.display)]
    PermissionDenied { location: Location },
    /// The destination volume has too little room. Zero means the provider could not say.
    #[error("not enough space ({needed} bytes needed, {free} free)")]
    NotEnoughSpace {
        #[ts(type = "number")]
        needed: u64,
        #[ts(type = "number")]
        free: u64,
    },
    #[error("invalid name {name:?}: {reason}")]
    InvalidName { name: String, reason: String },
    #[error("{} is already taken", .location.display)]
    NameInUse { location: Location },
    /// The operation would put the entries where they already are.
    #[error("the entries are already in that folder")]
    SameFolder,
    /// The destination is inside one of the sources.
    #[error("a folder cannot be put inside itself")]
    IntoItself,
    /// A root, the home folder or a mount point, which this operation never touches.
    #[error("{} is protected", .location.display)]
    Protected { location: Location },
    #[error("the Trash is unavailable: {reason}")]
    TrashUnavailable { reason: String },
    /// A restore needs a folder that no longer exists: `location` is that folder, which the
    /// `CreateParents` decision makes.
    #[error("{} no longer exists", .location.display)]
    OriginMissingParent { location: Location },
    #[error("cancelled")]
    Cancelled,
    #[error("unsupported: {what}")]
    Unsupported { what: String },
    /// An entry is no longer what the plan saw.
    #[error("{} changed since it was planned", .location.display)]
    ChangedSince { location: Location },
    /// A copy did not read back as it was read: the partial file was removed (A51).
    #[error("{} did not verify: expected {expected}, found {actual}", .location.display)]
    VerifyFailed {
        location: Location,
        /// The digest of what was read from the source, in hex.
        expected: String,
        /// The digest of what was read back, in hex.
        actual: String,
    },
    /// `Replace` was chosen for a clash it cannot resolve: a file and a folder share the name, and
    /// replacing one with the other would delete a tree (or bury a file) without being asked.
    #[error("{} cannot be replaced by an entry of another kind", .location.display)]
    CannotReplace { location: Location },
    /// An undo found the entry changed since the job left it, and did nothing.
    #[error("{} cannot be undone: {reason:?}", .location.display)]
    UndoStale {
        location: Location,
        reason: StaleReason,
    },
    /// There is no such entry in the journal, or it is not in a state this can be done in.
    #[error("{reason}")]
    UndoUnavailable { reason: String },
    /// A server could not be reached, asked for a login or a trust decision, or dropped the
    /// connection (A80). One kind, so Skip all covers every connection failure; Retry reconnects.
    #[error("connection problem: {error:?}")]
    Connection { error: VfsError },
    #[error("{message}")]
    Io { message: String },
}

impl From<VfsError> for OpsError {
    fn from(error: VfsError) -> Self {
        match error {
            VfsError::NotFound { location } => OpsError::NotFound { location },
            VfsError::PermissionDenied { location } | VfsError::ReadOnly { location } => {
                OpsError::PermissionDenied { location }
            }
            VfsError::AlreadyExists { location } => OpsError::NameInUse { location },
            VfsError::StorageFull { .. } => OpsError::NotEnoughSpace { needed: 0, free: 0 },
            VfsError::InvalidName { name, reason } => OpsError::InvalidName { name, reason },
            VfsError::Cancelled => OpsError::Cancelled,
            VfsError::Unsupported { what } => OpsError::Unsupported { what },
            VfsError::NotADirectory { location } => OpsError::Io {
                message: format!("{} is not a folder", location.display),
            },
            VfsError::IsADirectory { location } => OpsError::Io {
                message: format!("{} is a folder", location.display),
            },
            VfsError::NotEmpty { location } => OpsError::Io {
                message: format!("{} is not empty", location.display),
            },
            VfsError::NotText { location } => OpsError::Io {
                message: format!("{} is not a text file", location.display),
            },
            VfsError::InUse { location } => OpsError::Io {
                message: format!("{} is in use by another program", location.display),
            },
            VfsError::CrossesDevices { from, to } => OpsError::Io {
                message: format!(
                    "{} and {} are on different volumes",
                    from.display, to.display
                ),
            },
            VfsError::InvalidLocation { input } => OpsError::Io {
                message: format!("{input:?} is not a location"),
            },
            VfsError::StaleHandle => OpsError::Io {
                message: "the listing was closed".to_owned(),
            },
            VfsError::Corrupt { location } => OpsError::Io {
                message: format!("{} is damaged", location.display),
            },
            error @ (VfsError::Disconnected { .. }
            | VfsError::Unreachable { .. }
            | VfsError::Timeout { .. }
            | VfsError::AuthRequired { .. }
            | VfsError::AuthFailed { .. }
            | VfsError::HostKeyUnknown { .. }
            | VfsError::HostKeyChanged { .. }
            | VfsError::CertificateUntrusted { .. }
            | VfsError::RateLimited { .. }) => OpsError::Connection { error },
            VfsError::Io { message, .. } => OpsError::Io { message },
        }
    }
}

/// Why a job has stopped to wait for the user.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum WaitReason {
    /// Names are taken in the destination: answer with `Resolution`s.
    Conflicts { conflicts: Vec<Conflict> },
    /// An item failed: answer with a `Decision`.
    Error { error: OpsError, item: Location },
}

/// Where a job is in its life.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "state",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum JobState {
    /// Resolving sources and walking them.
    Planning,
    Queued,
    Running,
    Paused,
    /// Stopped for an answer from the user. It keeps its worker.
    Waiting {
        reason: WaitReason,
    },
    /// Asked to stop; unwinding what it started.
    Cancelling,
    Cancelled,
    Done,
    Failed {
        error: OpsError,
        /// The item that failed, when one did.
        item: Option<Location>,
        /// How many items were completed before the failure.
        #[ts(type = "number")]
        done: u64,
    },
}

impl JobState {
    /// The name of the state, for messages and the transition table.
    pub fn name(&self) -> &'static str {
        match self {
            JobState::Planning => "planning",
            JobState::Queued => "queued",
            JobState::Running => "running",
            JobState::Paused => "paused",
            JobState::Waiting { .. } => "waiting",
            JobState::Cancelling => "cancelling",
            JobState::Cancelled => "cancelled",
            JobState::Done => "done",
            JobState::Failed { .. } => "failed",
        }
    }

    /// Done, failed or cancelled: nothing more happens to the job except being dismissed.
    pub fn is_finished(&self) -> bool {
        matches!(
            self,
            JobState::Done | JobState::Failed { .. } | JobState::Cancelled
        )
    }

    /// Whether the job holds one of the worker slots: it is running, or parked mid-run (paused or
    /// waiting) or unwinding.
    pub fn holds_slot(&self) -> bool {
        matches!(
            self,
            JobState::Running | JobState::Paused | JobState::Waiting { .. } | JobState::Cancelling
        )
    }
}

/// How many items ended another way than done.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct Counts {
    #[ts(type = "number")]
    pub skipped: u64,
    #[ts(type = "number")]
    pub failed: u64,
}

/// What a job works on, in a line.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct SourcesSummary {
    /// How many top-level sources; `None` until the planner has resolved a selection.
    #[ts(type = "number | null")]
    pub count: Option<u64>,
    /// The name of the first source, once known.
    pub first: Option<String>,
}

/// One job as the frontend sees it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct JobSnapshot {
    pub id: JobId,
    pub kind: JobKind,
    pub state: JobState,
    /// A plain description for a list or a notice ("Trashing 3 items"); the frontend may word
    /// its own from `kind` and `sources`.
    pub title: String,
    pub sources: SourcesSummary,
    pub destination: Option<Location>,
    pub options: JobOptions,
    pub origin_window: String,
    pub counts: Counts,
    pub progress: Progress,
    #[ts(type = "number")]
    pub created_ms: i64,
    #[ts(type = "number | null")]
    pub started_ms: Option<i64>,
    #[ts(type = "number | null")]
    pub finished_ms: Option<i64>,
    /// Whether the journal holds an entry that can undo this job. The store starts it false; the
    /// plugin sets it with `OpsStore::mark_undoable` once the journal has committed the entry.
    pub undoable: bool,
    /// What verification recorded, once a verified copy or move has checked at least one file
    /// (A51); `None` when the job did not verify.
    pub verified: Option<Verification>,
}

/// The whole queue at one revision, in queue order.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct OpsSnapshot {
    #[ts(type = "number")]
    pub revision: u64,
    pub jobs: Vec<JobSnapshot>,
    /// Pause all is in force: no queued job starts until Resume all (D157).
    #[serde(default)]
    pub paused: bool,
    /// What Undo and Redo would do. The journal counts its own revision, so this part is mirrored
    /// by `OpsEvent::JournalChanged` on its own gate.
    pub journal: JournalSnapshot,
}

/// Something that happened to the queue. Applying the events in order to a snapshot at the
/// revision before the first reproduces the queue; each carries the revision it produced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum OpsEvent {
    /// A job joined the end of the queue.
    JobAdded {
        job: JobSnapshot,
        #[ts(type = "number")]
        revision: u64,
    },
    /// A job changed state or progress.
    JobChanged {
        job: JobSnapshot,
        #[ts(type = "number")]
        revision: u64,
    },
    JobRemoved {
        id: JobId,
        #[ts(type = "number")]
        revision: u64,
    },
    /// The queue is now in this order.
    QueueReordered {
        order: Vec<JobId>,
        #[ts(type = "number")]
        revision: u64,
    },
    /// Pause all or Resume all: whether queued jobs are held back from starting (D157). The jobs
    /// it paused or resumed arrive as `JobChanged` events of their own.
    QueuePaused {
        paused: bool,
        #[ts(type = "number")]
        revision: u64,
    },
    /// The undo history changed. `revision` is the journal's own, not the queue's.
    JournalChanged {
        #[ts(type = "number")]
        revision: u64,
        undo: Option<JournalEntrySummary>,
        redo: Option<JournalEntrySummary>,
    },
}

impl OpsEvent {
    pub fn revision(&self) -> u64 {
        match self {
            OpsEvent::JobAdded { revision, .. }
            | OpsEvent::JobChanged { revision, .. }
            | OpsEvent::JobRemoved { revision, .. }
            | OpsEvent::QueueReordered { revision, .. }
            | OpsEvent::QueuePaused { revision, .. }
            | OpsEvent::JournalChanged { revision, .. } => *revision,
        }
    }
}

impl OpsSnapshot {
    /// Applies one event, as a mirror does. An event at or below the snapshot's revision is stale
    /// and ignored; a journal event is judged against the journal's revision instead.
    pub fn apply(&mut self, event: &OpsEvent) {
        if let OpsEvent::JournalChanged {
            revision,
            undo,
            redo,
        } = event
        {
            if *revision > self.journal.revision {
                self.journal = JournalSnapshot {
                    revision: *revision,
                    undo: undo.clone(),
                    redo: redo.clone(),
                };
            }
            return;
        }
        if event.revision() <= self.revision {
            return;
        }
        self.revision = event.revision();
        match event {
            OpsEvent::JobAdded { job, .. } => self.jobs.push(job.clone()),
            OpsEvent::JobChanged { job, .. } => {
                if let Some(slot) = self.jobs.iter_mut().find(|j| j.id == job.id) {
                    *slot = job.clone();
                }
            }
            OpsEvent::JobRemoved { id, .. } => self.jobs.retain(|j| j.id != *id),
            OpsEvent::JournalChanged { .. } => {}
            OpsEvent::QueuePaused { paused, .. } => self.paused = *paused,
            OpsEvent::QueueReordered { order, .. } => {
                let mut taken: Vec<Option<JobSnapshot>> = std::mem::take(&mut self.jobs)
                    .into_iter()
                    .map(Some)
                    .collect();
                for id in order {
                    if let Some(at) = taken
                        .iter()
                        .position(|j| j.as_ref().is_some_and(|j| j.id == *id))
                    {
                        self.jobs.extend(taken[at].take());
                    }
                }
                self.jobs.extend(taken.into_iter().flatten());
            }
        }
    }
}

/// Which hash verification uses (A51).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum VerifyAlgorithm {
    Blake3,
    Sha256,
}

/// What a verified job recorded (A51): how many files read back as they were read, and one digest
/// over all of them, so two runs over the same bytes record the same value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct Verification {
    pub algorithm: VerifyAlgorithm,
    /// The hex digest, in the job's algorithm, of the files' own digests concatenated in the order
    /// they were verified.
    pub digest: String,
    #[ts(type = "number")]
    pub files: u64,
}

/// The settings the engine reads (read through a `SettingsReader`; the Settings window owns them).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct OpsSettings {
    /// How many jobs run at once (A55).
    pub concurrency: u32,
    /// Verify after copy unless a job says otherwise (D103).
    pub verify_after_copy: bool,
    pub verify_algorithm: VerifyAlgorithm,
    /// Ask before moving to the Trash (D104).
    pub confirm_trash: bool,
    /// How many journal entries to keep (A52).
    pub undo_depth: u32,
    /// Empty items that have been in the Trash this many days or more when the app starts; `None`
    /// (the default) never empties it by itself.
    #[serde(default)]
    #[ts(type = "number | null")]
    pub trash_expiry_days: Option<u32>,
    /// The speed limit for every copy and move together, in bytes per second; `None` (the default)
    /// is no limit (D157).
    #[serde(default)]
    #[ts(type = "number | null")]
    pub speed_limit_bps: Option<u64>,
}

impl Default for OpsSettings {
    fn default() -> Self {
        Self {
            concurrency: 2,
            verify_after_copy: false,
            verify_algorithm: VerifyAlgorithm::Blake3,
            confirm_trash: false,
            undo_depth: 50,
            trash_expiry_days: None,
            speed_limit_bps: None,
        }
    }
}

/// What the planner found out and the store needs to know about a job.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PlanTotals {
    pub sources: SourcesSummary,
    pub items: u64,
    pub bytes: u64,
    /// Every folder the job reads from or writes to, for the close guard (D29).
    pub touches: Vec<Location>,
    /// The entries the job removes or moves, which the guard also covers below.
    pub trees: Vec<Location>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_and_kinds_serialise_plainly() {
        assert_eq!(serde_json::to_string(&JobId(7)).unwrap(), "7");
        assert_eq!(
            serde_json::to_string(&JobKind::CreateFolder).unwrap(),
            r#"{"kind":"createFolder"}"#
        );
        assert_eq!(
            serde_json::to_string(&JobKind::Undo { of: JournalId(3) }).unwrap(),
            r#"{"kind":"undo","of":3}"#
        );
    }

    #[test]
    fn states_carry_a_tag_and_camel_case_fields() {
        let failed = JobState::Failed {
            error: OpsError::Cancelled,
            item: None,
            done: 2,
        };
        assert_eq!(
            serde_json::to_string(&failed).unwrap(),
            r#"{"state":"failed","error":{"kind":"cancelled"},"item":null,"done":2}"#
        );
        let waiting = JobState::Waiting {
            reason: WaitReason::Error {
                error: OpsError::NotEnoughSpace { needed: 5, free: 1 },
                item: Location::new("/a", "file:///a"),
            },
        };
        let json = serde_json::to_string(&waiting).unwrap();
        assert!(json.starts_with(r#"{"state":"waiting","reason":{"kind":"error","error":{"kind":"notEnoughSpace","needed":5,"free":1}"#));
        assert_eq!(serde_json::from_str::<JobState>(&json).unwrap(), waiting);
    }

    #[test]
    fn a_vfs_error_keeps_its_meaning() {
        let here = Location::new("/a", "file:///a");
        assert_eq!(
            OpsError::from(VfsError::AlreadyExists {
                location: here.clone()
            }),
            OpsError::NameInUse {
                location: here.clone()
            }
        );
        assert_eq!(
            OpsError::from(VfsError::ReadOnly {
                location: here.clone()
            }),
            OpsError::PermissionDenied { location: here }
        );
        assert_eq!(OpsError::from(VfsError::Cancelled), OpsError::Cancelled);
    }

    #[test]
    fn progress_is_a_fraction_of_bytes_then_items() {
        let mut p = Progress::default();
        assert_eq!(p.fraction(), 0.0);
        p.items_total = 4;
        p.items_done = 1;
        assert_eq!(p.fraction(), 0.25);
        p.bytes_total = 200;
        p.bytes_done = 50;
        assert_eq!(p.fraction(), 0.25);
        p.bytes_done = 500;
        assert_eq!(p.fraction(), 1.0);
    }
}
