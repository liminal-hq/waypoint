// The planner: turns a request into a plan before anything is written (A48). It resolves the
// sources, refuses what can never be right, finds name clashes at the top level of the destination
// and walks folders for their counts, and it only ever reads.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::{HashMap, HashSet};
use std::ffi::{OsStr, OsString};
use std::sync::Arc;

use waypoint_path::{CaseRule, VfsPath};
use waypoint_protocol::{Location, VfsError};
use waypoint_vfs::{child_path, validate_name, CancelToken, EntryKind, Provider, ScannedEntry};

use crate::model::{
    Conflict, ConflictKind, ConflictPolicy, JobKind, JobRequest, OpsError, PlanTotals, Sources,
    SourcesSummary,
};
use crate::names::{file_name_of, fold_name, is_within, same_name, same_path, unique_full_name};
use crate::traits::{Protected, Providers, SelectionResolver, Trash};

/// What the planner needs from the world.
pub struct PlanCtx<'a> {
    pub providers: &'a Providers,
    pub resolver: &'a dyn SelectionResolver,
    pub trash: &'a dyn Trash,
    pub protected: &'a Protected,
    pub cancel: &'a CancelToken,
}

/// How far a walk has got, reported as it goes.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PlanProgress {
    pub items: u64,
    pub bytes: u64,
    pub current: Option<String>,
}

/// One thing the job does to one entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanItem {
    /// The entry acted on: `None` for a create, which has no source.
    pub source: Option<VfsPath>,
    /// Where the result goes: the new entry of a create, the new name of a rename or duplicate,
    /// the place a copy or move puts the entry. `None` for trash, restore and delete.
    pub target: Option<VfsPath>,
    /// What the source is (what the target will be, for a create).
    pub kind: EntryKind,
    /// The source's own size, for a file.
    pub size: Option<u64>,
    /// The source and everything below it.
    pub entries: u64,
    /// The bytes of the files at and below the source.
    pub bytes: u64,
    /// A rename that only changes case, on a provider that treats such names as the same.
    pub case_only: bool,
}

/// Something the user may want to know about a plan that is not a reason to refuse it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanWarning {
    /// The same entry was named twice; it is done once.
    DuplicateSource { location: Location },
    /// The entry is already in the destination folder, so it was left out.
    AlreadyThere { location: Location },
}

/// Everything known before the first write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    pub kind: JobKind,
    pub items: Vec<PlanItem>,
    /// The folder the job writes into (the destination, or the folder a create is in).
    pub destination: Option<VfsPath>,
    /// Entries at every depth, including the top-level ones.
    pub total_items: u64,
    pub total_bytes: u64,
    /// Every source is on the destination's volume, known for certain. When the provider cannot
    /// say, this is false and the engine copies instead of renaming.
    pub same_volume: bool,
    /// Names already taken in the destination, or wanted twice by the request.
    pub conflicts: Vec<Conflict>,
    pub warnings: Vec<PlanWarning>,
}

impl Plan {
    /// What the queue records about the job once planning is done.
    pub fn totals(&self) -> PlanTotals {
        let mut touches: Vec<Location> = Vec::new();
        let mut note = |path: Option<VfsPath>| {
            if let Some(location) = path.map(|p| p.to_location()) {
                if !touches.contains(&location) {
                    touches.push(location);
                }
            }
        };
        note(self.destination.clone());
        for item in &self.items {
            note(item.source.as_ref().and_then(VfsPath::parent));
            note(item.target.as_ref().and_then(VfsPath::parent));
        }
        let trees: Vec<Location> = if matches!(
            self.kind,
            JobKind::Delete | JobKind::Trash | JobKind::Move | JobKind::Rename
        ) {
            self.items
                .iter()
                .filter_map(|i| i.source.as_ref().map(VfsPath::to_location))
                .collect()
        } else {
            Vec::new()
        };
        let first = self
            .items
            .first()
            .and_then(|i| i.source.as_ref().or(i.target.as_ref()))
            .and_then(file_name_of)
            .map(|n| n.to_string_lossy().into_owned());
        let sources = self.items.iter().filter(|i| i.source.is_some()).count() as u64;
        PlanTotals {
            sources: SourcesSummary {
                count: Some(sources),
                first,
            },
            items: self.total_items,
            bytes: self.total_bytes,
            touches,
            trees,
        }
    }
}

type Resolved = Vec<(VfsPath, Arc<dyn Provider>)>;

