// Implements the IPC commands exposed by the file system plugin
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::sync::{Arc, RwLock};

use serde::Deserialize;
use tauri::ipc::Channel;
use tauri::{Emitter, Manager, Runtime, State, Window};
use tauri_plugin_opener::OpenerExt;
use waypoint_connections::ConnectionsHub;
use waypoint_path::VfsPath;
use waypoint_protocol::{EntryId, Location, PluginStatus, VfsError};
use waypoint_vfs::{
    DirScanCache, DirScanEvent, DirScanOptions, DirScanResult, Entry, EntryDetails, EntryKind,
    Filter, FolderCheck, FolderSizeEvent, Listing, ListingEvent, ListingHandle, ListingLayout,
    ListingOptions, ListingSnapshot, LocalProvider, LocationInfo, Places, PlacesEnv, Provider,
    ProviderRegistry, SelectionSpec, SelectionSummary, SortSpec, TextHead, TrashInfo,
    TrashProvider, TrashSource, VolumeSpace,
};

use crate::error::Error;
use crate::registry::{Registry, SizeJobs};

/// The one event name every listing event is emitted under, to the window that owns the listing.
pub const LISTING_EVENT: &str = "waypoint-vfs://listing";

/// Lists the `Host` aliases of `~/.ssh/config` (the app reads the file; the plugin only offers them).
pub type Suggestions = Arc<dyn Fn() -> Vec<String> + Send + Sync>;

/// The plugin's managed state: the open listings and the providers that serve them.
pub struct Vfs {
    pub(crate) registry: Arc<Registry>,
    pub(crate) size_jobs: Arc<SizeJobs>,
    local: Arc<LocalProvider>,
    /// The `trash:` provider, once the app has given the plugin a Trash to read (A4: this plugin
    /// calls no other, so the composition root adapts the Trash plugin to `TrashSource`).
    trash: RwLock<Option<Arc<TrashProvider>>>,
    /// The server, archive and Git providers the app registered (A85), by scheme.
    remote: Arc<ProviderRegistry>,
    /// The saved connections and the connection manager, when the app gave them.
    connections: Option<Arc<ConnectionsHub>>,
    suggestions: Option<Suggestions>,
}

impl Vfs {
    /// The locations a selection over `handle` covers, in view order, for the app to hand to
    /// operations (A47): the `window` that opened the listing is the only one that may resolve it,
    /// and the frontend never builds a path. Ids the view no longer holds are dropped.
    pub fn resolve_selection(
        &self,
        window: &str,
        handle: ListingHandle,
        selection: &SelectionSpec,
    ) -> Result<Vec<Location>, VfsError> {
        let listing = self.registry.get(window, handle)?;
        Ok(listing
            .resolve_selection(selection)?
            .iter()
            .map(|path| path.to_location())
            .collect())
    }
}

impl Vfs {
    /// The location of each of `ids` in `window`'s listing `handle`, in the order given, with `None`
    /// for one the listing no longer holds. Each id is one lookup, so asking for the pictures of a
    /// screen of a 100 000-entry folder costs the screen, not the folder (`resolve_selection` walks
    /// the whole view to put what it finds in view order, which is not wanted here).
    pub fn locate_entries(
        &self,
        window: &str,
        handle: ListingHandle,
        ids: &[waypoint_protocol::EntryId],
    ) -> Result<Vec<Option<Location>>, VfsError> {
        let listing = self.registry.get(window, handle)?;
        Ok(ids
            .iter()
            .map(|id| listing.path_of(*id).ok().map(|path| path.to_location()))
            .collect())
    }

