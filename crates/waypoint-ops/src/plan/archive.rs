// Planning an extraction and a compression. An extraction reads the archive's entry list once and
// decides, before anything is written, what is extracted, what is left out and why, whether the
// archive is within the limits, and which names it will clash with; a compression names the archive
// it will make and walks what goes into it.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::{HashMap, HashSet};
use std::ffi::{OsStr, OsString};
use std::sync::Arc;

use waypoint_path::{ArchivePath, VfsPath};
use waypoint_protocol::VfsError;
use waypoint_vfs::{
    child_path, validate_name, ArchiveEntryInfo, ArchiveKind, EntryKind, Provider, UnsafeName,
};

use super::{check, failure, ExtractPlan, Plan, PlanItem, PlanWarning, Planner};
use crate::model::{
    ArchiveFormat, ArchiveLimit, ArchiveSpec, Conflict, ConflictPolicy, ExtractLayout, JobKind,
    OpsError,
};
use crate::names::{file_name_of, fold_name, split_name, unique_full_name};

/// Symlinks whose targets are read from their data to decide whether they point outside; a link
/// past this many is left out unread.
const MAX_LINK_READS: usize = 1_000;

/// Why an entry is not extracted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeftOut {
    /// The name the archive stored could not be used as it was (see `UnsafeName`).
    Name(UnsafeName),
    /// A symlink that points outside the archive, so extracting it could be made to write or read
    /// through it.
    LinkOutside,
    /// A device, a pipe or another entry that is neither a file, a folder nor a link.
    Special,
}

/// An archive's file name without its archive extension, for the folder it is extracted into.
pub(crate) fn folder_name(archive: &VfsPath) -> String {
    let name = file_name_of(archive)
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let (stem, _) = split_name(&name);
    if stem.is_empty() {
        "Extracted".to_owned()
    } else {
        stem.to_owned()
    }
}

/// `name` with the archive's extension on the end, unless it already ends so.
pub(crate) fn with_extension(name: &str, format: ArchiveFormat) -> String {
    let ext = format.extension();
    if name.to_lowercase().ends_with(ext) {
        name.to_owned()
    } else {
        format!("{name}{ext}")
    }
}

/// Resolves the text a link holds against its folder, within the archive: `None` when it is absolute
/// or climbs out.
fn link_stays_inside(folder: &[Vec<u8>], target: &[u8]) -> bool {
    // A root, a backslash (a root or a separator on Windows) or a drive letter points outside.
    let drive = target.len() >= 2 && target[0].is_ascii_alphabetic() && target[1] == b':';
    if target.first() == Some(&b'/') || target.contains(&b'\\') || drive || target.contains(&0) {
        return false;
    }
    let mut depth = folder.len() as i64;
    for part in target.split(|&b| b == b'/') {
        match part {
            b"" | b"." => {}
            b".." => {
                depth -= 1;
                if depth < 0 {
                    return false;
                }
            }
            _ => depth += 1,
        }
    }
    true
}

/// What the walk of one archive's entries found.
struct Examined {
    skip: HashSet<VfsPath>,
    left_out: Vec<(VfsPath, LeftOut)>,
    /// Entries that will be extracted, below each top-level name: how many and how many bytes.
    below: HashMap<Vec<u8>, (u64, u64)>,
    top: Vec<ArchiveEntryInfo>,
    entries: u64,
    bytes: u64,
}

