// The trait every file system provider implements, and the types it speaks.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::ffi::OsString;
use std::sync::Arc;

use waypoint_path::{CaseRule, VfsPath};
use waypoint_protocol::VfsError;

use crate::{CancelToken, EntryKind, IconGroup};

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
}
