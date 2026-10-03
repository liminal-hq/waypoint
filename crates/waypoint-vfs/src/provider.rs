// The trait every file system provider implements, and the types it speaks.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::ffi::{OsStr, OsString};
use std::io::Read;
use std::sync::Arc;

use waypoint_path::{CaseRule, VfsPath};
use waypoint_protocol::VfsError;

use crate::write::{FileTimes, Permissions, ReadStream, VolumeId, WriteOptions, WriteStream};
use crate::{CancelToken, EntryKind, IconGroup, VolumeSpace};

/// The error a write primitive a provider does not implement returns.
pub(crate) fn unsupported<T>(what: &str) -> Result<T, VfsError> {
    Err(VfsError::Unsupported {
        what: what.to_owned(),
    })
}

/// One entry as a provider reports it, before a listing gives it an `EntryId`.
///
/// `name` is the real name (a Linux name need not be UTF-8); the wire `Entry` carries a lossy
/// display string and the listing resolves paths from this name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScannedEntry {
    pub name: OsString,
    pub kind: EntryKind,
    /// For a symlink, the kind of what it points at; `None` for a broken link or one not yet
    /// resolved (see `link_pending`).
    pub link_target: Option<EntryKind>,
    /// A symlink whose target has not been looked at yet, because the scan hit its budget. The
    /// listing resolves these later in a bounded background pass (`Listing::resolve_pending_links`).
    pub link_pending: bool,
    pub group: IconGroup,
    pub size: Option<u64>,
    pub modified_ms: Option<i64>,
    pub hidden: bool,
    /// What only an item in the Trash has; `None` for every other entry.
    pub trashed: Option<Box<TrashedMeta>>,
}

/// What a trashed item adds to its entry: the name it had before it was trashed (the entry's own
/// name is the id that names it in the Trash), where it was, and when it was trashed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrashedMeta {
    pub display_name: String,
    /// The folder it was trashed from, as people read it.
    pub original_path: String,
    pub deleted_ms: i64,
}

/// What a provider can do at a location, so the UI hides what does not work (A9, A17).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capabilities {
    /// Whether `Provider::watch` works, or listings must be refreshed by rescanning.
    pub watch: bool,
    /// How names compare in this provider.
    pub case_rule: CaseRule,
}

/// One change to a folder's contents, by name. Changes are idempotent: applying one twice leaves
/// the listing as applying it once does, which is what makes coalescing safe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    /// The entry exists with these attributes: added if the name is new, updated otherwise.
    Upsert(ScannedEntry),
    /// No entry has this name.
    Remove(OsString),
    /// `from` is now `to`. The entry keeps its `EntryId`, so a selection follows a rename.
    Rename { from: OsString, to: ScannedEntry },
}

/// Why a watcher asks for a full rescan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RescanReason {
    /// The operating system dropped events (inotify queue overflow, `ReadDirectoryChangesW`
    /// buffer overflow), so the changes are unknown.
    Overflow,
    /// The watcher could not tell what changed.
    Unknown(String),
}

/// What a watcher tells its listing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WatchEvent {
    /// A coalesced batch of changes.
    Changes(Vec<Change>),
    /// Some changes are unknown: rescan and diff.
    Rescan(RescanReason),
    /// Native watching is unavailable or was lost, so the provider now polls. `reason` is shown in
    /// the Services status panel.
    Degraded { reason: String },
    /// The watched folder went away or became unreadable.
    Lost(VfsError),
}

/// Where a watcher sends its events. It may be called from any thread.
pub type WatchSink = Arc<dyn Fn(WatchEvent) + Send + Sync>;

/// Keeps a watch alive; dropping it stops the watch.
pub trait Watch: Send {}

/// A source of folders. `waypoint-vfs` ships the local provider; SFTP, SMB and the rest join as
/// crates of their own.
pub trait Provider: Send + Sync {
    /// The URI scheme this provider serves.
    fn scheme(&self) -> &'static str;

    fn capabilities(&self) -> Capabilities;

    /// Whether this provider changes nothing, so a view of it offers no way to. Default `false`.
    fn read_only(&self) -> bool {
        false
    }

    /// What the entries of this provider's listings look like. Default: ordinary folders.
    fn layout(&self) -> crate::ListingLayout {
        crate::ListingLayout::Folder
    }

    /// Describes one entry without listing its folder.
    fn stat(&self, path: &VfsPath) -> Result<ScannedEntry, VfsError>;

