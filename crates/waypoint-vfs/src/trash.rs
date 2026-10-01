// The Trash as a read-only provider: `trash:/` lists what the Trash holds, each item under the name
// it had, with where it came from and when it was trashed.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The provider is pure. What the Trash holds comes through the `TrashSource` trait, which the app
// implements over the Trash plugin (A4: plugins never call each other) and tests implement over
// `MemoryTrashSource`. Entries are named by the item's receipt id, which is unique and travels
// losslessly in the `trash:/{id}` location (A19); the name people see, the original location and the
// deletion date ride along in `ScannedEntry::trashed`. Everything that writes is `Unsupported`:
// restoring and deleting are jobs of the operations engine, not primitives of a provider.

use std::collections::HashMap;
use std::ffi::OsString;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use waypoint_path::{CaseRule, TrashPath, VfsPath};
use waypoint_protocol::VfsError;

use crate::icon::group_for;
use crate::provider::{
    Capabilities, Change, Provider, ScannedEntry, TrashedMeta, Watch, WatchEvent, WatchSink,
};
use crate::{CancelToken, EntryKind, IconGroup, ListingLayout};

/// One item in the Trash, as the Trash knows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrashedItem {
    /// Names the item in the Trash and in its `trash:/{id}` location; stable until it leaves.
    pub id: String,
    /// The name it had before it was trashed.
    pub name: String,
    /// The folder it was trashed from, as people read it.
    pub original_path: String,
    /// When it was trashed, in milliseconds since the Unix epoch.
    pub deleted_ms: i64,
    /// Size in bytes; for a folder, the total of what is in it.
    pub size: u64,
    pub is_dir: bool,
}

/// What the Trash provider reads the Trash through, and what the operations that change it call.
pub trait TrashSource: Send + Sync {
    /// Whether the Trash can be read here, and if not why (shown where the list would be, and in
    /// the Services panel).
    fn available(&self) -> Result<(), String>;

    /// Everything in the Trash, on every volume, in no particular order.
    fn list(&self) -> Result<Vec<TrashedItem>, VfsError>;

    /// Puts the item back where it was and returns that place. A name already taken there is
    /// `AlreadyExists`, and nothing is replaced.
    fn restore(&self, id: &str) -> Result<waypoint_protocol::Location, VfsError>;

    /// Removes one item for good.
    fn delete(&self, id: &str) -> Result<(), VfsError>;

    /// Removes everything, or only what was trashed more than `older_than_days` days ago, and
    /// returns how many items went.
    fn empty(&self, older_than_days: Option<u32>) -> Result<u64, VfsError>;

    /// A cheap token that differs whenever the Trash may have changed since the last time it was
    /// asked, so a watcher can skip a full `list` when nothing happened. `None` (the default)
    /// means the source cannot tell, and the watcher lists on every tick.
    fn changed(&self) -> Option<u64> {
        None
    }
}

/// What the sidebar's Trash place needs to know without opening a listing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct TrashInfo {
    /// Whether the Trash can be browsed here.
    pub available: bool,
    /// Why it cannot, when it cannot.
    pub reason: Option<String>,
    /// How many items it holds (0 when it cannot be read).
    pub count: u32,
}

impl TrashInfo {
    /// Reads the state of `source`: availability first, then the count.
    pub fn of(source: &dyn TrashSource) -> Self {
        if let Err(reason) = source.available() {
            return Self {
                available: false,
                reason: Some(reason),
                count: 0,
            };
        }
        match source.list() {
            Ok(items) => Self {
                available: true,
                reason: None,
                count: u32::try_from(items.len()).unwrap_or(u32::MAX),
            },
            Err(error) => Self {
                available: false,
                reason: Some(format!("{error:?}")),
                count: 0,
            },
        }
    }
}

/// How often a listing of the Trash looks for changes when the source gives no hint of its own.
pub const POLL_INTERVAL: Duration = Duration::from_secs(2);

/// How many failed looks in a row end a watch.
const POLL_FAILURES: u32 = 3;

