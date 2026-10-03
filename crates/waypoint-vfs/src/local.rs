// The local provider: folders on this machine, read with `std::fs`.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::ffi::{OsStr, OsString};
use std::fs::{self, File, FileType, Metadata};
use std::io::{self, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use waypoint_path::{CaseRule, FilePath, VfsPath};
use waypoint_protocol::{Location, VfsError};

use crate::error::{cause, from_io, from_io_pair, Cause};
use crate::icon::group_for;
use crate::model::EntryKind;
use crate::names::validate_new_path;
use crate::provider::{Capabilities, Provider, ScannedEntry, Watch, WatchSink};
use crate::sys;
use crate::watch::{self, WatchOptions};
use crate::write::{FileTimes, Permissions, ReadStream, VolumeId, WriteOptions, WriteStream};
use crate::{CancelToken, DetailField, EntryDetails, FolderSizeTotals, VolumeSpace};

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

pub(crate) fn file_path(path: &VfsPath) -> Result<&FilePath, VfsError> {
    match path {
        VfsPath::File(path) => Ok(path),
        other => Err(VfsError::Unsupported {
            what: format!("the {} scheme in the local provider", other.scheme()),
        }),
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

/// The start of the name a file gets while a copy is still writing it. It is never a file the
/// person made, so every platform lists it as hidden, a leading dot meaning nothing on Windows.
#[cfg(windows)]
const PARTIAL_PREFIX: &[u8] = b".waypoint-partial-";

#[cfg(windows)]
fn is_partial(name: &OsStr) -> bool {
    name.as_encoded_bytes().starts_with(PARTIAL_PREFIX)
}

/// Whether the platform calls an entry hidden: a leading dot on Linux, the hidden attribute on
/// Windows (where a leading dot means nothing).
#[cfg(unix)]
pub(crate) fn is_hidden(name: &OsStr, _meta: Option<&Metadata>) -> bool {
    name.as_encoded_bytes().first() == Some(&b'.')
}

#[cfg(windows)]
pub(crate) fn is_hidden(name: &OsStr, meta: Option<&Metadata>) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
    is_partial(name) || meta.is_some_and(|m| m.file_attributes() & FILE_ATTRIBUTE_HIDDEN != 0)
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
        trashed: None,
    }
}

/// Whether the entry is a cloud placeholder whose data is not on this machine (a OneDrive file
/// that is online-only): reading it would download it, so size and preview code leave it alone.
#[cfg(windows)]
pub(crate) fn is_placeholder(meta: &Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_OFFLINE: u32 = 0x1000;
    const FILE_ATTRIBUTE_RECALL_ON_OPEN: u32 = 0x4_0000;
    const FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS: u32 = 0x40_0000;
    meta.file_attributes()
        & (FILE_ATTRIBUTE_OFFLINE
            | FILE_ATTRIBUTE_RECALL_ON_OPEN
            | FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS)
        != 0
}

#[cfg(not(windows))]
pub(crate) fn is_placeholder(_meta: &Metadata) -> bool {
    false
}

/// Reads the first bytes of a regular file for a content sniff; `None` when it cannot be read.
fn sniff(path: &Path) -> Option<Vec<u8>> {
    use std::io::Read;
    let mut head = Vec::with_capacity(crate::SNIFF_LEN);
    File::open(path)
        .ok()?
        .take(crate::SNIFF_LEN as u64)
        .read_to_end(&mut head)
        .ok()?;
    Some(head)
}

fn local_details(path: &VfsPath) -> Result<EntryDetails, VfsError> {
    let file = file_path(path)?;
    let location = path.to_location();
    let link_meta = fs::symlink_metadata(file.as_path()).map_err(|e| from_io(&e, &location))?;
    let kind = kind_of(&link_meta.file_type());
    let name = file.file_name().unwrap_or_default();
    // A link reports its target's size, mode and times, as a listing shows them; a broken one only
    // has its own.
    let (meta, resolves_to, symlink_target) = if kind == EntryKind::Symlink {
        let target = fs::read_link(file.as_path())
            .ok()
            .map(|t| t.to_string_lossy().into_owned());
        let followed = follow(file.as_path());
        let resolves_to = followed.as_ref().map(|m| kind_of(&m.file_type()));
        (followed.unwrap_or(link_meta), resolves_to, target)
    } else {
        (link_meta, None, None)
    };
    let shown_kind = resolves_to.unwrap_or(kind);
    let is_file = shown_kind == EntryKind::File;
    let placeholder = is_placeholder(&meta);
    let display_name = name.to_string_lossy().into_owned();
    let head = if is_file && !placeholder {
        sniff(file.as_path())
    } else {
        None
    };
    let mime_type = if shown_kind == EntryKind::Directory {
        Some("inode/directory".to_owned())
    } else if is_file {
        crate::guess_mime(&display_name, head.as_deref())
    } else {
        None
    };
    let mut unavailable = Vec::new();
    let created_ms = meta.created().ok().map(to_ms);
    if created_ms.is_none() {
        unavailable.push(DetailField::Created);
    }
    let mut details = EntryDetails {
        name: display_name,
        kind,
        resolves_to,
        symlink_target,
        size: is_file.then_some(meta.len()),
        allocated_size: None,
        created_ms,
        modified_ms: meta.modified().ok().map(to_ms),
        accessed_ms: meta.accessed().ok().map(to_ms),
        owner: None,
        group: None,
        mode: None,
        read_only: meta.permissions().readonly(),
        hidden: is_hidden(&name, Some(&meta)),
        mime_type,
        unavailable,
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if is_file {
            details.allocated_size = Some(meta.blocks().saturating_mul(512));
        }
        details.owner = Some(sys::user_name(meta.uid()).unwrap_or_else(|| meta.uid().to_string()));
        details.group = Some(sys::group_name(meta.gid()).unwrap_or_else(|| meta.gid().to_string()));
        details.mode = Some(meta.mode() & 0o7777);
    }
    #[cfg(windows)]
    {
        if is_file && !placeholder {
            details.allocated_size = sys::allocated_size(file.as_path());
        }
        if details.allocated_size.is_none() && is_file {
            details.unavailable.push(DetailField::AllocatedSize);
        }
        details.unavailable.extend([
            DetailField::Owner,
            DetailField::Group,
            DetailField::Permissions,
        ]);
    }
    Ok(details)
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

    fn create_dir(&self, path: &VfsPath) -> Result<(), VfsError> {
        validate_new_path(path, CaseRule::NATIVE)?;
        let file = file_path(path)?;
        fs::create_dir(file.as_path()).map_err(|e| from_io(&e, &path.to_location()))
    }

    fn create_file(&self, path: &VfsPath) -> Result<(), VfsError> {
        validate_new_path(path, CaseRule::NATIVE)?;
        let file = file_path(path)?;
        sys::open_write(file.as_path(), true, None)
            .map(drop)
            .map_err(|e| from_io(&e, &path.to_location()))
    }

    fn rename(&self, from: &VfsPath, to: &VfsPath, overwrite: bool) -> Result<(), VfsError> {
        validate_new_path(to, CaseRule::NATIVE)?;
        let (source, target) = (file_path(from)?, file_path(to)?);
        let (from_loc, to_loc) = (from.to_location(), to.to_location());
        sys::rename(source.as_path(), target.as_path(), overwrite).map_err(|error| {
            match from_io_pair(&error, &from_loc, &to_loc) {
                // The error alone cannot say which side is missing. A source that is still there
                // means the destination's parent is the missing part.
                VfsError::NotFound { .. } if fs::symlink_metadata(source.as_path()).is_ok() => {
                    VfsError::NotFound { location: to_loc }
                }
                // A folder that would be replaced is not empty: that is the destination.
                VfsError::NotEmpty { .. } => VfsError::NotEmpty { location: to_loc },
                other => other,
            }
        })
    }

    fn remove_file(&self, path: &VfsPath) -> Result<(), VfsError> {
        let location = path.to_location();
        let file = file_path(path)?.as_path();
        #[cfg(windows)]
        {
            use std::os::windows::fs::FileTypeExt;
            // Windows removes a link to a folder as a folder, and refuses a real folder.
            if let Ok(meta) = fs::symlink_metadata(file) {
                let kind = meta.file_type();
                if kind.is_symlink_dir() {
                    return fs::remove_dir(file).map_err(|e| from_io(&e, &location));
                }
                if kind.is_dir() {
                    return Err(VfsError::IsADirectory { location });
                }
            }
        }
        fs::remove_file(file).map_err(|error| {
            // Some systems answer unlinking a folder with "not permitted" rather than "is a folder".
            let is_folder = matches!(cause(&error), Cause::PermissionDenied | Cause::IsADirectory)
                && fs::symlink_metadata(file).is_ok_and(|m| m.is_dir());
            if is_folder {
                VfsError::IsADirectory {
                    location: location.clone(),
                }
            } else {
                from_io(&error, &location)
            }
        })
    }

    fn remove_dir(&self, path: &VfsPath) -> Result<(), VfsError> {
        let location = path.to_location();
        let file = file_path(path)?.as_path();
        #[cfg(windows)]
        if fs::symlink_metadata(file).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(VfsError::NotADirectory { location });
        }
        fs::remove_dir(file).map_err(|error| match cause(&error) {
            // POSIX lets a file system answer "not empty" with `EEXIST`.
            Cause::AlreadyExists => VfsError::NotEmpty {
                location: location.clone(),
            },
            _ => from_io(&error, &location),
        })
    }

    fn open_read(&self, path: &VfsPath) -> Result<ReadStream, VfsError> {
        let location = path.to_location();
        let file = File::open(file_path(path)?.as_path()).map_err(|e| from_io(&e, &location))?;
        // Opening a folder succeeds on Linux and fails at the first read; say so now.
        if file.metadata().is_ok_and(|m| m.is_dir()) {
            return Err(VfsError::IsADirectory { location });
        }
        Ok(Box::new(file))
    }

    fn open_read_at(&self, path: &VfsPath, start: u64) -> Result<ReadStream, VfsError> {
        use std::io::{Seek, SeekFrom};
        let location = path.to_location();
        let mut file =
            File::open(file_path(path)?.as_path()).map_err(|e| from_io(&e, &location))?;
        if file.metadata().is_ok_and(|m| m.is_dir()) {
            return Err(VfsError::IsADirectory { location });
        }
        file.seek(SeekFrom::Start(start))
            .map_err(|e| from_io(&e, &location))?;
        Ok(Box::new(file))
    }

    fn details(&self, path: &VfsPath) -> Result<EntryDetails, VfsError> {
        local_details(path)
    }

    fn folder_size(
        &self,
        path: &VfsPath,
        cancel: &CancelToken,
        report: &mut dyn FnMut(&FolderSizeTotals),
    ) -> Result<crate::FolderSizeRun, VfsError> {
        crate::size::local_folder_size(path, cancel, report)
    }

    fn create_write(
        &self,
        path: &VfsPath,
        options: WriteOptions,
    ) -> Result<Box<dyn WriteStream>, VfsError> {
        validate_new_path(path, CaseRule::NATIVE)?;
        let location = path.to_location();
        let file = sys::open_write(file_path(path)?.as_path(), options.exclusive, options.mode)
            .map_err(|e| from_io(&e, &location))?;
        Ok(Box::new(LocalWrite { file, location }))
    }

    fn set_times(&self, path: &VfsPath, times: FileTimes) -> Result<(), VfsError> {
        let file = file_path(path)?.as_path();
        let location = path.to_location();
        if times == FileTimes::default() {
            // Nothing to change, but a missing entry is still an error (the kernel skips the
            // lookup when both times are omitted).
            return fs::symlink_metadata(file)
                .map(drop)
                .map_err(|e| from_io(&e, &location));
        }
        sys::set_times(file, times).map_err(|e| from_io(&e, &location))
    }

    fn permissions(&self, path: &VfsPath) -> Result<Permissions, VfsError> {
        let meta = fs::metadata(file_path(path)?.as_path())
            .map_err(|e| from_io(&e, &path.to_location()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = meta.permissions().mode() & 0o7777;
            Ok(Permissions {
                mode: Some(mode),
                readonly: mode & 0o222 == 0,
            })
        }
        #[cfg(windows)]
        Ok(Permissions {
            mode: None,
            readonly: meta.permissions().readonly(),
        })
    }

    fn set_permissions(&self, path: &VfsPath, permissions: Permissions) -> Result<(), VfsError> {
        let location = path.to_location();
        let file = file_path(path)?.as_path();
        let meta = fs::symlink_metadata(file).map_err(|e| from_io(&e, &location))?;
        if meta.file_type().is_symlink() {
            return Err(VfsError::Unsupported {
                what: "setting the permissions of a symlink".to_owned(),
            });
        }
        let mut new = meta.permissions();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = match permissions.mode {
                Some(mode) => mode & 0o7777,
                None if permissions.readonly => new.mode() & !0o222,
                None => new.mode() | 0o200,
            };
            new.set_mode(mode);
        }
        #[cfg(windows)]
        new.set_readonly(permissions.readonly);
        fs::set_permissions(file, new).map_err(|e| from_io(&e, &location))
    }

    fn symlink(&self, link: &VfsPath, target: &OsStr) -> Result<(), VfsError> {
        validate_new_path(link, CaseRule::NATIVE)?;
        let location = link.to_location();
        let at = file_path(link)?.as_path();
        #[cfg(unix)]
        let made = std::os::unix::fs::symlink(target, at);
        #[cfg(windows)]
        let made = {
            // Windows needs to know whether the link points at a folder.
            let resolved = at.parent().map(|dir| dir.join(target));
            if resolved.is_some_and(|p| fs::metadata(p).is_ok_and(|m| m.is_dir())) {
                std::os::windows::fs::symlink_dir(target, at)
            } else {
                std::os::windows::fs::symlink_file(target, at)
            }
        };
        made.map_err(|e| from_io(&e, &location))
    }

    fn read_link(&self, path: &VfsPath) -> Result<OsString, VfsError> {
        fs::read_link(file_path(path)?.as_path())
            .map(|target| target.into_os_string())
            .map_err(|e| from_io(&e, &path.to_location()))
    }

    fn canonicalize(&self, path: &VfsPath) -> Result<VfsPath, VfsError> {
        let location = path.to_location();
        let resolved =
            fs::canonicalize(file_path(path)?.as_path()).map_err(|e| from_io(&e, &location))?;
        // Windows answers with a verbatim `\\?\` path; drop the prefix where the rest is a plain drive path.
        #[cfg(windows)]
        let resolved = {
            let text = resolved.to_string_lossy().into_owned();
            match text.strip_prefix(r"\\?\") {
                Some(rest) if !rest.starts_with("UNC\\") => std::path::PathBuf::from(rest),
                _ => resolved,
            }
        };
        FilePath::from_path(&resolved)
            .map(VfsPath::File)
            .map_err(|_| VfsError::InvalidLocation {
                input: resolved.to_string_lossy().into_owned(),
            })
    }

    fn volume_id(&self, path: &VfsPath) -> Option<VolumeId> {
        let file = file_path(path).ok()?;
        sys::volume_id(file.as_path()).ok().map(VolumeId)
    }

    fn free_space(&self, path: &VfsPath) -> Option<VolumeSpace> {
        crate::space::query(file_path(path).ok()?)
    }

    fn copy_file_within(
        &self,
        src: &VfsPath,
        dst: &VfsPath,
        progress: &mut dyn FnMut(u64),
        cancel: &CancelToken,
    ) -> Option<Result<u64, VfsError>> {
        if let Err(error) = validate_new_path(dst, CaseRule::NATIVE) {
            return Some(Err(error));
        }
        let (from, to) = (file_path(src).ok()?, file_path(dst).ok()?);
        match sys::copy_fast(from.as_path(), to.as_path(), progress, cancel) {
            sys::Fast::Unhandled => None,
            sys::Fast::Done(bytes) => Some(Ok(bytes)),
            sys::Fast::Cancelled => Some(Err(VfsError::Cancelled)),
            sys::Fast::Failed(error) => Some(Err(from_io_pair(
                &error,
                &src.to_location(),
                &dst.to_location(),
            ))),
        }
    }
}

