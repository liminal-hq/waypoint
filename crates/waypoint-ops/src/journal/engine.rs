// The journal itself: entries newest last, the write-ahead records of jobs in flight, and the
// rules that keep the history coherent (a cap, a redo chain that a new action clears).
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// A `Journal` is plain data with one owner, like `OpsStore`: every method takes `&mut self` and
// returns the events it made, so the plugin wraps it in a mutex. It does no work on files. A job
// goes through it in four calls:
//
//   begin   before the first write: stores a `PendingRecord` through the storage, synchronously
//   commit  after the job ends (done, failed or cancelled): replaces the record with an entry
//           holding what the job did, or drops it when it did nothing
//   abort   drops the record of a job that never wrote
//
// An `Undo` or `Redo` job is bracketed the same way, and ends with `finish_undo` or `finish_redo`
// instead of `commit`. Everything after `begin` asks for a save through `SaveRequest` and is
// written when the plugin calls `flush`.

use std::sync::Arc;

use waypoint_protocol::Location;

use super::model::{
    EntryState, ForwardSpec, InverseStep, JournalBody, JournalDocument, JournalEntry,
    JournalEntrySummary, JournalId, JournalSnapshot, PendingRecord, ScheduledRecord,
};
use super::storage::{JournalStorage, SaveRequest, StorageError};
use crate::model::{JobId, JobKind, JobOptions, JobRequest, OpsError, OpsEvent, Sources};
use crate::traits::{Clock, SettingsReader};

/// What the journal reaches the world through.
#[derive(Clone)]
pub struct JournalDeps {
    pub settings: Arc<dyn SettingsReader>,
    pub clock: Arc<dyn Clock>,
    pub storage: Arc<dyn JournalStorage>,
    pub saver: Arc<dyn SaveRequest>,
}

/// What a finished job leaves in the journal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recorded {
    pub kind: JobKind,
    pub label: String,
    pub forward: JobRequest,
    pub inverse: Vec<InverseStep>,
}

pub struct Journal {
    body: JournalBody,
    deps: JournalDeps,
    dirty: bool,
}

fn summary(entry: &JournalEntry) -> JournalEntrySummary {
    JournalEntrySummary {
        id: entry.id,
        label: entry.label.clone(),
        at_ms: entry.at_ms,
        undoable: entry.state == EntryState::Applied && !entry.inverse.is_empty(),
        redoable: entry.state == EntryState::Undone && !entry.partly_undone,
        partly_undone: entry.partly_undone,
    }
}

fn unavailable(reason: impl Into<String>) -> OpsError {
    OpsError::UndoUnavailable {
        reason: reason.into(),
    }
}

fn request_of(kind: JobKind, origin_window: &str) -> JobRequest {
    JobRequest {
        kind,
        sources: Sources::Locations {
            locations: Vec::<Location>::new(),
        },
        destination: None,
        name: None,
        options: JobOptions::default(),
        origin_window: origin_window.to_owned(),
        rename: None,
    }
}

impl Journal {
    /// An empty journal.
    pub fn new(deps: JournalDeps) -> Self {
        Self::from_body(deps, JournalBody::default())
    }

    pub fn from_body(deps: JournalDeps, mut body: JournalBody) -> Self {
        body.next_id = body
            .next_id
            .max(body.entries.iter().map(|e| e.id.0).max().unwrap_or(0) + 1)
            .max(1);
        Self {
            body,
            deps,
            dirty: false,
        }
    }

    /// How many entries are kept: the setting `undo_depth`.
    pub fn depth(&self) -> usize {
        self.deps.settings.ops_settings().undo_depth as usize
    }

    pub fn revision(&self) -> u64 {
        self.body.revision
    }

    /// Every entry, oldest first.
    pub fn entries(&self) -> &[JournalEntry] {
        &self.body.entries
    }

    pub fn entry(&self, id: JournalId) -> Option<&JournalEntry> {
        self.body.entries.iter().find(|e| e.id == id)
    }

    /// The jobs that have begun and not ended.
    pub fn pending(&self) -> &[PendingRecord] {
        &self.body.pending
    }

    pub fn document(&self) -> JournalDocument {
        JournalDocument::new(self.body.clone())
    }

    /// The history for a menu or the palette, newest first.
    pub fn summaries(&self) -> Vec<JournalEntrySummary> {
        self.body.entries.iter().rev().map(summary).collect()
    }

    /// The entry Undo would undo: the newest one still applied.
    pub fn last_undoable(&self) -> Option<JournalId> {
        self.body
            .entries
            .iter()
            .rev()
            .find(|e| summary(e).undoable)
            .map(|e| e.id)
    }

