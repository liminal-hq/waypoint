// Runs a compression: the sources are walked and added to one new archive, which is written under a
// partial name beside its final one and renamed into place, so a cancel or a failure leaves nothing
// under the final name (A49, A92).
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::ffi::OsStr;
use std::io::{self, Read};
use std::sync::Arc;

use waypoint_path::VfsPath;
use waypoint_protocol::{Location, VfsError};
use waypoint_vfs::{
    child_path, ArchiveBuilder, CancelToken, EntryAttrs, EntryKind, InjectedError, Provider,
    RenameSupport, ScannedEntry, WriteOptions,
};

use super::copy_resolve::{action_for, Action};
use super::remove::remove_tree;
use super::{ExecEnv, ExecFailure, ExecReport, ExecSink, RunOptions};
use crate::journal::InverseStep;
use crate::model::{Conflict, Counts, Decision, JobId, OpsError, Progress};
use crate::names::{file_name_of, name_bytes, unique_full_name};
use crate::plan::{CompressPlan, Plan};
use crate::speed::SpeedEstimator;

/// How much a file has to have been read for the progress to be sent again.
const REPORT_EVERY: u64 = 1024 * 1024;

type R<T> = Result<T, Stop>;

/// Why the run stops.
enum Stop {
    Cancelled,
    /// Fails the job at this item.
    Failed(Box<OpsError>, Option<Box<Location>>),
}

impl Stop {
    fn failed(error: OpsError, item: Option<Location>) -> Self {
        Stop::Failed(Box::new(error), item.map(Box::new))
    }
}

impl From<OpsError> for Stop {
    fn from(error: OpsError) -> Self {
        match error {
            OpsError::Cancelled => Stop::Cancelled,
            other => Stop::failed(other, None),
        }
    }
}

impl From<VfsError> for Stop {
    fn from(error: VfsError) -> Self {
        OpsError::from(error).into()
    }
}

/// What a read of a source file reports as it goes, and where it stops for a cancel.
struct Meter<'a, 'b> {
    inner: Box<dyn Read + Send>,
    cancel: &'a CancelToken,
    sink: &'b mut dyn ExecSink,
    progress: &'b mut Progress,
    counts: &'b Counts,
    speed: &'b mut SpeedEstimator,
    clock: &'b dyn crate::traits::Clock,
    since_report: u64,
}

impl Read for Meter<'_, '_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.cancel.is_cancelled() {
            return Err(InjectedError(VfsError::Cancelled).into_io());
        }
        let read = self.inner.read(buf)?;
        self.progress.bytes_done += read as u64;
        self.since_report += read as u64;
        if self.since_report >= REPORT_EVERY {
            self.since_report = 0;
            self.progress.speed_bps = self
                .speed
                .update(self.clock.now_ms(), self.progress.bytes_done);
            self.progress.eta_ms = self
                .speed
                .eta_ms(self.progress.bytes_done, self.progress.bytes_total);
            self.sink.progress(self.progress, self.counts);
        }
        Ok(read)
    }
}

struct Compress<'a> {
    env: &'a ExecEnv,
    job: JobId,
    cancel: &'a CancelToken,
    sink: &'a mut dyn ExecSink,
    options: RunOptions,
    progress: Progress,
    counts: Counts,
    speed: SpeedEstimator,
    report: ExecReport,
    skip_all: bool,
}

fn attrs_of(provider: &dyn Provider, path: &VfsPath, entry: &ScannedEntry) -> EntryAttrs {
    EntryAttrs {
        mode: provider.permissions(path).ok().and_then(|p| p.mode),
        modified_ms: entry.modified_ms,
    }
}

