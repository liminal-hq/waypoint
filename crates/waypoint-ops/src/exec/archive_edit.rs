// Runs a change to an archive: the archive is rewritten, entry by entry, into a new file beside it
// under a partial name; once that file is complete and reads back as written, the old archive goes
// to the Trash (so Undo can bring it back) and the new file is renamed into its place. Until then
// the original is untouched, so a cancel or a failure leaves it exactly as it was (D170, A112).
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::io::Read;

use waypoint_path::VfsPath;
use waypoint_protocol::Location;
use waypoint_vfs::{
    ArchiveBuilder, CancelToken, EntryAttrs, EntryKind, Provider, RenameSupport, ScannedEntry,
    WriteOptions,
};

use super::compress::{attrs_of, partial_beside, Meter, Stop, R};
use super::copy_resolve::{action_for, Action};
use super::remove::remove_tree;
use super::{ExecEnv, ExecFailure, ExecReport, ExecSink, RunOptions};
use crate::journal::InverseStep;
use crate::model::{Conflict, Counts, Decision, JobId, OpsError, Progress};
use crate::names::{archive_path, file_name_of, name_bytes, unique_full_name};
use crate::plan::{conflict_kind, ArchiveChange, ArchiveEditPlan, Plan};
use crate::speed::SpeedEstimator;

type Comps = Vec<Vec<u8>>;

/// An entry to be added to the archive.
struct New {
    comps: Comps,
    kind: EntryKind,
    source: Option<VfsPath>,
    attrs: EntryAttrs,
    /// Left out after all: a later clash replaced it.
    dropped: bool,
}

/// What the new archive is expected to hold, to compare with what it reads back as.
type Manifest = Vec<(Comps, EntryKind, Option<u64>)>;

fn joined(comps: &[Vec<u8>]) -> Vec<u8> {
    comps.join(&b'/')
}

fn below(path: &[Vec<u8>], ancestor: &[Vec<u8>]) -> bool {
    path.len() >= ancestor.len() && path[..ancestor.len()] == *ancestor
}

