// The wire types only this plugin sends: progress ticks, the clipboard, a dry-run plan and the
// plugin's error. The engine's own types (jobs, requests, events, settings) are the protocol's.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use waypoint_ops::{
    Conflict, Counts, JobId, JobKind, JournalId, OpsError, Progress, QueueError, SourcesSummary,
};
use waypoint_protocol::Location;

/// How far one job has got, sent on a window's progress channel at the rate the queue's gate
/// allows (100 ms and 1 %). Progress is not broadcast: only windows that subscribed hear it, and
/// the events carry state changes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct JobProgress {
    pub job: JobId,
    /// The queue's revision when the tick was made. Ticks use up revisions, so the events a window
    /// receives have gaps.
    #[ts(type = "number")]
    pub revision: u64,
    pub progress: Progress,
    pub counts: Counts,
}

/// The journal entry a job made, sent once on `waypoint-ops://job-journal` when the entry is
/// recorded (a notice's Undo names the entry, so it undoes this job's work and nothing newer).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct JobJournal {
    pub job: JobId,
    pub entry: JournalId,
}

/// What a paste does with the entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum ClipboardMode {
    Copy,
    Cut,
}

/// Who put the entries on the clipboard: Waypoint's own Cut or Copy, or another application's file
/// clipboard that Waypoint adopted when it was pasted (or the window regained focus).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum ClipboardSource {
    #[default]
    App,
    Os,
}

/// The entries Cut or Copy last put on the shared clipboard, which every window sees.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct Clipboard {
    pub mode: ClipboardMode,
    pub items: Vec<Location>,
    /// Who set it; a clear is the app's.
    #[serde(default)]
    pub source: ClipboardSource,
    /// Counts up with every change, including a clear.
    #[ts(type = "number")]
    pub revision: u64,
}

impl Default for Clipboard {
    fn default() -> Self {
        Self {
            mode: ClipboardMode::Copy,
            items: Vec::new(),
            source: ClipboardSource::App,
            revision: 0,
        }
    }
}

/// Something the planner noticed that is not a reason to refuse the job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum PlanNote {
    /// The same entry was named twice; it is done once.
    DuplicateSource { location: Location },
    /// The entry is already in the destination folder, so it is left out.
    AlreadyThere { location: Location },
}

/// What a request would do, found without writing anything: for the drag's default action (a move
/// on one volume, a copy across) and for the conflict dialog before the job is submitted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct PlanPreview {
    pub kind: JobKind,
    pub sources: SourcesSummary,
    /// Entries at every depth.
    #[ts(type = "number")]
    pub items: u64,
    #[ts(type = "number")]
    pub bytes: u64,
    /// Every source is known to be on the destination's volume.
    pub same_volume: bool,
    pub conflicts: Vec<Conflict>,
    pub notes: Vec<PlanNote>,
}

/// Why a command failed. Serialised as `{ kind, message }`, with `error` holding the engine's typed
/// error when `kind` is `ops`.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("internal error: {0}")]
    Internal(String),
    /// The queue refused a change (an unknown job, or a state that cannot do that).
    #[error(transparent)]
    Queue(#[from] QueueError),
    /// The engine refused or failed.
    #[error(transparent)]
    Ops(#[from] OpsError),
    /// A setting is out of range; nothing changed.
    #[error("{0}")]
    Invalid(String),
    /// Settings could not be saved; nothing changed.
    #[error("could not save the settings: {0}")]
    Storage(String),
}

impl Error {
    fn kind(&self) -> &'static str {
        match self {
            Error::Internal(_) => "internal",
            Error::Queue(_) => "queue",
            Error::Ops(_) => "ops",
            Error::Invalid(_) => "invalid",
            Error::Storage(_) => "storage",
        }
    }
}

impl Serialize for Error {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("kind", self.kind())?;
        map.serialize_entry("message", &self.to_string())?;
        if let Error::Ops(error) = self {
            map.serialize_entry("error", error)?;
        }
        map.end()
    }
}
