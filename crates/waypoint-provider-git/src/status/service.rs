// One tracker per repository, shared by every window and tab that shows it.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, Weak};

use super::tracker::{Snapshot, TrackEvent, TrackSink, Tracker, TrackerOptions};

struct Listeners {
    sinks: Vec<(u64, TrackSink)>,
    next_id: u64,
    /// The last reason watching was unavailable, replayed to a listener that joins later.
    degraded: Option<String>,
}

struct Shared {
    tracker: Tracker,
    listeners: Arc<Mutex<Listeners>>,
}

/// Hands out trackers by repository. A repository shown in three tabs has one watch and one
/// status; when the last of them goes away the tracker stops.
pub struct StatusService {
    options: TrackerOptions,
    repos: Mutex<HashMap<PathBuf, Weak<Shared>>>,
}

/// A listener's hold on a repository's tracker. Dropping it stops the events, and the tracker too
/// when no one else listens.
pub struct Subscription {
    shared: Arc<Shared>,
    id: u64,
}

impl StatusService {
    pub fn new(options: TrackerOptions) -> Arc<Self> {
        Arc::new(Self {
            options,
            repos: Mutex::new(HashMap::new()),
        })
    }

    /// The working folder of the repository holding `dir`, or `None` (see `find_repository`).
    pub fn repository_of(&self, dir: &Path) -> Option<PathBuf> {
        crate::repository::find_repository(dir)
    }

    /// Starts hearing about `root`, a working folder from `repository_of`. The current snapshot, if
    /// there is one, is delivered at once; every later change follows.
    pub fn subscribe(&self, root: &Path, sink: TrackSink) -> Subscription {
        let mut repos = self.repos.lock().unwrap_or_else(|e| e.into_inner());
        repos.retain(|_, weak| weak.strong_count() > 0);
        let shared = match repos.get(root).and_then(Weak::upgrade) {
            Some(shared) => shared,
            None => {
                let listeners = Arc::new(Mutex::new(Listeners {
                    sinks: Vec::new(),
                    next_id: 0,
                    degraded: None,
                }));
                let fanout: TrackSink = {
                    let listeners = Arc::clone(&listeners);
                    Arc::new(move |event: TrackEvent| {
                        let sinks: Vec<TrackSink> = {
                            let mut guard = listeners.lock().unwrap_or_else(|e| e.into_inner());
                            if let TrackEvent::Degraded { reason } = &event {
                                guard.degraded = Some(reason.clone());
                            }
                            guard
                                .sinks
                                .iter()
                                .map(|(_, sink)| Arc::clone(sink))
                                .collect()
                        };
                        for sink in sinks {
                            sink(event.clone());
                        }
                    })
                };
                let shared = Arc::new(Shared {
                    tracker: Tracker::start(root.to_path_buf(), self.options, fanout),
                    listeners,
                });
                repos.insert(root.to_path_buf(), Arc::downgrade(&shared));
                shared
            }
        };
        drop(repos);
        let (id, degraded) = {
            let mut guard = shared.listeners.lock().unwrap_or_else(|e| e.into_inner());
            let id = guard.next_id;
            guard.next_id += 1;
            guard.sinks.push((id, Arc::clone(&sink)));
            (id, guard.degraded.clone())
        };
        if let Some(reason) = degraded {
            sink(TrackEvent::Degraded { reason });
        }
        if let Some(snapshot) = shared.tracker.snapshot() {
            sink(TrackEvent::Updated(snapshot));
        }
        Subscription { shared, id }
    }

    /// The latest snapshot of a repository that is being tracked, without starting to track it.
    pub fn peek(&self, root: &Path) -> Option<Arc<Snapshot>> {
        let repos = self.repos.lock().unwrap_or_else(|e| e.into_inner());
        repos.get(root)?.upgrade()?.tracker.snapshot()
    }

    /// How many repositories are being tracked now.
    pub fn tracked(&self) -> usize {
        let repos = self.repos.lock().unwrap_or_else(|e| e.into_inner());
        repos
            .values()
            .filter(|weak| weak.strong_count() > 0)
            .count()
    }
}

impl Subscription {
    pub fn snapshot(&self) -> Option<Arc<Snapshot>> {
        self.shared.tracker.snapshot()
    }

    /// Asks for a full recompute.
    pub fn refresh(&self) {
        self.shared.tracker.refresh();
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        let mut guard = self
            .shared
            .listeners
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        guard.sinks.retain(|(id, _)| *id != self.id);
    }
}
