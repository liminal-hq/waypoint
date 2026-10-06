// A listing: one open folder, its sorted and filtered index, and the events it raises.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use waypoint_path::VfsPath;
use waypoint_protocol::{EntryId, Location, VfsError};

use crate::index::Index;
use crate::model::{
    Entry, Filter, GroupBy, GroupRun, ListingEvent, ListingHandle, ListingLayout, ListingPhase,
    ListingSnapshot, PatchOp, SortSpec,
};
use crate::overlay::{FolderOverlay, OverlayGuard};
use crate::provider::{Change, Provider, Watch, WatchEvent, WatchSink};
use crate::CancelToken;

/// Where a listing sends its events. It may be called from any thread, and never while the listing
/// holds a lock a reader needs, so a sink can read the listing back.
pub type EventSink = Arc<dyn Fn(ListingEvent) + Send + Sync>;

/// How many symlinks the background pass resolves between patches.
const LINK_BATCH: usize = 256;

#[derive(Debug, Clone, Copy)]
pub struct ListingOptions {
    /// Whether `Listing::open` watches the folder and keeps the listing up to date.
    pub watch: bool,
    /// How many symlink targets a scan resolves inline before leaving the rest to the background
    /// pass (`Listing::resolve_pending_links`). Symlinks are rare, so this is almost never reached.
    pub inline_link_budget: usize,
    /// The least time between two scan progress events.
    pub progress_interval: Duration,
}

