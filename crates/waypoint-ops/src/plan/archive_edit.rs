// Planning a change to an open archive: files added to it, an entry renamed or entries deleted
// (D170). Every one is a rewrite of the archive file, so the plan reads the archive's entries once,
// refuses what cannot be rewritten with a typed reason, applies the same size limits as an
// extraction and finds the names the added files clash with, before anything is written.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;

use waypoint_path::{CaseRule, VfsPath};
use waypoint_vfs::{validate_name, ArchiveEntryInfo, EntryKind, Writability};

use super::archive::check_limits;
use super::{
    check, conflict_kind, failure, ArchiveChange, ArchiveEditPlan, ArchiveUndo, Plan, PlanItem,
    Planner,
};
use crate::model::{ArchiveSpec, Conflict, ConflictPolicy, JobKind, OpsError};
use crate::names::{archive_path, file_name_of, name_bytes, unique_full_name};

/// The names inside an archive of an `archive:` path.
pub(crate) fn inner_names(path: &VfsPath) -> Option<(&VfsPath, &[Vec<u8>])> {
    match path {
        VfsPath::Archive(archive) => Some((archive.container(), archive.inner())),
        _ => None,
    }
}

/// Whether `path` is one of `ancestor` or lies below it, by their names inside the archive.
pub(crate) fn below(path: &[Vec<u8>], ancestor: &[Vec<u8>]) -> bool {
    path.len() >= ancestor.len() && path[..ancestor.len()] == *ancestor
}

impl Planner<'_, '_> {
    /// The change a request makes to an archive, when it makes one: a copy into one, a rename in
    /// one, a delete or a move to the Trash from one. `None` for every other request.
    pub(super) fn archive_edit(&mut self) -> Result<Option<Plan>, OpsError> {
        match self.request.kind {
            JobKind::Copy | JobKind::Move | JobKind::Link => {
                let Some(destination) = &self.request.destination else {
                    return Ok(None);
                };
                if !waypoint_path::ArchivePath::is_archive_uri(&destination.uri) {
                    return Ok(None);
                }
                if self.request.kind != JobKind::Copy {
                    return Err(OpsError::Unsupported {
                        what: "moving or linking into an archive; copy instead".to_owned(),
                    });
                }
                self.archive_add().map(Some)
            }
            JobKind::CreateFolder | JobKind::CreateFile => {
                let Some(destination) = &self.request.destination else {
                    return Ok(None);
                };
                if !waypoint_path::ArchivePath::is_archive_uri(&destination.uri) {
                    return Ok(None);
                }
                self.archive_make().map(Some)
            }
            JobKind::Rename | JobKind::Delete | JobKind::Trash => {
                let first_is_archive = match &self.request.sources {
                    crate::model::Sources::Locations { locations } => locations
                        .first()
                        .is_some_and(|l| waypoint_path::ArchivePath::is_archive_uri(&l.uri)),
                    crate::model::Sources::Selection { .. } => {
                        // The selection is resolved once here and again by the planner that
                        // takes the request when it is not an archive's.
                        let sources = self.sources()?;
                        self.warnings.clear();
                        sources
                            .first()
                            .is_some_and(|(path, _)| matches!(path, VfsPath::Archive(_)))
                    }
                };
                if !first_is_archive {
                    return Ok(None);
                }
                if self.request.kind == JobKind::Rename {
                    self.archive_rename().map(Some)
                } else {
                    self.archive_delete().map(Some)
                }
            }
            _ => Ok(None),
        }
    }