impl Planner<'_, '_> {
    /// Reads the entries of `archive`, enforces the limits and works out what is left out.
    fn examine(
        &mut self,
        archive: &VfsPath,
        archive_provider: &dyn Provider,
        file_size: Option<u64>,
        allow_large: bool,
    ) -> Result<Examined, OpsError> {
        let catalog = self.ctx.providers.catalog()?;
        let location =
            archive_container(archive).map_or_else(|| archive.to_location(), VfsPath::to_location);
        let walked = &mut self.walked;
        let progress = &mut self.progress;
        let entries = catalog
            .archive_entries(archive, self.ctx.cancel, &mut |n| {
                walked.items = u64::from(n);
                progress(walked);
            })
            .map_err(OpsError::from)?;
        check(self.ctx.cancel)?;
        let total: u64 = entries
            .iter()
            .filter(|e| e.kind == EntryKind::File)
            .map(|e| e.size.unwrap_or(0))
            .sum();
        let limits = self.ctx.archive_limits;
        if !allow_large {
            if entries.len() as u64 > limits.max_entries {
                return Err(OpsError::ArchiveLimit {
                    location,
                    limit: ArchiveLimit::Entries {
                        found: entries.len() as u64,
                        max: limits.max_entries,
                    },
                });
            }
            if total > limits.max_bytes {
                return Err(OpsError::ArchiveLimit {
                    location,
                    limit: ArchiveLimit::Bytes {
                        found: total,
                        max: limits.max_bytes,
                    },
                });
            }
            if let Some(stored) = file_size.filter(|stored| *stored > 0) {
                let ratio = total / stored;
                if total >= limits.ratio_floor_bytes && ratio > u64::from(limits.max_ratio) {
                    return Err(OpsError::ArchiveLimit {
                        location,
                        limit: ArchiveLimit::Ratio {
                            ratio,
                            max: u64::from(limits.max_ratio),
                        },
                    });
                }
            }
        }
        let mut examined = Examined {
            skip: HashSet::new(),
            left_out: Vec::new(),
            below: HashMap::new(),
            top: Vec::new(),
            entries: 0,
            bytes: 0,
        };
        let mut link_reads = 0usize;
        let mut kept: Vec<&ArchiveEntryInfo> = Vec::new();
        for entry in &entries {
            let reason = match entry.kind {
                _ if entry.unsafe_name.is_some() => entry.unsafe_name.map(LeftOut::Name),
                EntryKind::Other => Some(LeftOut::Special),
                EntryKind::Symlink => {
                    let text = match &entry.link_target {
                        Some(text) => Some(text.clone()),
                        None if link_reads < MAX_LINK_READS => {
                            link_reads += 1;
                            archive_provider
                                .read_link(&entry.path)
                                .ok()
                                .map(|t| crate::names::name_bytes(&t))
                        }
                        None => None,
                    };
                    let inner = inner_of(&entry.path);
                    let folder = &inner[..inner.len().saturating_sub(1)];
                    match text {
                        Some(text) if link_stays_inside(folder, &text) => None,
                        _ => Some(LeftOut::LinkOutside),
                    }
                }
                _ => None,
            };
            match reason {
                Some(reason) => {
                    examined.skip.insert(entry.path.clone());
                    examined.left_out.push((entry.path.clone(), reason));
                }
                None => kept.push(entry),
            }
        }
        // A folder the archive does not list exists only to hold what is below it: with nothing
        // below it extracted, it is not made either.
        let mut holds: HashSet<VfsPath> = HashSet::new();
        for entry in kept.iter().filter(|e| !e.synthetic) {
            let mut at = entry.path.parent();
            while let Some(parent) = at {
                if !matches!(&parent, VfsPath::Archive(a) if !a.is_root())
                    || !holds.insert(parent.clone())
                {
                    break;
                }
                at = parent.parent();
            }
        }
        for entry in kept {
            if entry.synthetic && !holds.contains(&entry.path) {
                examined.skip.insert(entry.path.clone());
                continue;
            }
            examined.entries += 1;
            if entry.kind == EntryKind::File {
                examined.bytes += entry.size.unwrap_or(0);
            }
            let inner = inner_of(&entry.path);
            let Some(first) = inner.first() else { continue };
            let slot = examined.below.entry(first.clone()).or_default();
            slot.0 += 1;
            if entry.kind == EntryKind::File {
                slot.1 += entry.size.unwrap_or(0);
            }
            if inner.len() == 1 {
                examined.top.push(entry.clone());
            }
        }
        Ok(examined)
    }