impl Compress<'_> {
    fn check(&self) -> R<()> {
        if self.cancel.is_cancelled() {
            Err(Stop::Cancelled)
        } else {
            Ok(())
        }
    }

    /// A name beside `target` that nothing else uses.
    fn partial(&self, provider: &dyn Provider, target: &VfsPath) -> R<VfsPath> {
        let folder = target.parent().ok_or_else(|| OpsError::Protected {
            location: target.to_location(),
        })?;
        let name = file_name_of(target).unwrap_or_default();
        let prefix = format!(".waypoint-partial-{}-{}-", self.job.0, self.env.ids.next());
        let mut text = name.to_string_lossy().into_owned();
        while prefix.len() + text.len() > 255 && text.pop().is_some() {}
        let text = text.trim_end_matches(['.', ' ']);
        Ok(child_path(
            &folder,
            OsStr::new(&format!("{prefix}{text}")),
            provider.capabilities().case_rule,
        )
        .map_err(OpsError::from)?)
    }

    fn discard(&self, provider: &dyn Provider, path: &VfsPath) {
        let _ = remove_tree(provider, path, &CancelToken::new(), &mut |_| {});
    }

    /// What to do about the archive's own name being taken: the final path to write, and whether it
    /// replaces what is there. `None` leaves the job with nothing to do.
    fn settle(
        &mut self,
        plan: &Plan,
        provider: &dyn Provider,
        target: &VfsPath,
    ) -> R<Option<(VfsPath, bool)>> {
        let existing = match provider.stat(target) {
            Ok(entry) => entry,
            Err(VfsError::NotFound { .. } | VfsError::NotADirectory { .. }) => {
                return Ok(Some((target.clone(), false)))
            }
            Err(error) => return Err(error.into()),
        };
        let conflict: Conflict = plan.conflicts.first().cloned().unwrap_or_else(|| Conflict {
            source: plan
                .items
                .first()
                .and_then(|i| i.source.as_ref())
                .map_or_else(|| target.to_location(), VfsPath::to_location),
            existing: target.to_location(),
            name: file_name_of(target)
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            kind: crate::plan::conflict_kind(EntryKind::File, existing.kind),
            within_batch: false,
            source_size: None,
            existing_size: existing.size,
            source_modified_ms: None,
            existing_modified_ms: existing.modified_ms,
        });
        loop {
            let policy = match self.options.resolutions.policy_for(&conflict.source) {
                Some(policy) => policy,
                None => match self.sink.on_conflict(&conflict) {
                    Some(resolution) => {
                        self.options.resolutions.apply(&resolution);
                        resolution.policy
                    }
                    None => {
                        return Err(Stop::failed(
                            OpsError::NameInUse {
                                location: target.to_location(),
                            },
                            Some(target.to_location()),
                        ))
                    }
                },
            };
            match action_for(policy, conflict.kind, None, &target.to_location())? {
                Some(Action::Skip) => {
                    self.report.skipped.push((
                        target.to_location(),
                        OpsError::NameInUse {
                            location: target.to_location(),
                        },
                    ));
                    self.counts.skipped += 1;
                    return Ok(None);
                }
                Some(Action::Replace) => return Ok(Some((target.clone(), true))),
                Some(Action::KeepBoth) => {
                    let rule = provider.capabilities().case_rule;
                    let folder = target.parent().ok_or_else(|| OpsError::Protected {
                        location: target.to_location(),
                    })?;
                    let wanted = file_name_of(target)
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    let mut failed = None;
                    let name = unique_full_name(
                        &mut |n| match child_path(&folder, OsStr::new(n), rule) {
                            Ok(path) => match provider.stat(&path) {
                                Ok(_) => true,
                                Err(VfsError::NotFound { .. }) => false,
                                Err(error) => {
                                    failed.get_or_insert(error);
                                    true
                                }
                            },
                            Err(_) => true,
                        },
                        &wanted,
                    );
                    if let Some(error) = failed {
                        return Err(error.into());
                    }
                    return Ok(Some((
                        child_path(&folder, OsStr::new(&name), rule).map_err(OpsError::from)?,
                        false,
                    )));
                }
                // A folder cannot be merged into an archive file: ask again.
                Some(Action::Merge) | None => {
                    self.options.resolutions = crate::exec::Resolutions::default();
                    match self.sink.on_conflict(&conflict) {
                        Some(resolution) => self.options.resolutions.apply(&resolution),
                        None => {
                            return Err(Stop::failed(
                                OpsError::NameInUse {
                                    location: target.to_location(),
                                },
                                Some(target.to_location()),
                            ))
                        }
                    }
                }
            }
        }
    }

    /// Asks what to do about an item that failed before it was added: `true` skips it, `false`
    /// tries again, and anything else stops the job.
    fn decide(&mut self, location: &Location, error: OpsError) -> R<bool> {
        if matches!(error, OpsError::Cancelled) {
            return Err(Stop::Cancelled);
        }
        let decision = if self.skip_all {
            Some(Decision::Skip)
        } else {
            self.sink.on_error(location, &error)
        };
        match decision {
            Some(Decision::Retry | Decision::CreateParents) => Ok(false),
            Some(Decision::Skip) => {
                self.skipped(location, error);
                Ok(true)
            }
            Some(Decision::SkipAll) => {
                self.skip_all = true;
                self.skipped(location, error);
                Ok(true)
            }
            Some(Decision::Cancel) => Err(Stop::Cancelled),
            None => Err(Stop::failed(error, Some(location.clone()))),
        }
    }

    fn skipped(&mut self, location: &Location, error: OpsError) {
        self.counts.skipped += 1;
        self.report.skipped.push((location.clone(), error));
        self.sink.progress(&self.progress, &self.counts);
    }

    fn entry_done(&mut self, name: Option<&OsStr>) {
        self.progress.items_done += 1;
        self.progress.current = name.map(|n| n.to_string_lossy().into_owned());
        self.sink.progress(&self.progress, &self.counts);
    }

    /// Adds `root` and everything below it to the archive.
    fn add_tree(
        &mut self,
        builder: &mut dyn ArchiveBuilder,
        provider: &Arc<dyn Provider>,
        root: &VfsPath,
        exclude: &[VfsPath],
    ) -> R<()> {
        let entry = provider.stat(root)?;
        let name = file_name_of(root).ok_or_else(|| OpsError::InvalidName {
            name: root.display(),
            reason: "a root has no name to compress".to_owned(),
        })?;
        let mut stack: Vec<(VfsPath, Vec<u8>, ScannedEntry)> =
            vec![(root.clone(), name_bytes(&name), entry)];
        while let Some((path, inside, entry)) = stack.pop() {
            self.check()?;
            if exclude.contains(&path) {
                continue;
            }
            let location = path.to_location();
            match entry.kind {
                EntryKind::Directory => {
                    let children = loop {
                        match provider.list(&path, self.cancel, 0, &mut |_| {}) {
                            Ok(children) => break Some(children),
                            Err(error) => {
                                if self.decide(&location, error.into())? {
                                    break None;
                                }
                            }
                        }
                    };
                    let Some(mut children) = children else {
                        continue;
                    };
                    let attrs = attrs_of(provider.as_ref(), &path, &entry);
                    builder.add_dir(&inside, attrs).map_err(Stop::from)?;
                    children.sort_by(|a, b| b.name.cmp(&a.name));
                    for child in children {
                        let Ok(child_path) = path.join(&child.name) else {
                            self.skipped(
                                &location,
                                OpsError::InvalidName {
                                    name: child.name.to_string_lossy().into_owned(),
                                    reason: "not a usable name".to_owned(),
                                },
                            );
                            continue;
                        };
                        let mut child_inside = inside.clone();
                        child_inside.push(b'/');
                        child_inside.extend_from_slice(&name_bytes(&child.name));
                        stack.push((child_path, child_inside, child));
                    }
                    self.entry_done(Some(&entry.name));
                }
                EntryKind::File => {
                    // The size now, not as the folder was listed.
                    let (current, mut stream) = loop {
                        let opened = provider
                            .stat(&path)
                            .and_then(|now| provider.open_read(&path).map(|s| (now, s)));
                        match opened {
                            Ok(found) => break (Some(found.0), Some(found.1)),
                            Err(error) => {
                                if self.decide(&location, error.into())? {
                                    break (None, None);
                                }
                            }
                        }
                    };
                    let (Some(now), Some(stream)) = (current, stream.take()) else {
                        continue;
                    };
                    let size = now.size.unwrap_or(0);
                    let attrs = attrs_of(provider.as_ref(), &path, &now);
                    let mut meter = Meter {
                        inner: stream,
                        cancel: self.cancel,
                        sink: &mut *self.sink,
                        progress: &mut self.progress,
                        counts: &self.counts,
                        speed: &mut self.speed,
                        clock: self.options.clock.as_ref(),
                        since_report: 0,
                    };
                    builder
                        .add_file(&inside, size, attrs, &mut meter)
                        .map_err(Stop::from)?;
                    self.entry_done(Some(&entry.name));
                }
                EntryKind::Symlink => {
                    let target = loop {
                        match provider.read_link(&path) {
                            Ok(text) => break Some(text),
                            Err(error) => {
                                if self.decide(&location, error.into())? {
                                    break None;
                                }
                            }
                        }
                    };
                    let Some(text) = target else { continue };
                    let attrs = attrs_of(provider.as_ref(), &path, &entry);
                    builder
                        .add_symlink(&inside, &name_bytes(&text), attrs)
                        .map_err(Stop::from)?;
                    self.entry_done(Some(&entry.name));
                }
                EntryKind::Other => {
                    self.skipped(
                        &location,
                        OpsError::Unsupported {
                            what: "a device or a pipe in an archive".to_owned(),
                        },
                    );
                }
            }
        }
        Ok(())
    }

    fn build(&mut self, plan: &Plan, compress: &CompressPlan) -> R<()> {
        let dest = self.env.providers.for_path(&compress.target)?;
        let Some((target, replacing)) = self.settle(plan, dest.as_ref(), &compress.target)? else {
            return Ok(());
        };
        let writers = self.env.providers.writers()?;
        let caps = dest.capabilities();
        // A provider that cannot rename takes the archive under its final name, as a copy does.
        let direct = caps.rename == RenameSupport::None;
        let write_to = if direct {
            target.clone()
        } else {
            self.partial(dest.as_ref(), &target)?
        };
        let options = WriteOptions {
            exclusive: !(direct && replacing),
            mode: None,
        };
        let stream = dest.create_write(&write_to, options)?;
        let mut builder = writers.begin(compress.kind, stream, target.to_location())?;
        let exclude = vec![write_to.clone(), target.clone()];
        let walked = (|| -> R<()> {
            for item in &plan.items {
                self.sink.between_items();
                self.check()?;
                let source = item.source.as_ref().expect("a compression has sources");
                let provider = self.env.providers.for_path(source)?;
                self.add_tree(builder.as_mut(), &provider, source, &exclude)?;
                self.report
                    .transfer
                    .placed_sources
                    .push(source.to_location());
            }
            Ok(())
        })();
        let finished = match walked {
            Ok(()) => builder.finish(false).map_err(Stop::from),
            Err(stop) => {
                drop(builder);
                Err(stop)
            }
        };
        if let Err(stop) = finished {
            self.discard(dest.as_ref(), &write_to);
            return Err(stop);
        }
        if !direct {
            if let Err(error) = dest.rename(&write_to, &target, replacing) {
                self.discard(dest.as_ref(), &write_to);
                return Err(error.into());
            }
        }
        if replacing {
            self.report.transfer.replaced.push(target.to_location());
            self.report.transfer.unjournalled =
                Some("the archive it replaced is gone for good".to_owned());
        }
        self.report.created.push(target.to_location());
        // What was replaced is gone for good, so there is nothing to undo to.
        if !replacing {
            self.report.inverse.push(InverseStep::RemoveCreated {
                location: target.to_location(),
                fingerprint: None,
            });
        }
        Ok(())
    }

    fn failure(&mut self, stop: Stop, done: u64) -> Box<ExecFailure> {
        let (error, item) = match stop {
            Stop::Cancelled => (OpsError::Cancelled, None),
            Stop::Failed(error, item) => (*error, item.map(|item| *item)),
        };
        self.report.progress = self.progress.clone();
        self.report.counts = self.counts;
        Box::new(ExecFailure {
            error,
            item,
            done,
            report: std::mem::take(&mut self.report),
        })
    }
}

pub(super) fn run(
    env: &ExecEnv,
    job: JobId,
    plan: &Plan,
    cancel: &CancelToken,
    sink: &mut dyn ExecSink,
    options: RunOptions,
) -> Result<ExecReport, Box<ExecFailure>> {
    let mut run = Compress {
        env,
        job,
        cancel,
        sink,
        options,
        progress: Progress {
            items_total: plan.total_items,
            bytes_total: plan.total_bytes,
            ..Progress::default()
        },
        counts: Counts::default(),
        speed: SpeedEstimator::new(),
        report: ExecReport::default(),
        skip_all: false,
    };
    let Some(compress) = &plan.compress else {
        return Err(run.failure(
            Stop::failed(
                OpsError::Unsupported {
                    what: "a compression without its plan".to_owned(),
                },
                None,
            ),
            0,
        ));
    };
    match run.build(plan, compress) {
        Ok(()) => {
            run.report.progress = run.progress.clone();
            run.report.counts = run.counts;
            Ok(std::mem::take(&mut run.report))
        }
        Err(stop) => Err(run.failure(stop, 0)),
    }
}