    /// What the archive holds and how it will be written, or why it will not be.
    fn archive_ground(
        &mut self,
        archive: &VfsPath,
    ) -> Result<(Vec<ArchiveEntryInfo>, ArchiveEditPlan), OpsError> {
        let VfsPath::Archive(top) = archive else {
            return Err(failure("this is not inside an archive"));
        };
        let container = top.container().clone();
        let top_path = archive_path(&container, &[]).map_err(|_| failure("not an archive"))?;
        let catalog = self.ctx.providers.catalog()?;
        let kind = match catalog.writability(&top_path, self.ctx.cancel)? {
            Writability::Writable(kind) => kind,
            Writability::Refused(reason) => {
                return Err(OpsError::ArchiveNotWritable {
                    location: container.to_location(),
                    reason: reason.into(),
                })
            }
        };
        let holder = self.ctx.providers.for_path(&container)?;
        let file = holder.stat(&container)?;
        let allow_large = matches!(
            self.request.archive,
            Some(ArchiveSpec::Edit { allow_large: true })
        );
        let walked = &mut self.walked;
        let progress = &mut self.progress;
        let entries = catalog.archive_entries(&top_path, self.ctx.cancel, &mut |n| {
            walked.items = u64::from(n);
            progress(walked);
        })?;
        check(self.ctx.cancel)?;
        check_limits(
            &entries,
            self.ctx.archive_limits,
            file.size,
            allow_large,
            &container.to_location(),
        )?;
        if entries.iter().any(|e| e.kind == EntryKind::Other) {
            return Err(OpsError::Unsupported {
                what: "an archive that holds a device or a pipe".to_owned(),
            });
        }
        let undo = match self.ctx.trash.available() {
            _ if !matches!(container, VfsPath::File(_)) => ArchiveUndo::Remote,
            Ok(()) => ArchiveUndo::Trash,
            Err(reason) => ArchiveUndo::Unavailable(reason),
        };
        let plan = ArchiveEditPlan {
            undo,
            container,
            kind,
            change: ArchiveChange::Delete { paths: Vec::new() },
            entries: Vec::new(),
            size: file.size.unwrap_or(0),
            modified_ms: file.modified_ms,
        };
        Ok((entries, plan))
    }

    fn finish_edit(
        &mut self,
        items: Vec<PlanItem>,
        conflicts: Vec<Conflict>,
        entries: Vec<ArchiveEntryInfo>,
        mut edit: ArchiveEditPlan,
        change: ArchiveChange,
        new: (u64, u64),
    ) -> Result<Plan, OpsError> {
        let holder = self.ctx.providers.for_path(&edit.container)?;
        let folder = edit.container.parent().ok_or_else(|| OpsError::Protected {
            location: edit.container.to_location(),
        })?;
        // The new file is written beside the old one before it replaces it.
        let kept_bytes: u64 = entries
            .iter()
            .filter(|e| e.kind == EntryKind::File)
            .map(|e| e.size.unwrap_or(0))
            .sum();
        Self::enough_space(holder.as_ref(), &folder, edit.size + new.1)?;
        let total_items = entries.iter().filter(|e| !e.synthetic).count() as u64 + new.0;
        edit.change = change;
        edit.entries = entries;
        let mut plan = self.finish(items, Some(folder), false, conflicts);
        plan.total_items = total_items;
        plan.total_bytes = kept_bytes + new.1;
        plan.archive_edit = Some(edit);
        Ok(plan)
    }