    /// Starts totalling the folder `window`'s listing `handle` holds as entry `id`, on a thread of
    /// its own at low priority, and returns the run's id at once. `sink` receives the events and
    /// returns `false` when nobody is listening any more (the page went away), which cancels the
    /// run; the run also stops when the listing closes, when `cancel_folder_size` asks, and when
    /// the window is destroyed. Exactly one of `done`, `cancelled` and `failed` ends the stream.
    ///
    /// An entry that is not a folder (or a link to one) is `NotADirectory` and starts nothing.
    pub fn start_folder_size(
        &self,
        window: &str,
        handle: ListingHandle,
        id: EntryId,
        sink: impl Fn(FolderSizeEvent) -> bool + Send + 'static,
    ) -> Result<u64, VfsError> {
        let listing = self.registry.get(window, handle)?;
        let path = listing.path_of(id)?;
        let provider = listing.provider().clone();
        let entry = provider.stat(&path)?;
        let is_folder = entry.kind == EntryKind::Directory
            || (entry.kind == EntryKind::Symlink
                && entry.link_target == Some(EntryKind::Directory));
        if !is_folder {
            return Err(VfsError::NotADirectory {
                location: path.to_location(),
            });
        }
        let (job, cancel) = self.size_jobs.start(window);
        let cancel_for_walk = cancel.clone();
        let (jobs, registry, label) = (
            self.size_jobs.clone(),
            self.registry.clone(),
            window.to_owned(),
        );
        let spawned = std::thread::Builder::new()
            .name("waypoint-folder-size".to_owned())
            .spawn(move || {
                waypoint_vfs::lower_thread_priority();
                let mut report = |totals: &waypoint_vfs::FolderSizeTotals| {
                    if registry.get(&label, handle).is_err()
                        || !sink(FolderSizeEvent::Progress { totals: *totals })
                    {
                        cancel.cancel();
                    }
                };
                let event = match provider.folder_size(&path, &cancel_for_walk, &mut report) {
                    Ok(run) if run.cancelled => FolderSizeEvent::Cancelled { totals: run.totals },
                    Ok(run) => FolderSizeEvent::Done { totals: run.totals },
                    Err(error) => FolderSizeEvent::Failed { error },
                };
                sink(event);
                jobs.finish(job);
            });
        if let Err(error) = spawned {
            self.size_jobs.finish(job);
            return Err(VfsError::Io {
                message: format!("could not start the folder size: {error}"),
                location: None,
            });
        }
        Ok(job)
    }
}

impl Vfs {
    /// Starts a directory-size scan of `root`, on a thread of its own at low priority (never on
    /// the operations pool, A70), and returns the run's id at once. `sink` receives the events and
    /// returns `false` when nobody is listening any more, which cancels the run; the run also
    /// stops when `cancel_dir_scan` asks and when the window is destroyed. A scan that finishes is
    /// remembered in `cache`. Exactly one of `done`, `cancelled` and `failed` ends the stream.
    pub fn start_dir_scan(
        &self,
        window: &str,
        root: Location,
        options: DirScanOptions,
        cache: Option<DirScanCache>,
        sink: impl Fn(DirScanEvent) -> bool + Send + 'static,
    ) -> Result<u64, VfsError> {
        let (job, cancel) = self.size_jobs.start(window);
        let (jobs, token) = (self.size_jobs.clone(), cancel.clone());
        let spawned = std::thread::Builder::new()
            .name("waypoint-dir-scan".to_owned())
            .spawn(move || {
                waypoint_vfs::lower_thread_priority();
                waypoint_vfs::scan_dir_sizes(&root, options, &token, &mut |event| {
                    if let (DirScanEvent::Done { result }, Some(cache)) = (&event, &cache) {
                        if let Err(error) = cache.store(result) {
                            log::warn!("{error:?}");
                        }
                    }
                    if !sink(event) {
                        cancel.cancel();
                    }
                });
                jobs.finish(job);
            });
        if let Err(error) = spawned {
            self.size_jobs.finish(job);
            return Err(VfsError::Io {
                message: format!("could not start the directory-size scan: {error}"),
                location: None,
            });
        }
        Ok(job)
    }
}

impl Default for Vfs {
    fn default() -> Self {
        Self::new(Arc::new(ProviderRegistry::new()), None, None)
    }
}

impl Vfs {
    /// The state over the app's remote providers, saved connections and SSH host suggestions. A
    /// listing on a login holds it open (the manager never closes it as idle) until it closes.
    pub fn new(
        remote: Arc<ProviderRegistry>,
        connections: Option<Arc<ConnectionsHub>>,
        suggestions: Option<Suggestions>,
    ) -> Self {
        let registry = Registry::default();
        if let Some(hub) = &connections {
            let manager = hub.manager().clone();
            registry.set_on_close(Arc::new(move |listing: &Listing| {
                if let Some(key) = listing.provider().connection_key(listing.path()) {
                    manager.release(&key);
                }
            }));
        }
        Self {
            registry: Arc::new(registry),
            size_jobs: Arc::new(SizeJobs::default()),
            local: Arc::new(LocalProvider::new()),
            trash: RwLock::new(None),
            remote,
            connections,
            suggestions,
        }
    }