/// The path to ask for the volume an entry is on. A symlink is on the volume of the folder that
/// holds it, not of what it points at (which `volume_id` would follow, and which may be elsewhere
/// or nowhere), so a link is asked about by its folder.
pub(crate) fn volume_probe(path: &VfsPath, entry: &ScannedEntry) -> VfsPath {
    if entry.kind == EntryKind::Symlink {
        path.parent().unwrap_or_else(|| path.clone())
    } else {
        path.clone()
    }
}

fn failure(message: impl Into<String>) -> OpsError {
    OpsError::Io {
        message: message.into(),
    }
}

/// Whether copying or moving the folder `source` into the folder `dest` would put it inside
/// itself. The lexical comparison catches the plain case. A destination reached through a symlinked
/// ancestor (`/link/x` where `/link` is `/a`, copying `/a`) only shows once both ends have their
/// links resolved, so both are canonicalised and compared again; a provider that cannot resolve
/// links (`Unsupported`) or fails to is left with the lexical answer.
fn lands_inside(
    source_provider: &dyn Provider,
    dest_provider: &dyn Provider,
    source: &VfsPath,
    dest: &VfsPath,
    rule: CaseRule,
) -> bool {
    if is_within(dest, source, rule) {
        return true;
    }
    match (
        source_provider.canonicalize(source),
        dest_provider.canonicalize(dest),
    ) {
        (Ok(source), Ok(dest)) => is_within(&dest, &source, rule),
        _ => false,
    }
}

fn check(cancel: &CancelToken) -> Result<(), OpsError> {
    if cancel.is_cancelled() {
        Err(OpsError::Cancelled)
    } else {
        Ok(())
    }
}

/// Plans a request, reading only.
pub fn plan(request: &JobRequest, ctx: &PlanCtx<'_>) -> Result<Plan, OpsError> {
    plan_with_progress(request, ctx, &mut |_| {})
}

/// Plans a request and reports the walk's progress as it goes.
pub fn plan_with_progress(
    request: &JobRequest,
    ctx: &PlanCtx<'_>,
    progress: &mut dyn FnMut(&PlanProgress),
) -> Result<Plan, OpsError> {
    check(ctx.cancel)?;
    let mut planner = Planner {
        request,
        ctx,
        progress,
        walked: PlanProgress::default(),
        warnings: Vec::new(),
    };
    match request.kind {
        JobKind::CreateFolder | JobKind::CreateFile => planner.create(),
        JobKind::Rename => planner.rename(),
        JobKind::Duplicate => planner.duplicate(),
        JobKind::Trash => planner.trash(),
        JobKind::Restore => planner.restore(),
        JobKind::Delete => planner.delete(),
        JobKind::Copy | JobKind::Move => planner.transfer(),
        JobKind::BatchRename => Err(OpsError::Unsupported {
            what: "renaming in a batch".to_owned(),
        }),
        JobKind::Undo { .. } | JobKind::Redo { .. } => Err(OpsError::Unsupported {
            what: "undo and redo".to_owned(),
        }),
    }
}

struct Planner<'a, 'p> {
    request: &'a JobRequest,
    ctx: &'a PlanCtx<'a>,
    progress: &'p mut dyn FnMut(&PlanProgress),
    walked: PlanProgress,
    warnings: Vec<PlanWarning>,
}

impl Planner<'_, '_> {
    fn sources(&mut self) -> Result<Resolved, OpsError> {
        let locations = match &self.request.sources {
            Sources::Locations { locations } => locations.clone(),
            Sources::Selection { handle, spec } => {
                self.ctx
                    .resolver
                    .resolve(*handle, spec, &self.request.origin_window)?
            }
        };
        let mut out: Resolved = Vec::new();
        for location in &locations {
            check(self.ctx.cancel)?;
            let (path, provider) = self.ctx.providers.for_location(location)?;
            let rule = provider.capabilities().case_rule;
            if out.iter().any(|(seen, _)| same_path(seen, &path, rule)) {
                self.warnings.push(PlanWarning::DuplicateSource {
                    location: location.clone(),
                });
                continue;
            }
            out.push((path, provider));
        }
        Ok(out)
    }