    pub(super) fn extract(&mut self) -> Result<Plan, OpsError> {
        let (layout, allow_large) = match self.request.archive {
            Some(ArchiveSpec::Extract {
                layout,
                allow_large,
            }) => (layout, allow_large),
            _ => (ExtractLayout::Auto, false),
        };
        let chosen_destination = match self.request.destination {
            Some(_) => Some(self.destination()?),
            None => None,
        };
        let sources = self.sources()?;
        let mut items: Vec<PlanItem> = Vec::new();
        let mut item_archives: Vec<VfsPath> = Vec::new();
        let mut skip: HashSet<VfsPath> = HashSet::new();
        let mut declared = 0u64;
        let mut targets: Vec<(usize, VfsPath, Arc<dyn Provider>)> = Vec::new();
        for (source, provider) in sources {
            check(self.ctx.cancel)?;
            let (archive, container) = match &source {
                VfsPath::Archive(top) if top.is_root() => (source.clone(), top.container().clone()),
                // A file inside another archive is an archive of its own (they nest).
                other => {
                    let top = ArchivePath::new(other.clone()).map_err(|_| {
                        failure(format!(
                            "{} cannot be opened as an archive",
                            other.display()
                        ))
                    })?;
                    (VfsPath::Archive(top), other.clone())
                }
            };
            let file = provider.stat(&source).or_else(|error| {
                // The top of an archive is a folder; its own file is the container's.
                match (&source, error) {
                    (VfsPath::Archive(_), _) => self
                        .ctx
                        .providers
                        .for_path(&container)
                        .map_err(|_| VfsError::Unsupported {
                            what: container.scheme().to_owned(),
                        })
                        .and_then(|p| p.stat(&container)),
                    (_, error) => Err(error),
                }
            })?;
            let file_size = match &source {
                VfsPath::Archive(top) if top.is_root() => self
                    .ctx
                    .providers
                    .for_path(&container)
                    .ok()
                    .and_then(|p| p.stat(&container).ok())
                    .and_then(|e| e.size),
                _ => file.size,
            };
            if file.kind == EntryKind::Directory
                && !matches!(&source, VfsPath::Archive(top) if top.is_root())
            {
                return Err(failure(format!(
                    "{} is a folder, not an archive",
                    source.display()
                )));
            }
            let archive_provider = self.ctx.providers.for_path(&archive)?;
            let examined =
                self.examine(&archive, archive_provider.as_ref(), file_size, allow_large)?;
            for (path, why) in &examined.left_out {
                self.warnings.push(PlanWarning::LeftOut {
                    location: path.to_location(),
                    why: *why,
                });
            }
            skip.extend(examined.skip.iter().cloned());
            declared += examined.bytes;

            let (folder, folder_provider) = match &chosen_destination {
                Some((path, provider)) => (path.clone(), provider.clone()),
                None => {
                    let parent = container.parent().ok_or_else(|| OpsError::Protected {
                        location: container.to_location(),
                    })?;
                    (parent.clone(), self.ctx.providers.for_path(&parent)?)
                }
            };
            if !folder_provider.capabilities().write || folder_provider.read_only() {
                return Err(OpsError::PermissionDenied {
                    location: folder.to_location(),
                });
            }
            let rule = folder_provider.capabilities().case_rule;
            if examined.top.is_empty() {
                self.warnings.push(PlanWarning::EmptyArchive {
                    location: container.to_location(),
                });
                continue;
            }
            let into_folder = match layout {
                ExtractLayout::Folder => true,
                ExtractLayout::Contents => false,
                ExtractLayout::Auto => examined.top.len() > 1,
            };
            let before = items.len();
            if into_folder {
                let name = folder_name(&container);
                validate_name(OsStr::new(&name), rule)?;
                let target = child_path(&folder, OsStr::new(&name), rule)?;
                let root = archive_provider.stat(&archive)?;
                items.push(PlanItem {
                    source: Some(archive.clone()),
                    target: Some(target),
                    kind: EntryKind::Directory,
                    size: root.size,
                    entries: examined.entries + 1,
                    bytes: examined.bytes,
                    case_only: false,
                });
            } else {
                for top in &examined.top {
                    let name = file_name_of(&top.path).unwrap_or_default();
                    validate_name(&name, rule)?;
                    let target = child_path(&folder, &name, rule)?;
                    let (entries, bytes) = inner_of(&top.path)
                        .first()
                        .and_then(|first| examined.below.get(first))
                        .copied()
                        .unwrap_or((1, 0));
                    items.push(PlanItem {
                        source: Some(top.path.clone()),
                        target: Some(target),
                        kind: top.kind,
                        size: top.size,
                        entries,
                        bytes,
                        case_only: false,
                    });
                }
            }
            for index in before..items.len() {
                item_archives.push(container.clone());
                targets.push((index, folder.clone(), folder_provider.clone()));
            }
        }

        // Clashes with what is already there (each destination folder is read once), and between
        // the items themselves.
        let mut existing: HashMap<VfsPath, HashMap<OsString, waypoint_vfs::ScannedEntry>> =
            HashMap::new();
        let mut claimed: HashMap<(VfsPath, OsString), VfsPath> = HashMap::new();
        let mut conflicts: Vec<Conflict> = Vec::new();
        for (index, folder, provider) in &targets {
            check(self.ctx.cancel)?;
            let rule = provider.capabilities().case_rule;
            if !existing.contains_key(folder) {
                let mut listed = HashMap::new();
                for entry in provider.list(folder, self.ctx.cancel, 0, &mut |_| {})? {
                    listed.insert(fold_name(&entry.name, rule), entry);
                }
                existing.insert(folder.clone(), listed);
            }
            let item = &items[*index];
            let target = item.target.clone().expect("an extraction has targets");
            let source = item.source.clone().expect("an extraction has sources");
            let name = file_name_of(&target).unwrap_or_default();
            let key = fold_name(&name, rule);
            let archive_provider = self.ctx.providers.for_path(&source)?;
            let entry = archive_provider.stat(&source)?;
            if let Some(clash) = existing[folder].get(&key) {
                conflicts.push(Self::conflict(&source, &entry, &target, clash, false));
            } else if let Some(first) = claimed.get(&(folder.clone(), key.clone())) {
                conflicts.push(Conflict {
                    source: source.to_location(),
                    existing: first.to_location(),
                    name: name.to_string_lossy().into_owned(),
                    kind: Self::kind_of(&entry, None),
                    within_batch: true,
                    source_size: entry.size,
                    existing_size: None,
                    source_modified_ms: entry.modified_ms,
                    existing_modified_ms: None,
                });
            } else {
                claimed.insert((folder.clone(), key), target);
            }
        }
        let mut checked: HashSet<VfsPath> = HashSet::new();
        for (_, folder, provider) in &targets {
            if checked.insert(folder.clone()) {
                let share: u64 = targets
                    .iter()
                    .filter(|(_, f, _)| f == folder)
                    .map(|(i, _, _)| items[*i].bytes)
                    .sum();
                Self::enough_space(provider.as_ref(), folder, share)?;
            }
        }
        let destination = targets.first().map(|(_, folder, _)| folder.clone());
        let mut plan = self.finish(items, destination, false, conflicts);
        plan.extract = Some(ExtractPlan {
            skip,
            declared_bytes: declared,
            item_archives,
        });
        Ok(plan)
    }