    /// The server, archive and Git providers, for the operations engine to serve the same schemes.
    pub fn remote(&self) -> &Arc<ProviderRegistry> {
        &self.remote
    }

    /// The saved connections and the connection manager.
    pub fn connections(&self) -> Option<&Arc<ConnectionsHub>> {
        self.connections.as_ref()
    }

    pub(crate) fn suggestions(&self) -> Option<Suggestions> {
        self.suggestions.clone()
    }

    /// Tells the connection manager what a call on `path` found.
    pub(crate) fn observe(&self, path: &VfsPath, outcome: Result<(), &VfsError>) {
        if let (Some(hub), VfsPath::Remote(_)) = (&self.connections, path) {
            if let Some(key) = hub.manager().key_of(path) {
                hub.manager().observe(&key, outcome);
            }
        }
    }
}

impl Vfs {
    /// Serves `trash:/` from `source`. Listings already open keep the provider they opened with.
    pub fn set_trash_source(&self, source: Arc<dyn TrashSource>) {
        *self.trash.write().unwrap_or_else(|e| e.into_inner()) =
            Some(Arc::new(TrashProvider::new(source)));
    }

    /// The Trash provider, when the app gave the plugin one.
    pub fn trash_provider(&self) -> Option<Arc<TrashProvider>> {
        self.trash.read().unwrap_or_else(|e| e.into_inner()).clone()
    }

    fn provider_for(&self, path: &VfsPath) -> Result<Arc<dyn Provider>, VfsError> {
        match path {
            VfsPath::File(_) => Ok(self.local.clone()),
            VfsPath::Trash(_) => self
                .trash_provider()
                .map(|provider| provider as Arc<dyn Provider>)
                .ok_or_else(|| VfsError::Unsupported {
                    what: "the Trash cannot be read here".to_owned(),
                }),
            // Server, archive and Git providers are the ones the app registered (A85).
            VfsPath::Remote(_) | VfsPath::Archive(_) | VfsPath::Git(_) => {
                self.remote.for_path(path)
            }
        }
    }
}

/// How a listing opens. Both parts default: sorted by name with folders first, hidden files
/// hidden.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenOptions {
    pub sort: Option<SortSpec>,
    pub filter: Option<Filter>,
}

/// Runs blocking work off the async runtime's threads.
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> T + Send + 'static,
) -> Result<T, Error> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|error| Error::Internal(error.to_string()))
}

fn parse(location: &Location) -> Result<VfsPath, VfsError> {
    VfsPath::from_location(location).map_err(|_| VfsError::InvalidLocation {
        input: location.uri.clone(),
    })
}

/// Reports what the plugin can do here. `polling-fallback` appears while any open listing is
/// being kept up to date by polling because notifications are unavailable; `connections` when
/// saved connections are kept, and `remote-{scheme}` for each server protocol a provider serves
/// (`remote-sftp`).
#[tauri::command]
pub async fn get_status(state: State<'_, Vfs>) -> Result<PluginStatus, Error> {
    let mut features = vec![
        "listing".to_owned(),
        "watch".to_owned(),
        "places".to_owned(),
        "entry-details".to_owned(),
        "folder-size".to_owned(),
        "dir-size-scan".to_owned(),
        "text-head".to_owned(),
        "preview-protocol".to_owned(),
    ];
    if state.registry.any_polling() {
        features.push("polling-fallback".to_owned());
    }
    if state.connections().is_some() {
        features.push("connections".to_owned());
    }
    features.extend(
        state
            .remote()
            .schemes()
            .map(|scheme| format!("remote-{scheme}")),
    );
    if let Some(provider) = state.trash_provider() {
        if provider.source().available().is_ok() {
            features.push("trash-view".to_owned());
        }
    }
    Ok(PluginStatus::available(features))
}