    fn destination(&self) -> Result<(VfsPath, Arc<dyn Provider>), OpsError> {
        let location = self
            .request
            .destination
            .as_ref()
            .ok_or_else(|| failure("the request has no destination folder"))?;
        let (path, provider) = self.ctx.providers.for_location(location)?;
        let entry = provider.stat(&path)?;
        if entry.kind != EntryKind::Directory {
            return Err(failure(format!("{} is not a folder", path.display())));
        }
        Ok((path, provider))
    }

    fn finish(
        &mut self,
        items: Vec<PlanItem>,
        destination: Option<VfsPath>,
        same_volume: bool,
        conflicts: Vec<Conflict>,
    ) -> Plan {
        Plan {
            kind: self.request.kind,
            total_items: items.iter().map(|i| i.entries).sum(),
            total_bytes: items.iter().map(|i| i.bytes).sum(),
            items,
            destination,
            same_volume,
            conflicts,
            warnings: std::mem::take(&mut self.warnings),
        }
    }

    fn one_source(&mut self) -> Result<(VfsPath, Arc<dyn Provider>), OpsError> {
        let mut sources = self.sources()?;
        if sources.len() != 1 {
            return Err(failure("this operation takes exactly one entry"));
        }
        Ok(sources.remove(0))
    }

    /// Whether `name` is taken in `folder`, asked of the provider one name at a time (cheap for
    /// the single names of a create, a rename and a duplicate).
    fn probe(
        provider: &dyn Provider,
        folder: &VfsPath,
        name: &str,
        rule: CaseRule,
        failed: &mut Option<OpsError>,
    ) -> bool {
        // After an error every name counts as free, so a search for a free name ends (and the
        // caller reports the error) instead of looping on a provider that keeps failing.
        if failed.is_some() {
            return false;
        }
        let path = match child_path(folder, OsStr::new(name), rule) {
            Ok(path) => path,
            Err(error) => {
                failed.get_or_insert(error.into());
                return true;
            }
        };
        match provider.stat(&path) {
            Ok(_) => true,
            Err(VfsError::NotFound { .. } | VfsError::NotADirectory { .. }) => false,
            Err(error) => {
                failed.get_or_insert(error.into());
                true
            }
        }
    }

    fn create(&mut self) -> Result<Plan, OpsError> {
        let (folder, provider) = self.destination()?;
        let rule = provider.capabilities().case_rule;
        let is_folder = self.request.kind == JobKind::CreateFolder;
        let fallback = if is_folder { "New folder" } else { "New file" };
        let wanted = self.request.name.as_deref().unwrap_or(fallback);
        validate_name(OsStr::new(wanted), rule)?;
        let keep_both = self.request.name.is_none()
            || self.request.options.conflict == Some(ConflictPolicy::KeepBoth);
        let mut failed = None;
        let name = if keep_both {
            let name = unique_full_name(
                &mut |n| Self::probe(provider.as_ref(), &folder, n, rule, &mut failed),
                wanted,
            );
            if let Some(error) = failed {
                return Err(error);
            }
            name
        } else {
            if Self::probe(provider.as_ref(), &folder, wanted, rule, &mut failed) {
                return Err(failed.unwrap_or_else(|| OpsError::NameInUse {
                    location: folder
                        .join(wanted)
                        .map_or_else(|_| folder.to_location(), |p| p.to_location()),
                }));
            }
            wanted.to_owned()
        };
        let target = child_path(&folder, OsStr::new(&name), rule)?;
        let item = PlanItem {
            source: None,
            target: Some(target),
            kind: if is_folder {
                EntryKind::Directory
            } else {
                EntryKind::File
            },
            size: None,
            entries: 1,
            bytes: 0,
            case_only: false,
        };
        Ok(self.finish(vec![item], Some(folder), true, Vec::new()))
    }