    /// The entry Redo would redo: the one undone most recently.
    pub fn last_redoable(&self) -> Option<JournalId> {
        self.body
            .entries
            .iter()
            .filter(|e| summary(e).redoable)
            .max_by_key(|e| e.undone_order)
            .map(|e| e.id)
    }

    pub fn snapshot(&self) -> JournalSnapshot {
        let find = |id: Option<JournalId>| id.and_then(|id| self.entry(id)).map(summary);
        JournalSnapshot {
            revision: self.body.revision,
            undo: find(self.last_undoable()),
            redo: find(self.last_redoable()),
        }
    }

    fn changed(&mut self) -> Vec<OpsEvent> {
        self.body.revision += 1;
        self.touched();
        let snapshot = self.snapshot();
        vec![OpsEvent::JournalChanged {
            revision: snapshot.revision,
            undo: snapshot.undo,
            redo: snapshot.redo,
        }]
    }

    /// Notes that the document differs from what was saved and asks for a save.
    fn touched(&mut self) {
        self.dirty = true;
        self.deps.saver.save_requested();
    }

    /// Whether there are changes not yet saved.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Saves now when there are unsaved changes.
    pub fn flush(&mut self) -> Result<(), StorageError> {
        if !self.dirty {
            return Ok(());
        }
        self.deps.storage.save(&self.document())?;
        self.dirty = false;
        Ok(())
    }

    /// Stores the write-ahead record of a job before its first write, synchronously, together with
    /// whatever else was waiting to be saved. An error means the job could not be made
    /// recoverable; the caller decides whether to run it anyway (the plugin logs and does).
    pub fn begin(&mut self, record: PendingRecord) -> Result<(), StorageError> {
        self.body.pending.retain(|p| p.job != record.job);
        self.body.pending.push(record);
        self.dirty = true;
        self.flush()
    }

    /// Replaces the jobs held by a schedule with `records`, asking for a save when that is a
    /// change.
    pub fn set_scheduled(&mut self, records: Vec<ScheduledRecord>) {
        if self.body.scheduled != records {
            self.body.scheduled = records;
            self.touched();
        }
    }

    /// The jobs the last run left held by a schedule, taken so the plugin can queue them again
    /// (which records them anew). The saved file keeps them until the next save, which the plugin
    /// makes after queueing them, so one that could not be queued does not come back for ever.
    pub fn take_scheduled(&mut self) -> Vec<ScheduledRecord> {
        let taken = std::mem::take(&mut self.body.scheduled);
        if !taken.is_empty() {
            self.touched();
        }
        taken
    }

    /// The jobs held by a schedule now.
    pub fn scheduled(&self) -> &[ScheduledRecord] {
        &self.body.scheduled
    }

    /// Drops the record of a job that wrote nothing.
    pub fn abort(&mut self, job: JobId) {
        if self.drop_pending(job) {
            self.touched();
        }
    }

    fn drop_pending(&mut self, job: JobId) -> bool {
        let before = self.body.pending.len();
        self.body.pending.retain(|p| p.job != job);
        self.body.pending.len() != before
    }

    pub(super) fn request_save(&mut self) {
        self.touched();
    }

    pub(super) fn trim_to_depth(&mut self) {
        self.trim();
    }

    fn trim(&mut self) {
        let depth = self.depth();
        let extra = self.body.entries.len().saturating_sub(depth);
        self.body.entries.drain(..extra);
    }

    /// Replaces the record of `job` with an entry, which is what the job did and how to reverse
    /// it. A job that did nothing leaves no entry. A new entry ends the redo chain: the entries
    /// that were undone are dropped. The oldest entries go when the history is over the cap.
    pub fn commit(&mut self, job: JobId, recorded: Recorded) -> (Option<JournalId>, Vec<OpsEvent>) {
        let had_pending = self.drop_pending(job);
        if recorded.inverse.is_empty() || self.depth() == 0 {
            if had_pending {
                self.touched();
            }
            return (None, Vec::new());
        }
        self.body.entries.retain(|e| e.state != EntryState::Undone);
        let id = JournalId(self.body.next_id.max(1));
        self.body.next_id = id.0 + 1;
        self.body.entries.push(JournalEntry {
            id,
            at_ms: self.deps.clock.now_ms(),
            label: recorded.label,
            kind: recorded.kind,
            job,
            inverse: recorded.inverse,
            forward: ForwardSpec {
                request: recorded.forward,
            },
            state: EntryState::Applied,
            undone_order: 0,
            partly_undone: false,
        });
        self.trim();
        (Some(id), self.changed())
    }