/// Opens a listing of a folder. The reply is the first snapshot, in the `scanning` phase; the scan
/// runs in the background and reports through `waypoint-vfs://listing` events (`progress` with
/// counts, ending in one with phase `ready`, or `failed`). A location that does not exist or is
/// not a folder is rejected here instead.
#[tauri::command]
pub async fn open_listing<R: Runtime>(
    window: Window<R>,
    state: State<'_, Vfs>,
    location: Location,
    options: Option<OpenOptions>,
) -> Result<ListingSnapshot, Error> {
    let path = parse(&location)?;
    let provider = state.provider_for(&path)?;
    let options = options.unwrap_or_default();

    let probe = (provider.clone(), path.clone());
    let entry = blocking(move || probe.0.stat(&probe.1)).await?;
    state.observe(&path, entry.as_ref().map(|_| ()));
    let entry = entry?;
    let is_folder = entry.kind == EntryKind::Directory
        || (entry.kind == EntryKind::Symlink && entry.link_target == Some(EntryKind::Directory));
    if !is_folder {
        return Err(VfsError::NotADirectory { location }.into());
    }

    let handle = state.registry.allocate();
    let label = window.label().to_owned();
    let registry = state.registry.clone();
    // Events stop the moment the listing closes, so a late patch never reaches a view that has
    // moved on.
    let sink: Arc<dyn Fn(ListingEvent) + Send + Sync> = {
        let (window, label, registry) = (window.clone(), label.clone(), registry.clone());
        Arc::new(move |event| {
            if registry.get(&label, handle).is_err() {
                return;
            }
            if let Err(error) = window.emit_to(&label, LISTING_EVENT, event) {
                log::warn!("could not emit a listing event to window={label}: {error}");
            }
        })
    };
    let listing = Listing::new(
        handle,
        path,
        provider,
        options.sort.unwrap_or_default(),
        options.filter.unwrap_or_default(),
        ListingOptions::default(),
        sink.clone(),
    );
    let first = listing.snapshot();
    registry.insert(&label, listing.clone());
    let manager = state.connections().map(|hub| hub.manager().clone());
    let key = listing.provider().connection_key(listing.path());
    if let (Some(manager), Some(key)) = (&manager, &key) {
        manager.acquire(key);
    }

    tauri::async_runtime::spawn_blocking(move || {
        let outcome = scan(&listing, &sink);
        if let (Some(manager), Some(key)) = (manager, key) {
            manager.observe(&key, outcome.as_ref().map(|_| ()));
        }
    });
    Ok(first)
}

/// Watches first (so nothing is missed while scanning), scans, then resolves any symlinks the scan
/// left for later. A scan that fails tells the window; one that was cancelled stays quiet. Returns
/// what the scan found, for the connection manager.
fn scan(
    listing: &Arc<Listing>,
    sink: &Arc<dyn Fn(ListingEvent) + Send + Sync>,
) -> Result<(), VfsError> {
    let _ = listing.start_watching();
    match listing.scan() {
        Ok(_) => {
            if let Err(error) = listing.resolve_pending_links() {
                log::debug!("could not resolve every symlink: {error:?}");
            }
            Ok(())
        }
        Err(VfsError::Cancelled) => Err(VfsError::Cancelled),
        Err(error) => {
            sink(ListingEvent::Failed {
                handle: listing.handle(),
                error: error.clone(),
            });
            Err(error)
        }
    }
}

fn listing_of<R: Runtime>(
    window: &Window<R>,
    state: &State<'_, Vfs>,
    handle: ListingHandle,
) -> Result<Arc<Listing>, Error> {
    Ok(state.registry.get(window.label(), handle)?)
}

/// Reads `count` entries from view position `start`; shorter at the end of the listing.
#[tauri::command]
pub async fn get_range<R: Runtime>(
    window: Window<R>,
    state: State<'_, Vfs>,
    handle: ListingHandle,
    start: u32,
    count: u32,
) -> Result<Vec<Entry>, Error> {
    Ok(listing_of(&window, &state, handle)?.get_range(start, count))
}