    fn rename(&mut self) -> Result<Plan, OpsError> {
        let (source, provider) = self.one_source()?;
        let rule = provider.capabilities().case_rule;
        let entry = provider.stat(&source)?;
        let name = self
            .request
            .name
            .as_deref()
            .ok_or_else(|| failure("a rename needs the new name"))?;
        validate_name(OsStr::new(name), rule)?;
        let folder = source.parent().ok_or_else(|| OpsError::Protected {
            location: source.to_location(),
        })?;
        let old = file_name_of(&source).unwrap_or_default();
        if old == OsStr::new(name) {
            return Err(OpsError::SameFolder);
        }
        let target = child_path(&folder, OsStr::new(name), rule)?;
        let case_only = same_name(&old, OsStr::new(name), rule);
        let mut failed = None;
        let mut target = target;
        if !case_only {
            let taken = Self::probe(provider.as_ref(), &folder, name, rule, &mut failed);
            if taken && self.request.options.conflict == Some(ConflictPolicy::KeepBoth) {
                let unique = unique_full_name(
                    &mut |n| Self::probe(provider.as_ref(), &folder, n, rule, &mut failed),
                    name,
                );
                target = child_path(&folder, OsStr::new(&unique), rule)?;
            } else if taken {
                return Err(failed.unwrap_or(OpsError::NameInUse {
                    location: target.to_location(),
                }));
            }
            if let Some(error) = failed {
                return Err(error);
            }
        }
        let item = PlanItem {
            source: Some(source),
            target: Some(target),
            kind: entry.kind,
            size: entry.size,
            entries: 1,
            bytes: 0,
            case_only,
        };
        Ok(self.finish(vec![item], Some(folder), true, Vec::new()))
    }

    fn duplicate(&mut self) -> Result<Plan, OpsError> {
        let sources = self.sources()?;
        let mut items = Vec::new();
        let mut handed_out: HashMap<VfsPath, HashSet<OsString>> = HashMap::new();
        let mut same_volume = true;
        for (source, provider) in sources {
            check(self.ctx.cancel)?;
            let rule = provider.capabilities().case_rule;
            let entry = provider.stat(&source)?;
            let folder = source.parent().ok_or_else(|| OpsError::Protected {
                location: source.to_location(),
            })?;
            let old = file_name_of(&source).unwrap_or_default();
            let used = handed_out.entry(folder.clone()).or_default();
            let mut failed = None;
            let name = unique_full_name(
                &mut |n| {
                    same_name(OsStr::new(n), &old, rule)
                        || used.contains(&fold_name(OsStr::new(n), rule))
                        || Self::probe(provider.as_ref(), &folder, n, rule, &mut failed)
                },
                &old.to_string_lossy(),
            );
            if let Some(error) = failed {
                return Err(error);
            }
            used.insert(fold_name(OsStr::new(&name), rule));
            let target = child_path(&folder, OsStr::new(&name), rule)?;
            let (entries, bytes) = self.measure(provider.as_ref(), &source, &entry, false)?;
            let free = Self::enough_space(provider.as_ref(), &folder, bytes);
            free?;
            same_volume &= provider.volume_id(&volume_probe(&source, &entry)).is_some();
            items.push(PlanItem {
                source: Some(source),
                target: Some(target),
                kind: entry.kind,
                size: entry.size,
                entries,
                bytes,
                case_only: false,
            });
        }
        Ok(self.finish(items, None, same_volume, Vec::new()))
    }

    fn trash(&mut self) -> Result<Plan, OpsError> {
        self.ctx
            .trash
            .available()
            .map_err(|reason| OpsError::TrashUnavailable { reason })?;
        let sources = self.sources()?;
        let mut items = Vec::new();
        for (source, provider) in sources {
            check(self.ctx.cancel)?;
            let rule = provider.capabilities().case_rule;
            if self.ctx.protected.contains(&source, rule) {
                return Err(OpsError::Protected {
                    location: source.to_location(),
                });
            }
            let entry = provider.stat(&source)?;
            items.push(PlanItem {
                source: Some(source),
                target: None,
                kind: entry.kind,
                size: entry.size,
                entries: 1,
                bytes: 0,
                case_only: false,
            });
        }
        Ok(self.finish(items, None, false, Vec::new()))
    }

    fn restore(&mut self) -> Result<Plan, OpsError> {
        self.ctx
            .trash
            .available()
            .map_err(|reason| OpsError::TrashUnavailable { reason })?;
        let sources = self.sources()?;
        let items = sources
            .into_iter()
            .map(|(source, _)| PlanItem {
                source: Some(source),
                target: None,
                kind: EntryKind::Other,
                size: None,
                entries: 1,
                bytes: 0,
                case_only: false,
            })
            .collect();
        Ok(self.finish(items, None, false, Vec::new()))
    }

    fn delete(&mut self) -> Result<Plan, OpsError> {
        let sources = self.sources()?;
        let mut items = Vec::new();
        for (source, provider) in sources {
            check(self.ctx.cancel)?;
            let rule = provider.capabilities().case_rule;
            if self.ctx.protected.contains(&source, rule) {
                return Err(OpsError::Protected {
                    location: source.to_location(),
                });
            }
            let entry = provider.stat(&source)?;
            let (entries, bytes) = self.measure(provider.as_ref(), &source, &entry, true)?;
            items.push(PlanItem {
                source: Some(source),
                target: None,
                kind: entry.kind,
                size: entry.size,
                entries,
                bytes,
                case_only: false,
            });
        }
        Ok(self.finish(items, None, false, Vec::new()))
    }