    fn applied(&self, id: JournalId) -> Result<&JournalEntry, OpsError> {
        let entry = self
            .entry(id)
            .ok_or_else(|| unavailable("that is no longer in the undo history"))?;
        if entry.state != EntryState::Applied || entry.inverse.is_empty() {
            return Err(unavailable("that has already been undone"));
        }
        Ok(entry)
    }

    fn undone(&self, id: JournalId) -> Result<&JournalEntry, OpsError> {
        let entry = self
            .entry(id)
            .ok_or_else(|| unavailable("that is no longer in the undo history"))?;
        if entry.state != EntryState::Undone {
            return Err(unavailable("that has not been undone"));
        }
        if entry.partly_undone {
            return Err(unavailable("that was only partly undone"));
        }
        Ok(entry)
    }

    /// The request that undoes `id`, to put on the queue like any other.
    pub fn undo_request(&self, id: JournalId, origin_window: &str) -> Result<JobRequest, OpsError> {
        self.applied(id)?;
        Ok(request_of(JobKind::Undo { of: id }, origin_window))
    }

    /// The request that undoes the newest applied entry.
    pub fn undo_last_request(&self, origin_window: &str) -> Result<JobRequest, OpsError> {
        let id = self
            .last_undoable()
            .ok_or_else(|| unavailable("there is nothing to undo"))?;
        self.undo_request(id, origin_window)
    }

    pub fn redo_request(&self, id: JournalId, origin_window: &str) -> Result<JobRequest, OpsError> {
        self.undone(id)?;
        Ok(request_of(JobKind::Redo { of: id }, origin_window))
    }

    pub fn redo_last_request(&self, origin_window: &str) -> Result<JobRequest, OpsError> {
        let id = self
            .last_redoable()
            .ok_or_else(|| unavailable("there is nothing to redo"))?;
        self.redo_request(id, origin_window)
    }

    /// What an undo of `id` would do, in the order it does it (the reverse of how the job went).
    pub fn undo_steps(&self, id: JournalId) -> Result<Vec<InverseStep>, OpsError> {
        let mut steps = self.applied(id)?.inverse.clone();
        steps.reverse();
        Ok(steps)
    }

    /// The request a redo of `id` runs through the planner.
    pub fn redo_forward(&self, id: JournalId) -> Result<JobRequest, OpsError> {
        Ok(self.undone(id)?.forward.request.clone())
    }

    /// Ends an undo job. `applied` steps were done (counted from the first of `undo_steps`): all of
    /// them marks the entry undone; some leaves it applied with only the steps still to do, marked
    /// partly undone, so that undoing again finishes the work and nothing is done twice.
    pub fn finish_undo(&mut self, job: JobId, id: JournalId, applied: usize) -> Vec<OpsEvent> {
        let had_pending = self.drop_pending(job);
        let counter = self.body.undo_counter + 1;
        let Some(entry) = self.body.entries.iter_mut().find(|e| e.id == id) else {
            if had_pending {
                self.touched();
            }
            return Vec::new();
        };
        if applied == 0 {
            if had_pending {
                self.touched();
            }
            return Vec::new();
        }
        let total = entry.inverse.len();
        if applied >= total {
            entry.inverse.clear();
            entry.state = EntryState::Undone;
            entry.partly_undone = false;
            entry.undone_order = counter;
            self.body.undo_counter = counter;
        } else {
            entry.inverse.truncate(total - applied);
            entry.partly_undone = true;
        }
        self.changed()
    }

    /// Ends a redo job that did the whole forward spec: the entry is applied again, with the
    /// inverse steps of this run (the names and receipts may differ from the first time).
    pub fn finish_redo(
        &mut self,
        job: JobId,
        id: JournalId,
        inverse: Vec<InverseStep>,
    ) -> Vec<OpsEvent> {
        let had_pending = self.drop_pending(job);
        let now = self.deps.clock.now_ms();
        let Some(entry) = self.body.entries.iter_mut().find(|e| e.id == id) else {
            if had_pending {
                self.touched();
            }
            return Vec::new();
        };
        entry.inverse = inverse;
        entry.state = EntryState::Applied;
        entry.partly_undone = false;
        entry.undone_order = 0;
        entry.at_ms = now;
        self.changed()
    }

    /// Ends a redo job that stopped part way. What it did becomes a new entry (a new action, so the
    /// redo chain, including this entry, is dropped), or nothing when it did nothing.
    pub fn finish_redo_partly(
        &mut self,
        job: JobId,
        recorded: Recorded,
    ) -> (Option<JournalId>, Vec<OpsEvent>) {
        self.commit(job, recorded)
    }
}