/// Re-sorts a listing. Cached pages are stale afterwards; the reply carries the new revision.
#[tauri::command]
pub async fn set_sort<R: Runtime>(
    window: Window<R>,
    state: State<'_, Vfs>,
    handle: ListingHandle,
    sort: SortSpec,
) -> Result<ListingSnapshot, Error> {
    let listing = listing_of(&window, &state, handle)?;
    blocking(move || listing.set_sort(sort)).await
}

/// Changes what a listing hides. Cached pages are stale afterwards.
#[tauri::command]
pub async fn set_filter<R: Runtime>(
    window: Window<R>,
    state: State<'_, Vfs>,
    handle: ListingHandle,
    filter: Filter,
) -> Result<ListingSnapshot, Error> {
    let listing = listing_of(&window, &state, handle)?;
    blocking(move || listing.set_filter(filter)).await
}

/// Closes a listing, cancelling a scan in flight and stopping its watcher. An unknown handle is
/// not an error.
#[tauri::command]
pub async fn close_listing<R: Runtime>(
    window: Window<R>,
    state: State<'_, Vfs>,
    handle: ListingHandle,
) -> Result<(), Error> {
    state.registry.close(window.label(), handle);
    Ok(())
}

/// Turns typed text into a `Location`, resolving relative text against `base` and `~` against the
/// home folder. It does not check that the location exists.
#[tauri::command]
pub async fn parse_location(input: String, base: Location) -> Result<Location, Error> {
    blocking(move || {
        let env = PlacesEnv::detect()?;
        waypoint_vfs::parse_location(&input, &base, &env.home)
    })
    .await?
    .map_err(Error::from)
}

/// The parent and breadcrumb segments of a location.
#[tauri::command]
pub async fn describe_location(location: Location) -> Result<LocationInfo, Error> {
    Ok(waypoint_vfs::describe_location(&location)?)
}

/// Where an entry of an open listing lives.
#[tauri::command]
pub async fn entry_location<R: Runtime>(
    window: Window<R>,
    state: State<'_, Vfs>,
    handle: ListingHandle,
    id: EntryId,
) -> Result<Location, Error> {
    Ok(listing_of(&window, &state, handle)?
        .path_of(id)?
        .to_location())
}

/// The count and total file size of a selection over a listing's current view.
#[tauri::command]
pub async fn summarise_selection<R: Runtime>(
    window: Window<R>,
    state: State<'_, Vfs>,
    handle: ListingHandle,
    selection: SelectionSpec,
) -> Result<SelectionSummary, Error> {
    let listing = listing_of(&window, &state, handle)?;
    blocking(move || listing.summarise_selection(&selection)).await
}

/// Whether a location is a folder and can be written to, for a destination picker. A location
/// that is not there is rejected (`NotFound`, or `PermissionDenied` where it cannot be seen).
#[tauri::command]
pub async fn check_folder(state: State<'_, Vfs>, location: Location) -> Result<FolderCheck, Error> {
    let path = parse(&location)?;
    let provider = state.provider_for(&path)?;
    blocking(move || {
        let entry = provider.stat(&path)?;
        let is_folder = entry.kind == EntryKind::Directory
            || (entry.kind == EntryKind::Symlink
                && entry.link_target == Some(EntryKind::Directory));
        let writable = is_folder
            && !provider.read_only()
            && provider
                .permissions(&path)
                .map(|permissions| !permissions.readonly)
                .unwrap_or(true);
        Ok::<_, VfsError>(FolderCheck {
            is_folder,
            writable,
        })
    })
    .await?
    .map_err(Error::from)
}

/// Free and total space on the volume holding a location; `null` when it cannot be determined.
#[tauri::command]
pub async fn get_free_space(location: Location) -> Result<Option<VolumeSpace>, Error> {
    blocking(move || waypoint_vfs::free_space(&location)).await
}