    fn archive_add(&mut self) -> Result<Plan, OpsError> {
        let location = self
            .request
            .destination
            .clone()
            .ok_or_else(|| failure("the request has no destination folder"))?;
        let (dest, _) = self.ctx.providers.for_location(&location)?;
        let (_, into) = inner_names(&dest)
            .map(|(c, i)| (c.clone(), i.to_vec()))
            .ok_or_else(|| failure("the destination is not inside an archive"))?;
        let (entries, edit) = self.archive_ground(&dest)?;
        let by_path: HashMap<Vec<Vec<u8>>, &ArchiveEntryInfo> = entries
            .iter()
            .filter_map(|e| inner_names(&e.path).map(|(_, inner)| (inner.to_vec(), e)))
            .collect();
        if !into.is_empty() {
            match by_path.get(&into) {
                Some(entry) if entry.kind == EntryKind::Directory => {}
                Some(_) => {
                    return Err(failure(format!("{} is not a folder", dest.display())));
                }
                None => {
                    return Err(OpsError::NotFound {
                        location: dest.to_location(),
                    })
                }
            }
        }
        let sources = self.sources()?;
        if sources.is_empty() {
            return Err(failure("there is nothing to add"));
        }
        let mut items = Vec::new();
        let mut conflicts = Vec::new();
        let mut claimed: HashSet<Vec<u8>> = HashSet::new();
        let mut new = (0u64, 0u64);
        let mut paths = Vec::new();
        for (source, provider) in sources {
            check(self.ctx.cancel)?;
            if source == edit.container {
                return Err(failure("an archive cannot be added to itself"));
            }
            let entry = provider.stat(&source)?;
            let name = file_name_of(&source).ok_or_else(|| OpsError::InvalidName {
                name: source.display(),
                reason: "a root has no name to add".to_owned(),
            })?;
            validate_name(&name, CaseRule::Sensitive)?;
            let (entries_n, bytes) = self.measure(provider.as_ref(), &source, &entry, false)?;
            new.0 += entries_n;
            new.1 += bytes;
            let mut comps = into.clone();
            comps.push(name_bytes(&name));
            let target = archive_path(&edit.container, &comps)
                .map_err(|_| failure("not a usable name in an archive"))?;
            let clash = by_path.get(&comps);
            let key = name_bytes(&name);
            if let Some(existing) = clash {
                conflicts.push(Conflict {
                    source: source.to_location(),
                    existing: target.to_location(),
                    name: name.to_string_lossy().into_owned(),
                    kind: conflict_kind(entry.kind, existing.kind),
                    within_batch: false,
                    source_size: entry.size,
                    existing_size: existing.size,
                    source_modified_ms: entry.modified_ms,
                    existing_modified_ms: existing.modified_ms,
                });
            } else if !claimed.insert(key) {
                conflicts.push(Conflict {
                    source: source.to_location(),
                    existing: target.to_location(),
                    name: name.to_string_lossy().into_owned(),
                    kind: conflict_kind(entry.kind, EntryKind::File),
                    within_batch: true,
                    source_size: entry.size,
                    existing_size: None,
                    source_modified_ms: entry.modified_ms,
                    existing_modified_ms: None,
                });
            }
            paths.push(source.clone());
            items.push(PlanItem {
                source: Some(source),
                target: Some(target),
                kind: entry.kind,
                size: entry.size,
                entries: entries_n,
                bytes,
                case_only: false,
            });
        }
        let change = ArchiveChange::Add {
            into,
            sources: paths,
        };
        self.finish_edit(items, conflicts, entries, edit, change, new)
    }

    fn archive_make(&mut self) -> Result<Plan, OpsError> {
        let location = self
            .request
            .destination
            .clone()
            .ok_or_else(|| failure("the request has no destination folder"))?;
        let (dest, _) = self.ctx.providers.for_location(&location)?;
        let (_, into) = inner_names(&dest)
            .map(|(c, i)| (c.clone(), i.to_vec()))
            .ok_or_else(|| failure("the destination is not inside an archive"))?;
        let (entries, edit) = self.archive_ground(&dest)?;
        let by_path: HashMap<Vec<Vec<u8>>, &ArchiveEntryInfo> = entries
            .iter()
            .filter_map(|e| inner_names(&e.path).map(|(_, inner)| (inner.to_vec(), e)))
            .collect();
        if !into.is_empty() && by_path.get(&into).map(|e| e.kind) != Some(EntryKind::Directory) {
            return Err(failure(format!("{} is not a folder", dest.display())));
        }
        let is_folder = self.request.kind == JobKind::CreateFolder;
        let fallback = if is_folder { "New folder" } else { "New file" };
        let wanted = self.request.name.as_deref().unwrap_or(fallback);
        validate_name(OsStr::new(wanted), CaseRule::Sensitive)?;
        let keep_both = self.request.name.is_none()
            || self.request.options.conflict == Some(ConflictPolicy::KeepBoth);
        let taken = |name: &str| {
            let mut comps = into.clone();
            comps.push(name.as_bytes().to_vec());
            by_path.contains_key(&comps)
        };
        let name = if keep_both {
            unique_full_name(&mut |n| taken(n), wanted)
        } else if taken(wanted) {
            let mut comps = into.clone();
            comps.push(wanted.as_bytes().to_vec());
            return Err(OpsError::NameInUse {
                location: archive_path(&edit.container, &comps)
                    .map_or_else(|_| dest.to_location(), |p| p.to_location()),
            });
        } else {
            wanted.to_owned()
        };
        let mut comps = into.clone();
        comps.push(name.clone().into_bytes());
        let target = archive_path(&edit.container, &comps)
            .map_err(|_| failure("not a usable name in an archive"))?;
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
        let change = ArchiveChange::Make {
            into,
            name: name.into_bytes(),
            folder: is_folder,
        };
        self.finish_edit(vec![item], Vec::new(), entries, edit, change, (1, 0))
    }