/// The Trash behind the `trash` scheme. Read-only: every write primitive is `Unsupported`.
pub struct TrashProvider {
    source: Arc<dyn TrashSource>,
    poll: Duration,
}

impl TrashProvider {
    pub fn new(source: Arc<dyn TrashSource>) -> Self {
        Self::with_poll(source, POLL_INTERVAL)
    }

    /// A provider whose watchers look this often.
    pub fn with_poll(source: Arc<dyn TrashSource>, poll: Duration) -> Self {
        Self { source, poll }
    }

    pub fn source(&self) -> &Arc<dyn TrashSource> {
        &self.source
    }

    fn trash_path(path: &VfsPath) -> Result<&TrashPath, VfsError> {
        match path {
            VfsPath::Trash(path) => Ok(path),
            other => Err(VfsError::Unsupported {
                what: format!("the {} scheme in the Trash provider", other.scheme()),
            }),
        }
    }

    fn check_available(&self) -> Result<(), VfsError> {
        self.source
            .available()
            .map_err(|what| VfsError::Unsupported { what })
    }
}

/// The entry for an item. Its name is the id (so it is unique and resolves to `trash:/{id}`); a
/// folder reads as a folder and anything else as a file, whatever it was.
fn entry_of(item: &TrashedItem) -> ScannedEntry {
    let kind = if item.is_dir {
        EntryKind::Directory
    } else {
        EntryKind::File
    };
    ScannedEntry {
        name: OsString::from(&item.id),
        kind,
        link_target: None,
        link_pending: false,
        group: group_for(item.name.as_bytes(), kind, None),
        size: Some(item.size),
        // What a modification time would say is not known; the deletion date is its own column.
        modified_ms: None,
        hidden: false,
        trashed: Some(Box::new(TrashedMeta {
            display_name: item.name.clone(),
            original_path: item.original_path.clone(),
            deleted_ms: item.deleted_ms,
        })),
    }
}

fn root_entry() -> ScannedEntry {
    ScannedEntry {
        name: OsString::from("Trash"),
        kind: EntryKind::Directory,
        link_target: None,
        link_pending: false,
        group: IconGroup::Folder,
        size: None,
        modified_ms: None,
        hidden: false,
        trashed: None,
    }
}

impl Provider for TrashProvider {
    fn scheme(&self) -> &'static str {
        waypoint_path::TRASH_SCHEME
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            watch: true,
            // Ids are compared as they are.
            case_rule: CaseRule::Sensitive,
        }
    }

    fn read_only(&self) -> bool {
        true
    }

    fn layout(&self) -> ListingLayout {
        ListingLayout::Trash
    }

    fn stat(&self, path: &VfsPath) -> Result<ScannedEntry, VfsError> {
        match Self::trash_path(path)? {
            TrashPath::Root => {
                self.check_available()?;
                Ok(root_entry())
            }
            TrashPath::Item(id) => {
                self.check_available()?;
                self.source
                    .list()?
                    .iter()
                    .find(|item| &item.id == id)
                    .map(entry_of)
                    .ok_or_else(|| VfsError::NotFound {
                        location: path.to_location(),
                    })
            }
        }
    }

    fn list(
        &self,
        path: &VfsPath,
        cancel: &CancelToken,
        _inline_link_budget: usize,
        progress: &mut dyn FnMut(u32),
    ) -> Result<Vec<ScannedEntry>, VfsError> {
        match Self::trash_path(path)? {
            TrashPath::Root => {}
            TrashPath::Item(_) => {
                return Err(VfsError::Unsupported {
                    what: "looking inside the Trash".to_owned(),
                })
            }
        }
        self.check_available()?;
        let mut entries = Vec::new();
        for item in self.source.list()? {
            if cancel.is_cancelled() {
                return Err(VfsError::Cancelled);
            }
            entries.push(entry_of(&item));
        }
        progress(entries.len() as u32);
        Ok(entries)
    }

    fn resolve_link(
        &self,
        _folder: &VfsPath,
        entry: &ScannedEntry,
    ) -> Result<ScannedEntry, VfsError> {
        // Nothing in the Trash is left unresolved by a scan.
        Ok(entry.clone())
    }

    /// Watches the Trash by looking at it every `poll`, or when the source's `changed` token
    /// moves, and reporting the difference as changes by id.
    fn watch(&self, path: &VfsPath, sink: WatchSink) -> Result<Box<dyn Watch>, VfsError> {
        match Self::trash_path(path)? {
            TrashPath::Root => {}
            TrashPath::Item(_) => {
                return Err(VfsError::Unsupported {
                    what: "watching inside the Trash".to_owned(),
                })
            }
        }
        self.check_available()?;
        let source = self.source.clone();
        let poll = self.poll;
        let token = source.changed();
        let known: HashMap<String, ScannedEntry> = source
            .list()?
            .iter()
            .map(|item| (item.id.clone(), entry_of(item)))
            .collect();
        let (stop, stopped) = mpsc::channel::<()>();
        std::thread::Builder::new()
            .name("trash-watch".to_owned())
            .spawn(move || poll_loop(source, poll, token, known, stopped, sink))
            .map_err(|error| VfsError::Io {
                message: error.to_string(),
                location: None,
            })?;
        Ok(Box::new(PollWatch { _stop: stop }))
    }
}

