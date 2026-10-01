// The copy and move executor (A48 to A51): the items of a plan, and the folders below them, placed
// one at a time through providers.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Every item reaches its final name in one atomic step. A file or a link is written under
// `.waypoint-partial-{job}-{n}-{name}` and renamed into place; a folder that is copied is built as a
// partial folder and renamed whole; whatever a name was already used for is renamed aside to
// `.waypoint-replaced-{job}-{n}-{name}` and removed only once the new entry is in, or put back if
// anything fails. A failure or a cancel removes its partials.
//
// A move renames when the source and the destination share a volume. Otherwise (or when the rename
// says `CrossesDevices`) it copies, verifies if asked, and removes the source, one item at a time:
// a folder is created at its destination, filled item by item with each source item removed after
// its copy, and its emptied source folder is removed last, only if it is empty. Stopping at any
// point leaves every item either moved or untouched.
//
// Errors are decided per item, at the file or link or folder that failed, so one unreadable file in
// a thousand does not end the job. Conflicts are decided per clash, by the answers the job holds
// (`Resolutions`) and by asking the sink for the rest.

use std::collections::HashSet;
use std::ffi::{OsStr, OsString};
use std::mem::{discriminant, Discriminant};
use std::sync::Arc;

use waypoint_path::{CaseRule, VfsPath};
use waypoint_protocol::{Location, VfsError};
use waypoint_vfs::{child_path, CancelToken, EntryKind, Provider, ScannedEntry};

use super::copy_engine::{copy_file_bytes, hash_file, FileCopy, CHUNK_BYTES};
use super::copy_resolve::{action_for, Action, Resolutions};
use super::{remove::remove_tree, ExecEnv, ExecFailure, ExecReport, ExecSink};
use crate::journal::InverseStep;
use crate::model::{
    Conflict, ConflictKind, ConflictPolicy, Counts, Decision, JobId, JobKind, JobOptions, OpsError,
    OpsSettings, Progress, Resolution, Verification, VerifyAlgorithm,
};
use crate::names::{file_name_of, is_within, unique_full_name};
use crate::plan::{volume_probe, Plan, PlanItem};
use crate::speed::SpeedEstimator;
use crate::traits::{Clock, SystemClock};
use crate::verify::{hex, Manifest};

/// How a run differs from the defaults: the answers already given, whether and how to verify, the
/// size of a chunk, and the clock the speed is measured by.
#[derive(Clone)]
pub struct RunOptions {
    pub resolutions: Resolutions,
    /// Verify every copied file with this algorithm; `None` does not verify.
    pub verify: Option<VerifyAlgorithm>,
    pub chunk_bytes: usize,
    pub clock: Arc<dyn Clock>,
}

impl Default for RunOptions {
    fn default() -> Self {
        Self {
            resolutions: Resolutions::default(),
            verify: None,
            chunk_bytes: CHUNK_BYTES,
            clock: Arc::new(SystemClock),
        }
    }
}

impl RunOptions {
    /// The options a job runs with: verification as the request says or else as the settings do,
    /// and the answers the job holds.
    pub fn for_job(options: &JobOptions, settings: &OpsSettings, resolutions: Resolutions) -> Self {
        let verify = options.verify.unwrap_or(settings.verify_after_copy);
        Self {
            resolutions,
            verify: verify.then_some(settings.verify_algorithm),
            ..Self::default()
        }
    }
}

/// What a copy or move did that the journal and the notices care about, beyond `ExecReport`'s
/// `created` (the maximal new entries of a copy) and `renamed` (the moves, as `(from, to)`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TransferReport {
    /// The existing entries that were replaced and are gone for good.
    pub replaced: Vec<Location>,
    /// The existing folders that entries were merged into.
    pub merged: Vec<Location>,
    /// What verification recorded, when it ran.
    pub verified: Option<Verification>,
    /// `.waypoint-replaced-*` entries that could not be removed after their replacement was in
    /// place (safe to delete), or that could not be put back after a replacement failed (the only
    /// copy of what the replaced entry held).
    pub leftovers: Vec<Location>,
    /// The top-level sources of the plan that something was placed for, in order: what a redo
    /// runs again.
    pub placed_sources: Vec<Location>,
    /// The policy that settled the clashes the job was not asked about one by one (the request's,
    /// or the answer given as "apply to all"), which a redo runs with so it meets the same clashes
    /// the same way.
    pub policy: Option<ConflictPolicy>,
    /// Why the job cannot be undone and so is not journalled, when it cannot (A52): a replaced
    /// entry is gone for good once the job commits.
    pub unjournalled: Option<String>,
}

/// How a unit came to be at its target, which decides how an undo reverses it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum How {
    /// A copy or a link: the source was left where it was.
    Created,
    /// A move by rename: the entry is the very one that was at the source.
    Renamed,
    /// A move across volumes: a copy was made and the source removed.
    Copied,
}

/// What a run did, in order, as far as an undo needs to know.
#[derive(Debug, Clone)]
enum Unit {
    /// An entry in place at `target`, with everything below it.
    Placed {
        src: VfsPath,
        target: VfsPath,
        how: How,
    },
    /// A folder the job made at its destination that still stands, holding entries placed below.
    MadeDir { target: VfsPath },
    /// A source folder a move emptied and removed after merging it into another.
    RemovedDir {
        src: VfsPath,
        modified_ms: Option<i64>,
        mode: Option<u32>,
    },
}

/// How an item or a step ended, short of an error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome {
    /// Placed, with everything below it.
    Done,
    /// Placed, but something below it was left behind (skipped or failed).
    Partial,
    /// Not placed.
    Skipped,
}

/// Why a step stopped.
enum Flow {
    /// The item failed: the sink decides what to do about it.
    Item(Box<OpsError>),
    Cancelled,
    /// Nobody answered an error or a conflict: the job fails here.
    Fatal(Box<OpsError>, Box<Location>),
}

impl Flow {
    fn item(error: OpsError) -> Self {
        Flow::Item(Box::new(error))
    }

    fn fatal(error: OpsError, at: Location) -> Self {
        Flow::Fatal(Box::new(error), Box::new(at))
    }
}

impl From<OpsError> for Flow {
    fn from(error: OpsError) -> Self {
        match error {
            OpsError::Cancelled => Flow::Cancelled,
            other => Flow::item(other),
        }
    }
}

impl From<VfsError> for Flow {
    fn from(error: VfsError) -> Self {
        OpsError::from(error).into()
    }
}

type R<T> = Result<T, Flow>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Copy,
    Move,
    /// A symbolic link to each source, which is left where it is.
    Link,
}

/// What a step inherits from the item above it.
#[derive(Debug, Clone, Copy)]
struct Ctx {
    mode: Mode,
    /// A rename can move entries: they are on the destination's volume.
    rename_ok: bool,
    /// The entry is below a folder that is being merged, so a folder clash here merges too: a
    /// choice to replace files never deletes the contents of a folder inside a merge.
    nested: bool,
}

/// A clash resolved into what to do about it.
enum Placement {
    Skip,
    /// Nothing is in the way at this path.
    Free(VfsPath),
    /// Replace what is at this path.
    Replace(VfsPath, Box<ScannedEntry>),
    /// Fold into the existing folder at this path.
    Merge(VfsPath),
}

/// A place the running job reports from. Held apart from the rest of the run so a callback can
/// borrow it next to the sink.
struct Meter {
    progress: Progress,
    counts: Counts,
    speed: SpeedEstimator,
    clock: Arc<dyn Clock>,
}