    fn transfer(&mut self) -> Result<Plan, OpsError> {
        let moving = self.request.kind == JobKind::Move;
        let (dest, dest_provider) = self.destination()?;
        let rule = dest_provider.capabilities().case_rule;
        let sources = self.sources()?;
        let keep_both = self.request.options.conflict == Some(ConflictPolicy::KeepBoth);

        // Refusals first: each is a read, and none leaves anything behind.
        let mut kept: Vec<(VfsPath, Arc<dyn Provider>, ScannedEntry)> = Vec::new();
        for (source, provider) in sources {
            check(self.ctx.cancel)?;
            let src_rule = provider.capabilities().case_rule;
            if moving && self.ctx.protected.contains(&source, src_rule) {
                return Err(OpsError::Protected {
                    location: source.to_location(),
                });
            }
            let entry = provider.stat(&source)?;
            let name = file_name_of(&source).ok_or_else(|| OpsError::InvalidName {
                name: source.display(),
                reason: "a root has no name to copy".to_owned(),
            })?;
            validate_name(&name, rule)?;
            if entry.kind == EntryKind::Directory
                && provider.scheme() == dest_provider.scheme()
                && lands_inside(
                    provider.as_ref(),
                    dest_provider.as_ref(),
                    &source,
                    &dest,
                    src_rule,
                )
            {
                return Err(OpsError::IntoItself);
            }
            let in_place = source
                .parent()
                .is_some_and(|parent| same_path(&parent, &dest, rule));
            if in_place && !(keep_both && !moving) {
                self.warnings.push(PlanWarning::AlreadyThere {
                    location: source.to_location(),
                });
                continue;
            }
            kept.push((source, provider, entry));
        }
        if kept.is_empty() && !self.warnings.is_empty() {
            return Err(OpsError::SameFolder);
        }

        // The destination's top level, read once.
        let mut existing: HashMap<OsString, ScannedEntry> = HashMap::new();
        if !kept.is_empty() {
            let mut counted = 0u32;
            for entry in dest_provider.list(&dest, self.ctx.cancel, 0, &mut |n| counted = n)? {
                existing.insert(fold_name(&entry.name, rule), entry);
            }
        }

        let mut items = Vec::new();
        let mut conflicts = Vec::new();
        let mut claimed: HashMap<OsString, VfsPath> = HashMap::new();
        let mut same_volume = true;
        let dest_volume = dest_provider.volume_id(&dest);
        for (source, provider, entry) in kept {
            check(self.ctx.cancel)?;
            let name = file_name_of(&source).unwrap_or_default();
            let key = fold_name(&name, rule);
            let mut target = child_path(&dest, &name, rule)?;
            let clash = existing.get(&key);
            let in_place = source
                .parent()
                .is_some_and(|parent| same_path(&parent, &dest, rule));
            if in_place {
                // A copy of an entry into its own folder with `KeepBoth`: it gets a free name.
                let unique = unique_full_name(
                    &mut |n| {
                        let k = fold_name(OsStr::new(n), rule);
                        existing.contains_key(&k) || claimed.contains_key(&k)
                    },
                    &name.to_string_lossy(),
                );
                target = child_path(&dest, OsStr::new(&unique), rule)?;
                claimed.insert(fold_name(OsStr::new(&unique), rule), target.clone());
            } else if let Some(clash) = clash {
                conflicts.push(Self::conflict(&source, &entry, &target, clash, false));
            } else if let Some(first) = claimed.get(&key) {
                let first_target = first.clone();
                conflicts.push(Conflict {
                    source: source.to_location(),
                    existing: first_target.to_location(),
                    name: name.to_string_lossy().into_owned(),
                    kind: Self::kind_of(&entry, None),
                    within_batch: true,
                    source_size: entry.size,
                    existing_size: None,
                    source_modified_ms: entry.modified_ms,
                    existing_modified_ms: None,
                });
            } else {
                claimed.insert(key, target.clone());
            }
            let known_same = match (
                provider.volume_id(&volume_probe(&source, &entry)),
                dest_volume,
            ) {
                (Some(a), Some(b)) => provider.scheme() == dest_provider.scheme() && a == b,
                _ => false,
            };
            same_volume &= known_same;
            let (entries, bytes) = self.measure(provider.as_ref(), &source, &entry, false)?;
            items.push(PlanItem {
                source: Some(source),
                target: Some(target),
                kind: entry.kind,
                size: entry.size,
                entries,
                bytes,
                case_only: false,
            });
        }
        let total_bytes: u64 = items.iter().map(|i| i.bytes).sum();
        if !(moving && same_volume) {
            Self::enough_space(dest_provider.as_ref(), &dest, total_bytes)?;
        }
        Ok(self.finish(items, Some(dest), same_volume, conflicts))
    }

