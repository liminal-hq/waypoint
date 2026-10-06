// The trait every file system provider implements, and the types it speaks.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::ffi::{OsStr, OsString};
use std::io::Read;
use std::sync::Arc;

use waypoint_path::{CaseRule, ConnectionKey, VfsPath};
use waypoint_protocol::{ConnectionState, VfsError};

use crate::remote::ConnectAnswer;
use crate::special::SpecialFolder;
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
    /// Which standard folder of the user's this is, for a folder that is one.
    pub special: Option<SpecialFolder>,
    pub size: Option<u64>,
    pub modified_ms: Option<i64>,
    pub hidden: bool,
    /// What only an item in the Trash has; `None` for every other entry.
    pub trashed: Option<Box<TrashedMeta>>,
    /// Extra facts only this provider has (an S3 object's storage class), for plugin columns;
    /// `None` for an entry with none, which is nearly every entry.
    pub attributes: Option<Box<crate::EntryAttributes>>,
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

/// How a provider renames (A78).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenameSupport {
    /// No rename at all (S3): a move is a copy and a delete.
    None,
    /// A rename may replace what has the target name, so a rename that must not is a check and
    /// then a rename, which can race with another writer.
    Replacing,
    /// An atomic rename that refuses an existing target (`rename(from, to, false)` as locally).
    NoReplace,
}

/// What permissions a provider has (A78).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionModel {
    /// None at all: the Inspector shows no permission rows and a copy sets none.
    None,
    /// A read-only state, nothing more.
    ReadOnlyFlag,
    /// Unix mode bits.
    Unix,
}

/// What a provider can do, so the UI hides what does not work and the engine picks its strategy
/// (A6, A78). It is non-exhaustive: build one with `Capabilities::new` (which claims nothing) or
/// `Capabilities::local`, then set fields, so a flag added later breaks no provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct Capabilities {
    /// Whether `Provider::watch` works, or listings must be refreshed by rescanning.
    pub watch: bool,
    /// How names compare in this provider.
    pub case_rule: CaseRule,
    /// Reads cost time or money (a server): nothing reads content without being asked, so no
    /// thumbnails by default, no sniffing and no folder sizes unless started.
    pub remote: bool,
    /// The provider implements the write primitives. `Provider::read_only` still says whether a
    /// location refuses changes now.
    pub write: bool,
    pub rename: RenameSupport,
    /// `copy_file_within` copies on the server, so a copy within one connection sends no bytes
    /// through Waypoint.
    pub server_copy: bool,
    /// A written stream becomes visible only when `finish` succeeds, so the engine writes straight
    /// to the final name instead of a partial name and a rename.
    pub atomic_write: bool,
    /// `resume_write` continues a partial file from an offset.
    pub resume_write: bool,
    /// `open_read_at` seeks instead of reading past the start.
    pub range_read: bool,
    pub permissions: PermissionModel,
    /// Links exist and are reported as links.
    pub symlinks: bool,
    /// A modification time can be set, so a copy keeps it.
    pub set_times: bool,
    /// The longest name in bytes, when known; a longer one is `InvalidName`.
    pub max_name_len: Option<u32>,
    /// The finest modification time the provider keeps, in milliseconds: 1000 for SFTP, which
    /// drops the fraction of a second. Times are compared at this precision so a copy that was
    /// given the source's time is the same date as its source.
    pub time_resolution_ms: u32,
}

impl Capabilities {
    /// A provider that claims nothing beyond listing: no watching, no writing, not remote.
    pub const fn new(case_rule: CaseRule) -> Self {
        Self {
            watch: false,
            case_rule,
            remote: false,
            write: false,
            rename: RenameSupport::None,
            server_copy: false,
            atomic_write: false,
            resume_write: false,
            range_read: false,
            permissions: PermissionModel::None,
            symlinks: false,
            set_times: false,
            max_name_len: None,
            time_resolution_ms: 1,
        }
    }

    /// Everything a local disk does, on this platform.
    pub const fn local() -> Self {
        Self {
            watch: true,
            case_rule: CaseRule::NATIVE,
            remote: false,
            write: true,
            rename: RenameSupport::NoReplace,
            server_copy: false,
            atomic_write: false,
            resume_write: false,
            range_read: true,
            permissions: if cfg!(unix) {
                PermissionModel::Unix
            } else {
                PermissionModel::ReadOnlyFlag
            },
            symlinks: true,
            set_times: true,
            max_name_len: Some(255),
            time_resolution_ms: 1,
        }
    }

    /// `modified_ms` as this provider would store it: rounded down to `time_resolution_ms`.
    pub const fn at_time_resolution(&self, modified_ms: i64) -> i64 {
        let step = if self.time_resolution_ms == 0 {
            1
        } else {
            self.time_resolution_ms as i64
        };
        modified_ms.div_euclid(step) * step
    }
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

    /// Whether the folder at `path` is read only in itself but its changes are made by rewriting the
    /// file that holds it (an archive, D170), so the view still offers drops, rename and delete.
    /// Default `false`.
    fn rewritable(&self, path: &VfsPath) -> bool {
        let _ = path;
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

    /// Lists a folder in batches, so a large or slow folder shows its first rows early (A83).
    /// `sink` receives each batch as it is read. The default lists the whole folder with `list`
    /// and hands it over as one batch.
    fn list_batches(
        &self,
        path: &VfsPath,
        cancel: &CancelToken,
        inline_link_budget: usize,
        sink: &mut dyn FnMut(Vec<ScannedEntry>),
    ) -> Result<(), VfsError> {
        let entries = self.list(path, cancel, inline_link_budget, &mut |_| {})?;
        sink(entries);
        Ok(())
    }

    // Connections (A78). A provider with sessions (a server) opens one lazily on the first call
    // that needs it; these let the app show and steer that. The defaults describe a provider with
    // no sessions, which is always ready.

    /// The login `path` belongs to, or `None` when this provider has no sessions.
    fn connection_key(&self, path: &VfsPath) -> Option<ConnectionKey> {
        let _ = path;
        None
    }

    /// The state of one connection.
    fn connection_state(&self, key: &ConnectionKey) -> ConnectionState {
        let _ = key;
        ConnectionState::Connected
    }

    /// Opens a session now, with the person's `answer` to the question the last attempt asked (a
    /// credential, or trust in a host key or a certificate). An answer that does not satisfy the
    /// server fails with the typed error that says what is still needed. `cancel` stops a slow
    /// attempt with `Cancelled`.
    fn connect(
        &self,
        key: &ConnectionKey,
        answer: Option<ConnectAnswer>,
        cancel: &CancelToken,
    ) -> Result<(), VfsError> {
        let _ = (key, answer, cancel);
        Ok(())
    }

    /// Closes a session. Calls in flight on it end with `Disconnected`; the next call reconnects.
    fn disconnect(&self, key: &ConnectionKey) {
        let _ = key;
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

    /// Opens a partial file to continue writing it at `offset` (its length so far), for a
    /// transfer that resumes after a failure (D62, A84). Data past `offset` is discarded. Only a
    /// provider whose `Capabilities::resume_write` is set implements it.
    fn resume_write(&self, path: &VfsPath, offset: u64) -> Result<Box<dyn WriteStream>, VfsError> {
        let _ = (path, offset);
        unsupported("resuming a write here")
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
