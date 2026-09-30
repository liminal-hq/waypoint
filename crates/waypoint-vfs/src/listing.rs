// A listing: one open folder, its sorted and filtered index, and the events it raises.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use waypoint_path::VfsPath;
use waypoint_protocol::{Location, VfsError};

use crate::index::Index;
use crate::model::{
    Entry, Filter, ListingEvent, ListingHandle, ListingPhase, ListingSnapshot, PatchOp, SortSpec,
};
use crate::provider::{Change, Provider};
use crate::CancelToken;

/// Where a listing sends its events. It may be called from any thread, and never while the listing
/// holds a lock a reader needs, so a sink can read the listing back.
pub type EventSink = Arc<dyn Fn(ListingEvent) + Send + Sync>;

/// How many symlinks the background pass resolves between patches.
const LINK_BATCH: usize = 256;

#[derive(Debug, Clone, Copy)]
pub struct ListingOptions {
    /// How many symlink targets a scan resolves inline before leaving the rest to the background
    /// pass (`Listing::resolve_pending_links`). Symlinks are rare, so this is almost never reached.
    pub inline_link_budget: usize,
    /// The least time between two scan progress events.
    pub progress_interval: Duration,
}

impl Default for ListingOptions {
    fn default() -> Self {
        Self {
            inline_link_budget: 10_000,
            progress_interval: Duration::from_millis(50),
        }
    }
}

struct State {
    revision: u32,
    phase: ListingPhase,
    index: Index,
}

/// One open folder.
///
/// The listing owns the rows. A caller reads ranges of the current view (`get_range`) and follows
/// `ListingEvent`s; the events carry counts and patch positions, never entries. Every change bumps
/// `revision`, so a late reply can never overwrite newer state.
///
/// Operations that change the listing are serialised, and events are emitted in revision order,
/// after the change is visible to readers.
pub struct Listing {
    handle: ListingHandle,
    path: VfsPath,
    location: Location,
    provider: Arc<dyn Provider>,
    sink: EventSink,
    options: ListingOptions,
    cancel: CancelToken,
    /// Held across a change and its event, so events leave in revision order.
    mutation: Mutex<()>,
    state: RwLock<State>,
}

impl Listing {
    /// Creates a listing in the `Scanning` phase with nothing in it. Call `scan` (blocking) on a
    /// worker thread to fill it.
    pub fn new(
        handle: ListingHandle,
        path: VfsPath,
        provider: Arc<dyn Provider>,
        sort: SortSpec,
        filter: Filter,
        options: ListingOptions,
        sink: EventSink,
    ) -> Arc<Self> {
        Arc::new(Self {
            handle,
            location: path.to_location(),
            path,
            provider,
            sink,
            options,
            cancel: CancelToken::new(),
            mutation: Mutex::new(()),
            state: RwLock::new(State {
                revision: 1,
                phase: ListingPhase::Scanning,
                index: Index::new(sort, filter),
            }),
        })
    }

    /// Creates a listing and scans it to completion.
    pub fn open(
        handle: ListingHandle,
        path: VfsPath,
        provider: Arc<dyn Provider>,
        sort: SortSpec,
        filter: Filter,
        options: ListingOptions,
        sink: EventSink,
    ) -> Result<Arc<Self>, VfsError> {
        let listing = Self::new(handle, path, provider, sort, filter, options, sink);
        listing.scan()?;
        Ok(listing)
    }

    pub fn handle(&self) -> ListingHandle {
        self.handle
    }

    pub fn path(&self) -> &VfsPath {
        &self.path
    }

    pub fn provider(&self) -> &Arc<dyn Provider> {
        &self.provider
    }

    /// The token that stops an in-flight scan. `close` cancels it.
    pub fn cancel_token(&self) -> &CancelToken {
        &self.cancel
    }

    /// Stops any scan in flight. Reading the listing afterwards still works until it is dropped.
    pub fn close(&self) {
        self.cancel.cancel();
    }

    fn snapshot_of(&self, state: &State) -> ListingSnapshot {
        ListingSnapshot {
            handle: self.handle,
            location: self.location.clone(),
            revision: state.revision,
            count: state.index.count(),
            phase: state.phase,
            sort: state.index.sort(),
            filter: state.index.filter(),
        }
    }

