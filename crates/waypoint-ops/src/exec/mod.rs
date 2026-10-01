// The executor for the simple operations: create, rename, duplicate, trash, restore and delete.
// Every step goes through a `Provider` (or the injected `Trash`), so the same code runs against the
// local file system, the in-memory provider and, later, remote ones.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Writes are atomic per item (A49). A file is written under `.waypoint-partial-{job}-{n}-{name}`
// beside its destination and renamed into place; a folder is built as a partial folder and renamed
// whole; a failure or a cancel removes what it made. A permanent delete cannot be atomic across a
// tree, so it removes children before their folder and a failure leaves a smaller valid tree.

mod batch;
mod copy;
mod copy_engine;
mod copy_job;
mod copy_resolve;
mod remove;

use std::ffi::OsStr;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use waypoint_path::{CaseRule, VfsPath};
use waypoint_protocol::{Location, VfsError};
use waypoint_vfs::{child_path, CancelToken, EntryKind, FileTimes, Provider};

pub use batch::TEMP_PREFIX;
pub use copy::{CopyFile, CopyRequest, SimpleCopy};
pub use copy_engine::CHUNK_BYTES;
pub(crate) use copy_engine::{copy_file_bytes, FileCopy};
pub(crate) use copy_job::copy_metadata;
pub use copy_job::{RunOptions, TransferReport};
pub use copy_resolve::{action_for, Action, Resolutions};

use crate::journal::InverseStep;
use crate::model::{Conflict, Counts, Decision, JobId, JobKind, OpsError, Progress, Resolution};
use crate::names::{file_name_of, unique_full_name};
use crate::plan::{conflict_kind, Plan, PlanItem};
use crate::traits::{IdSource, Protected, Providers, Trash, TrashReceipt};
use remove::remove_tree;

/// Everything the executor reaches the world through.
#[derive(Clone)]
pub struct ExecEnv {
    pub providers: Providers,
    pub trash: Arc<dyn Trash>,
    pub protected: Protected,
    pub ids: Arc<dyn IdSource>,
    pub copier: Arc<dyn CopyFile>,
}

/// How the executor talks to whoever runs it (the worker, which holds the queue).
pub trait ExecSink {
    /// Progress so far. Called often; the queue's gate decides what becomes an event.
    fn progress(&mut self, progress: &Progress, counts: &Counts) {
        let _ = (progress, counts);
    }

    /// An item failed. `Some` answers (the worker asked the user); `None` fails the job there.
    fn on_error(&mut self, item: &Location, error: &OpsError) -> Option<Decision> {
        let _ = (item, error);
        None
    }

    /// A copy or move met a name that is taken and no answer covers it (a clash below a merged
    /// folder, or a choice that cannot settle this kind of clash). `Some` answers, and an answer
    /// with no source is kept as the policy for every later clash; `None` fails the job there:
    /// nothing is overwritten without a decision.
    fn on_conflict(&mut self, conflict: &Conflict) -> Option<Resolution> {
        let _ = conflict;
        None
    }

    /// Called before each top-level item, where a paused job parks.
    fn between_items(&mut self) {}
}

/// A sink that listens to nothing.
#[derive(Debug, Clone, Copy, Default)]
pub struct NullSink;

impl ExecSink for NullSink {}

/// What a finished job did, for the journal and for notices.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExecReport {
    /// The new top-level entries of a create or a duplicate.
    pub created: Vec<Location>,
    /// Renames done, as `(from, to)`.
    pub renamed: Vec<(Location, Location)>,
    pub trashed: Vec<TrashReceipt>,
    /// Where restored items went.
    pub restored: Vec<Location>,
    /// Top-level entries removed for good.
    pub deleted: Vec<Location>,
    /// How many items an `EmptyTrash` removed.
    pub emptied: u64,
    /// Items left out, with why.
    pub skipped: Vec<(Location, OpsError)>,
    pub progress: Progress,
    pub counts: Counts,
    /// What a copy or move did beyond the lists above.
    pub transfer: TransferReport,
    /// What the journal needs to reverse the job, in the order the originals were done. The
    /// fingerprints are filled in after the job ends (`fingerprint_steps`).
    pub inverse: Vec<InverseStep>,
}