impl Meter {
    fn emit(&mut self, sink: &mut dyn ExecSink) {
        let now = self.clock.now_ms();
        self.progress.speed_bps = self.speed.update(now, self.progress.bytes_done);
        self.progress.eta_ms = self
            .speed
            .eta_ms(self.progress.bytes_done, self.progress.bytes_total);
        sink.progress(&self.progress, &self.counts);
    }
}

struct Transfer<'a> {
    env: &'a ExecEnv,
    job: JobId,
    cancel: &'a CancelToken,
    sink: &'a mut dyn ExecSink,
    destination: Option<VfsPath>,
    moving: bool,
    linking: bool,
    verify: Option<VerifyAlgorithm>,
    chunk: usize,
    resolutions: Resolutions,
    skip_kinds: Vec<Discriminant<OpsError>>,
    meter: Meter,
    report: ExecReport,
    manifest: Option<Manifest>,
    buf: Vec<u8>,
    /// On a case-insensitive destination: the names this job has put in place, folded, so a later
    /// entry that lands on one of them (`a` after `A`) is known to clash with the job's own work
    /// and never replaces it.
    placed: HashSet<String>,
    /// Every completed placement that is not inside a folder created by another one, in order,
    /// with the folders made and removed around them.
    units: Vec<Unit>,
    /// While a folder replaced by a move is built as a copy: every source entry copied into it, as
    /// the job met it and in the order copied (children before their folder), so the source can be
    /// removed item by item afterwards and anything that changed meanwhile stays.
    copied: Option<Vec<(VfsPath, ScannedEntry)>>,
}

/// Runs a copy or move plan.
pub(super) fn run(
    env: &ExecEnv,
    job: JobId,
    plan: &Plan,
    cancel: &CancelToken,
    sink: &mut dyn ExecSink,
    options: RunOptions,
) -> Result<ExecReport, Box<ExecFailure>> {
    let mut t = Transfer {
        env,
        job,
        cancel,
        sink,
        destination: plan.destination.clone(),
        moving: plan.kind == JobKind::Move,
        linking: plan.kind == JobKind::Link,
        verify: options.verify,
        chunk: options.chunk_bytes.max(1),
        resolutions: options.resolutions,
        skip_kinds: Vec::new(),
        meter: Meter {
            progress: Progress {
                items_total: plan.total_items,
                bytes_total: plan.total_bytes,
                ..Progress::default()
            },
            counts: Counts::default(),
            speed: SpeedEstimator::new(),
            clock: options.clock,
        },
        report: ExecReport::default(),
        manifest: options.verify.map(Manifest::new),
        buf: Vec::new(),
        units: Vec::new(),
        placed: HashSet::new(),
        copied: None,
    };
    let mut done = 0u64;
    for item in &plan.items {
        t.sink.between_items();
        let at = item_location(item);
        let before = t.units.len();
        let result = t.top(item);
        if t.units.len() > before {
            t.report.transfer.placed_sources.push(at.clone());
        }
        let outcome = match result {
            Ok(outcome) => outcome,
            Err(Flow::Cancelled) => return Err(t.fail(OpsError::Cancelled, Some(at), done)),
            Err(Flow::Fatal(error, location)) => return Err(t.fail(*error, Some(*location), done)),
            Err(Flow::Item(error)) => return Err(t.fail(*error, Some(at), done)),
        };
        if outcome == Outcome::Done {
            done += 1;
        }
        t.meter.emit(t.sink);
    }
    t.finish_report();
    Ok(t.report)
}

fn item_location(item: &PlanItem) -> Location {
    item.source
        .as_ref()
        .or(item.target.as_ref())
        .map(VfsPath::to_location)
        .unwrap_or_else(|| Location::new("", ""))
}

fn name_error(name: &OsStr) -> OpsError {
    OpsError::Io {
        message: format!("{name:?} is not a usable name"),
    }
}