    pub(super) fn compress(&mut self) -> Result<Plan, OpsError> {
        let format = match self.request.archive {
            Some(ArchiveSpec::Compress { format }) => format,
            _ => ArchiveFormat::Zip,
        };
        let kind = ArchiveKind::from(format);
        if !self.ctx.providers.writers()?.kinds().contains(&kind) {
            return Err(OpsError::Unsupported {
                what: format!("making {} archives", format.extension()),
            });
        }
        let (folder, provider) = self.destination()?;
        if !provider.capabilities().write || provider.read_only() {
            return Err(OpsError::PermissionDenied {
                location: folder.to_location(),
            });
        }
        let rule = provider.capabilities().case_rule;
        let sources = self.sources()?;
        if sources.is_empty() {
            return Err(failure("there is nothing to compress"));
        }
        let wanted = match self.request.name.as_deref() {
            Some(name) => name.to_owned(),
            None if sources.len() == 1 => file_name_of(&sources[0].0)
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "Archive".to_owned()),
            None => "Archive".to_owned(),
        };
        let wanted = with_extension(&wanted, format);
        validate_name(OsStr::new(&wanted), rule)?;
        let mut failed = None;
        let keep_both = self.request.options.conflict == Some(ConflictPolicy::KeepBoth);
        let taken = Self::probe(provider.as_ref(), &folder, &wanted, rule, &mut failed);
        if let Some(error) = failed.take() {
            return Err(error);
        }
        let mut name = wanted.clone();
        let mut conflicts = Vec::new();
        if taken && keep_both {
            name = unique_full_name(
                &mut |n| Self::probe(provider.as_ref(), &folder, n, rule, &mut failed),
                &wanted,
            );
            if let Some(error) = failed.take() {
                return Err(error);
            }
        }
        let target = child_path(&folder, OsStr::new(&name), rule)?;
        let mut items = Vec::new();
        let mut first: Option<(VfsPath, waypoint_vfs::ScannedEntry)> = None;
        for (source, source_provider) in sources {
            check(self.ctx.cancel)?;
            let entry = source_provider.stat(&source)?;
            if crate::names::same_path(&source, &target, rule) {
                continue;
            }
            let (entries, bytes) =
                self.measure(source_provider.as_ref(), &source, &entry, false)?;
            if first.is_none() {
                first = Some((source.clone(), entry.clone()));
            }
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
        let Some((first_source, first_entry)) = first else {
            return Err(failure("there is nothing to compress"));
        };
        if taken && !keep_both {
            let existing = provider.stat(&target)?;
            conflicts.push(Self::conflict(
                &first_source,
                &waypoint_vfs::ScannedEntry {
                    kind: EntryKind::File,
                    ..first_entry
                },
                &target,
                &existing,
                false,
            ));
        }
        let mut plan = self.finish(items, Some(folder), false, conflicts);
        plan.kind = JobKind::Compress;
        plan.compress = Some(super::CompressPlan { kind, target });
        Ok(plan)
    }
}