impl Default for ListingOptions {
    fn default() -> Self {
        Self {
            watch: true,
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

/// How a listing is being kept up to date, for the Services status panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WatchState {
    /// Not watching: before `start_watching`, or after `close`.
    Off,
    /// The operating system reports changes.
    Native,
    /// Native watching failed or is unavailable, so the folder is polled. `reason` says why.
    Polling { reason: String },
    /// Nothing keeps this listing current; it changes only when rescanned.
    Unavailable { reason: String },
}

/// Watcher events that arrive while a scan is running, replayed when it finishes.
#[derive(Default)]
struct Held {
    changes: Vec<Change>,
    rescan: bool,
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
    watch: Mutex<Option<Box<dyn Watch>>>,
    watch_state: Mutex<WatchState>,
    held: Mutex<Held>,
    /// Keeps an overlay's marks coming; dropped by `close`.
    overlay: Mutex<Option<Box<dyn OverlayGuard>>>,
}

impl Listing {
    /// The sort a listing of this layout can use: the Trash lists the items it holds with their
    /// own columns, so it does not group.
    fn sort_for(layout: ListingLayout, sort: SortSpec) -> SortSpec {
        match layout {
            ListingLayout::Trash => SortSpec {
                group_by: GroupBy::None,
                ..sort
            },
            ListingLayout::Folder => sort,
        }
    }

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
        let sort = Self::sort_for(provider.layout(), sort);
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
            watch: Mutex::new(None),
            watch_state: Mutex::new(WatchState::Off),
            held: Mutex::new(Held::default()),
            overlay: Mutex::new(None),
        })
    }

    /// Creates a listing, starts watching it if `options.watch` is set (before the scan, so no
    /// change between the scan and the watch is missed), and scans it to completion.
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
        if options.watch {
            // A folder that cannot be watched is still worth listing; the scan reports real errors.
            let _ = listing.start_watching();
        }
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

    /// Stops any scan in flight and the watcher. Reading the listing afterwards still works until
    /// it is dropped.
    pub fn close(&self) {
        self.cancel.cancel();
        let watch = self.watch.lock().unwrap_or_else(|e| e.into_inner()).take();
        drop(watch);
        let overlay = self
            .overlay
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
        drop(overlay);
        *self.watch_state.lock().unwrap_or_else(|e| e.into_inner()) = WatchState::Off;
    }

    /// How this listing is being kept up to date.
    pub fn watch_state(&self) -> WatchState {
        self.watch_state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    fn set_watch_state(&self, state: WatchState) {
        *self.watch_state.lock().unwrap_or_else(|e| e.into_inner()) = state;
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
            read_only: self.provider.read_only(),
            rewritable: self.provider.rewritable(&self.path),
            layout: self.provider.layout(),
            groups: state.index.groups(),
        }
    }

    /// The groups to send with an event: the view's, when the sort groups.
    fn groups_of(state: &State) -> Option<Vec<GroupRun>> {
        (state.index.sort().group_by != GroupBy::None).then(|| state.index.groups())
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
    ///
    /// Changes the watcher reports while the scan runs are held and applied right after it, so a
    /// change between the scan reading an entry and the listing going live is not lost.
    ///
    /// A remote provider's listing streams (A83, A115): each batch `list_batches` hands over joins
    /// the index at its sorted place and is published with a new revision and the count readers
    /// can fetch, so the first rows are drawn while the rest are on their way. Publishing waits at
    /// least `progress_interval` after the first batch, so a reader's page request outlives the
    /// revision it was made for. A local folder is read whole, as before, with progress counts.
    pub fn scan(&self) -> Result<ListingSnapshot, VfsError> {
        let streamed = self.provider.capabilities().remote;
        let scanned = if streamed {
            self.stream_folder().map(|rest| (rest, true))
        } else {
            self.list_folder(ListingPhase::Scanning)
                .map(|entries| (entries, false))
        };
        let turn = self.serialise();
        let (entries, extend) = match scanned {
            Ok(scanned) => scanned,
            Err(error) => {
                self.write().phase = ListingPhase::Failed;
                return Err(error);
            }
        };
        let event = {
            let mut state = self.write();
            if state.phase == ListingPhase::Failed {
                // The watch was lost while the folder was read; stay failed rather than go live
                // with nothing keeping the view current.
                return Err(VfsError::StaleHandle);
            }
            if extend {
                state.index.extend(entries);
            } else {
                state.index.load(entries);
            }
            let total = state.index.held();
            state.revision += 1;
            state.phase = ListingPhase::Ready;
            ListingEvent::Progress {
                handle: self.handle,
                revision: state.revision,
                phase: ListingPhase::Ready,
                scanned: total,
                count: state.index.count(),
                groups: Self::groups_of(&state),
            }
        };
        self.emit(event);
        let rescan = self.replay_held();
        drop(turn);
        if rescan {
            // The watcher lost events during the scan; the view may be stale.
            self.rescan()?;
        }
        Ok(self.snapshot())
    }

    /// Reads the folder in batches, publishing what has arrived into the index as it goes (see
    /// `scan`), and returns the entries not yet published, for the caller to add under its turn.
    fn stream_folder(&self) -> Result<Vec<crate::ScannedEntry>, VfsError> {
        let mut pending: Vec<crate::ScannedEntry> = Vec::new();
        let mut received = 0u32;
        let mut last = None::<Instant>;
        let mut publish = |batch: Vec<crate::ScannedEntry>| {
            received = received.saturating_add(batch.len() as u32);
            if pending.is_empty() {
                pending = batch;
            } else {
                pending.extend(batch);
            }
            let due = last.is_none_or(|at| at.elapsed() >= self.options.progress_interval);
            if !due || pending.is_empty() {
                return;
            }
            last = Some(Instant::now());
            let _turn = self.serialise();
            let event = {
                let mut state = self.write();
                if state.phase != ListingPhase::Scanning {
                    // Failed meanwhile: the scan's end reports it.
                    return;
                }
                state.index.extend(std::mem::take(&mut pending));
                state.revision += 1;
                ListingEvent::Progress {
                    handle: self.handle,
                    revision: state.revision,
                    phase: ListingPhase::Scanning,
                    scanned: received,
                    count: state.index.count(),
                    groups: Self::groups_of(&state),
                }
            };
            self.emit(event);
        };
        self.provider.list_batches(
            &self.path,
            &self.cancel,
            self.options.inline_link_budget,
            &mut publish,
        )?;
        Ok(pending)
    }

    /// Reads the folder, reporting progress in `phase`.
    fn list_folder(&self, phase: ListingPhase) -> Result<Vec<crate::ScannedEntry>, VfsError> {
        let mut last = None::<Instant>;
        let mut progress = |scanned: u32| {
            let due = last.is_none_or(|at| at.elapsed() >= self.options.progress_interval);
            if due {
                last = Some(Instant::now());
                self.emit(ListingEvent::Progress {
                    handle: self.handle,
                    revision: self.read().revision,
                    phase,
                    scanned,
                    count: scanned,
                    groups: None,
                });
            }
        };
        self.provider.list(
            &self.path,
            &self.cancel,
            self.options.inline_link_budget,
            &mut progress,
        )
    }

    /// Applies what the watcher reported during a scan. The caller holds the mutation turn and
    /// learns whether a rescan was asked for.
    fn replay_held(&self) -> bool {
        let held = std::mem::take(&mut *self.held.lock().unwrap_or_else(|e| e.into_inner()));
        if !held.changes.is_empty() {
            self.apply_locked(held.changes);
        }
        held.rescan
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
        let sort = Self::sort_for(self.provider.layout(), sort);
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

    /// The count and total file size of a selection over the current view.
    pub fn summarise_selection(&self, selection: &crate::SelectionSpec) -> crate::SelectionSummary {
        self.read().index.summarise(selection)
    }

    /// The paths a selection covers over the current view, in view order, so an operation takes
    /// its sources from the listing and the frontend never builds a path. Ids the view no longer
    /// holds are dropped, as they are by `summarise_selection`.
    pub fn resolve_selection(
        &self,
        selection: &crate::SelectionSpec,
    ) -> Result<Vec<VfsPath>, VfsError> {
        let state = self.read();
        state
            .index
            .selected_names(selection)
            .into_iter()
            .map(|name| {
                self.path.join(name).map_err(|_| VfsError::InvalidLocation {
                    input: name.to_string_lossy().into_owned(),
                })
            })
            .collect()
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
        self.apply_locked(changes)
    }

    /// `apply_changes` for a caller that already holds the mutation turn.
    fn apply_locked(&self, changes: Vec<Change>) -> Vec<PatchOp> {
        let (ops, event) = {
            let mut state = self.write();
            if state.phase == ListingPhase::Failed {
                return Vec::new();
            }
            let mut moved = Vec::new();
            let ops = state.index.apply_tracking(changes, &mut moved);
            if ops.is_empty() {
                return ops;
            }
            state.revision += 1;
            let event = ListingEvent::Changed {
                handle: self.handle,
                revision: state.revision,
                count: state.index.count(),
                ops: ops.clone(),
                moved: moved.into_iter().map(EntryId).collect(),
                groups: Self::groups_of(&state),
            };
            (ops, event)
        };
        self.emit(event);
        ops
    }

    /// Replaces the marks an overlay puts on the entries (the Git status, D161) and raises one
    /// `Changed` event if any row changed. Marks for names the listing does not hold yet wait for
    /// them, and a name that arrives later takes its mark.
    pub fn set_marks(&self, marks: crate::FolderMarks) -> Vec<PatchOp> {
        let _turn = self.serialise();
        let (ops, event) = {
            let mut state = self.write();
            let mut moved = Vec::new();
            let ops = state.index.set_marks(marks, &mut moved);
            if ops.is_empty() {
                return ops;
            }
            state.revision += 1;
            let event = ListingEvent::Changed {
                handle: self.handle,
                revision: state.revision,
                count: state.index.count(),
                ops: ops.clone(),
                moved: moved.into_iter().map(EntryId).collect(),
                groups: Self::groups_of(&state),
            };
            (ops, event)
        };
        self.emit(event);
        ops
    }

    /// Lets `overlay` decorate this listing until it closes: its marks arrive through `set_marks`.
    /// The listing is held weakly, so an overlay that outlives it does no harm.
    pub fn attach_overlay(self: &Arc<Self>, overlay: &dyn FolderOverlay) {
        let weak = Arc::downgrade(self);
        let sink: crate::MarkSink = Arc::new(move |marks| {
            if let Some(listing) = weak.upgrade() {
                listing.set_marks(marks);
            }
        });
        if let Some(guard) = overlay.attach(&self.path, sink) {
            *self.overlay.lock().unwrap_or_else(|e| e.into_inner()) = Some(guard);
        }
    }

    /// Starts keeping this listing up to date through the provider's watcher. Call it before
    /// `scan` (`open` does), so nothing between the scan and the watch is missed.
    ///
    /// When the provider cannot watch, the listing still works and `watch_state` says why it will
    /// not update by itself. An error means the folder cannot be watched at all (it does not exist
    /// or may not be read).
    pub fn start_watching(self: &Arc<Self>) -> Result<(), VfsError> {
        if !self.provider.capabilities().watch {
            self.set_watch_state(WatchState::Unavailable {
                reason: "this location cannot be watched".to_owned(),
            });
            return Ok(());
        }
        let weak = Arc::downgrade(self);
        let sink: WatchSink = Arc::new(move |event| {
            if let Some(listing) = weak.upgrade() {
                listing.on_watch_event(event);
            }
        });
        match self.provider.watch(&self.path, sink) {
            Ok(watch) => {
                *self.watch.lock().unwrap_or_else(|e| e.into_inner()) = Some(watch);
                let mut state = self.watch_state.lock().unwrap_or_else(|e| e.into_inner());
                // A polling fallback has already said so, from inside `watch`.
                if *state == WatchState::Off {
                    *state = WatchState::Native;
                }
                Ok(())
            }
            Err(VfsError::Unsupported { what }) => {
                self.set_watch_state(WatchState::Unavailable { reason: what });
                Ok(())
            }
            Err(error) => {
                self.set_watch_state(WatchState::Unavailable {
                    reason: format!("{error:?}"),
                });
                Err(error)
            }
        }
    }

    fn on_watch_event(&self, event: WatchEvent) {
        match event {
            WatchEvent::Degraded { reason } => self.set_watch_state(WatchState::Polling { reason }),
            WatchEvent::Changes(changes) => {
                let _turn = self.serialise();
                if self.is_scanning() {
                    self.held
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .changes
                        .extend(changes);
                } else {
                    self.apply_locked(changes);
                }
            }
            WatchEvent::Rescan(_) => {
                let scanning = {
                    let _turn = self.serialise();
                    let scanning = self.is_scanning();
                    if scanning {
                        self.held.lock().unwrap_or_else(|e| e.into_inner()).rescan = true;
                    }
                    scanning
                };
                if !scanning {
                    // A failed rescan has already raised `Failed`.
                    let _ = self.rescan();
                }
            }
            WatchEvent::Lost(error) => {
                let _turn = self.serialise();
                self.fail_locked(error);
            }
        }
    }

    fn is_scanning(&self) -> bool {
        matches!(
            self.read().phase,
            ListingPhase::Scanning | ListingPhase::Rescanning
        )
    }

    /// Marks the listing failed and tells whoever is listening. The caller holds the turn.
    fn fail_locked(&self, error: VfsError) {
        {
            let mut state = self.write();
            if state.phase == ListingPhase::Failed {
                return;
            }
            state.phase = ListingPhase::Failed;
            state.revision += 1;
        }
        self.emit(ListingEvent::Failed {
            handle: self.handle,
            error,
        });
    }

    /// Reads the folder again and patches the view to match, keeping the `EntryId` of every entry
    /// that is still there. The watcher asks for this when it lost events (an overflow), and the
    /// owner may ask for it when the listing is not watched. Raises `Rescanning`, then a `Changed`
    /// patch if anything moved, then `Ready`; if the folder cannot be read any more, `Failed`.
    ///
    /// A rename seen only by a rescan is a removal plus an addition, so the entry gets a new id.
    pub fn rescan(&self) -> Result<ListingSnapshot, VfsError> {
        loop {
            {
                let _turn = self.serialise();
                let event = {
                    let mut state = self.write();
                    if state.phase == ListingPhase::Failed {
                        return Err(VfsError::StaleHandle);
                    }
                    state.phase = ListingPhase::Rescanning;
                    state.revision += 1;
                    ListingEvent::Progress {
                        handle: self.handle,
                        revision: state.revision,
                        phase: ListingPhase::Rescanning,
                        scanned: 0,
                        count: state.index.count(),
                        groups: None,
                    }
                };
                self.emit(event);
            }
            let fresh = self.list_folder(ListingPhase::Rescanning);
            let turn = self.serialise();
            let entries = match fresh {
                Ok(entries) => entries,
                Err(VfsError::Cancelled) => return Err(VfsError::Cancelled),
                Err(error) => {
                    self.fail_locked(error.clone());
                    return Err(error);
                }
            };
            if self.read().phase == ListingPhase::Failed {
                return Err(VfsError::StaleHandle);
            }
            let total = entries.len() as u32;
            let changes = self.read().index.diff(entries);
            self.apply_locked(changes);
            let event = {
                let mut state = self.write();
                state.phase = ListingPhase::Ready;
                state.revision += 1;
                ListingEvent::Progress {
                    handle: self.handle,
                    revision: state.revision,
                    phase: ListingPhase::Ready,
                    scanned: total,
                    count: state.index.count(),
                    groups: None,
                }
            };
            self.emit(event);
            let again = self.replay_held();
            drop(turn);
            if !again {
                return Ok(self.snapshot());
            }
        }
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
                match self.provider.resolve_link(&self.path, entry) {
                    Ok(entry) => changes.push(Change::Upsert(entry)),
                    // Gone since the snapshot: the watcher reports the removal or rename.
                    Err(VfsError::NotFound { .. }) => {}
                    Err(e) => return Err(e),
                }
            }
            resolved += changes.len();
            self.apply_changes(changes);
        }
        Ok(resolved)
    }
}