impl Transfer<'_> {
    fn finish_report(&mut self) {
        let placed = self.units.iter().filter_map(|unit| match unit {
            Unit::Placed { src, target, .. } => Some((src, target)),
            _ => None,
        });
        if self.moving {
            self.report.renamed = placed
                .map(|(from, to)| (from.to_location(), to.to_location()))
                .collect();
        } else {
            self.report.created = placed.map(|(_, to)| to.to_location()).collect();
        }
        self.report.transfer.verified = self.manifest.as_ref().and_then(Manifest::snapshot);
        self.report.progress = self.meter.progress.clone();
        self.report.counts = self.meter.counts;
        self.report.transfer.policy = self.resolutions.all();
        self.report.inverse = self.inverse_steps();
        self.report.transfer.unjournalled =
            (!self.report.transfer.replaced.is_empty()).then(|| {
                "an entry that was replaced is gone for good, so this cannot be undone".to_owned()
            });
    }

    /// What reverses the run, in the order the originals were done. Empty when anything was
    /// replaced: the replaced entry is removed once the job commits, and nothing can put it back.
    fn inverse_steps(&self) -> Vec<InverseStep> {
        if !self.report.transfer.replaced.is_empty() {
            return Vec::new();
        }
        self.units
            .iter()
            .map(|unit| match unit {
                Unit::Placed { target, how, .. } if *how == How::Created => {
                    InverseStep::RemoveCreated {
                        location: target.to_location(),
                        fingerprint: None,
                    }
                }
                Unit::Placed { src, target, how } if *how == How::Renamed => {
                    InverseStep::MoveBack {
                        from: target.to_location(),
                        to: src.to_location(),
                    }
                }
                Unit::Placed { src, target, .. } => InverseStep::CopyBack {
                    from: target.to_location(),
                    to: src.to_location(),
                    fingerprint: None,
                },
                Unit::MadeDir { target } => InverseStep::RemoveEmptyDir {
                    location: target.to_location(),
                },
                Unit::RemovedDir {
                    src,
                    modified_ms,
                    mode,
                } => InverseStep::CreateDir {
                    location: src.to_location(),
                    modified_ms: *modified_ms,
                    mode: *mode,
                },
            })
            .collect()
    }

    fn placed(&mut self, src: &VfsPath, target: &VfsPath, how: How) {
        self.units.push(Unit::Placed {
            src: src.clone(),
            target: target.clone(),
            how,
        });
    }

    fn fail(&mut self, error: OpsError, item: Option<Location>, done: u64) -> Box<ExecFailure> {
        self.finish_report();
        Box::new(ExecFailure {
            error,
            item,
            done,
            report: std::mem::take(&mut self.report),
        })
    }

    fn check(&self) -> R<()> {
        if self.cancel.is_cancelled() {
            Err(Flow::Cancelled)
        } else {
            Ok(())
        }
    }

    fn provider(&self, path: &VfsPath) -> R<Arc<dyn Provider>> {
        Ok(self.env.providers.for_path(path)?)
    }

    fn rule(provider: &dyn Provider) -> CaseRule {
        provider.capabilities().case_rule
    }

    /// Runs one step of an item, and on an error asks the sink what to do (A48): `Retry` runs the
    /// step again, `Skip` records the item as failed and goes on (`Ok(None)`), `SkipAll` does that
    /// for this and every later error of the same kind, `Cancel` stops cleanly. No answer fails the
    /// job at the item.
    fn attempt<T>(
        &mut self,
        at: &Location,
        mut step: impl FnMut(&mut Self) -> R<T>,
    ) -> R<Option<T>> {
        loop {
            self.check()?;
            match step(self) {
                Ok(value) => return Ok(Some(value)),
                Err(Flow::Item(error)) => {
                    let error = *error;
                    let kind = discriminant(&error);
                    if self.skip_kinds.contains(&kind) {
                        self.fail_item(at, error);
                        return Ok(None);
                    }
                    match self.sink.on_error(at, &error) {
                        // Making a missing folder is a restore's: here it is a retry.
                        Some(Decision::Retry | Decision::CreateParents) => continue,
                        Some(Decision::Skip) => {
                            self.fail_item(at, error);
                            return Ok(None);
                        }
                        Some(Decision::SkipAll) => {
                            self.skip_kinds.push(kind);
                            self.fail_item(at, error);
                            return Ok(None);
                        }
                        Some(Decision::Cancel) => return Err(Flow::Cancelled),
                        None => return Err(Flow::fatal(error, at.clone())),
                    }
                }
                Err(other) => return Err(other),
            }
        }
    }

    /// An item that failed and was skipped over.
    fn fail_item(&mut self, at: &Location, error: OpsError) {
        self.meter.counts.failed += 1;
        self.report.skipped.push((at.clone(), error));
        self.meter.emit(self.sink);
    }

    /// An item left out because a conflict was settled with Skip.
    fn skip_item(&mut self, at: &Location, existing: &Location) {
        self.meter.counts.skipped += 1;
        self.report.skipped.push((
            at.clone(),
            OpsError::NameInUse {
                location: existing.clone(),
            },
        ));
        self.meter.emit(self.sink);
    }

    /// The bytes a file counts for in the plan: none when only a link to it is made.
    fn bytes_of(&self, entry: &ScannedEntry) -> u64 {
        if self.linking {
            0
        } else {
            entry.size.unwrap_or(0)
        }
    }

    /// Counts an entry that will not be placed as done, so progress still reaches the end.
    fn account(&mut self, entries: u64, bytes: u64) {
        self.meter.progress.items_done += entries;
        self.meter.progress.bytes_done += bytes;
    }

    fn entry_done(&mut self, name: Option<OsString>) {
        self.meter.progress.items_done += 1;
        self.meter.progress.current = name.map(|n| n.to_string_lossy().into_owned());
        self.meter.emit(self.sink);
    }

    /// How many entries and bytes are at and below `path`, as far as can be read.
    fn measure(&self, provider: &dyn Provider, path: &VfsPath, entry: &ScannedEntry) -> (u64, u64) {
        match entry.kind {
            EntryKind::File => (1, entry.size.unwrap_or(0)),
            EntryKind::Directory => {
                let (mut entries, mut bytes) = (1u64, 0u64);
                let mut stack = vec![path.clone()];
                while let Some(folder) = stack.pop() {
                    let Ok(children) = provider.list(&folder, self.cancel, 0, &mut |_| {}) else {
                        continue;
                    };
                    for child in children {
                        entries += 1;
                        match child.kind {
                            EntryKind::Directory => {
                                if let Ok(next) = folder.join(&child.name) {
                                    stack.push(next);
                                }
                            }
                            EntryKind::File => bytes += child.size.unwrap_or(0),
                            _ => {}
                        }
                    }
                }
                (entries, bytes)
            }
            _ => (1, 0),
        }
    }

    /// A name beside `target` that nothing else uses: the partial file or folder an entry is built
    /// under (`partial`), or where a replaced entry waits (`replaced`).
    fn sibling(&self, word: &str, target: &VfsPath) -> R<VfsPath> {
        let provider = self.provider(target)?;
        let folder = target.parent().ok_or_else(|| OpsError::Protected {
            location: target.to_location(),
        })?;
        let name = file_name_of(target).unwrap_or_default();
        let prefix = format!(".waypoint-{word}-{}-{}-", self.job.0, self.env.ids.next());
        let mut text = name.to_string_lossy().into_owned();
        while prefix.len() + text.len() > 255 && text.pop().is_some() {}
        let text = text.trim_end_matches(['.', ' ']);
        Ok(child_path(
            &folder,
            OsStr::new(&format!("{prefix}{text}")),
            Self::rule(provider.as_ref()),
        )
        .map_err(OpsError::from)?)
    }

    /// Removes a partial file or folder after a failure. It must finish whatever stopped the job,
    /// so it ignores the cancel, and what it cannot remove it leaves.
    fn discard(&self, provider: &dyn Provider, path: &VfsPath) {
        let _ = remove_tree(provider, path, &CancelToken::new(), &mut |_| {});
    }

    // ---- the items ----

    /// One top-level item of the plan.
    fn top(&mut self, item: &PlanItem) -> R<Outcome> {
        let src = item.source.as_ref().expect("a copy has a source");
        let want = item.target.as_ref().expect("a copy has a target");
        let at = src.to_location();
        let sp = self.provider(src)?;
        let dp = self.provider(want)?;
        let confirmed = self.attempt(&at, |_| {
            let entry = sp.stat(src)?;
            if entry.kind != item.kind {
                return Err(Flow::item(OpsError::ChangedSince {
                    location: src.to_location(),
                }));
            }
            Ok(entry)
        })?;
        let Some(entry) = confirmed else {
            self.account(item.entries, item.bytes);
            return Ok(Outcome::Skipped);
        };
        let ctx = Ctx {
            mode: if self.moving {
                Mode::Move
            } else if self.linking {
                Mode::Link
            } else {
                Mode::Copy
            },
            rename_ok: self.moving
                && self.same_volume(sp.as_ref(), &volume_probe(src, &entry), dp.as_ref()),
            nested: false,
        };
        self.place(ctx, src, want, &entry, Some((item.entries, item.bytes)))
    }

    /// Whether `src` is known to be on the destination folder's volume.
    fn same_volume(&self, sp: &dyn Provider, src: &VfsPath, dp: &dyn Provider) -> bool {
        let Some(dest) = &self.destination else {
            return false;
        };
        sp.scheme() == dp.scheme()
            && matches!((sp.volume_id(src), dp.volume_id(dest)), (Some(a), Some(b)) if a == b)
    }

    fn place(
        &mut self,
        ctx: Ctx,
        src: &VfsPath,
        want: &VfsPath,
        entry: &ScannedEntry,
        size: Option<(u64, u64)>,
    ) -> R<Outcome> {
        self.check()?;
        match entry.kind {
            EntryKind::File | EntryKind::Symlink => self.leaf(ctx, src, want, entry),
            // A link to a folder is made like a link to a file.
            EntryKind::Directory if ctx.mode == Mode::Link => self.leaf(ctx, src, want, entry),
            EntryKind::Directory => self.dir(ctx, src, want, entry, size),
            EntryKind::Other => {
                let at = src.to_location();
                self.meter.counts.skipped += 1;
                self.report.skipped.push((
                    at,
                    OpsError::Unsupported {
                        what: "copying a special file".to_owned(),
                    },
                ));
                self.account(1, 0);
                self.meter.emit(self.sink);
                Ok(Outcome::Skipped)
            }
        }
    }

    // ---- conflicts ----

    fn conflict_of(
        src: &VfsPath,
        entry: &ScannedEntry,
        target: &VfsPath,
        existing: &ScannedEntry,
        linking: bool,
        within_batch: bool,
    ) -> Conflict {
        // A link to a folder is a link, not a folder, so only a like clash replaces it.
        let src_dir = entry.kind == EntryKind::Directory && !linking;
        let dst_dir = existing.kind == EntryKind::Directory;
        Conflict {
            source: src.to_location(),
            existing: target.to_location(),
            name: existing.name.to_string_lossy().into_owned(),
            kind: match (src_dir, dst_dir) {
                (false, false) => ConflictKind::FileOverFile,
                (true, true) => ConflictKind::FolderOverFolder,
                (false, true) => ConflictKind::FileOverFolder,
                (true, false) => ConflictKind::FolderOverFile,
            },
            within_batch,
            source_size: entry.size,
            existing_size: existing.size,
            source_modified_ms: entry.modified_ms,
            existing_modified_ms: existing.modified_ms,
        }
    }

    /// Asks the sink about a clash and keeps the answer. No answer fails the job: nothing is
    /// overwritten, merged or renamed without a decision.
    fn ask(&mut self, conflict: &Conflict) -> R<ConflictPolicy> {
        match self.sink.on_conflict(conflict) {
            Some(Resolution { source, policy }) => {
                match source {
                    None => self.resolutions.set_all(policy),
                    Some(_) => self.resolutions.set_for(&conflict.source, policy),
                }
                Ok(policy)
            }
            None => Err(Flow::fatal(
                OpsError::NameInUse {
                    location: conflict.existing.clone(),
                },
                conflict.source.clone(),
            )),
        }
    }

    /// Finds out what is at `want` and what to do about it.
    fn resolve_target(
        &mut self,
        ctx: Ctx,
        src: &VfsPath,
        want: &VfsPath,
        entry: &ScannedEntry,
    ) -> R<Placement> {
        let provider = self.provider(want)?;
        let rule = Self::rule(provider.as_ref());
        let mut target = want.clone();
        // The answer for this entry, once it has one: a free name found by Keep both is not asked
        // about again, and a retried clash does not ask twice.
        let mut chosen: Option<ConflictPolicy> = None;
        let mut asked = 0;
        loop {
            self.check()?;
            let existing = match provider.stat(&target) {
                Ok(existing) => existing,
                Err(VfsError::NotFound { .. }) => return Ok(Placement::Free(target)),
                Err(error) => return Err(error.into()),
            };
            let within_batch = self.placed_by_this_job(&target, rule);
            let conflict = Self::conflict_of(
                src,
                entry,
                &target,
                &existing,
                ctx.mode == Mode::Link,
                within_batch,
            );
            let mut policy = match chosen.or_else(|| self.resolutions.policy_for(&conflict.source))
            {
                Some(policy) => policy,
                None => self.ask(&conflict)?,
            };
            let newer = match (entry.modified_ms, existing.modified_ms) {
                (Some(a), Some(b)) => Some(a > b),
                _ => None,
            };
            let action = loop {
                match action_for(policy, conflict.kind, newer, &conflict.existing)? {
                    Some(action) => break action,
                    None => {
                        // The choice cannot settle this clash (Merge for a file): ask again, a
                        // few times at most.
                        asked += 1;
                        if asked > 3 {
                            return Err(Flow::fatal(
                                OpsError::NameInUse {
                                    location: conflict.existing.clone(),
                                },
                                conflict.source.clone(),
                            ));
                        }
                        policy = self.ask(&conflict)?;
                    }
                }
            };
            chosen = Some(policy);
            let action = match action {
                // A folder the job has just put in place is never replaced by another entry of
                // the same job: a source holding `A` and `a` meeting a case-insensitive
                // destination keeps both, folders merge.
                Action::Replace if within_batch => {
                    if conflict.kind == ConflictKind::FolderOverFolder {
                        Action::Merge
                    } else {
                        Action::KeepBoth
                    }
                }
                // A policy for all, chosen for the file clashes, never replaces a whole folder:
                // only an answer given for this folder does. The rest merge into it.
                Action::Replace
                    if conflict.kind == ConflictKind::FolderOverFolder
                        && !self.resolutions.has_own(&conflict.source) =>
                {
                    Action::Merge
                }
                other => other,
            };
            match action {
                Action::Skip => {
                    self.skip_item(&conflict.source, &conflict.existing);
                    return Ok(Placement::Skip);
                }
                Action::KeepBoth => {
                    target = self.free_name(provider.as_ref(), &target, rule)?;
                }
                Action::Merge => return Ok(Placement::Merge(target)),
                Action::Replace
                    if ctx.nested
                        && existing.kind == EntryKind::Directory
                        && entry.kind == EntryKind::Directory =>
                {
                    return Ok(Placement::Merge(target));
                }
                Action::Replace => {
                    if existing.kind == EntryKind::Directory
                        && is_within(src, &target, rule)
                        && src.scheme() == target.scheme()
                    {
                        // The source is inside the folder it would replace.
                        return Err(Flow::item(OpsError::CannotReplace {
                            location: conflict.existing,
                        }));
                    }
                    return Ok(Placement::Replace(target, Box::new(existing)));
                }
            }
        }
    }

    /// Whether `target` is a name this job put in place on a case-insensitive destination. Always
    /// false on a case-sensitive one, where a name is only ever the same name exactly.
    fn placed_by_this_job(&self, target: &VfsPath, rule: CaseRule) -> bool {
        rule == CaseRule::Insensitive && self.placed.contains(&Self::placed_key(target))
    }

    fn placed_key(target: &VfsPath) -> String {
        waypoint_path::windows::fold(&target.display())
    }

    /// Notes that the job put `target` in place, if the destination cannot tell it from another
    /// name that differs by case.
    fn note_placed(&mut self, target: &VfsPath) {
        if let Ok(dp) = self.provider(target) {
            if Self::rule(dp.as_ref()) == CaseRule::Insensitive {
                self.placed.insert(Self::placed_key(target));
            }
        }
    }

    /// A name beside `target` that is free: `name (2).ext`, continuing a count already there.
    fn free_name(&self, provider: &dyn Provider, target: &VfsPath, rule: CaseRule) -> R<VfsPath> {
        let folder = target.parent().ok_or_else(|| OpsError::Protected {
            location: target.to_location(),
        })?;
        let name = file_name_of(target).unwrap_or_default();
        let mut failed: Option<OpsError> = None;
        let unique = unique_full_name(
            &mut |n| match child_path(&folder, OsStr::new(n), rule) {
                Err(error) => {
                    failed.get_or_insert(error.into());
                    true
                }
                Ok(path) => match provider.stat(&path) {
                    Ok(_) => true,
                    Err(VfsError::NotFound { .. }) => false,
                    Err(error) => {
                        failed.get_or_insert(error.into());
                        true
                    }
                },
            },
            &name.to_string_lossy(),
        );
        if let Some(error) = failed {
            return Err(error.into());
        }
        Ok(child_path(&folder, OsStr::new(&unique), rule).map_err(OpsError::from)?)
    }

    // ---- files and links ----

    fn leaf(
        &mut self,
        ctx: Ctx,
        src: &VfsPath,
        want: &VfsPath,
        entry: &ScannedEntry,
    ) -> R<Outcome> {
        let at = src.to_location();
        let attempted = self.attempt(&at, |t| {
            // A name that is free when it is looked at can be taken before the entry is in place;
            // that is a clash like any other, decided again.
            let mut round = 0;
            loop {
                round += 1;
                let placement = t.resolve_target(ctx, src, want, entry)?;
                let (target, existing) = match placement {
                    Placement::Skip => return Ok(Outcome::Skipped),
                    Placement::Free(target) => (target, None),
                    Placement::Replace(target, existing) => (target, Some(*existing)),
                    Placement::Merge(_) => {
                        return Err(Flow::item(OpsError::Unsupported {
                            what: "merging a file".to_owned(),
                        }))
                    }
                };
                match t.place_leaf(ctx, src, &target, entry, existing.as_ref()) {
                    Err(Flow::Item(e)) if matches!(*e, OpsError::NameInUse { .. }) && round < 4 => {
                        continue
                    }
                    other => return other.map(|()| Outcome::Done),
                }
            }
        })?;
        match attempted {
            Some(outcome) => {
                if outcome == Outcome::Skipped {
                    self.account(1, self.bytes_of(entry));
                    self.meter.emit(self.sink);
                }
                Ok(outcome)
            }
            None => {
                self.account(1, self.bytes_of(entry));
                Ok(Outcome::Skipped)
            }
        }
    }

    /// Renames what is at `target` to a waiting name. Returns where it went.
    fn set_aside(&self, provider: &dyn Provider, target: &VfsPath) -> R<VfsPath> {
        let aside = self.sibling("replaced", target)?;
        provider.rename(target, &aside, false)?;
        Ok(aside)
    }

    /// Puts an entry that was set aside back at `target`, which is free. It is the undo of a
    /// failed step, so it tries twice; if the entry still cannot go back it stays under its waiting
    /// name, which is reported in `TransferReport::leftovers` since it holds the only copy.
    fn put_back(&mut self, provider: &dyn Provider, aside: &VfsPath, target: &VfsPath) -> bool {
        let back = (0..2).any(|_| provider.rename(aside, target, false).is_ok());
        if !back {
            self.report.transfer.leftovers.push(aside.to_location());
        }
        back
    }

    /// Takes away a copy that was put in place and brings back what it replaced, in one step where
    /// there was something to bring back: renaming the old entry over the copy cannot leave the
    /// name empty or holding both. Without a replaced entry the copy is removed.
    fn undo_placement(
        &mut self,
        provider: &dyn Provider,
        target: &VfsPath,
        aside: Option<&VfsPath>,
    ) {
        match aside {
            Some(aside) => {
                let back = (0..2).any(|_| provider.rename(aside, target, true).is_ok());
                if !back {
                    self.report.transfer.leftovers.push(aside.to_location());
                }
            }
            None => {
                let _ = (0..2).any(|_| provider.remove_file(target).is_ok());
            }
        }
    }

    /// Removes an entry that was set aside once its replacement is in. What cannot be removed is
    /// reported, not failed: the job's work is done.
    fn drop_aside(&mut self, provider: &dyn Provider, aside: &VfsPath) {
        // Twice, since what stops a removal is often over by the second try.
        let gone =
            (0..2).any(|_| remove_tree(provider, aside, &CancelToken::new(), &mut |_| {}).is_ok());
        if !gone {
            self.report.transfer.leftovers.push(aside.to_location());
        }
    }

    /// Places one file or link at `target` (free, or holding `existing` that is replaced), by
    /// rename for a move on one volume and by copy otherwise.
    fn place_leaf(
        &mut self,
        ctx: Ctx,
        src: &VfsPath,
        target: &VfsPath,
        entry: &ScannedEntry,
        existing: Option<&ScannedEntry>,
    ) -> R<()> {
        let sp = self.provider(src)?;
        let dp = self.provider(target)?;
        let same_provider = Arc::ptr_eq(&sp, &dp);
        let size = entry.size.unwrap_or(0);
        let mut aside: Option<VfsPath> = None;

        if ctx.mode == Mode::Move && ctx.rename_ok && same_provider {
            if existing.is_some() {
                aside = Some(self.set_aside(dp.as_ref(), target)?);
            }
            match sp.rename(src, target, false) {
                Ok(()) => {
                    if let Some(aside) = &aside {
                        self.drop_aside(dp.as_ref(), aside);
                    }
                    self.leaf_placed(src, target, existing, size, How::Renamed);
                    return Ok(());
                }
                Err(error) => {
                    if let Some(moved) = &aside {
                        if !self.put_back(dp.as_ref(), moved, target) {
                            // The old entry is stranded under its waiting name: stop here rather
                            // than copy over a name that is empty.
                            return Err(Flow::item(OpsError::Io {
                                message: format!(
                                    "{} could not be put back after a failed replace",
                                    target.display()
                                ),
                            }));
                        }
                    }
                    if !matches!(error, VfsError::CrossesDevices { .. }) {
                        return Err(error.into());
                    }
                    aside = None;
                }
            }
        }

        let partial = self.sibling("partial", target)?;
        let base = self.meter.progress.bytes_done;
        if let Err(error) = self.write_partial(
            sp.as_ref(),
            src,
            dp.as_ref(),
            &partial,
            target,
            entry,
            same_provider,
        ) {
            self.discard(dp.as_ref(), &partial);
            self.meter.progress.bytes_done = base;
            return Err(error);
        }
        if existing.is_some() {
            match self.set_aside(dp.as_ref(), target) {
                Ok(moved) => aside = Some(moved),
                Err(error) => {
                    self.discard(dp.as_ref(), &partial);
                    self.meter.progress.bytes_done = base;
                    return Err(error);
                }
            }
        }
        if let Err(error) = dp.rename(&partial, target, false) {
            if let Some(moved) = &aside {
                self.put_back(dp.as_ref(), moved, target);
            }
            self.discard(dp.as_ref(), &partial);
            self.meter.progress.bytes_done = base;
            return Err(error.into());
        }
        if ctx.mode == Mode::Move {
            if let Err(error) = Self::remove_moved_source(sp.as_ref(), src, entry) {
                // The copy is in place but the source would not go (or is not what was copied):
                // take the copy away again and put back what it replaced, so the item is untouched.
                self.undo_placement(dp.as_ref(), target, aside.as_ref());
                self.meter.progress.bytes_done = base;
                return Err(error);
            }
        }
        if let Some(aside) = &aside {
            self.drop_aside(dp.as_ref(), aside);
        }
        if let Some(copied) = self.copied.as_mut() {
            copied.push((src.clone(), entry.clone()));
        }
        let how = if ctx.mode == Mode::Move {
            How::Copied
        } else {
            How::Created
        };
        self.leaf_placed(src, target, existing, 0, how);
        Ok(())
    }

    /// Removes the source of a file or link that has been copied, provided it is still what the
    /// job met: a file that was changed while it was copied (a different size or time) is not
    /// removed, since its newer content is in no copy. A source that is already gone stays gone.
    fn remove_moved_source(sp: &dyn Provider, src: &VfsPath, met: &ScannedEntry) -> R<()> {
        match sp.stat(src) {
            Ok(now) => {
                if met.kind == EntryKind::File
                    && (now.kind != met.kind
                        || now.size != met.size
                        || now.modified_ms != met.modified_ms)
                {
                    return Err(Flow::item(OpsError::ChangedSince {
                        location: src.to_location(),
                    }));
                }
            }
            Err(VfsError::NotFound { .. }) => return Ok(()),
            Err(error) => return Err(error.into()),
        }
        match sp.remove_file(src) {
            Ok(()) | Err(VfsError::NotFound { .. }) => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    /// Bookkeeping for a file or link that is in place. `renamed_bytes` is how much a rename moved
    /// without the copy having counted it.
    fn leaf_placed(
        &mut self,
        src: &VfsPath,
        target: &VfsPath,
        existing: Option<&ScannedEntry>,
        renamed_bytes: u64,
        how: How,
    ) {
        self.meter.progress.bytes_done += renamed_bytes;
        self.note_placed(target);
        if existing.is_some() {
            self.report.transfer.replaced.push(target.to_location());
        }
        self.placed(src, target, how);
        self.entry_done(file_name_of(target));
    }

    /// Writes a file or a link under the partial name, verifies it if asked, and gives it the
    /// source's times and permissions. `target` is where it is headed, for messages.
    #[allow(clippy::too_many_arguments)]
    fn write_partial(
        &mut self,
        sp: &dyn Provider,
        src: &VfsPath,
        dp: &dyn Provider,
        partial: &VfsPath,
        target: &VfsPath,
        entry: &ScannedEntry,
        same_provider: bool,
    ) -> R<()> {
        self.check()?;
        if self.linking {
            // The link holds where the source is, as an absolute path.
            let VfsPath::File(file) = src else {
                return Err(VfsError::Unsupported {
                    what: "a link to something that is not a local file".to_owned(),
                }
                .into());
            };
            dp.symlink(partial, file.as_path().as_os_str())?;
            return Ok(());
        }
        if entry.kind == EntryKind::Symlink {
            let text = sp.read_link(src)?;
            dp.symlink(partial, &text)?;
            return Ok(());
        }
        let base = self.meter.progress.bytes_done;
        let request = FileCopy {
            src_provider: sp,
            src,
            dst_provider: dp,
            dst: partial,
            same_provider,
            durable: self.moving,
            verify: self.verify,
            chunk: self.chunk,
            size_hint: entry.size.unwrap_or(0),
        };
        let copied = {
            let Transfer {
                meter,
                sink,
                buf,
                cancel,
                ..
            } = self;
            copy_file_bytes(
                &request,
                buf,
                &mut |done| {
                    meter.progress.bytes_done = base + done;
                    meter.emit(*sink);
                },
                cancel,
            )?
        };
        if let (Some(algorithm), Some(expected)) = (self.verify, copied.digest.as_deref()) {
            self.verify_copy(
                sp,
                src,
                dp,
                partial,
                target,
                algorithm,
                expected,
                copied.bytes,
            )?;
        }
        if self.moving {
            self.check_complete(src, dp, partial, entry, copied.bytes)?;
        }
        self.meter.progress.bytes_done = base + copied.bytes;
        copy_metadata(sp, src, dp, partial, None)?;
        Ok(())
    }

    /// Checks that the copy of a file that is about to be moved holds every byte the job met: what
    /// was read is the size the source had (else the source changed under the copy and keeps its
    /// newer content), and what the destination now holds is that size (else bytes were lost).
    /// Without this a short copy would be renamed into place and its only other copy removed.
    fn check_complete(
        &self,
        src: &VfsPath,
        dp: &dyn Provider,
        partial: &VfsPath,
        entry: &ScannedEntry,
        read: u64,
    ) -> R<()> {
        if entry.size.is_some_and(|size| size != read) {
            return Err(Flow::item(OpsError::ChangedSince {
                location: src.to_location(),
            }));
        }
        let held = dp.stat(partial)?.size;
        if held != Some(read) {
            return Err(Flow::item(OpsError::Io {
                message: format!(
                    "{} holds {} of the {read} bytes that were copied to it",
                    partial.display(),
                    held.map_or_else(|| "an unknown number".to_owned(), |n| n.to_string()),
                ),
            }));
        }
        Ok(())
    }

    /// Reads the written file back, and the source again, and compares both with what was read
    /// while copying (A51).
    #[allow(clippy::too_many_arguments)]
    fn verify_copy(
        &mut self,
        sp: &dyn Provider,
        src: &VfsPath,
        dp: &dyn Provider,
        partial: &VfsPath,
        target: &VfsPath,
        algorithm: VerifyAlgorithm,
        expected: &[u8],
        size: u64,
    ) -> R<()> {
        let written = hash_file(
            dp,
            partial,
            algorithm,
            self.chunk,
            size,
            &mut self.buf,
            self.cancel,
        )?;
        if written != expected {
            return Err(Flow::item(OpsError::VerifyFailed {
                location: target.to_location(),
                expected: hex(expected),
                actual: hex(&written),
            }));
        }
        let again = hash_file(
            sp,
            src,
            algorithm,
            self.chunk,
            size,
            &mut self.buf,
            self.cancel,
        )?;
        if again != expected {
            return Err(Flow::item(OpsError::VerifyFailed {
                location: src.to_location(),
                expected: hex(expected),
                actual: hex(&again),
            }));
        }
        if let Some(manifest) = self.manifest.as_mut() {
            manifest.add(expected);
        }
        Ok(())
    }

    // ---- folders ----

    fn dir(
        &mut self,
        ctx: Ctx,
        src: &VfsPath,
        want: &VfsPath,
        entry: &ScannedEntry,
        size: Option<(u64, u64)>,
    ) -> R<Outcome> {
        let at = src.to_location();
        let sp = self.provider(src)?;
        let dp = self.provider(want)?;
        let placed = self.attempt(&at, |t| t.resolve_target(ctx, src, want, entry))?;
        let Some(placement) = placed else {
            let (entries, bytes) = size.unwrap_or_else(|| self.measure(sp.as_ref(), src, entry));
            self.account(entries, bytes);
            return Ok(Outcome::Skipped);
        };
        match placement {
            Placement::Skip => {
                let (entries, bytes) =
                    size.unwrap_or_else(|| self.measure(sp.as_ref(), src, entry));
                self.account(entries, bytes);
                self.meter.emit(self.sink);
                Ok(Outcome::Skipped)
            }
            Placement::Free(target) => match ctx.mode {
                // (A folder is never placed in link mode; it is made like a file.)
                Mode::Copy | Mode::Link => {
                    self.copy_dir_new(sp.as_ref(), src, dp.as_ref(), &target, entry, size)
                }
                Mode::Move => {
                    self.move_dir_new(ctx, sp.as_ref(), src, dp.as_ref(), &target, entry, size)
                }
            },
            Placement::Merge(target) => {
                self.merge_dir(ctx, sp.as_ref(), src, dp.as_ref(), &target, entry)
            }
            Placement::Replace(target, _) => {
                self.replace_dir(ctx, sp.as_ref(), src, dp.as_ref(), &target, entry, size)
            }
        }
    }

    /// The entries of a folder, by name, so a run is the same every time.
    fn children(&mut self, provider: &dyn Provider, folder: &VfsPath) -> R<Vec<ScannedEntry>> {
        let mut children = provider.list(folder, self.cancel, 0, &mut |_| {})?;
        children.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(children)
    }

    /// Places each child of `src` below `target`. Returns whether every one was placed whole.
    fn place_children(
        &mut self,
        ctx: Ctx,
        sp: &dyn Provider,
        src: &VfsPath,
        dp: &dyn Provider,
        target: &VfsPath,
    ) -> R<bool> {
        let at = src.to_location();
        let Some(children) = self.attempt(&at, |t| t.children(sp, src))? else {
            return Ok(false);
        };
        let rule = Self::rule(dp);
        let mut whole = true;
        for child in children {
            self.check()?;
            let from = src.join(&child.name).map_err(|_| name_error(&child.name))?;
            let to = match child_path(target, &child.name, rule) {
                Ok(to) => to,
                Err(error) => {
                    // A name the destination cannot hold (a reserved name on a case-insensitive
                    // provider) fails that entry, which the sink may skip.
                    let at = from.to_location();
                    let outcome = self.attempt::<()>(&at, |_| Err(Flow::from(error.clone())))?;
                    debug_assert!(outcome.is_none());
                    let (entries, bytes) = self.measure(sp, &from, &child);
                    self.account(entries, bytes);
                    whole = false;
                    continue;
                }
            };
            let outcome = self.place(ctx, &from, &to, &child, None)?;
            whole &= outcome == Outcome::Done;
        }
        Ok(whole)
    }

    /// Builds a copy of the folder `src` as a partial folder beside `target`, with its children,
    /// times and permissions, ready to be renamed into place. `None` when an error decision skipped
    /// the folder.
    fn build_partial_dir(
        &mut self,
        sp: &dyn Provider,
        src: &VfsPath,
        dp: &dyn Provider,
        target: &VfsPath,
        entry: &ScannedEntry,
        size: Option<(u64, u64)>,
    ) -> R<Option<(VfsPath, bool)>> {
        let at = src.to_location();
        let ctx = Ctx {
            mode: Mode::Copy,
            rename_ok: false,
            nested: false,
        };
        let created = self.attempt(&at, |t| {
            let partial = t.sibling("partial", target)?;
            dp.create_dir(&partial)?;
            Ok(partial)
        })?;
        let Some(partial) = created else {
            // Nothing below the folder is going to be copied: count it as passed over.
            let (entries, bytes) = size.unwrap_or_else(|| self.measure(sp, src, entry));
            self.account(entries, bytes);
            return Ok(None);
        };
        let mark = self.units.len();
        let built = (|| -> R<Option<bool>> {
            let whole = self.place_children(ctx, sp, src, dp, &partial)?;
            let finished = self.attempt(&at, |_| {
                copy_metadata(sp, src, dp, &partial, Some(entry.modified_ms))?;
                Ok(())
            })?;
            Ok(finished.map(|()| whole))
        })();
        match built {
            Ok(Some(whole)) => {
                // The children are inside the new folder: it is the one thing created.
                self.units.truncate(mark);
                Ok(Some((partial, whole)))
            }
            Ok(None) => {
                self.units.truncate(mark);
                self.discard(dp, &partial);
                Ok(None)
            }
            Err(flow) => {
                self.units.truncate(mark);
                self.discard(dp, &partial);
                Err(flow)
            }
        }
    }

    /// Copies a folder to a name nothing holds.
    fn copy_dir_new(
        &mut self,
        sp: &dyn Provider,
        src: &VfsPath,
        dp: &dyn Provider,
        target: &VfsPath,
        entry: &ScannedEntry,
        size: Option<(u64, u64)>,
    ) -> R<Outcome> {
        let at = src.to_location();
        let Some((partial, whole)) = self.build_partial_dir(sp, src, dp, target, entry, size)?
        else {
            return Ok(Outcome::Skipped);
        };
        let renamed = self.attempt(&at, |_| {
            dp.rename(&partial, target, false)?;
            Ok(())
        });
        match renamed {
            Ok(Some(())) => {
                if let Some(copied) = self.copied.as_mut() {
                    copied.push((src.clone(), entry.clone()));
                }
                self.note_placed(target);
                self.placed(src, target, How::Created);
                self.entry_done(file_name_of(target));
                Ok(if whole {
                    Outcome::Done
                } else {
                    Outcome::Partial
                })
            }
            Ok(None) => {
                self.discard(dp, &partial);
                Ok(Outcome::Skipped)
            }
            Err(flow) => {
                self.discard(dp, &partial);
                Err(flow)
            }
        }
    }

    /// Moves a folder to a name nothing holds: a rename on one volume, otherwise the folder is
    /// created at its destination and filled item by item, each source item removed after its copy.
    #[allow(clippy::too_many_arguments)]
    fn move_dir_new(
        &mut self,
        ctx: Ctx,
        sp: &dyn Provider,
        src: &VfsPath,
        dp: &dyn Provider,
        target: &VfsPath,
        entry: &ScannedEntry,
        size: Option<(u64, u64)>,
    ) -> R<Outcome> {
        let at = src.to_location();
        if ctx.rename_ok {
            let (entries, bytes) = size.unwrap_or_else(|| self.measure(sp, src, entry));
            let renamed = self.attempt(&at, |_| match sp.rename(src, target, false) {
                Ok(()) => Ok(true),
                Err(VfsError::CrossesDevices { .. }) => Ok(false),
                Err(error) => Err(error.into()),
            })?;
            match renamed {
                Some(true) => {
                    self.account(entries, bytes);
                    self.note_placed(target);
                    self.placed(src, target, How::Renamed);
                    self.meter.emit(self.sink);
                    return Ok(Outcome::Done);
                }
                Some(false) => {}
                None => {
                    self.account(entries, bytes);
                    return Ok(Outcome::Skipped);
                }
            }
        }
        let created = self.attempt(&at, |_| {
            dp.create_dir(target)?;
            Ok(())
        })?;
        if created.is_none() {
            let (entries, bytes) = size.unwrap_or_else(|| self.measure(sp, src, entry));
            self.account(entries, bytes);
            return Ok(Outcome::Skipped);
        }
        self.note_placed(target);
        let mark = self.units.len();
        let whole = match self.place_children(ctx, sp, src, dp, target) {
            Ok(whole) => whole,
            Err(flow) => {
                // Stopped part way: what was moved stays moved. A folder that holds nothing is
                // not left behind.
                self.drop_or_note_dir(dp, target, mark);
                return Err(flow);
            }
        };
        if !whole {
            self.drop_or_note_dir(dp, target, mark);
            self.entry_done(file_name_of(target));
            return Ok(Outcome::Partial);
        }
        let finished = match self.attempt(&at, |_| {
            copy_metadata(sp, src, dp, target, Some(entry.modified_ms))?;
            sp.remove_dir(src)?;
            Ok(())
        }) {
            Ok(finished) => finished,
            Err(flow) => {
                // The job stops here with the folder standing and what was moved into it.
                self.units.insert(
                    mark,
                    Unit::MadeDir {
                        target: target.clone(),
                    },
                );
                return Err(flow);
            }
        };
        self.entry_done(file_name_of(target));
        match finished {
            Some(()) => {
                // Everything went: the folder as a whole is what moved.
                self.units.truncate(mark);
                self.placed(src, target, How::Copied);
                Ok(Outcome::Done)
            }
            None => {
                // The folder stands with what was moved into it.
                self.units.insert(
                    mark,
                    Unit::MadeDir {
                        target: target.clone(),
                    },
                );
                Ok(Outcome::Partial)
            }
        }
    }

    /// Removes the folder a move made at its destination if nothing was moved into it; otherwise
    /// it stays and the run notes it, so an undo takes it away once what is in it is back.
    fn drop_or_note_dir(&mut self, dp: &dyn Provider, target: &VfsPath, mark: usize) {
        if dp.remove_dir(target).is_err() {
            self.units.insert(
                mark,
                Unit::MadeDir {
                    target: target.clone(),
                },
            );
        }
    }

    /// Removes what a copy of the folder `src` took from it, once the copy is in place: each file
    /// and link only if it is still what the job met (`remove_moved_source`), each folder only if it
    /// is empty. Whatever changed, or is not in the copy, stays and is reported. Returns whether
    /// the source folder is gone.
    fn remove_copied_source(
        &mut self,
        sp: &dyn Provider,
        src: &VfsPath,
        copied: &[(VfsPath, ScannedEntry)],
    ) -> R<bool> {
        let mut left = false;
        for (path, met) in copied {
            self.check()?;
            let result = if met.kind == EntryKind::Directory {
                match sp.remove_dir(path) {
                    Ok(()) | Err(VfsError::NotFound { .. }) => Ok(()),
                    Err(VfsError::NotEmpty { .. }) => Err(Flow::item(OpsError::ChangedSince {
                        location: path.to_location(),
                    })),
                    Err(error) => Err(error.into()),
                }
            } else {
                Self::remove_moved_source(sp, path, met)
            };
            match result {
                Ok(()) => {}
                Err(Flow::Item(error)) if matches!(*error, OpsError::ChangedSince { .. }) => {
                    left = true;
                    self.fail_item(&path.to_location(), *error);
                }
                Err(other) => return Err(other),
            }
        }
        // The folder itself, last; it holds what was left or arrived meanwhile unless it is empty.
        match sp.remove_dir(src) {
            Ok(()) | Err(VfsError::NotFound { .. }) => Ok(true),
            Err(VfsError::NotEmpty { .. }) => {
                if !left {
                    // Something new arrived: nothing in the copy, so it stays.
                    self.fail_item(
                        &src.to_location(),
                        OpsError::ChangedSince {
                            location: src.to_location(),
                        },
                    );
                }
                Ok(false)
            }
            Err(error) => Err(error.into()),
        }
    }

    /// Folds the folder `src` into the existing folder `target`. The clashes below it are decided
    /// one by one; the existing folder's own times and permissions are left alone.
    fn merge_dir(
        &mut self,
        ctx: Ctx,
        sp: &dyn Provider,
        src: &VfsPath,
        dp: &dyn Provider,
        target: &VfsPath,
        entry: &ScannedEntry,
    ) -> R<Outcome> {
        let at = src.to_location();
        self.report.transfer.merged.push(target.to_location());
        let mode = sp.permissions(src).ok().and_then(|p| p.mode);
        let inside = Ctx {
            nested: true,
            ..ctx
        };
        let whole = self.place_children(inside, sp, src, dp, target)?;
        let mut outcome = if whole {
            Outcome::Done
        } else {
            Outcome::Partial
        };
        if ctx.mode == Mode::Move && whole {
            // Everything that was in the source folder is in the destination now. A folder that is
            // not empty (something arrived meanwhile) stays.
            let removed = self.attempt(&at, |_| match sp.remove_dir(src) {
                Ok(()) => Ok(true),
                Err(VfsError::NotEmpty { .. }) => Ok(false),
                Err(error) => Err(error.into()),
            })?;
            match removed {
                None => outcome = Outcome::Partial,
                Some(true) => self.units.push(Unit::RemovedDir {
                    src: src.clone(),
                    modified_ms: entry.modified_ms,
                    mode,
                }),
                Some(false) => {}
            }
        }
        self.entry_done(file_name_of(target));
        Ok(outcome)
    }

    /// Replaces the existing folder `target` with the folder `src`: the new folder is built beside
    /// it, the old one is set aside, the new one is renamed in, and only then is the old one
    /// removed. A move takes the source's entries away, one by one with the changed-since check of any
    /// move, after the new folder is in place.
    #[allow(clippy::too_many_arguments)]
    fn replace_dir(
        &mut self,
        ctx: Ctx,
        sp: &dyn Provider,
        src: &VfsPath,
        dp: &dyn Provider,
        target: &VfsPath,
        entry: &ScannedEntry,
        size: Option<(u64, u64)>,
    ) -> R<Outcome> {
        let at = src.to_location();
        if ctx.rename_ok {
            let (entries, bytes) = size.unwrap_or_else(|| self.measure(sp, src, entry));
            let swapped = self.attempt(&at, |t| {
                let aside = t.set_aside(dp, target)?;
                match sp.rename(src, target, false) {
                    Ok(()) => Ok(Some(aside)),
                    Err(error) => {
                        if !t.put_back(dp, &aside, target) {
                            // Stranded under its waiting name: stop rather than go on with an
                            // empty name.
                            return Err(Flow::item(OpsError::Io {
                                message: format!(
                                    "{} could not be put back after a failed replace",
                                    target.display()
                                ),
                            }));
                        }
                        match error {
                            VfsError::CrossesDevices { .. } => Ok(None),
                            other => Err(other.into()),
                        }
                    }
                }
            })?;
            match swapped {
                Some(Some(aside)) => {
                    self.drop_aside(dp, &aside);
                    self.note_placed(target);
                    self.account(entries, bytes);
                    self.report.transfer.replaced.push(target.to_location());
                    self.placed(src, target, How::Renamed);
                    self.meter.emit(self.sink);
                    return Ok(Outcome::Done);
                }
                Some(None) => {}
                None => {
                    self.account(entries, bytes);
                    return Ok(Outcome::Skipped);
                }
            }
        }
        let moving = ctx.mode == Mode::Move;
        let before = if moving {
            self.copied.replace(Vec::new())
        } else {
            None
        };
        let built = self.build_partial_dir(sp, src, dp, target, entry, size);
        let copied = if moving {
            std::mem::replace(&mut self.copied, before)
        } else {
            None
        };
        let Some((partial, whole)) = built? else {
            return Ok(Outcome::Skipped);
        };
        let swapped = self.attempt(&at, |t| {
            let aside = t.set_aside(dp, target)?;
            if let Err(error) = dp.rename(&partial, target, false) {
                t.put_back(dp, &aside, target);
                return Err(error.into());
            }
            Ok(aside)
        });
        let aside = match swapped {
            Ok(Some(aside)) => aside,
            Ok(None) => {
                self.discard(dp, &partial);
                return Ok(Outcome::Skipped);
            }
            Err(flow) => {
                self.discard(dp, &partial);
                return Err(flow);
            }
        };
        self.drop_aside(dp, &aside);
        self.note_placed(target);
        self.report.transfer.replaced.push(target.to_location());
        let how = if ctx.mode == Mode::Move {
            How::Copied
        } else {
            How::Created
        };
        self.placed(src, target, how);
        self.entry_done(file_name_of(target));
        let mut outcome = if whole {
            Outcome::Done
        } else {
            Outcome::Partial
        };
        if ctx.mode == Mode::Move && whole {
            // The new folder is complete; the source goes, item by item as for any move: what was
            // copied and is unchanged is removed, what changed or arrived meanwhile stays (and is
            // reported), and the folders go only if they are empty. A failure part way leaves the
            // items that were removed at the destination and the rest at both.
            let copied = copied.unwrap_or_default();
            let removed = self.attempt(&at, |t| t.remove_copied_source(sp, src, &copied))?;
            if removed != Some(true) {
                outcome = Outcome::Partial;
            }
        } else if ctx.mode == Mode::Move {
            outcome = Outcome::Partial;
        }
        Ok(outcome)
    }
}

/// Gives the copy the original's modification time and permissions, where the provider has them
/// (and not its owner). `known` is the time the original had when the job met it, for a folder
/// whose own time moves as its entries are taken out (a move); `None` reads it now.
pub(crate) fn copy_metadata(
    sp: &dyn Provider,
    src: &VfsPath,
    dp: &dyn Provider,
    target: &VfsPath,
    known: Option<Option<i64>>,
) -> Result<(), VfsError> {
    use waypoint_vfs::FileTimes;
    let modified_ms = match known {
        Some(ms) => ms,
        None => sp.stat(src)?.modified_ms,
    };
    if let Some(ms) = modified_ms {
        match dp.set_times(
            target,
            FileTimes {
                accessed: None,
                modified: Some(super::from_ms(ms)),
            },
        ) {
            Err(VfsError::Unsupported { .. }) => {}
            other => other?,
        }
    }
    match sp.permissions(src) {
        Ok(permissions) => match dp.set_permissions(target, permissions) {
            Err(VfsError::Unsupported { .. }) => Ok(()),
            other => other,
        },
        Err(VfsError::Unsupported { .. }) => Ok(()),
        Err(error) => Err(error),
    }
}