/// The names inside the archive of an `archive:` path.
fn inner_of(path: &VfsPath) -> Vec<Vec<u8>> {
    match path {
        VfsPath::Archive(archive) => archive.inner().to_vec(),
        _ => Vec::new(),
    }
}

fn archive_container(path: &VfsPath) -> Option<&VfsPath> {
    match path {
        VfsPath::Archive(archive) => Some(archive.container()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_folder_is_named_after_the_archive_without_its_extensions() {
        let path = |name: &str| {
            VfsPath::parse_input(&format!(
                "{}{name}",
                if cfg!(windows) { r"C:\" } else { "/" }
            ))
            .unwrap()
        };
        assert_eq!(folder_name(&path("photos.zip")), "photos");
        assert_eq!(folder_name(&path("src.tar.gz")), "src");
        assert_eq!(folder_name(&path("a.b.7z")), "a.b");
        assert_eq!(folder_name(&path("noext")), "noext");
    }

    #[test]
    fn the_extension_is_added_once() {
        assert_eq!(with_extension("notes", ArchiveFormat::Zip), "notes.zip");
        assert_eq!(with_extension("notes.zip", ArchiveFormat::Zip), "notes.zip");
        assert_eq!(with_extension("notes.ZIP", ArchiveFormat::Zip), "notes.ZIP");
        assert_eq!(with_extension("src", ArchiveFormat::TarGz), "src.tar.gz");
    }

    #[test]
    fn a_link_may_not_climb_out_of_the_archive() {
        let folder = vec![b"a".to_vec(), b"b".to_vec()];
        assert!(link_stays_inside(&folder, b"x"));
        assert!(link_stays_inside(&folder, b"../x"));
        assert!(link_stays_inside(&folder, b"../../x"));
        assert!(!link_stays_inside(&folder, b"../../../x"));
        assert!(!link_stays_inside(&folder, b"/etc/passwd"));
        assert!(!link_stays_inside(&folder, b"C:\\Windows"));
        assert!(!link_stays_inside(&folder, b"..\\..\\x"));
        assert!(!link_stays_inside(&folder, b"c:x"));
        assert!(link_stays_inside(&[], b"a/../b"));
        assert!(!link_stays_inside(&[], b".."));
    }
}