/// Why a job stopped before the end. `error` is `Cancelled` for a cancel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecFailure {
    pub error: OpsError,
    /// The item that failed.
    pub item: Option<Location>,
    /// Top-level items completed before it.
    pub done: u64,
    /// What had been done up to then.
    pub report: ExecReport,
}

pub struct Executor {
    env: ExecEnv,
}

impl Executor {
    pub fn new(env: ExecEnv) -> Self {
        Self { env }
    }

    pub fn env(&self) -> &ExecEnv {
        &self.env
    }

    /// Runs a plan. The first error that nobody answers ends the job; a cancel ends it with
    /// `OpsError::Cancelled`.
    pub fn run(
        &self,
        job: JobId,
        plan: &Plan,
        cancel: &CancelToken,
        sink: &mut dyn ExecSink,
    ) -> Result<ExecReport, Box<ExecFailure>> {
        self.run_with(job, plan, cancel, sink, RunOptions::default())
    }

    /// Runs a plan with the answers, verification and chunk size a copy or move takes; the simple
    /// operations ignore them.
    pub fn run_with(
        &self,
        job: JobId,
        plan: &Plan,
        cancel: &CancelToken,
        sink: &mut dyn ExecSink,
        options: RunOptions,
    ) -> Result<ExecReport, Box<ExecFailure>> {
        if matches!(plan.kind, JobKind::Copy | JobKind::Move | JobKind::Link) {
            return copy_job::run(&self.env, job, plan, cancel, sink, options);
        }
        if plan.kind == JobKind::BatchRename {
            return batch::run(&self.env, job, plan, cancel, sink);
        }
        let mut run = Run {
            env: &self.env,
            job,
            cancel,
            sink,
            progress: Progress {
                items_total: plan.total_items,
                bytes_total: plan.total_bytes,
                ..Progress::default()
            },
            counts: Counts::default(),
            report: ExecReport::default(),
            resolutions: options.resolutions,
        };
        let mut done = 0u64;
        if let JobKind::EmptyTrash { older_than_days } = plan.kind {
            return match run.empty_trash(older_than_days) {
                Ok(()) => {
                    run.report.progress = run.progress.clone();
                    run.report.counts = run.counts;
                    Ok(run.report)
                }
                Err(error) => Err(run.fail(error, None, 0)),
            };
        }
        if matches!(
            plan.kind,
            JobKind::BatchRename | JobKind::Undo { .. } | JobKind::Redo { .. }
        ) {
            return Err(run.fail(
                OpsError::Unsupported {
                    what: "this operation is not built yet".to_owned(),
                },
                None,
                done,
            ));
        }
        let mut skip_all = false;
        for item in &plan.items {
            run.sink.between_items();
            let location = item_location(item);
            loop {
                if run.cancel.is_cancelled() {
                    return Err(run.fail(OpsError::Cancelled, Some(location), done));
                }
                match run.item(plan.kind, item) {
                    Ok(()) => {
                        done += 1;
                        break;
                    }
                    Err(OpsError::Cancelled) => {
                        return Err(run.fail(OpsError::Cancelled, Some(location), done));
                    }
                    Err(error) => {
                        let decision = if skip_all {
                            Some(Decision::Skip)
                        } else {
                            run.sink.on_error(&location, &error)
                        };
                        match decision {
                            Some(Decision::Retry) => continue,
                            Some(Decision::CreateParents) => match run.create_parents(&error) {
                                Ok(()) => continue,
                                Err(OpsError::Cancelled) => {
                                    return Err(run.fail(
                                        OpsError::Cancelled,
                                        Some(location),
                                        done,
                                    ));
                                }
                                Err(failed) => {
                                    return Err(run.fail(failed, Some(location), done));
                                }
                            },
                            Some(Decision::Skip) => {
                                run.skip(&location, error);
                                break;
                            }
                            Some(Decision::SkipAll) => {
                                skip_all = true;
                                run.skip(&location, error);
                                break;
                            }
                            Some(Decision::Cancel) => {
                                return Err(run.fail(OpsError::Cancelled, Some(location), done));
                            }
                            None => return Err(run.fail(error, Some(location), done)),
                        }
                    }
                }
            }
            run.progress.items_done = run.progress.items_done.max(done);
            run.emit();
        }
        run.report.progress = run.progress.clone();
        run.report.counts = run.counts;
        Ok(run.report)
    }
}