/// Opens a file of an open listing in its default application. Only the window that owns the
/// listing can open its entries, and only entries that are there: the path is always resolved
/// from `(handle, id)`, never taken from the caller. A folder is rejected; the frontend opens
/// folders by navigating.
#[tauri::command]
pub async fn open_entry<R: Runtime>(
    window: Window<R>,
    state: State<'_, Vfs>,
    handle: ListingHandle,
    id: EntryId,
) -> Result<(), Error> {
    let listing = listing_of(&window, &state, handle)?;
    let path = listing.path_of(id)?;
    let provider = listing.provider().clone();
    if provider.layout() == ListingLayout::Trash {
        return Err(VfsError::Unsupported {
            what: "opening an item in the Trash".to_owned(),
        }
        .into());
    }
    let probe = path.clone();
    let entry = blocking(move || provider.stat(&probe)).await??;
    if entry.kind == EntryKind::Directory || entry.link_target == Some(EntryKind::Directory) {
        return Err(VfsError::Unsupported {
            what: "opening a folder as a file".to_owned(),
        }
        .into());
    }
    let app = window.app_handle().clone();
    let target = path.display();
    blocking(move || {
        app.opener()
            .open_path(target.clone(), None::<&str>)
            .map_err(|error| VfsError::Io {
                message: error.to_string(),
                location: Some(path.to_location()),
            })
    })
    .await?
    .map_err(Error::from)
}

/// Everything the Inspector shows about one entry of an open listing. Fields only a local provider
/// reads (times, owner, permissions, allocated size) are named in `unavailable` for a remote one.
#[tauri::command]
pub async fn entry_details<R: Runtime>(
    window: Window<R>,
    state: State<'_, Vfs>,
    handle: ListingHandle,
    id: EntryId,
) -> Result<EntryDetails, Error> {
    let listing = listing_of(&window, &state, handle)?;
    let path = listing.path_of(id)?;
    let provider = listing.provider().clone();
    blocking(move || provider.details(&path))
        .await?
        .map_err(Error::from)
}

/// Starts totalling a folder of an open listing and returns the run's id. The totals arrive on
/// `on_event`: `progress` about every 100 ms, then one `done`, `cancelled` or `failed`. The walk
/// is low priority, stays on one volume, never follows a symlink and, on Windows, never downloads a
/// cloud placeholder. `cancel_folder_size` stops it; so does closing the listing or the window.
#[tauri::command]
pub async fn folder_size<R: Runtime>(
    window: Window<R>,
    state: State<'_, Vfs>,
    handle: ListingHandle,
    id: EntryId,
    on_event: Channel<FolderSizeEvent>,
) -> Result<u64, Error> {
    // `stat` runs on the calling task, which is quick; the walk has a thread of its own.
    state
        .start_folder_size(window.label(), handle, id, move |event| {
            on_event.send(event).is_ok()
        })
        .map_err(Error::from)
}

/// Stops one of this window's folder-size runs. A run that has ended, or one that is not this
/// window's, is not an error.
#[tauri::command]
pub async fn cancel_folder_size<R: Runtime>(
    window: Window<R>,
    state: State<'_, Vfs>,
    job: u64,
) -> Result<(), Error> {
    state.size_jobs.cancel(window.label(), job);
    Ok(())
}

/// Starts scanning the top-level folders of `location` for their sizes and returns the run's id.
/// The events arrive on `on_event`: `progress` about every 100 ms, a `partial` result after each
/// top-level folder, then one `done`, `cancelled` or `failed`. The scan is low priority on a
/// thread of its own, stays on one volume, never follows a symlink and, on Windows, never
/// downloads a cloud placeholder. A finished scan is cached; `cancel_dir_scan` stops one, and so
/// does destroying the window.
#[tauri::command]
pub async fn scan_dir_sizes<R: Runtime>(
    window: Window<R>,
    state: State<'_, Vfs>,
    location: Location,
    options: Option<DirScanOptions>,
    on_event: Channel<DirScanEvent>,
) -> Result<u64, Error> {
    let cache = window
        .path()
        .app_data_dir()
        .ok()
        .map(|dir| DirScanCache::in_dir(&dir));
    state
        .start_dir_scan(
            window.label(),
            location,
            options.unwrap_or_default(),
            cache,
            move |event| on_event.send(event).is_ok(),
        )
        .map_err(Error::from)
}

/// Stops one of this window's directory-size scans. A scan that has ended, or one that is not this
/// window's, is not an error.
#[tauri::command]
pub async fn cancel_dir_scan<R: Runtime>(
    window: Window<R>,
    state: State<'_, Vfs>,
    job: u64,
) -> Result<(), Error> {
    state.size_jobs.cancel(window.label(), job);
    Ok(())
}