/// Dropping this disconnects the channel the poll thread waits on, which ends it.
struct PollWatch {
    _stop: Sender<()>,
}

impl Watch for PollWatch {}

fn poll_loop(
    source: Arc<dyn TrashSource>,
    poll: Duration,
    mut token: Option<u64>,
    mut known: HashMap<String, ScannedEntry>,
    stopped: mpsc::Receiver<()>,
    sink: WatchSink,
) {
    let mut failures = 0;
    loop {
        match stopped.recv_timeout(poll) {
            Err(RecvTimeoutError::Timeout) => {}
            // A message or a closed channel: the watch was dropped.
            _ => return,
        }
        let now = source.changed();
        if now.is_some() && now == token {
            continue;
        }
        match source.list() {
            Ok(items) => {
                failures = 0;
                token = now;
                let changes = diff(&mut known, &items);
                if !changes.is_empty() {
                    sink(WatchEvent::Changes(changes));
                }
            }
            Err(error) => {
                failures += 1;
                if failures >= POLL_FAILURES {
                    sink(WatchEvent::Lost(error));
                    return;
                }
            }
        }
    }
}

/// The changes that take `known` to what `items` says, which becomes the new `known`.
fn diff(known: &mut HashMap<String, ScannedEntry>, items: &[TrashedItem]) -> Vec<Change> {
    let mut changes = Vec::new();
    let mut seen: HashMap<String, ScannedEntry> = HashMap::with_capacity(items.len());
    for item in items {
        let entry = entry_of(item);
        if known.get(&item.id) != Some(&entry) {
            changes.push(Change::Upsert(entry.clone()));
        }
        seen.insert(item.id.clone(), entry);
    }
    for id in known.keys() {
        if !seen.contains_key(id) {
            changes.push(Change::Remove(OsString::from(id)));
        }
    }
    *known = seen;
    changes
}

#[cfg(any(test, feature = "testing"))]
pub use memory::MemoryTrashSource;

#[cfg(any(test, feature = "testing"))]
mod memory {
    use std::sync::{Mutex, MutexGuard};

    use waypoint_path::FilePath;
    use waypoint_protocol::{Location, VfsError};

    use super::{TrashSource, TrashedItem};

    #[derive(Default)]
    struct Inner {
        items: Vec<TrashedItem>,
        unavailable: Option<String>,
        counter: u64,
        hints: bool,
        now_ms: i64,
        lists: u32,
        fail_lists: Option<VfsError>,
        /// The ids restored, deleted and the calls to empty, in order, for assertions.
        log: Vec<String>,
    }

    /// A Trash that lives in memory, for tests.
    #[derive(Default)]
    pub struct MemoryTrashSource {
        inner: Mutex<Inner>,
    }

    impl MemoryTrashSource {
        pub fn new() -> Self {
            Self::default()
        }

        fn lock(&self) -> MutexGuard<'_, Inner> {
            self.inner.lock().unwrap_or_else(|e| e.into_inner())
        }