fn digest(manifest: &Manifest) -> String {
    let mut lines: Vec<String> = manifest
        .iter()
        .map(|(comps, kind, size)| {
            format!(
                "{}\u{0}{}\u{0}{:?}\u{0}{:?}",
                String::from_utf8_lossy(&joined(comps)),
                comps.len(),
                kind,
                size
            )
        })
        .collect();
    lines.sort();
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    lines.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

struct Edit<'a> {
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

impl Edit<'_> {
    fn check(&self) -> R<()> {
        if self.cancel.is_cancelled() {
            Err(Stop::Cancelled)
        } else {
            Ok(())
        }
    }

    fn discard(&self, provider: &dyn Provider, path: &VfsPath) {
        let _ = remove_tree(provider, path, &CancelToken::new(), &mut |_| {});
    }

    fn skipped(&mut self, location: &Location, error: OpsError) {
        self.counts.skipped += 1;
        self.report.skipped.push((location.clone(), error));
        self.sink.progress(&self.progress, &self.counts);
    }

    /// Asks what to do about a source that failed before it was added: `true` skips it, `false`
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

    fn entry_done(&mut self, name: &str) {
        self.progress.items_done += 1;
        self.progress.current = Some(name.to_owned());
        self.sink.progress(&self.progress, &self.counts);
    }

    /// What to do about a clash, asking when no answer covers it; `None` leaves the entry out.
    fn resolve(&mut self, conflict: &Conflict) -> R<Action> {
        loop {
            let policy = match self.options.resolutions.policy_for(&conflict.source) {
                Some(policy) => policy,
                None => match self.sink.on_conflict(conflict) {
                    Some(resolution) => {
                        self.options.resolutions.apply(&resolution);
                        resolution.policy
                    }
                    None => {
                        return Err(Stop::failed(
                            OpsError::NameInUse {
                                location: conflict.existing.clone(),
                            },
                            Some(conflict.existing.clone()),
                        ))
                    }
                },
            };
            match action_for(policy, conflict.kind, None, &conflict.existing)? {
                Some(action) => return Ok(action),
                None => {
                    // The answer cannot settle this clash: ask again.
                    self.options.resolutions = crate::exec::Resolutions::default();
                    match self.sink.on_conflict(conflict) {
                        Some(resolution) => self.options.resolutions.apply(&resolution),
                        None => {
                            return Err(Stop::failed(
                                OpsError::NameInUse {
                                    location: conflict.existing.clone(),
                                },
                                Some(conflict.existing.clone()),
                            ))
                        }
                    }
                }
            }
        }
    }

    /// Walks what is added, settling every clash with what the archive holds, and says which
    /// original entries a replacement drops.
    fn collect(
        &mut self,
        edit: &ArchiveEditPlan,
        into: &[Vec<u8>],
        sources: &[VfsPath],
        replaced: &mut HashSet<Comps>,
    ) -> R<Vec<New>> {
        // What exists, by its names: the originals, then what has been added.
        let mut taken: HashMap<Comps, EntryKind> = HashMap::new();
        for entry in &edit.entries {
            if let VfsPath::Archive(a) = &entry.path {
                taken.insert(a.inner().to_vec(), entry.kind);
            }
        }
        let mut added: HashMap<Comps, usize> = HashMap::new();
        let mut out: Vec<New> = Vec::new();
        for source in sources {
            self.check()?;
            let provider = self.env.providers.for_path(source)?;
            let entry = provider.stat(source)?;
            let name = file_name_of(source).ok_or_else(|| OpsError::InvalidName {
                name: source.display(),
                reason: "a root has no name to add".to_owned(),
            })?;
            let mut comps = into.to_vec();
            comps.push(name_bytes(&name));
            // Each stack item: where it is, the names it will have, and what it is.
            let mut stack: Vec<(VfsPath, Comps, ScannedEntry)> =
                vec![(source.clone(), comps, entry)];
            while let Some((path, mut comps, entry)) = stack.pop() {
                self.check()?;
                let location = path.to_location();
                if let Some(&old) = taken.get(&comps) {
                    if old == EntryKind::Directory && entry.kind == EntryKind::Directory {
                        // Folders merge: nothing to settle for the folder itself.
                        self.push_children(&mut stack, &provider, &path, &comps, &location)?;
                        continue;
                    }
                    let target = archive_path(&edit.container, &comps).map_err(|_| {
                        OpsError::InvalidName {
                            name: path.display(),
                            reason: "not a usable name in an archive".to_owned(),
                        }
                    })?;
                    let conflict = Conflict {
                        source: location.clone(),
                        existing: target.to_location(),
                        name: file_name_of(&path)
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_default(),
                        kind: conflict_kind(entry.kind, old),
                        within_batch: added.contains_key(&comps),
                        source_size: entry.size,
                        existing_size: None,
                        source_modified_ms: entry.modified_ms,
                        existing_modified_ms: None,
                    };
                    match self.resolve(&conflict)? {
                        Action::Skip => {
                            self.skipped(
                                &location,
                                OpsError::NameInUse {
                                    location: target.to_location(),
                                },
                            );
                            continue;
                        }
                        Action::KeepBoth => {
                            let folder = comps[..comps.len() - 1].to_vec();
                            let wanted =
                                String::from_utf8_lossy(&comps[comps.len() - 1]).into_owned();
                            let unique = unique_full_name(
                                &mut |n| {
                                    let mut probe = folder.clone();
                                    probe.push(n.as_bytes().to_vec());
                                    taken.contains_key(&probe)
                                },
                                &wanted,
                            );
                            let last = comps.len() - 1;
                            comps[last] = unique.into_bytes();
                        }
                        Action::Replace => {
                            match added.remove(&comps) {
                                Some(index) => out[index].dropped = true,
                                None => {
                                    replaced.insert(comps.clone());
                                }
                            }
                            taken.remove(&comps);
                        }
                        Action::Merge => {}
                    }
                }
                let attrs = EntryAttrs {
                    mode: provider.permissions(&path).ok().and_then(|p| p.mode),
                    modified_ms: entry.modified_ms,
                };
                match entry.kind {
                    EntryKind::Other => {
                        self.skipped(
                            &location,
                            OpsError::Unsupported {
                                what: "a device or a pipe in an archive".to_owned(),
                            },
                        );
                        continue;
                    }
                    EntryKind::Directory => {
                        taken.insert(comps.clone(), EntryKind::Directory);
                        added.insert(comps.clone(), out.len());
                        out.push(New {
                            comps: comps.clone(),
                            kind: EntryKind::Directory,
                            source: Some(path.clone()),
                            attrs,
                            dropped: false,
                        });
                        self.push_children(&mut stack, &provider, &path, &comps, &location)?;
                    }
                    kind => {
                        taken.insert(comps.clone(), kind);
                        added.insert(comps.clone(), out.len());
                        out.push(New {
                            comps,
                            kind,
                            source: Some(path),
                            attrs,
                            dropped: false,
                        });
                    }
                }
            }
        }
        Ok(out)
    }

    fn push_children(
        &mut self,
        stack: &mut Vec<(VfsPath, Comps, ScannedEntry)>,
        provider: &std::sync::Arc<dyn Provider>,
        path: &VfsPath,
        comps: &Comps,
        location: &Location,
    ) -> R<()> {
        let children = loop {
            match provider.list(path, self.cancel, 0, &mut |_| {}) {
                Ok(children) => break Some(children),
                Err(error) => {
                    if self.decide(location, error.into())? {
                        break None;
                    }
                }
            }
        };
        let Some(mut children) = children else {
            return Ok(());
        };
        children.sort_by(|a, b| b.name.cmp(&a.name));
        for child in children {
            let Ok(child_path) = path.join(&child.name) else {
                self.skipped(
                    location,
                    OpsError::InvalidName {
                        name: child.name.to_string_lossy().into_owned(),
                        reason: "not a usable name".to_owned(),
                    },
                );
                continue;
            };
            let mut child_comps = comps.clone();
            child_comps.push(name_bytes(&child.name));
            stack.push((child_path, child_comps, child));
        }
        Ok(())
    }

    /// Writes an entry the archive already held.
    fn copy_original(
        &mut self,
        builder: &mut dyn ArchiveBuilder,
        entry: &waypoint_vfs::ArchiveEntryInfo,
        comps: &[Vec<u8>],
        manifest: &mut Manifest,
    ) -> R<()> {
        let name = joined(comps);
        let attrs = EntryAttrs {
            mode: entry.mode,
            modified_ms: entry.modified_ms,
        };
        let shown = String::from_utf8_lossy(comps.last().map_or(&[][..], |c| c)).into_owned();
        match entry.kind {
            EntryKind::Directory => {
                builder.add_dir(&name, attrs)?;
                manifest.push((comps.to_vec(), EntryKind::Directory, None));
            }
            EntryKind::File => {
                let provider = self.env.providers.for_path(&entry.path)?;
                let size = entry.size.ok_or_else(|| OpsError::Unsupported {
                    what: "an entry that does not say how large it is".to_owned(),
                })?;
                let stream = provider.open_read(&entry.path)?;
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
                builder.add_file(&name, size, attrs, &mut meter)?;
                manifest.push((comps.to_vec(), EntryKind::File, Some(size)));
            }
            EntryKind::Symlink => {
                let target = match &entry.link_target {
                    Some(text) => text.clone(),
                    None => {
                        let provider = self.env.providers.for_path(&entry.path)?;
                        name_bytes(&provider.read_link(&entry.path)?)
                    }
                };
                builder.add_symlink(&name, &target, attrs)?;
                manifest.push((comps.to_vec(), EntryKind::Symlink, None));
            }
            EntryKind::Other => {
                return Err(Stop::failed(
                    OpsError::Unsupported {
                        what: "an archive that holds a device or a pipe".to_owned(),
                    },
                    None,
                ))
            }
        }
        self.entry_done(&shown);
        Ok(())
    }

    /// Writes a file, folder or link that is being added.
    fn write_new(
        &mut self,
        builder: &mut dyn ArchiveBuilder,
        new: &New,
        manifest: &mut Manifest,
    ) -> R<()> {
        let name = joined(&new.comps);
        let shown = String::from_utf8_lossy(new.comps.last().map_or(&[][..], |c| c)).into_owned();
        let source = new.source.as_ref().expect("an added entry has a source");
        let provider = self.env.providers.for_path(source)?;
        let location = source.to_location();
        match new.kind {
            EntryKind::Directory => {
                builder.add_dir(&name, new.attrs)?;
                manifest.push((new.comps.clone(), EntryKind::Directory, None));
            }
            EntryKind::File => {
                let (current, stream) = loop {
                    let opened = provider
                        .stat(source)
                        .and_then(|now| provider.open_read(source).map(|s| (now, s)));
                    match opened {
                        Ok(found) => break (Some(found.0), Some(found.1)),
                        Err(error) => {
                            if self.decide(&location, error.into())? {
                                break (None, None);
                            }
                        }
                    }
                };
                let (Some(now), Some(stream)) = (current, stream) else {
                    return Ok(());
                };
                let size = now.size.unwrap_or(0);
                let attrs = attrs_of(provider.as_ref(), source, &now);
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
                builder.add_file(&name, size, attrs, &mut meter)?;
                manifest.push((new.comps.clone(), EntryKind::File, Some(size)));
            }
            EntryKind::Symlink => {
                let text = loop {
                    match provider.read_link(source) {
                        Ok(text) => break Some(text),
                        Err(error) => {
                            if self.decide(&location, error.into())? {
                                break None;
                            }
                        }
                    }
                };
                let Some(text) = text else { return Ok(()) };
                builder.add_symlink(&name, &name_bytes(&text), new.attrs)?;
                manifest.push((new.comps.clone(), EntryKind::Symlink, None));
            }
            EntryKind::Other => return Ok(()),
        }
        self.entry_done(&shown);
        Ok(())
    }

    /// Checks that the archive just written holds what was written, and with verification on that
    /// every file in it reads back whole.
    fn verify(&mut self, partial: &VfsPath, expected: &Manifest, location: &Location) -> R<()> {
        let catalog = self.env.providers.catalog()?;
        let top = archive_path(partial, &[]).map_err(|_| OpsError::Unsupported {
            what: "reading back the new archive".to_owned(),
        })?;
        let listed = catalog.archive_entries(&top, self.cancel, &mut |_| {})?;
        let mut actual: Manifest = Vec::new();
        for entry in listed.iter().filter(|e| !e.synthetic) {
            if let VfsPath::Archive(a) = &entry.path {
                let size = (entry.kind == EntryKind::File).then_some(entry.size.unwrap_or(0));
                actual.push((a.inner().to_vec(), entry.kind, size));
            }
        }
        let (want, got) = (digest(expected), digest(&actual));
        if want != got || expected.len() != actual.len() {
            return Err(Stop::failed(
                OpsError::VerifyFailed {
                    location: location.clone(),
                    expected: want,
                    actual: got,
                },
                Some(location.clone()),
            ));
        }
        if self.options.verify.is_some() {
            let provider = self.env.providers.for_path(&top)?;
            let mut buffer = vec![0u8; 64 * 1024];
            for entry in listed.iter().filter(|e| e.kind == EntryKind::File) {
                self.check()?;
                let mut stream = provider.open_read(&entry.path)?;
                let mut total = 0u64;
                loop {
                    self.check()?;
                    let read = stream
                        .read(&mut buffer)
                        .map_err(|e| OpsError::from(waypoint_vfs::from_io(&e, location)))?;
                    if read == 0 {
                        break;
                    }
                    total += read as u64;
                }
                if Some(total) != entry.size {
                    return Err(Stop::failed(
                        OpsError::VerifyFailed {
                            location: location.clone(),
                            expected: format!("{:?}", entry.size),
                            actual: total.to_string(),
                        },
                        Some(location.clone()),
                    ));
                }
            }
        }
        Ok(())
    }

    fn rewrite(&mut self, edit: &ArchiveEditPlan) -> R<()> {
        let container = edit.container.clone();
        let location = container.to_location();
        let dest = self.env.providers.for_path(&container)?;
        let now = dest.stat(&container)?;
        if now.size.unwrap_or(0) != edit.size || now.modified_ms != edit.modified_ms {
            return Err(Stop::failed(
                OpsError::ChangedSince {
                    location: location.clone(),
                },
                Some(location),
            ));
        }
        if dest.capabilities().rename == RenameSupport::None {
            return Err(Stop::failed(
                OpsError::ArchiveNotWritable {
                    location: location.clone(),
                    reason: crate::model::ArchiveWriteRefusal::NoAtomicReplace,
                },
                Some(location),
            ));
        }
        // Settle every clash before writing anything.
        let mut replaced: HashSet<Comps> = HashSet::new();
        let new: Vec<New> = match &edit.change {
            ArchiveChange::Add { into, sources } => {
                self.collect(edit, into, sources, &mut replaced)?
            }
            _ => Vec::new(),
        };
        self.check()?;
        if matches!(edit.change, ArchiveChange::Add { .. })
            && new.iter().all(|n| n.dropped)
            && replaced.is_empty()
        {
            // Everything was skipped: the archive stays as it is.
            return Ok(());
        }
        let writers = self.env.providers.writers()?;
        let partial = partial_beside(self.env, self.job, dest.as_ref(), &container)?;
        let mode = dest.permissions(&container).ok().and_then(|p| p.mode);
        let stream = dest.create_write(
            &partial,
            WriteOptions {
                exclusive: true,
                mode,
            },
        )?;
        let mut builder = writers.begin(edit.kind, stream, location.clone())?;
        let mut manifest: Manifest = Vec::new();
        let written = (|| -> R<()> {
            for entry in edit.entries.iter().filter(|e| !e.synthetic) {
                self.check()?;
                let VfsPath::Archive(a) = &entry.path else {
                    continue;
                };
                let inner = a.inner();
                let comps: Comps = match &edit.change {
                    ArchiveChange::Delete { paths } => {
                        if paths.iter().any(|p| below(inner, p)) {
                            continue;
                        }
                        inner.to_vec()
                    }
                    ArchiveChange::Rename { from, name } => {
                        if below(inner, from) {
                            let mut renamed = from[..from.len() - 1].to_vec();
                            renamed.push(name.clone());
                            renamed.extend_from_slice(&inner[from.len()..]);
                            renamed
                        } else {
                            inner.to_vec()
                        }
                    }
                    ArchiveChange::Add { .. } => {
                        if replaced.contains(inner) {
                            continue;
                        }
                        inner.to_vec()
                    }
                    ArchiveChange::Make { .. } => inner.to_vec(),
                };
                self.copy_original(builder.as_mut(), entry, &comps, &mut manifest)?;
            }
            if let ArchiveChange::Make { into, name, folder } = &edit.change {
                let mut comps = into.clone();
                comps.push(name.clone());
                let attrs = EntryAttrs {
                    mode: Some(if *folder { 0o755 } else { 0o644 }),
                    modified_ms: Some(self.options.clock.now_ms()),
                };
                if *folder {
                    builder.add_dir(&joined(&comps), attrs)?;
                    manifest.push((comps, EntryKind::Directory, None));
                } else {
                    builder.add_file(&joined(&comps), 0, attrs, &mut std::io::empty())?;
                    manifest.push((comps, EntryKind::File, Some(0)));
                }
                self.entry_done(&String::from_utf8_lossy(name));
            }
            for item in new.iter().filter(|n| !n.dropped) {
                self.sink.between_items();
                self.check()?;
                self.write_new(builder.as_mut(), item, &mut manifest)?;
            }
            Ok(())
        })();
        let finished = match written {
            Ok(()) => builder.finish(true).map_err(Stop::from),
            Err(stop) => {
                drop(builder);
                Err(stop)
            }
        };
        if let Err(stop) = finished {
            self.discard(dest.as_ref(), &partial);
            return Err(stop);
        }
        if let Err(stop) = self.verify(&partial, &manifest, &location) {
            self.discard(dest.as_ref(), &partial);
            return Err(stop);
        }
        if let Err(stop) = self.check() {
            self.discard(dest.as_ref(), &partial);
            return Err(stop);
        }
        self.swap(dest.as_ref(), &partial, &container)?;
        if let ArchiveChange::Add { sources, .. } = &edit.change {
            for source in sources {
                self.report
                    .transfer
                    .placed_sources
                    .push(source.to_location());
            }
        }
        self.report.transfer.policy = self.options.resolutions.all();
        Ok(())
    }

    /// Puts the finished file in the archive's place: the old archive goes to the Trash first, so
    /// that Undo can bring it back; where it cannot be trashed (a server) it is replaced for good
    /// in one atomic step.
    fn swap(&mut self, dest: &dyn Provider, partial: &VfsPath, target: &VfsPath) -> R<()> {
        let location = target.to_location();
        let trashed = self
            .env
            .trash
            .trash(std::slice::from_ref(&location))
            .into_iter()
            .next()
            .and_then(Result::ok);
        match trashed {
            Some(receipt) => {
                if let Err(error) = dest.rename(partial, target, false) {
                    // Put the old archive back: nothing was changed.
                    let _ = self.env.trash.restore(&receipt);
                    self.discard(dest, partial);
                    return Err(error.into());
                }
                self.report.trashed.push(receipt.clone());
                self.report
                    .inverse
                    .push(InverseStep::RestoreTrashed { receipt });
                self.report.inverse.push(InverseStep::RemoveCreated {
                    location: location.clone(),
                    fingerprint: None,
                });
            }
            None => {
                if let Err(error) = dest.rename(partial, target, true) {
                    self.discard(dest, partial);
                    return Err(error.into());
                }
                self.report.transfer.unjournalled =
                    Some("the archive it replaced is gone for good".to_owned());
            }
        }
        self.report.created.push(location);
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
    let mut run = Edit {
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
    let Some(edit) = &plan.archive_edit else {
        return Err(run.failure(
            Stop::failed(
                OpsError::Unsupported {
                    what: "an archive change without its plan".to_owned(),
                },
                None,
            ),
            0,
        ));
    };
    match run.rewrite(edit) {
        Ok(()) => {
            run.report.progress = run.progress.clone();
            run.report.counts = run.counts;
            Ok(std::mem::take(&mut run.report))
        }
        Err(stop) => Err(run.failure(stop, 0)),
    }
}