/// The last finished directory-size result for `location`, with the time it was measured, or
/// `null` when there is none (or the cache is from another version of the app).
#[tauri::command]
pub async fn get_cached_dir_scan<R: Runtime>(
    window: Window<R>,
    location: Location,
) -> Result<Option<DirScanResult>, Error> {
    let Ok(dir) = window.path().app_data_dir() else {
        return Ok(None);
    };
    blocking(move || DirScanCache::in_dir(&dir).load(&location)).await
}

/// The first bytes of a file of an open listing as text, at most `max` of them (and never more
/// than 256 KiB). A binary file is refused with `notText`, a folder with `isADirectory`.
#[tauri::command]
pub async fn read_text_head<R: Runtime>(
    window: Window<R>,
    state: State<'_, Vfs>,
    handle: ListingHandle,
    id: EntryId,
    max: Option<u32>,
) -> Result<TextHead, Error> {
    let listing = listing_of(&window, &state, handle)?;
    let path = listing.path_of(id)?;
    let provider = listing.provider().clone();
    let max = max.map_or(waypoint_vfs::TEXT_HEAD_MAX, |max| max as usize);
    blocking(move || waypoint_vfs::read_text_head(provider.as_ref(), &path, max))
        .await?
        .map_err(Error::from)
}

/// Whether the Trash can be browsed, why not, and how many items it holds, for the sidebar's Trash
/// place. Reading it lists the Trash, so ask when the sidebar needs the number, not on a timer
/// shorter than a few seconds. `with_bytes` also adds up the sizes (`total_bytes`); Overview asks
/// for that while it is visible and the sidebar never does.
#[tauri::command]
pub async fn get_trash_info(
    state: State<'_, Vfs>,
    with_bytes: Option<bool>,
) -> Result<TrashInfo, Error> {
    let Some(provider) = state.trash_provider() else {
        return Ok(TrashInfo {
            available: false,
            reason: Some("the Trash cannot be read here".to_owned()),
            count: 0,
            total_bytes: None,
        });
    };
    let with_bytes = with_bytes.unwrap_or(false);
    blocking(move || {
        if with_bytes {
            TrashInfo::with_bytes(provider.source().as_ref())
        } else {
            TrashInfo::of(provider.source().as_ref())
        }
    })
    .await
}

/// The folder a window opens at first.
#[tauri::command]
pub async fn get_home() -> Result<Location, Error> {
    blocking(|| waypoint_vfs::home_location(&PlacesEnv::detect()?))
        .await?
        .map_err(Error::from)
}

/// Home, the user folders that exist, and the favourites.
#[tauri::command]
pub async fn list_places() -> Result<Places, Error> {
    blocking(|| waypoint_vfs::list_places(&PlacesEnv::detect()?))
        .await?
        .map_err(Error::from)
}

/// Pins a folder to the favourites, with an optional label. Returns the updated places.
#[tauri::command]
pub async fn add_favourite(location: Location, label: Option<String>) -> Result<Places, Error> {
    blocking(move || {
        waypoint_vfs::add_favourite(&PlacesEnv::detect()?, &location, label.as_deref())
    })
    .await?
    .map_err(Error::from)
}

/// Unpins a folder. Returns the updated places.
#[tauri::command]
pub async fn remove_favourite(location: Location) -> Result<Places, Error> {
    blocking(move || waypoint_vfs::remove_favourite(&PlacesEnv::detect()?, &location))
        .await?
        .map_err(Error::from)
}

/// Labels a favourite, or clears its label with `null` or blank text. Returns the updated places.
#[tauri::command]
pub async fn rename_favourite(location: Location, label: Option<String>) -> Result<Places, Error> {
    blocking(move || {
        waypoint_vfs::rename_favourite(&PlacesEnv::detect()?, &location, label.as_deref())
    })
    .await?
    .map_err(Error::from)
}

/// Moves a favourite to position `to` in the list. Returns the updated places.
#[tauri::command]
pub async fn move_favourite(location: Location, to: u32) -> Result<Places, Error> {
    blocking(move || waypoint_vfs::move_favourite(&PlacesEnv::detect()?, &location, to as usize))
        .await?
        .map_err(Error::from)
}
