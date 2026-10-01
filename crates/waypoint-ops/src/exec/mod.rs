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

mod copy;
mod remove;

use std::ffi::OsStr;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use waypoint_path::{CaseRule, VfsPath};
use waypoint_protocol::{Location, VfsError};
use waypoint_vfs::{child_path, CancelToken, EntryKind, FileTimes, Provider};

pub use copy::{CopyFile, CopyRequest, SimpleCopy};

use crate::model::{Counts, Decision, JobId, JobKind, OpsError, Progress};
use crate::names::file_name_of;
use crate::plan::{Plan, PlanItem};
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
    /// Items left out, with why.
    pub skipped: Vec<(Location, OpsError)>,
    pub progress: Progress,
    pub counts: Counts,
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
        };
        let mut done = 0u64;
        if matches!(
            plan.kind,
            JobKind::Copy
                | JobKind::Move
                | JobKind::BatchRename
                | JobKind::Undo { .. }
                | JobKind::Redo { .. }
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

fn from_ms(ms: i64) -> SystemTime {
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
        self.entry_done(file_name_of(target).as_deref());
        Ok(())
    }

    fn rename(&mut self, item: &PlanItem) -> Result<(), OpsError> {
        let source = item.source.as_ref().expect("a rename has a source");
        let target = item.target.as_ref().expect("a rename has a target");
        let provider = self.env.providers.for_path(source)?;
        Self::confirm_source(provider.as_ref(), source, item)?;
        if item.case_only {
            // The file system sees both spellings as one name, so the rename goes through a
            // third name; if the second step fails the first is undone.
            let aside = self.partial_path(source)?;
            provider.rename(source, &aside, false)?;
            if let Err(error) = provider.rename(&aside, target, false) {
                let _ = provider.rename(&aside, source, false);
                return Err(error.into());
            }
        } else {
            provider.rename(source, target, false)?;
        }
        self.report
            .renamed
            .push((source.to_location(), target.to_location()));
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
        self.report.trashed.push(receipt);
        self.entry_done(file_name_of(source).as_deref());
        Ok(())
    }

    fn restore(&mut self, item: &PlanItem) -> Result<(), OpsError> {
        let source = item.source.as_ref().expect("a restore has a source");
        let receipt = self.env.trash.receipt_for(&source.to_location())?;
        let back = self.env.trash.restore(&receipt)?;
        self.report.restored.push(back);
        self.entry_done(file_name_of(source).as_deref());
        Ok(())
    }

    fn delete(&mut self, item: &PlanItem) -> Result<(), OpsError> {
        let source = item.source.as_ref().expect("a delete has a source");
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