    /// Lists a folder. `progress` receives the number of entries read so far (never the entries).
    /// Implementations check `cancel` between entries and return `VfsError::Cancelled`.
    ///
    /// At most `inline_link_budget` symlinks are resolved during the scan; the rest come back with
    /// `link_pending` set.
    fn list(
        &self,
        path: &VfsPath,
        cancel: &CancelToken,
        inline_link_budget: usize,
        progress: &mut dyn FnMut(u32),
    ) -> Result<Vec<ScannedEntry>, VfsError>;

    /// Resolves a symlink left `link_pending` by `list`: returns the entry with its target's kind
    /// (and size and modified time, for a file) filled in and `link_pending` cleared. A broken link
    /// comes back with `link_target` `None`. A link that no longer exists is `VfsError::NotFound`,
    /// and the caller drops the update.
    fn resolve_link(
        &self,
        folder: &VfsPath,
        entry: &ScannedEntry,
    ) -> Result<ScannedEntry, VfsError>;

    /// Watches a folder and reports changes to `sink` until the returned `Watch` is dropped.
    fn watch(&self, path: &VfsPath, sink: WatchSink) -> Result<Box<dyn Watch>, VfsError> {
        let _ = (path, sink);
        Err(VfsError::Unsupported {
            what: "watching this location".to_owned(),
        })
    }

    // The write primitives (A45). Each defaults to `Unsupported`, so a read-only provider stays
    // valid and a capability check can hide what it cannot do. None of them follows a symlink in the
    // final component of a path unless it says so: a link is always the thing acted on.

    /// Creates a folder whose parent exists. `AlreadyExists` when the name is taken (even by a
    /// file), `NotFound` when the parent is missing, `InvalidName` for a name the provider's case
    /// rule forbids.
    fn create_dir(&self, path: &VfsPath) -> Result<(), VfsError> {
        let _ = path;
        unsupported("creating folders here")
    }

    /// Creates an empty file, exclusively: `AlreadyExists` when the name is taken.
    fn create_file(&self, path: &VfsPath) -> Result<(), VfsError> {
        let _ = path;
        unsupported("creating files here")
    }

    /// Renames or moves an entry within one volume, atomically. A symlink is renamed, never
    /// followed. With `overwrite` false it fails with `AlreadyExists` when `to` exists (the check
    /// and the rename are one atomic step where the platform allows it); with it true, a file
    /// replaces a file. Replacing a folder is not promised: `rename(2)` lets a folder replace an
    /// empty one, but Windows cannot (`MoveFileExW` answers `PermissionDenied`), so a caller removes
    /// an empty target folder first. A rename that
    /// would have to cross volumes fails with `CrossesDevices` and changes nothing, so the caller
    /// falls back to copy and remove.
    fn rename(&self, from: &VfsPath, to: &VfsPath, overwrite: bool) -> Result<(), VfsError> {
        let _ = (from, to, overwrite);
        unsupported("renaming here")
    }

    /// Removes a file or a symlink (the link, never its target). A folder is `IsADirectory`.
    fn remove_file(&self, path: &VfsPath) -> Result<(), VfsError> {
        let _ = path;
        unsupported("removing files here")
    }

    /// Removes an empty folder. `NotEmpty` when it holds entries, `NotADirectory` for a file or a
    /// symlink (a link to a folder is removed with `remove_file`).
    fn remove_dir(&self, path: &VfsPath) -> Result<(), VfsError> {
        let _ = path;
        unsupported("removing folders here")
    }

    /// Opens a file for streaming reads.
    fn open_read(&self, path: &VfsPath) -> Result<ReadStream, VfsError> {
        let _ = path;
        unsupported("reading files here")
    }

    /// Opens a file for streaming reads starting `start` bytes in (a preview's `Range` request). The
    /// default opens the whole file and reads past the start; a provider that can seek overrides it.
    fn open_read_at(&self, path: &VfsPath, start: u64) -> Result<ReadStream, VfsError> {
        let mut stream = self.open_read(path)?;
        if start > 0 {
            let skipped = std::io::copy(&mut (&mut stream).take(start), &mut std::io::sink())
                .map_err(|error| crate::from_io(&error, &path.to_location()))?;
            if skipped < start {
                return Ok(Box::new(std::io::empty()));
            }
        }
        Ok(stream)
    }

