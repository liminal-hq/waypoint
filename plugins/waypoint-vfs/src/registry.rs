// Owns the open listings of every window: who opened what, and what closes with a window
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use waypoint_protocol::VfsError;
use waypoint_vfs::{Listing, ListingHandle, WatchState};

/// The table of open listings. A handle belongs to the window (by label) that opened it, so one
/// window can never read, change or close another's listing, and all of a window's listings go
/// when it does. It holds no Tauri types, so it is tested without an app.
#[derive(Default)]
pub struct Registry {
    next: AtomicU32,
    listings: Mutex<HashMap<ListingHandle, (String, Arc<Listing>)>>,
}

impl Registry {
    /// The next handle. Numbering starts at 1 and is per registry, so per plugin instance.
    pub fn allocate(&self) -> ListingHandle {
        ListingHandle(self.next.fetch_add(1, Ordering::Relaxed) + 1)
    }

    pub fn insert(&self, window: &str, listing: Arc<Listing>) {
        self.lock()
            .insert(listing.handle(), (window.to_owned(), listing));
    }

    /// The listing `window` opened under `handle`; `StaleHandle` when it is unknown, closed, or
    /// another window's.
    pub fn get(&self, window: &str, handle: ListingHandle) -> Result<Arc<Listing>, VfsError> {
        match self.lock().get(&handle) {
            Some((owner, listing)) if owner == window => Ok(listing.clone()),
            _ => Err(VfsError::StaleHandle),
        }
    }

    /// Closes a listing and stops its scan and watcher. Closing an unknown handle, or another
    /// window's, does nothing.
    pub fn close(&self, window: &str, handle: ListingHandle) {
        let removed = {
            let mut listings = self.lock();
            match listings.get(&handle) {
                Some((owner, _)) if owner == window => listings.remove(&handle),
                _ => None,
            }
        };
        if let Some((_, listing)) = removed {
            listing.close();
        }
    }

    /// Closes every listing `window` opened; returns how many there were.
    pub fn close_window(&self, window: &str) -> usize {
        let closing: Vec<_> = {
            let mut listings = self.lock();
            let handles: Vec<_> = listings
                .iter()
                .filter(|(_, (owner, _))| owner == window)
                .map(|(handle, _)| *handle)
                .collect();
            handles
                .into_iter()
                .filter_map(|handle| listings.remove(&handle))
                .collect()
        };
        for (_, listing) in &closing {
            listing.close();
        }
        closing.len()
    }

    #[cfg(all(test, unix))]
    pub fn len(&self) -> usize {
        self.lock().len()
    }

    #[cfg(all(test, unix))]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Whether any open listing is being kept up to date by polling rather than notifications.
    pub fn any_polling(&self) -> bool {
        self.lock()
            .values()
            .any(|(_, listing)| matches!(listing.watch_state(), WatchState::Polling { .. }))
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<ListingHandle, (String, Arc<Listing>)>> {
        self.listings.lock().unwrap_or_else(|e| e.into_inner())
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use waypoint_path::VfsPath;
    use waypoint_vfs::{Filter, ListingOptions, LocalProvider, SortSpec};

    fn listing(registry: &Registry) -> Arc<Listing> {
        Listing::new(
            registry.allocate(),
            VfsPath::parse_input("/").unwrap(),
            Arc::new(LocalProvider::new()),
            SortSpec::default(),
            Filter::default(),
            ListingOptions::default(),
            Arc::new(|_| {}),
        )
    }

    #[test]
    fn handles_count_up_from_one_per_registry() {
        let a = Registry::default();
        let b = Registry::default();
        assert_eq!(a.allocate(), ListingHandle(1));
        assert_eq!(a.allocate(), ListingHandle(2));
        assert_eq!(b.allocate(), ListingHandle(1));
    }

    #[test]
    fn a_handle_is_only_visible_to_its_window() {
        let registry = Registry::default();
        let l = listing(&registry);
        let handle = l.handle();
        registry.insert("main", l);
        assert!(registry.get("main", handle).is_ok());
        assert_eq!(
            registry.get("other", handle).err(),
            Some(VfsError::StaleHandle)
        );
        registry.close("other", handle);
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn closing_cancels_and_forgets_and_is_idempotent() {
        let registry = Registry::default();
        let l = listing(&registry);
        let handle = l.handle();
        let token = l.cancel_token().clone();
        registry.insert("main", l);
        registry.close("main", handle);
        registry.close("main", handle);
        registry.close("main", ListingHandle(999));
        assert!(token.is_cancelled());
        assert_eq!(
            registry.get("main", handle).err(),
            Some(VfsError::StaleHandle)
        );
    }

    #[test]
    fn closing_a_window_closes_only_its_listings() {
        let registry = Registry::default();
        let (a, b, c) = (listing(&registry), listing(&registry), listing(&registry));
        let tokens = [
            a.cancel_token().clone(),
            b.cancel_token().clone(),
            c.cancel_token().clone(),
        ];
        let kept = c.handle();
        registry.insert("one", a);
        registry.insert("one", b);
        registry.insert("two", c);
        assert_eq!(registry.close_window("one"), 2);
        assert!(tokens[0].is_cancelled() && tokens[1].is_cancelled());
        assert!(!tokens[2].is_cancelled());
        assert!(registry.get("two", kept).is_ok());
        assert_eq!(registry.close_window("one"), 0);
    }
}
