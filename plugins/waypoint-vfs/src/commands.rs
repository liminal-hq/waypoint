// Implements the IPC commands exposed by the file system plugin
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::sync::Arc;

use serde::Deserialize;
use tauri::{Emitter, Runtime, State, Window};
use waypoint_path::VfsPath;
use waypoint_protocol::{Location, PluginStatus, VfsError};
use waypoint_vfs::{
    Entry, EntryKind, Filter, Listing, ListingEvent, ListingHandle, ListingOptions,
    ListingSnapshot, LocalProvider, Places, PlacesEnv, Provider, SortSpec,
};

use crate::error::Error;
use crate::registry::Registry;

/// The one event name every listing event is emitted under, to the window that owns the listing.
pub const LISTING_EVENT: &str = "waypoint-vfs://listing";

/// The plugin's managed state: the open listings and the providers that serve them.
pub struct Vfs {
    pub(crate) registry: Arc<Registry>,
    local: Arc<LocalProvider>,
}

impl Default for Vfs {
    fn default() -> Self {
        Self {
            registry: Arc::new(Registry::default()),
            local: Arc::new(LocalProvider::new()),
        }
    }
}

impl Vfs {
    fn provider_for(&self, path: &VfsPath) -> Arc<dyn Provider> {
        match path {
            VfsPath::File(_) => self.local.clone(),
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
/// being kept up to date by polling because notifications are unavailable.
#[tauri::command]
pub async fn get_status(state: State<'_, Vfs>) -> Result<PluginStatus, Error> {
    let mut features = vec![
        "listing".to_owned(),
        "watch".to_owned(),
        "places".to_owned(),
    ];
    if state.registry.any_polling() {
        features.push("polling-fallback".to_owned());
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
    let provider = state.provider_for(&path);
    let options = options.unwrap_or_default();

    let probe = (provider.clone(), path.clone());
    let entry = blocking(move || probe.0.stat(&probe.1)).await??;
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

    tauri::async_runtime::spawn_blocking(move || scan(&listing, &sink));
    Ok(first)
}

/// Watches first (so nothing is missed while scanning), scans, then resolves any symlinks the scan
/// left for later. A scan that fails tells the window; one that was cancelled stays quiet.
fn scan(listing: &Arc<Listing>, sink: &Arc<dyn Fn(ListingEvent) + Send + Sync>) {
    let _ = listing.start_watching();
    match listing.scan() {
        Ok(_) => {
            if let Err(error) = listing.resolve_pending_links() {
                log::debug!("could not resolve every symlink: {error:?}");
            }
        }
        Err(VfsError::Cancelled) => {}
        Err(error) => sink(ListingEvent::Failed {
            handle: listing.handle(),
            error,
        }),
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