        /// Puts an item in the Trash.
        pub fn add(&self, item: TrashedItem) {
            let mut inner = self.lock();
            inner.items.retain(|have| have.id != item.id);
            inner.items.push(item);
            inner.counter += 1;
        }

        /// Takes an item out without any of the source's own steps, as another program would.
        pub fn remove(&self, id: &str) {
            let mut inner = self.lock();
            inner.items.retain(|have| have.id != id);
            inner.counter += 1;
        }

        /// Makes `available` fail with this reason (or work again with `None`).
        pub fn set_unavailable(&self, reason: Option<&str>) {
            self.lock().unavailable = reason.map(str::to_owned);
        }

        /// Makes `changed` report a token that moves with every change (the default is no token).
        pub fn give_hints(&self, on: bool) {
            self.lock().hints = on;
        }

        /// Sets the time `empty` measures age from.
        pub fn set_now_ms(&self, now_ms: i64) {
            self.lock().now_ms = now_ms;
        }

        /// Makes `list` fail with this error until it is cleared with `None`.
        pub fn fail_lists(&self, error: Option<VfsError>) {
            self.lock().fail_lists = error;
        }

        /// How many times `list` was called.
        pub fn list_calls(&self) -> u32 {
            self.lock().lists
        }

        pub fn len(&self) -> usize {
            self.lock().items.len()
        }

        pub fn is_empty(&self) -> bool {
            self.len() == 0
        }

        /// What was done to the Trash through the trait, as `restore:{id}`, `delete:{id}` and
        /// `empty:{days}` (`all` for no limit).
        pub fn log(&self) -> Vec<String> {
            self.lock().log.clone()
        }
    }

    impl TrashSource for MemoryTrashSource {
        fn available(&self) -> Result<(), String> {
            match &self.lock().unavailable {
                Some(reason) => Err(reason.clone()),
                None => Ok(()),
            }
        }

        fn list(&self) -> Result<Vec<TrashedItem>, VfsError> {
            let mut inner = self.lock();
            inner.lists += 1;
            if let Some(error) = &inner.fail_lists {
                return Err(error.clone());
            }
            Ok(inner.items.clone())
        }

        fn restore(&self, id: &str) -> Result<Location, VfsError> {
            let mut inner = self.lock();
            let at = inner
                .items
                .iter()
                .position(|item| item.id == id)
                .ok_or_else(|| VfsError::NotFound {
                    location: Location::new(id, format!("trash:/{id}")),
                })?;
            let item = inner.items.remove(at);
            inner.counter += 1;
            inner.log.push(format!("restore:{id}"));
            FilePath::parse(&item.original_path)
                .and_then(|folder| folder.join(&item.name))
                .map(|path| path.to_location())
                .map_err(|_| VfsError::InvalidLocation {
                    input: item.original_path,
                })
        }

        fn delete(&self, id: &str) -> Result<(), VfsError> {
            let mut inner = self.lock();
            let before = inner.items.len();
            inner.items.retain(|item| item.id != id);
            if inner.items.len() == before {
                return Err(VfsError::NotFound {
                    location: Location::new(id, format!("trash:/{id}")),
                });
            }
            inner.counter += 1;
            inner.log.push(format!("delete:{id}"));
            Ok(())
        }

        fn empty(&self, older_than_days: Option<u32>) -> Result<u64, VfsError> {
            let mut inner = self.lock();
            let cutoff = older_than_days.map(|days| inner.now_ms - i64::from(days) * 86_400_000);
            let before = inner.items.len();
            inner
                .items
                .retain(|item| cutoff.is_some_and(|cutoff| item.deleted_ms > cutoff));
            let removed = (before - inner.items.len()) as u64;
            inner.counter += 1;
            inner.log.push(format!(
                "empty:{}",
                older_than_days.map_or("all".to_owned(), |d| d.to_string())
            ));
            Ok(removed)
        }

        fn changed(&self) -> Option<u64> {
            let inner = self.lock();
            inner.hints.then_some(inner.counter)
        }
    }
}