    fn kind_of(source: &ScannedEntry, existing: Option<&ScannedEntry>) -> ConflictKind {
        let src_dir = source.kind == EntryKind::Directory;
        let dst_dir = existing.is_some_and(|e| e.kind == EntryKind::Directory);
        match (src_dir, dst_dir) {
            (false, false) => ConflictKind::FileOverFile,
            (true, true) => ConflictKind::FolderOverFolder,
            (false, true) => ConflictKind::FileOverFolder,
            (true, false) => ConflictKind::FolderOverFile,
        }
    }

    fn conflict(
        source: &VfsPath,
        entry: &ScannedEntry,
        target: &VfsPath,
        existing: &ScannedEntry,
        within_batch: bool,
    ) -> Conflict {
        Conflict {
            source: source.to_location(),
            existing: target.to_location(),
            name: existing.name.to_string_lossy().into_owned(),
            kind: Self::kind_of(entry, Some(existing)),
            within_batch,
            source_size: entry.size,
            existing_size: existing.size,
            source_modified_ms: entry.modified_ms,
            existing_modified_ms: existing.modified_ms,
        }
    }

    /// Fails when the volume holding `folder` has less room than `bytes`.
    fn enough_space(provider: &dyn Provider, folder: &VfsPath, bytes: u64) -> Result<(), OpsError> {
        match provider.free_space(folder) {
            Some(space) if bytes > space.free_bytes => Err(OpsError::NotEnoughSpace {
                needed: bytes,
                free: space.free_bytes,
            }),
            _ => Ok(()),
        }
    }

    /// The entries and bytes at and below `root`. Folders are walked and links are counted but
    /// never followed. With `one_volume`, a folder on another volume than `root` (a mount point)
    /// is refused as protected, so a permanent delete is refused before it removes anything.
    fn measure(
        &mut self,
        provider: &dyn Provider,
        root: &VfsPath,
        entry: &ScannedEntry,
        one_volume: bool,
    ) -> Result<(u64, u64), OpsError> {
        let mut entries = 1u64;
        let mut bytes = 0u64;
        self.walked.items += 1;
        self.walked.current = file_name_of(root).map(|n| n.to_string_lossy().into_owned());
        match entry.kind {
            EntryKind::File => {
                bytes = entry.size.unwrap_or(0);
                self.walked.bytes += bytes;
            }
            EntryKind::Directory => {
                let volume = one_volume.then(|| provider.volume_id(root)).flatten();
                let mut stack = vec![root.clone()];
                while let Some(folder) = stack.pop() {
                    check(self.ctx.cancel)?;
                    if volume.is_some() && provider.volume_id(&folder) != volume {
                        return Err(OpsError::Protected {
                            location: folder.to_location(),
                        });
                    }
                    let children = provider.list(&folder, self.ctx.cancel, 0, &mut |_| {})?;
                    for child in children {
                        entries += 1;
                        self.walked.items += 1;
                        match child.kind {
                            EntryKind::Directory => {
                                let path = folder.join(&child.name).map_err(|_| {
                                    failure(format!("{:?} is not a usable name", child.name))
                                })?;
                                stack.push(path);
                            }
                            EntryKind::File => {
                                let size = child.size.unwrap_or(0);
                                bytes += size;
                                self.walked.bytes += size;
                            }
                            EntryKind::Symlink | EntryKind::Other => {}
                        }
                        if self.walked.items.is_multiple_of(256) {
                            check(self.ctx.cancel)?;
                            (self.progress)(&self.walked);
                        }
                    }
                }
            }
            EntryKind::Symlink | EntryKind::Other => {}
        }
        (self.progress)(&self.walked);
        Ok((entries, bytes))
    }
}