    /// Everything the Inspector shows about one entry. The default builds it from `stat`, the name
    /// and nothing else, and lists every detail only a local provider can read as unavailable, so a
    /// remote provider needs to implement nothing to be honest about what it cannot say.
    fn details(&self, path: &VfsPath) -> Result<crate::EntryDetails, VfsError> {
        let entry = self.stat(path)?;
        Ok(crate::inspect::from_scanned(&entry))
    }

    /// The total size of everything under a folder, counted by walking it (see `size` for what a
    /// walk skips). `report` receives the running total about every 100 ms; `cancel` stops the walk,
    /// which then returns what it had counted with `cancelled` set. The default walks through `list`
    /// and trusts the sizes it reports; the local provider reads each entry itself.
    fn folder_size(
        &self,
        path: &VfsPath,
        cancel: &CancelToken,
        report: &mut dyn FnMut(&crate::FolderSizeTotals),
    ) -> Result<crate::FolderSizeRun, VfsError> {
        crate::size::listed_folder_size(self, path, cancel, report)
    }

    /// Opens a file for streaming writes, creating it. See `WriteOptions` for the exclusive mode.
    fn create_write(
        &self,
        path: &VfsPath,
        options: WriteOptions,
    ) -> Result<Box<dyn WriteStream>, VfsError> {
        let _ = (path, options);
        unsupported("writing files here")
    }

    /// Sets the access and modification times of an entry itself (a symlink's own times, not its
    /// target's).
    fn set_times(&self, path: &VfsPath, times: FileTimes) -> Result<(), VfsError> {
        let _ = (path, times);
        unsupported("setting times here")
    }

    /// The permissions of a file or folder (a symlink reports its target's, as it has none of its
    /// own on Linux).
    fn permissions(&self, path: &VfsPath) -> Result<Permissions, VfsError> {
        let _ = path;
        unsupported("reading permissions here")
    }

    /// Sets permissions: the Unix mode bits where `mode` is given and the platform has them,
    /// otherwise just the read-only state. A symlink is `Unsupported` (Linux symlinks carry no
    /// permissions, and changing the target would be acting through the link).
    fn set_permissions(&self, path: &VfsPath, permissions: Permissions) -> Result<(), VfsError> {
        let _ = (path, permissions);
        unsupported("setting permissions here")
    }

    /// Creates a symlink at `link` holding the text `target`, which is stored as given (relative
    /// stays relative) and need not exist. On Windows this needs the privilege to create symlinks
    /// and otherwise is `PermissionDenied`.
    fn symlink(&self, link: &VfsPath, target: &OsStr) -> Result<(), VfsError> {
        let _ = (link, target);
        unsupported("creating symlinks here")
    }

    /// The text a symlink holds.
    fn read_link(&self, path: &VfsPath) -> Result<OsString, VfsError> {
        let _ = path;
        unsupported("reading symlinks here")
    }

    /// The location with every symlink along it resolved (every component, the last included), so two
    /// spellings of one place compare equal. The entry must exist (`NotFound` otherwise; to canonicalise
    /// something not yet created, canonicalise its parent). `Unsupported`, the default, means the
    /// provider cannot tell and the caller falls back to comparing the paths as written.
    fn canonicalize(&self, path: &VfsPath) -> Result<VfsPath, VfsError> {
        let _ = path;
        unsupported("resolving symlinks here")
    }

    /// The volume the location is on, or `None` when the provider has no such notion or cannot
    /// tell. To ask about something not yet created, ask about its parent.
    fn volume_id(&self, path: &VfsPath) -> Option<VolumeId> {
        let _ = path;
        None
    }

    /// Free and total space on the volume holding the location (what an ordinary user may use), or
    /// `None` when unknown.
    fn free_space(&self, path: &VfsPath) -> Option<VolumeSpace> {
        let _ = path;
        None
    }

    /// An optional fast path for copying one file's bytes within this provider (a reflink, a
    /// server-side copy, `copy_file_range`, `CopyFileExW`): creates `dst` exclusively and returns
    /// the bytes copied. `None` means "not handled, nothing was touched; use `open_read` and
    /// `create_write`", and is the default. `Some(Err(_))` leaves no `dst` behind (a cancel is
    /// `Cancelled`). `progress` receives the bytes copied so far; `cancel` is checked between
    /// chunks. Only the data is promised: the caller sets times and permissions itself.
    fn copy_file_within(
        &self,
        src: &VfsPath,
        dst: &VfsPath,
        progress: &mut dyn FnMut(u64),
        cancel: &CancelToken,
    ) -> Option<Result<u64, VfsError>> {
        let _ = (src, dst, progress, cancel);
        None
    }
}
