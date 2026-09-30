// The local provider: folders on this machine, read with `std::fs`.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::ffi::OsStr;
use std::fs::{self, FileType, Metadata};
use std::io;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use waypoint_path::{CaseRule, FilePath, VfsPath};
use waypoint_protocol::{Location, VfsError};

use crate::error::from_io;
use crate::icon::group_for;
use crate::model::EntryKind;
use crate::provider::{Capabilities, Provider, ScannedEntry, Watch, WatchSink};
use crate::watch::{self, WatchOptions};
use crate::CancelToken;

/// How often a scan reports how far it has got. The listing throttles what it forwards.
const PROGRESS_EVERY: u32 = 1024;

/// Serves `file://` paths from the local file system.
#[derive(Debug, Default, Clone, Copy)]
pub struct LocalProvider {
    watch: WatchOptions,
}

impl LocalProvider {
    pub fn new() -> Self {
        Self::default()
    }

    /// A provider whose watches use the given mechanism and timings.
    pub fn with_watch_options(watch: WatchOptions) -> Self {
        Self { watch }
    }
}

fn file_path(path: &VfsPath) -> Result<&FilePath, VfsError> {
    match path {
        VfsPath::File(path) => Ok(path),
    }
}

fn kind_of(file_type: &FileType) -> EntryKind {
    if file_type.is_dir() {
        EntryKind::Directory
    } else if file_type.is_symlink() {
        EntryKind::Symlink
    } else if file_type.is_file() {
        EntryKind::File
    } else {
        EntryKind::Other
    }
}

fn to_ms(time: SystemTime) -> i64 {
    match time.duration_since(UNIX_EPOCH) {
        Ok(after) => after.as_millis() as i64,
        Err(before) => -(before.duration().as_millis() as i64),
    }
}

/// Whether the platform calls an entry hidden: a leading dot on Linux, the hidden attribute on
/// Windows (where a leading dot means nothing).
#[cfg(unix)]
fn is_hidden(name: &OsStr, _meta: Option<&Metadata>) -> bool {
    name.as_encoded_bytes().first() == Some(&b'.')
}

#[cfg(windows)]
fn is_hidden(_name: &OsStr, meta: Option<&Metadata>) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
    meta.is_some_and(|m| m.file_attributes() & FILE_ATTRIBUTE_HIDDEN != 0)
}

/// Follows a symlink to learn what it points at. A broken link, a loop or a permission error all
/// read as "no target".
fn follow(path: &Path) -> Option<Metadata> {
    fs::metadata(path).ok()
}

/// Builds an entry from what a directory read gave for free, resolving a symlink only when asked.
fn build(
    name: &OsStr,
    file_type: Option<FileType>,
    meta: Option<Metadata>,
    full_path: &Path,
    resolve_link: bool,
) -> ScannedEntry {
    let kind = file_type.as_ref().map_or(EntryKind::Other, kind_of);
    let hidden = is_hidden(name, meta.as_ref());
    let mut link_target = None;
    let mut link_pending = false;
    let mut shown = meta;
    if kind == EntryKind::Symlink {
        if resolve_link {
            let target = follow(full_path);
            link_target = target.as_ref().map(|m| kind_of(&m.file_type()));
            if target.is_some() {
                shown = target;
            }
        } else {
            link_pending = true;
        }
    }
    let shows_size = match kind {
        EntryKind::File => true,
        EntryKind::Symlink => link_target == Some(EntryKind::File),
        _ => false,
    };
    ScannedEntry {
        name: name.to_owned(),
        kind,
        link_target,
        link_pending,
        group: group_for(name.as_encoded_bytes(), kind, link_target),
        size: shown.as_ref().filter(|_| shows_size).map(Metadata::len),
        modified_ms: shown.as_ref().and_then(|m| m.modified().ok()).map(to_ms),
        hidden,
    }
}

impl Provider for LocalProvider {
    fn scheme(&self) -> &'static str {
        "file"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            watch: true,
            case_rule: CaseRule::NATIVE,
        }
    }

    fn stat(&self, path: &VfsPath) -> Result<ScannedEntry, VfsError> {
        let file = file_path(path)?;
        let location = path.to_location();
        let meta = fs::symlink_metadata(file.as_path()).map_err(|e| from_io(&e, &location))?;
        let name = file.file_name().unwrap_or_default();
        Ok(build(
            &name,
            Some(meta.file_type()),
            Some(meta),
            file.as_path(),
            true,
        ))
    }

    fn list(
        &self,
        path: &VfsPath,
        cancel: &CancelToken,
        inline_link_budget: usize,
        progress: &mut dyn FnMut(u32),
    ) -> Result<Vec<ScannedEntry>, VfsError> {
        let folder = file_path(path)?;
        list_folder(
            folder.as_path(),
            &path.to_location(),
            cancel,
            inline_link_budget,
            progress,
        )
    }

    fn resolve_link(
        &self,
        folder: &VfsPath,
        entry: &ScannedEntry,
    ) -> Result<ScannedEntry, VfsError> {
        let location = folder.to_location();
        let folder = file_path(folder)?;
        let full = folder
            .join(&entry.name)
            .map_err(|_| VfsError::InvalidLocation {
                input: entry.name.to_string_lossy().into_owned(),
            })?;
        // A link that can no longer be read (removed or renamed since the scan) is an error, so
        // the caller drops the update instead of resurrecting the entry.
        let meta = fs::symlink_metadata(full.as_path()).map_err(|e| from_io(&e, &location))?;
        Ok(build(
            &entry.name,
            Some(meta.file_type()),
            Some(meta),
            full.as_path(),
            true,
        ))
    }

    fn watch(&self, path: &VfsPath, sink: WatchSink) -> Result<Box<dyn Watch>, VfsError> {
        let folder = file_path(path)?;
        watch::start(
            folder.as_path().to_path_buf(),
            path.to_location(),
            self.watch,
            sink,
        )
    }
}

/// Reads a folder into entries, resolving up to `inline_link_budget` symlinks on the way.
pub(crate) fn list_folder(
    folder: &Path,
    location: &Location,
    cancel: &CancelToken,
    inline_link_budget: usize,
    progress: &mut dyn FnMut(u32),
) -> Result<Vec<ScannedEntry>, VfsError> {
    let read = fs::read_dir(folder).map_err(|e| from_io(&e, location))?;
    let mut budget = inline_link_budget;
    let mut entries = Vec::new();
    for item in read {
        if cancel.is_cancelled() {
            return Err(VfsError::Cancelled);
        }
        // An entry that vanishes or cannot be read mid-scan is skipped, not fatal.
        let Ok(item) = item else { continue };
        let name = item.file_name();
        let file_type = item.file_type().ok();
        let meta = item.metadata().ok();
        let is_link = file_type.as_ref().is_some_and(FileType::is_symlink);
        let resolve = is_link && budget > 0;
        if resolve {
            budget -= 1;
        }
        entries.push(build(&name, file_type, meta, &item.path(), resolve));
        if (entries.len() as u32).is_multiple_of(PROGRESS_EVERY) {
            progress(entries.len() as u32);
        }
    }
    progress(entries.len() as u32);
    Ok(entries)
}

/// Describes one child of `folder`, resolving it if it is a symlink.
pub(crate) fn stat_child(folder: &Path, name: &OsStr) -> io::Result<ScannedEntry> {
    let full = folder.join(name);
    let meta = fs::symlink_metadata(&full)?;
    Ok(build(name, Some(meta.file_type()), Some(meta), &full, true))
}