/// A file being written through `LocalProvider::create_write`.
struct LocalWrite {
    file: File,
    location: Location,
}

impl Write for LocalWrite {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.file.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

impl WriteStream for LocalWrite {
    fn finish(mut self: Box<Self>, sync: bool) -> Result<(), VfsError> {
        let done = self
            .file
            .flush()
            .and_then(|()| if sync { self.file.sync_all() } else { Ok(()) });
        done.map_err(|e| from_io(&e, &self.location))
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
        // An entry that vanished mid-scan is skipped. Any other failure is surfaced: a listing
        // that silently misses entries would let a copy or a duplicate finish with files absent.
        let item = match item {
            Ok(item) => item,
            Err(error) => match skip_vanished(&error, location) {
                Ok(()) => continue,
                Err(error) => return Err(error),
            },
        };
        let name = item.file_name();
        let file_type = match item.file_type() {
            Ok(file_type) => Some(file_type),
            Err(error) => match skip_vanished(&error, location) {
                Ok(()) => continue,
                Err(error) => return Err(error),
            },
        };
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

/// Decides what a failure while reading one directory entry means: `Ok` when the entry vanished
/// since the read began (nothing to list), otherwise the typed error for the folder.
fn skip_vanished(error: &io::Error, location: &Location) -> Result<(), VfsError> {
    match from_io(error, location) {
        VfsError::NotFound { .. } => Ok(()),
        other => Err(other),
    }
}

/// Describes one child of `folder`, resolving it if it is a symlink.
pub(crate) fn stat_child(folder: &Path, name: &OsStr) -> io::Result<ScannedEntry> {
    let full = folder.join(name);
    let meta = fs::symlink_metadata(&full)?;
    Ok(build(name, Some(meta.file_type()), Some(meta), &full, true))
}

#[cfg(test)]
mod tests {
    #[cfg(windows)]
    #[test]
    fn a_file_a_copy_is_still_writing_is_listed_as_hidden() {
        use std::ffi::OsStr;
        assert!(super::is_hidden(
            OsStr::new(".waypoint-partial-3-7-report.pdf"),
            None
        ));
        assert!(!super::is_hidden(OsStr::new("report.pdf"), None));
    }

    use super::*;

    #[test]
    fn a_vanished_entry_is_skipped() {
        let location = Location::new("/x", "file:///x");
        let gone = io::Error::from(io::ErrorKind::NotFound);
        assert_eq!(skip_vanished(&gone, &location), Ok(()));
    }

    #[test]
    fn any_other_entry_failure_is_surfaced() {
        let location = Location::new("/x", "file:///x");
        let denied = io::Error::from(io::ErrorKind::PermissionDenied);
        assert!(matches!(
            skip_vanished(&denied, &location),
            Err(VfsError::PermissionDenied { .. })
        ));
        let broken = io::Error::other("input/output error");
        assert!(skip_vanished(&broken, &location).is_err());
    }
}