    fn read(&self) -> std::sync::RwLockReadGuard<'_, State> {
        self.state.read().unwrap_or_else(|e| e.into_inner())
    }

    fn write(&self) -> std::sync::RwLockWriteGuard<'_, State> {
        self.state.write().unwrap_or_else(|e| e.into_inner())
    }

    fn serialise(&self) -> std::sync::MutexGuard<'_, ()> {
        self.mutation.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn emit(&self, event: ListingEvent) {
        (self.sink)(event);
    }

    pub fn snapshot(&self) -> ListingSnapshot {
        self.snapshot_of(&self.read())
    }

    /// Runs the first scan to completion: progress events while it reads, then one `Ready` event
    /// carrying the final count. Blocks, so call it from a worker thread. Stops early with
    /// `VfsError::Cancelled` when the listing is closed.
    pub fn scan(&self) -> Result<ListingSnapshot, VfsError> {
        let mut last = None::<Instant>;
        let mut progress = |scanned: u32| {
            let due = last.is_none_or(|at| at.elapsed() >= self.options.progress_interval);
            if due {
                last = Some(Instant::now());
                self.emit(ListingEvent::Progress {
                    handle: self.handle,
                    revision: self.read().revision,
                    phase: ListingPhase::Scanning,
                    scanned,
                    count: scanned,
                });
            }
        };
        let scanned = self.provider.list(
            &self.path,
            &self.cancel,
            self.options.inline_link_budget,
            &mut progress,
        );
        let _turn = self.serialise();
        let entries = match scanned {
            Ok(entries) => entries,
            Err(error) => {
                self.write().phase = ListingPhase::Failed;
                return Err(error);
            }
        };
        let total = entries.len() as u32;
        let (snapshot, event) = {
            let mut state = self.write();
            state.index.load(entries);
            state.revision += 1;
            state.phase = ListingPhase::Ready;
            let snapshot = self.snapshot_of(&state);
            let event = ListingEvent::Progress {
                handle: self.handle,
                revision: snapshot.revision,
                phase: ListingPhase::Ready,
                scanned: total,
                count: snapshot.count,
            };
            (snapshot, event)
        };
        self.emit(event);
        Ok(snapshot)
    }

    /// Up to `count` entries from view position `start`; shorter at the end.
    pub fn get_range(&self, start: u32, count: u32) -> Vec<Entry> {
        self.read().index.range(start, count)
    }

    /// The view position of an entry, or `None` if it is filtered out or gone.
    pub fn position_of(&self, id: waypoint_protocol::EntryId) -> Option<u32> {
        self.read().index.position_of(id.0)
    }

    /// The id a name has in this listing, or `None` if no entry has it.
    pub fn id_of(&self, name: &std::ffi::OsStr) -> Option<waypoint_protocol::EntryId> {
        self.write()
            .index
            .id_of(name)
            .map(waypoint_protocol::EntryId)
    }

    /// Re-sorts the view and returns the new state. Cached pages are stale afterwards.
    pub fn set_sort(&self, sort: SortSpec) -> ListingSnapshot {
        let _turn = self.serialise();
        let mut state = self.write();
        if state.index.sort() != sort {
            state.index.set_sort(sort);
            state.revision += 1;
        }
        self.snapshot_of(&state)
    }

    /// Changes what the view leaves out and returns the new state. Cached pages are stale.
    pub fn set_filter(&self, filter: Filter) -> ListingSnapshot {
        let _turn = self.serialise();
        let mut state = self.write();
        if state.index.filter() != filter {
            state.index.set_filter(filter);
            state.revision += 1;
        }
        self.snapshot_of(&state)
    }

    /// The path an entry names, for operations on it (opening it, for one). Rust resolves this
    /// from the `EntryId`; the frontend never holds a raw path.
    pub fn path_of(&self, id: waypoint_protocol::EntryId) -> Result<VfsPath, VfsError> {
        let state = self.read();
        let name = state.index.name_of(id.0).ok_or(VfsError::NotFound {
            location: self.location.clone(),
        })?;
        self.path.join(name).map_err(|_| VfsError::InvalidLocation {
            input: name.to_string_lossy().into_owned(),
        })
    }

    /// Applies changes by name and raises one `Changed` event if the view moved. Watchers and the
    /// rescan call this; it is idempotent, so a repeated change is harmless.
    pub fn apply_changes(&self, changes: Vec<Change>) -> Vec<PatchOp> {
        let _turn = self.serialise();
        let (ops, event) = {
            let mut state = self.write();
            let ops = state.index.apply(changes);
            if ops.is_empty() {
                return ops;
            }
            state.revision += 1;
            let event = ListingEvent::Changed {
                handle: self.handle,
                revision: state.revision,
                count: state.index.count(),
                ops: ops.clone(),
            };
            (ops, event)
        };
        self.emit(event);
        ops
    }

    /// The bounded background pass for symlinks the scan left unresolved: resolves them in batches,
    /// applying each batch as a patch, and stops early if the listing is closed. Returns how many
    /// it resolved. A no-op, and cheap, when the scan resolved everything inline.
    pub fn resolve_pending_links(&self) -> Result<usize, VfsError> {
        let pending = self.read().index.pending_links();
        let mut resolved = 0;
        for batch in pending.chunks(LINK_BATCH) {
            let mut changes = Vec::with_capacity(batch.len());
            for entry in batch {
                if self.cancel.is_cancelled() {
                    return Err(VfsError::Cancelled);
                }
                changes.push(Change::Upsert(
                    self.provider.resolve_link(&self.path, entry)?,
                ));
            }
            resolved += changes.len();
            self.apply_changes(changes);
        }
        Ok(resolved)
    }
}