fn item_location(item: &PlanItem) -> Location {
    item.source
        .as_ref()
        .or(item.target.as_ref())
        .map(VfsPath::to_location)
        .unwrap_or_else(|| Location::new("", ""))
}

pub(crate) fn from_ms(ms: i64) -> SystemTime {
    if ms >= 0 {
        UNIX_EPOCH + Duration::from_millis(ms as u64)
    } else {
        UNIX_EPOCH - Duration::from_millis(ms.unsigned_abs())
    }
}

/// A failure of something a provider may not do is not a reason to fail a copy that has its data.
fn tolerate_unsupported(result: Result<(), VfsError>) -> Result<(), VfsError> {
    match result {
        Err(VfsError::Unsupported { .. }) => Ok(()),
        other => other,
    }
}

struct Run<'a> {
    env: &'a ExecEnv,
    job: JobId,
    cancel: &'a CancelToken,
    sink: &'a mut dyn ExecSink,
    progress: Progress,
    counts: Counts,
    report: ExecReport,
    /// The answers to conflicts so far; a restore that meets a taken name asks when none covers it.
    resolutions: Resolutions,
}

impl Run<'_> {
    fn emit(&mut self) {
        self.sink.progress(&self.progress, &self.counts);
    }

    fn fail(&mut self, error: OpsError, item: Option<Location>, done: u64) -> Box<ExecFailure> {
        self.report.progress = self.progress.clone();
        self.report.counts = self.counts;
        Box::new(ExecFailure {
            error,
            item,
            done,
            report: std::mem::take(&mut self.report),
        })
    }

    fn skip(&mut self, location: &Location, error: OpsError) {
        self.counts.skipped += 1;
        self.report.skipped.push((location.clone(), error));
        self.emit();
    }

    fn check(&self) -> Result<(), OpsError> {
        if self.cancel.is_cancelled() {
            Err(OpsError::Cancelled)
        } else {
            Ok(())
        }
    }

    fn entry_done(&mut self, name: Option<&OsStr>) {
        self.progress.items_done += 1;
        self.progress.current = name.map(|n| n.to_string_lossy().into_owned());
        self.emit();
    }

    fn item(&mut self, kind: JobKind, item: &PlanItem) -> Result<(), OpsError> {
        match kind {
            JobKind::CreateFolder | JobKind::CreateFile => self.create(kind, item),
            JobKind::Rename => self.rename(item),
            JobKind::Duplicate => self.duplicate(item),
            JobKind::Trash => self.trash(item),
            JobKind::Restore => self.restore(item),
            JobKind::Delete => self.delete(item),
            _ => Err(OpsError::Unsupported {
                what: "this operation is not built yet".to_owned(),
            }),
        }
    }

    fn rule(provider: &dyn Provider) -> CaseRule {
        provider.capabilities().case_rule
    }

    /// The name of a partial file beside `final_path`, which no other job or step shares.
    fn partial_path(&self, final_path: &VfsPath) -> Result<VfsPath, OpsError> {
        let provider = self.env.providers.for_path(final_path)?;
        let folder = final_path.parent().ok_or_else(|| OpsError::Protected {
            location: final_path.to_location(),
        })?;
        let name = file_name_of(final_path).unwrap_or_default();
        let prefix = format!(".waypoint-partial-{}-{}-", self.job.0, self.env.ids.next());
        let mut text = name.to_string_lossy().into_owned();
        while prefix.len() + text.len() > 255 && text.pop().is_some() {}
        let text = text.trim_end_matches(['.', ' ']);
        Ok(child_path(
            &folder,
            OsStr::new(&format!("{prefix}{text}")),
            Self::rule(provider.as_ref()),
        )?)
    }

    /// The source as the plan saw it, or why it is not any more.
    fn confirm_source(
        provider: &dyn Provider,
        source: &VfsPath,
        item: &PlanItem,
    ) -> Result<waypoint_vfs::ScannedEntry, OpsError> {
        let entry = provider.stat(source)?;
        if entry.kind != item.kind {
            return Err(OpsError::ChangedSince {
                location: source.to_location(),
            });
        }
        Ok(entry)
    }

    fn create(&mut self, kind: JobKind, item: &PlanItem) -> Result<(), OpsError> {
        let target = item.target.as_ref().expect("a create has a target");
        let provider = self.env.providers.for_path(target)?;
        if kind == JobKind::CreateFolder {
            provider.create_dir(target)?;
        } else {
            provider.create_file(target)?;
        }
        self.report.created.push(target.to_location());
        self.report.inverse.push(InverseStep::RemoveCreated {
            location: target.to_location(),
            fingerprint: None,
        });
        self.entry_done(file_name_of(target).as_deref());
        Ok(())
    }

    fn rename(&mut self, item: &PlanItem) -> Result<(), OpsError> {
        let source = item.source.as_ref().expect("a rename has a source");
        let target = item.target.as_ref().expect("a rename has a target");
        let provider = self.env.providers.for_path(source)?;
        Self::confirm_source(provider.as_ref(), source, item)?;
        if item.case_only {
            // A provider that keeps the case of a name (Windows, macOS, a case-folded volume)
            // takes a case-only rename directly. One that answers "the name is taken" because it
            // sees both spellings as one name leaves the entry alone, and then the rename goes
            // through a third name instead; if the second step fails the first is undone. (A crash
            // between the two steps leaves the entry under that name, which recovery knows.)
            match provider.rename(source, target, false) {
                Ok(()) => {}
                Err(VfsError::AlreadyExists { .. }) => {
                    let aside = self.partial_path(source)?;
                    provider.rename(source, &aside, false)?;
                    if let Err(error) = provider.rename(&aside, target, false) {
                        let _ = provider.rename(&aside, source, false);
                        return Err(error.into());
                    }
                }
                Err(error) => return Err(error.into()),
            }
        } else {
            provider.rename(source, target, false)?;
        }
        self.report
            .renamed
            .push((source.to_location(), target.to_location()));
        self.report.inverse.push(InverseStep::Rename {
            from: target.to_location(),
            to: source.to_location(),
        });
        self.entry_done(file_name_of(target).as_deref());
        Ok(())
    }

    fn duplicate(&mut self, item: &PlanItem) -> Result<(), OpsError> {
        let source = item.source.as_ref().expect("a duplicate has a source");
        let target = item.target.as_ref().expect("a duplicate has a target");
        let provider = self.env.providers.for_path(source)?;
        let entry = Self::confirm_source(provider.as_ref(), source, item)?;
        match entry.kind {
            EntryKind::File => self.copy_file(provider.as_ref(), source, target)?,
            EntryKind::Symlink => self.copy_link(provider.as_ref(), source, target)?,
            EntryKind::Directory => {
                let partial = self.partial_path(target)?;
                let built = self
                    .copy_dir(provider.as_ref(), source, &partial)
                    .and_then(|()| {
                        provider
                            .rename(&partial, target, false)
                            .map_err(OpsError::from)
                    });
                if let Err(error) = built {
                    // Cleanup must finish whatever stopped the copy, so it ignores the cancel.
                    let _ = remove_tree(
                        provider.as_ref(),
                        &partial,
                        &CancelToken::new(),
                        &mut |_| {},
                    );
                    return Err(error);
                }
            }
            EntryKind::Other => {
                self.skip(
                    &source.to_location(),
                    OpsError::Unsupported {
                        what: "copying a special file".to_owned(),
                    },
                );
                return Ok(());
            }
        }
        self.report.created.push(target.to_location());
        self.report.inverse.push(InverseStep::RemoveCreated {
            location: target.to_location(),
            fingerprint: None,
        });
        Ok(())
    }

    fn copy_link(
        &mut self,
        provider: &dyn Provider,
        source: &VfsPath,
        target: &VfsPath,
    ) -> Result<(), OpsError> {
        self.check()?;
        let text = provider.read_link(source)?;
        provider.symlink(target, &text)?;
        self.entry_done(file_name_of(target).as_deref());
        Ok(())
    }

    fn copy_dir(
        &mut self,
        provider: &dyn Provider,
        source: &VfsPath,
        target: &VfsPath,
    ) -> Result<(), OpsError> {
        self.check()?;
        provider.create_dir(target)?;
        for child in provider.list(source, self.cancel, 0, &mut |_| {})? {
            self.check()?;
            let from = source.join(&child.name).map_err(|_| OpsError::Io {
                message: format!("{:?} is not a usable name", child.name),
            })?;
            let to = target.join(&child.name).map_err(|_| OpsError::Io {
                message: format!("{:?} is not a usable name", child.name),
            })?;
            match child.kind {
                EntryKind::File => self.copy_file(provider, &from, &to)?,
                EntryKind::Directory => self.copy_dir(provider, &from, &to)?,
                EntryKind::Symlink => self.copy_link(provider, &from, &to)?,
                EntryKind::Other => self.skip(
                    &from.to_location(),
                    OpsError::Unsupported {
                        what: "copying a special file".to_owned(),
                    },
                ),
            }
        }
        Self::copy_metadata(provider, source, target)?;
        self.entry_done(file_name_of(target).as_deref());
        Ok(())
    }

    /// Copies one file through a partial name and renames it into place.
    fn copy_file(
        &mut self,
        provider: &dyn Provider,
        source: &VfsPath,
        target: &VfsPath,
    ) -> Result<(), OpsError> {
        self.check()?;
        let partial = self.partial_path(target)?;
        let base = self.progress.bytes_done;
        let result = {
            let Run {
                env,
                cancel,
                progress,
                counts,
                sink,
                ..
            } = self;
            let request = CopyRequest {
                src_provider: provider,
                src: source,
                dst_provider: provider,
                dst: &partial,
            };
            env.copier
                .copy_file(
                    &request,
                    &mut |copied| {
                        progress.bytes_done = base + copied;
                        sink.progress(progress, counts);
                    },
                    cancel,
                )
                .and_then(|_| Self::copy_metadata(provider, source, &partial))
                .and_then(|()| provider.rename(&partial, target, false))
        };
        match result {
            Ok(()) => {
                self.entry_done(file_name_of(target).as_deref());
                Ok(())
            }
            Err(error) => {
                let _ = provider.remove_file(&partial);
                self.progress.bytes_done = base;
                Err(error.into())
            }
        }
    }

    /// Gives the copy the original's modification time and permissions, where the provider has
    /// them.
    fn copy_metadata(
        provider: &dyn Provider,
        source: &VfsPath,
        target: &VfsPath,
    ) -> Result<(), VfsError> {
        let entry = provider.stat(source)?;
        if let Some(ms) = entry.modified_ms {
            tolerate_unsupported(provider.set_times(
                target,
                FileTimes {
                    accessed: None,
                    modified: Some(from_ms(ms)),
                },
            ))?;
        }
        match provider.permissions(source) {
            Ok(permissions) => tolerate_unsupported(provider.set_permissions(target, permissions)),
            Err(VfsError::Unsupported { .. }) => Ok(()),
            Err(error) => Err(error),
        }
    }

    fn trash(&mut self, item: &PlanItem) -> Result<(), OpsError> {
        let source = item.source.as_ref().expect("a trash has a source");
        let provider = self.env.providers.for_path(source)?;
        if self
            .env
            .protected
            .contains(source, Self::rule(provider.as_ref()))
        {
            return Err(OpsError::Protected {
                location: source.to_location(),
            });
        }
        let mut results = self.env.trash.trash(&[source.to_location()]);
        let receipt = results.pop().unwrap_or(Err(OpsError::Io {
            message: "the Trash gave no answer".to_owned(),
        }))?;
        self.report.inverse.push(InverseStep::RestoreTrashed {
            receipt: receipt.clone(),
        });
        self.report.trashed.push(receipt);
        self.entry_done(file_name_of(source).as_deref());
        Ok(())
    }

    /// Puts an item back where it was. When that name is taken the run asks (or follows the answer
    /// it already has): keep both restores under a free name, skip leaves the item in the Trash, and
    /// replace swaps one file for another, the old one set aside until the new one is in place.
    fn restore(&mut self, item: &PlanItem) -> Result<(), OpsError> {
        let source = item.source.as_ref().expect("a restore has a source");
        let trashed = source.to_location();
        let trash = self.env.trash.clone();
        let receipt = trash.receipt_for(&trashed)?;
        let (origin, provider) = self.env.providers.for_location(&receipt.original)?;
        let taken = match provider.stat(&origin) {
            Ok(existing) => Some(existing),
            Err(VfsError::NotFound { .. } | VfsError::NotADirectory { .. }) => None,
            Err(error) => return Err(error.into()),
        };
        let back = match taken {
            None => match trash.restore(&receipt) {
                Ok(back) => Some(back),
                // Taken since it was looked at.
                Err(OpsError::NameInUse { .. }) => {
                    let existing = provider.stat(&origin)?;
                    self.restore_over(&receipt, &trashed, &origin, provider.as_ref(), &existing)?
                }
                Err(error) => return Err(error),
            },
            Some(existing) => {
                self.restore_over(&receipt, &trashed, &origin, provider.as_ref(), &existing)?
            }
        };
        if let Some(back) = back {
            self.report.restored.push(back);
        }
        self.entry_done(file_name_of(source).as_deref());
        Ok(())
    }

    /// Restores an item whose original place is taken by `existing`, as the answer says. `None` is
    /// an item that was skipped.
    fn restore_over(
        &mut self,
        receipt: &TrashReceipt,
        trashed: &Location,
        origin: &VfsPath,
        provider: &dyn Provider,
        existing: &waypoint_vfs::ScannedEntry,
    ) -> Result<Option<Location>, OpsError> {
        let item_kind = {
            let source = VfsPath::from_location(trashed).map_err(|_| OpsError::Io {
                message: format!("{} is not a usable location", trashed.display),
            })?;
            self.env.providers.for_path(&source)?.stat(&source)?.kind
        };
        let kind = conflict_kind(item_kind, existing.kind);
        let conflict = Conflict {
            source: trashed.clone(),
            existing: receipt.original.clone(),
            name: file_name_of(origin)
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            kind,
            within_batch: false,
            source_size: None,
            existing_size: existing.size,
            source_modified_ms: None,
            existing_modified_ms: existing.modified_ms,
        };
        let policy = match self.resolutions.policy_for(trashed) {
            Some(policy) => policy,
            None => match self.sink.on_conflict(&conflict) {
                Some(answer) => {
                    self.resolutions.apply(&answer);
                    answer.policy
                }
                None => {
                    return Err(OpsError::NameInUse {
                        location: receipt.original.clone(),
                    })
                }
            },
        };
        let taken = OpsError::NameInUse {
            location: receipt.original.clone(),
        };
        match action_for(policy, kind, None, &receipt.original)? {
            Some(Action::Skip) => {
                self.skip(trashed, taken);
                Ok(None)
            }
            Some(Action::KeepBoth) => {
                let folder = origin.parent().ok_or_else(|| OpsError::Protected {
                    location: origin.to_location(),
                })?;
                let rule = Self::rule(provider);
                let old = file_name_of(origin).unwrap_or_default();
                let name = unique_full_name(
                    &mut |candidate| {
                        child_path(&folder, OsStr::new(candidate), rule)
                            .map(|path| {
                                !matches!(provider.stat(&path), Err(VfsError::NotFound { .. }))
                            })
                            .unwrap_or(true)
                    },
                    &old.to_string_lossy(),
                );
                let target = child_path(&folder, OsStr::new(&name), rule)?;
                Ok(Some(
                    self.env.trash.restore_to(receipt, &target.to_location())?,
                ))
            }
            // Only a file replaces a file: a folder would be buried or lose a tree.
            Some(Action::Replace) if kind == crate::model::ConflictKind::FileOverFile => {
                let aside = self.partial_path(origin)?;
                provider.rename(origin, &aside, false)?;
                match self.env.trash.restore(receipt) {
                    Ok(back) => {
                        let _ = provider.remove_file(&aside);
                        self.report.transfer.replaced.push(receipt.original.clone());
                        Ok(Some(back))
                    }
                    Err(error) => {
                        let _ = provider.rename(&aside, origin, false);
                        Err(error)
                    }
                }
            }
            Some(Action::Replace | Action::Merge) | None => Err(OpsError::CannotReplace {
                location: receipt.original.clone(),
            }),
        }
    }

    /// Makes the folders a restore needs, from the nearest one that exists down to the one the error
    /// names, so the item can be tried again.
    fn create_parents(&mut self, error: &OpsError) -> Result<(), OpsError> {
        let OpsError::OriginMissingParent { location } = error else {
            // Not a missing folder: the decision is a plain retry.
            return Ok(());
        };
        let (folder, provider) = self.env.providers.for_location(location)?;
        let mut missing = Vec::new();
        let mut here = Some(folder);
        while let Some(path) = here {
            match provider.stat(&path) {
                Ok(_) => break,
                Err(VfsError::NotFound { .. }) => {
                    here = path.parent();
                    missing.push(path);
                }
                Err(other) => return Err(other.into()),
            }
        }
        for path in missing.into_iter().rev() {
            self.check()?;
            match provider.create_dir(&path) {
                Ok(()) | Err(VfsError::AlreadyExists { .. }) => {}
                Err(other) => return Err(other.into()),
            }
        }
        Ok(())
    }

    fn empty_trash(&mut self, older_than_days: Option<u32>) -> Result<(), OpsError> {
        self.check()?;
        self.env
            .trash
            .available()
            .map_err(|reason| OpsError::TrashUnavailable { reason })?;
        let removed = self.env.trash.empty(older_than_days)?;
        self.report.emptied = removed;
        self.progress.items_total = removed;
        self.progress.items_done = removed;
        self.emit();
        Ok(())
    }

    fn delete(&mut self, item: &PlanItem) -> Result<(), OpsError> {
        let source = item.source.as_ref().expect("a delete has a source");
        // Something in the Trash goes through the Trash, which removes it and its record together.
        if self.env.trash.is_trashed(&source.to_location()) {
            let receipt = self.env.trash.receipt_for(&source.to_location())?;
            self.env.trash.delete(&receipt)?;
            self.report.deleted.push(source.to_location());
            self.entry_done(file_name_of(source).as_deref());
            return Ok(());
        }
        let provider = self.env.providers.for_path(source)?;
        if self
            .env
            .protected
            .contains(source, Self::rule(provider.as_ref()))
        {
            return Err(OpsError::Protected {
                location: source.to_location(),
            });
        }
        Self::confirm_source(provider.as_ref(), source, item)?;
        {
            let Run {
                cancel,
                progress,
                counts,
                sink,
                ..
            } = self;
            remove_tree(provider.as_ref(), source, cancel, &mut |removed| {
                progress.items_done += 1;
                progress.current = file_name_of(removed).map(|n| n.to_string_lossy().into_owned());
                sink.progress(progress, counts);
            })?;
        }
        self.report.deleted.push(source.to_location());
        Ok(())
    }
}

/// Removes an entry and everything below it through `provider`, for the Trash fake and other
/// callers that hold a provider and no job.
pub fn remove_all(provider: &dyn Provider, path: &VfsPath) -> Result<(), VfsError> {
    remove_tree(provider, path, &CancelToken::new(), &mut |_| {})
}
