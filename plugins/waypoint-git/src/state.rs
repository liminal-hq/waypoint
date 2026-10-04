// The Git plugin's state: the shared status service, the windows' watches and the overlay
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use waypoint_provider_git::{
    compute, RepoStatus, StatusOptions, StatusService, Subscription, TrackerOptions,
};
use waypoint_vfs::{CancelToken, FolderOverlay};

use crate::overlay::GitOverlay;

/// How long a status computed once for the sidebar's badges is reused.
const BADGE_TTL: Duration = Duration::from_secs(10);

/// A status computed for a badge, by repository, and when.
type BadgeCache = HashMap<PathBuf, (Instant, Arc<RepoStatus>)>;

/// One window's watch of a repository.
struct Watch {
    window: String,
    _subscription: Subscription,
}

/// Held as Tauri state. Everything that reads a repository goes through the one `StatusService`, so
/// a repository shown in three tabs of two windows has one watch and one status.
pub struct Git {
    pub(crate) service: Arc<StatusService>,
    enabled: Arc<AtomicBool>,
    overlay: Arc<GitOverlay>,
    watches: Mutex<HashMap<u32, Watch>>,
    next_id: AtomicU32,
    /// Why native watching is unavailable, when a tracker said so.
    degraded: Arc<Mutex<Option<String>>>,
    badges: BadgeSource,
}

/// Reads the status of a repository for a badge, from anywhere (it is `Clone`, so a blocking task
/// can own one).
#[derive(Clone)]
pub(crate) struct BadgeSource {
    service: Arc<StatusService>,
    cache: Arc<Mutex<BadgeCache>>,
}

impl Default for Git {
    fn default() -> Self {
        Self::new(TrackerOptions::default())
    }
}

impl Git {
    pub fn new(options: TrackerOptions) -> Self {
        let service = StatusService::new(options);
        let enabled = Arc::new(AtomicBool::new(true));
        Self {
            overlay: Arc::new(GitOverlay::new(Arc::clone(&service), Arc::clone(&enabled))),
            enabled,
            watches: Mutex::new(HashMap::new()),
            next_id: AtomicU32::new(1),
            degraded: Arc::default(),
            badges: BadgeSource {
                service: Arc::clone(&service),
                cache: Arc::default(),
            },
            service,
        }
    }

    pub fn enabled(&self) -> bool {
        self.enabled.load(Ordering::SeqCst)
    }

    /// Turns the Git status on or off (the Settings switch): off, every watch ends and every
    /// listing it decorated loses its marks, and nothing reads a repository until it is on again.
    pub fn set_enabled(&self, on: bool) {
        if self.enabled.swap(on, Ordering::SeqCst) == on {
            return;
        }
        if !on {
            self.watches
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clear();
            self.badges
                .cache
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clear();
            *self.degraded.lock().unwrap_or_else(|e| e.into_inner()) = None;
        }
        self.overlay.enabled_changed(on);
    }

    /// The overlay the app hands the file system plugin, so listings carry Git marks.
    pub fn overlay(&self) -> Arc<dyn FolderOverlay> {
        self.overlay.clone()
    }

    pub(crate) fn degraded(&self) -> Option<String> {
        self.degraded
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    pub(crate) fn degraded_slot(&self) -> Arc<Mutex<Option<String>>> {
        Arc::clone(&self.degraded)
    }

    pub(crate) fn add_watch(&self, window: &str, subscription: Subscription) -> u32 {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        self.watches
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(
                id,
                Watch {
                    window: window.to_owned(),
                    _subscription: subscription,
                },
            );
        id
    }

    /// Ends one watch; `false` when it was not there (or is another window's).
    pub(crate) fn remove_watch(&self, window: &str, id: u32) -> bool {
        let mut watches = self.watches.lock().unwrap_or_else(|e| e.into_inner());
        if watches.get(&id).is_some_and(|watch| watch.window == window) {
            watches.remove(&id);
            true
        } else {
            false
        }
    }

    /// Ends every watch a window made (it closed).
    pub fn forget_window(&self, window: &str) -> usize {
        let mut watches = self.watches.lock().unwrap_or_else(|e| e.into_inner());
        let before = watches.len();
        watches.retain(|_, watch| watch.window != window);
        before - watches.len()
    }

    /// How many watches are open.
    pub fn watching(&self) -> usize {
        self.watches.lock().unwrap_or_else(|e| e.into_inner()).len()
    }

    pub(crate) fn badge_source(&self) -> BadgeSource {
        self.badges.clone()
    }
}

impl BadgeSource {
    /// The status of a repository for a badge: the live one when it is being watched, otherwise one
    /// computed now and kept briefly, so a sidebar of favourites does not walk each repository
    /// every time it redraws.
    pub(crate) fn status(&self, root: &Path) -> Option<Arc<RepoStatus>> {
        if let Some(snapshot) = self.service.peek(root) {
            return Some(Arc::clone(&snapshot.status));
        }
        {
            let cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
            if let Some((at, status)) = cache.get(root) {
                if at.elapsed() < BADGE_TTL {
                    return Some(Arc::clone(status));
                }
            }
        }
        let status = compute(
            root,
            &StatusOptions {
                renames: false,
                ..StatusOptions::default()
            },
            &CancelToken::new(),
            None,
        )
        .ok()?;
        let status = Arc::new(status);
        let mut cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        if cache.len() > 64 {
            cache.clear();
        }
        cache.insert(root.to_path_buf(), (Instant::now(), Arc::clone(&status)));
        Some(status)
    }
}