    fn archive_rename(&mut self) -> Result<Plan, OpsError> {
        let (source, _) = self.one_source()?;
        let (container, inner) = inner_names(&source)
            .map(|(c, i)| (c.clone(), i.to_vec()))
            .ok_or_else(|| failure("not an entry of an archive"))?;
        if inner.is_empty() {
            return Err(OpsError::Protected {
                location: source.to_location(),
            });
        }
        let name = self
            .request
            .name
            .clone()
            .ok_or_else(|| failure("a rename needs the new name"))?;
        validate_name(OsStr::new(&name), CaseRule::Sensitive)?;
        let (entries, edit) = self.archive_ground(&source)?;
        let by_path: HashMap<Vec<Vec<u8>>, &ArchiveEntryInfo> = entries
            .iter()
            .filter_map(|e| inner_names(&e.path).map(|(_, inner)| (inner.to_vec(), e)))
            .collect();
        let Some(entry) = by_path.get(&inner) else {
            return Err(OpsError::NotFound {
                location: source.to_location(),
            });
        };
        let new_name = name.clone().into_bytes();
        let mut target_comps = inner[..inner.len() - 1].to_vec();
        target_comps.push(new_name.clone());
        if inner.last() == Some(&new_name) {
            return Err(OpsError::SameFolder);
        }
        let target = archive_path(&container, &target_comps)
            .map_err(|_| failure("not a usable name in an archive"))?;
        if by_path.contains_key(&target_comps) {
            return Err(OpsError::NameInUse {
                location: target.to_location(),
            });
        }
        let item = PlanItem {
            source: Some(source.clone()),
            target: Some(target),
            kind: entry.kind,
            size: entry.size,
            entries: 1,
            bytes: 0,
            case_only: false,
        };
        let change = ArchiveChange::Rename {
            from: inner,
            name: new_name,
        };
        self.finish_edit(vec![item], Vec::new(), entries, edit, change, (0, 0))
    }

    fn archive_delete(&mut self) -> Result<Plan, OpsError> {
        let sources = self.sources()?;
        let Some((first, _)) = sources.first() else {
            return Err(failure("there is nothing to delete"));
        };
        let (container, _) = inner_names(first)
            .map(|(c, i)| (c.clone(), i.to_vec()))
            .ok_or_else(|| failure("not an entry of an archive"))?;
        let (entries, edit) = self.archive_ground(first)?;
        let by_path: HashMap<Vec<Vec<u8>>, &ArchiveEntryInfo> = entries
            .iter()
            .filter_map(|e| inner_names(&e.path).map(|(_, inner)| (inner.to_vec(), e)))
            .collect();
        let mut paths: Vec<Vec<Vec<u8>>> = Vec::new();
        let mut items = Vec::new();
        for (source, _) in &sources {
            check(self.ctx.cancel)?;
            let Some((same, inner)) = inner_names(source) else {
                return Err(failure(
                    "entries of an archive and other entries cannot be mixed",
                ));
            };
            if same != &container {
                return Err(failure(
                    "entries of two archives cannot be deleted together",
                ));
            }
            if inner.is_empty() {
                return Err(OpsError::Protected {
                    location: source.to_location(),
                });
            }
            let Some(entry) = by_path.get(inner) else {
                return Err(OpsError::NotFound {
                    location: source.to_location(),
                });
            };
            let count = entries
                .iter()
                .filter(|e| inner_names(&e.path).is_some_and(|(_, i)| below(i, inner)))
                .count() as u64;
            paths.push(inner.to_vec());
            items.push(PlanItem {
                source: Some(source.clone()),
                target: None,
                kind: entry.kind,
                size: entry.size,
                entries: count,
                bytes: 0,
                case_only: false,
            });
        }
        let change = ArchiveChange::Delete { paths };
        self.finish_edit(items, Vec::new(), entries, edit, change, (0, 0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_is_below_its_ancestors_and_itself() {
        let a = vec![b"a".to_vec()];
        let ab = vec![b"a".to_vec(), b"b".to_vec()];
        assert!(below(&ab, &a));
        assert!(below(&a, &a));
        assert!(!below(&a, &ab));
        assert!(below(&ab, &[]));
        assert!(!below(&[b"ab".to_vec()], &a));
    }

    #[test]
    fn names_make_os_names_back() {
        assert_eq!(crate::names::os_of(b"x.txt"), OsStr::new("x.txt"));
    }
}
