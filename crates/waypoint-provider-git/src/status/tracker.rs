// Keeping the status of one repository fresh: watch the working tree, gather changes, recompute
// only what they touch, and tell whoever is listening.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use notify::{EventKind, RecursiveMode, Watcher};
use waypoint_vfs::CancelToken;

use super::compute::{compute, StatusError, StatusOptions};
use super::model::{Change, RepoStatus};
use super::summary::{summarize, RepoSummary};
use super::throttle::Throttle;

/// The most folders watched in one working tree; a bigger one is polled instead (and says so).
const MAX_WATCHED_DIRS: usize = 50_000;

/// How a tracker behaves.
#[derive(Debug, Clone, Copy)]
pub struct TrackerOptions {
    pub status: StatusOptions,
    pub throttle: Throttle,
    /// More changed places than this in one burst is a full recompute: a scoped one would not be
    /// cheaper.
    pub max_scopes: usize,
    /// How often the status is recomputed when watching is not possible.
    pub poll_interval: Duration,
    /// Watch the working tree for changes. Off, the status is recomputed every `poll_interval`
    /// (what a tracker falls back to when the system cannot watch).
    pub watch: bool,
}

impl Default for TrackerOptions {
    fn default() -> Self {
        Self {
            status: StatusOptions::default(),
            throttle: Throttle::default(),
            max_scopes: 64,
            poll_interval: Duration::from_secs(5),
            watch: true,
        }
    }
}

/// The status of a repository at one moment, with the summary beside it. Cheap to share.
#[derive(Debug)]
pub struct Snapshot {
    /// Counts up by one for every change a tracker publishes, from 1.
    pub revision: u64,
    pub status: Arc<RepoStatus>,
    pub summary: Arc<RepoSummary>,
}

/// What a tracker tells its listeners.
#[derive(Debug, Clone)]
pub enum TrackEvent {
    /// The status or the summary changed.
    Updated(Arc<Snapshot>),
    /// Native watching is not possible here, so the status is recomputed on a timer; `reason` is
    /// for the Services panel.
    Degraded { reason: String },
    /// A status could not be computed (the repository went away, or is damaged). The last
    /// snapshot stays valid until the next `Updated`.
    Failed { message: String },
}

pub type TrackSink = Arc<dyn Fn(TrackEvent) + Send + Sync>;

enum Msg {
    Fs(Vec<PathBuf>),
    Rescan,
    Refresh,
    Stop,
}

struct Shared {
    snapshot: Mutex<Option<Arc<Snapshot>>>,
    cancel: CancelToken,
}

/// Watches one working tree. Dropping it stops the thread and cancels a run in progress.
pub struct Tracker {
    tx: mpsc::Sender<Msg>,
    shared: Arc<Shared>,
}

impl Tracker {
    /// Starts tracking `root`: the first status is computed at once, on the tracker's own thread
    /// (the caller never waits), and `sink` hears about it and every change after.
    pub fn start(root: PathBuf, options: TrackerOptions, sink: TrackSink) -> Tracker {
        let (tx, rx) = mpsc::channel();
        let shared = Arc::new(Shared {
            snapshot: Mutex::new(None),
            cancel: CancelToken::new(),
        });
        let worker = Worker {
            git_dir: crate::repository::git_dir_of(&root).unwrap_or_else(|| root.join(".git")),
            root,
            options,
            shared: Arc::clone(&shared),
            sink,
            tx: tx.clone(),
        };
        let name = format!("git-status {}", worker.root.display());
        let spawned = std::thread::Builder::new()
            .name(name)
            .spawn(move || worker.run(rx));
        if let Err(error) = spawned {
            log::warn!("git: could not start a status thread: {error}");
        }
        Tracker { tx, shared }
    }

    /// The latest snapshot, or `None` until the first status is done.
    pub fn snapshot(&self) -> Option<Arc<Snapshot>> {
        self.shared
            .snapshot
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// Recomputes everything at the next opportunity (a refresh the person asked for).
    pub fn refresh(&self) {
        let _ = self.tx.send(Msg::Refresh);
    }
}

impl Drop for Tracker {
    fn drop(&mut self) {
        self.shared.cancel.cancel();
        let _ = self.tx.send(Msg::Stop);
    }
}

/// The changes gathered since the last run.
#[derive(Default)]
struct Pending {
    full: bool,
    paths: BTreeSet<Vec<u8>>,
    first: Option<Instant>,
    last: Option<Instant>,
}

impl Pending {
    fn is_empty(&self) -> bool {
        !self.full && self.paths.is_empty()
    }

    fn touch(&mut self) {
        let now = Instant::now();
        self.first.get_or_insert(now);
        self.last = Some(now);
    }
}

/// Which directories a native watcher has been given.
struct WatchSet {
    watcher: notify::RecommendedWatcher,
    dirs: BTreeSet<PathBuf>,
    degraded: Option<String>,
}

impl WatchSet {
    fn new(tx: mpsc::Sender<Msg>) -> Result<Self, notify::Error> {
        let watcher =
            notify::recommended_watcher(
                move |result: notify::Result<notify::Event>| match result {
                    Ok(event) => {
                        if matches!(event.kind, EventKind::Access(_)) {
                            return;
                        }
                        if event.need_rescan() {
                            let _ = tx.send(Msg::Rescan);
                        }
                        if !event.paths.is_empty() {
                            let _ = tx.send(Msg::Fs(event.paths));
                        }
                    }
                    Err(_) => {
                        let _ = tx.send(Msg::Rescan);
                    }
                },
            )?;
        Ok(Self {
            watcher,
            dirs: BTreeSet::new(),
            degraded: None,
        })
    }

    fn add(&mut self, dir: &Path, mode: RecursiveMode) {
        if self.degraded.is_some() || self.dirs.contains(dir) {
            return;
        }
        if self.dirs.len() >= MAX_WATCHED_DIRS {
            self.degraded = Some("the working tree has too many folders to watch".to_owned());
            return;
        }
        match self.watcher.watch(dir, mode) {
            Ok(()) => {
                self.dirs.insert(dir.to_path_buf());
            }
            // The folder vanished between listing it and watching it: its parent's event says so.
            Err(error) if matches!(error.kind, notify::ErrorKind::PathNotFound) => {}
            Err(error) => {
                self.degraded = Some(match error.kind {
                    notify::ErrorKind::MaxFilesWatch => {
                        "the system's limit on watched folders is reached".to_owned()
                    }
                    _ => format!("watching the working tree failed: {error}"),
                });
            }
        }
    }

    /// Watches `dir` and every folder under it except `.git` and the folders `status` says are
    /// ignored (a `target` or `node_modules` that a build writes to all day).
    fn add_tree(&mut self, root: &Path, dir: &Path, status: &RepoStatus) {
        let mut stack = vec![dir.to_path_buf()];
        while let Some(folder) = stack.pop() {
            if self.degraded.is_some() {
                return;
            }
            self.add(&folder, RecursiveMode::NonRecursive);
            let Ok(read) = std::fs::read_dir(&folder) else {
                continue;
            };
            for entry in read.flatten() {
                let Ok(kind) = entry.file_type() else {
                    continue;
                };
                if !kind.is_dir() || entry.file_name() == ".git" {
                    continue;
                }
                let path = entry.path();
                if is_ignored(root, &path, status) {
                    continue;
                }
                stack.push(path);
            }
        }
    }

    /// Stops watching folders that turned out to be ignored.
    fn prune(&mut self, root: &Path, status: &RepoStatus) {
        let ignored: Vec<PathBuf> = status
            .iter()
            .filter(|(_, entry)| entry.unstaged == Some(Change::Ignored))
            .map(|(rel, _)| root.join(rel_to_path(rel)))
            .collect();
        if ignored.is_empty() {
            return;
        }
        let doomed: Vec<PathBuf> = self
            .dirs
            .iter()
            .filter(|dir| ignored.iter().any(|ig| dir.starts_with(ig)))
            .cloned()
            .collect();
        for dir in doomed {
            let _ = self.watcher.unwatch(&dir);
            self.dirs.remove(&dir);
        }
    }
}

fn is_ignored(root: &Path, path: &Path, status: &RepoStatus) -> bool {
    let Ok(rel) = path.strip_prefix(root) else {
        return false;
    };
    let rel = rel_bytes(rel);
    matches!(
        status.entry(&rel).and_then(|entry| entry.unstaged),
        Some(Change::Ignored)
    ) || matches!(
        status.inherited(&rel).and_then(|entry| entry.unstaged),
        Some(Change::Ignored)
    )
}

/// A relative path as `/`-separated bytes.
pub(crate) fn rel_bytes(rel: &Path) -> Vec<u8> {
    let mut out = Vec::new();
    for component in rel.components() {
        if let std::path::Component::Normal(name) = component {
            if !out.is_empty() {
                out.push(b'/');
            }
            out.extend_from_slice(&crate::names::os_bytes(name));
        }
    }
    out
}

fn rel_to_path(rel: &[u8]) -> PathBuf {
    rel.split(|&b| b == b'/')
        .map(crate::names::os_string)
        .collect()
}

struct Worker {
    root: PathBuf,
    git_dir: PathBuf,
    options: TrackerOptions,
    shared: Arc<Shared>,
    sink: TrackSink,
    tx: mpsc::Sender<Msg>,
}

impl Worker {
    fn run(self, rx: mpsc::Receiver<Msg>) {
        let mut watches = match WatchSet::new(self.tx.clone()) {
            Ok(set) if self.options.watch => Some(set),
            Ok(_) => None,
            Err(error) => {
                let reason = format!("watching is unavailable: {error}");
                (self.sink)(TrackEvent::Degraded { reason });
                None
            }
        };
        // Polling by choice is not a degradation worth reporting.
        let mut degraded_reported = watches.is_none();
        let mut snapshot: Option<Arc<Snapshot>> = None;
        let mut pending = Pending {
            full: true,
            ..Pending::default()
        };
        let mut immediate = true;
        let mut previous: Option<(Instant, Duration)> = None;
        let mut watched_once = false;

        loop {
            // Gather changes until a run is due.
            loop {
                let polling = watches.as_ref().is_none_or(|w| w.degraded.is_some());
                let wait = if immediate {
                    Some(Duration::ZERO)
                } else if pending.is_empty() {
                    polling.then_some(self.options.poll_interval)
                } else {
                    let (first, last) = (pending.first, pending.last);
                    let due = self.options.throttle.start_at(
                        first.unwrap_or_else(Instant::now),
                        last.unwrap_or_else(Instant::now),
                        previous,
                    );
                    Some(due.saturating_duration_since(Instant::now()))
                };
                let message = match wait {
                    Some(wait) => rx.recv_timeout(wait),
                    None => rx.recv().map_err(|_| RecvTimeoutError::Disconnected),
                };
                match message {
                    Ok(Msg::Stop) | Err(RecvTimeoutError::Disconnected) => return,
                    Ok(Msg::Rescan) | Ok(Msg::Refresh) => {
                        pending.full = true;
                        pending.touch();
                    }
                    Ok(Msg::Fs(paths)) => {
                        self.absorb(
                            paths,
                            &mut pending,
                            snapshot.as_deref().map(|s| &*s.status),
                            watches.as_mut(),
                        );
                    }
                    Err(RecvTimeoutError::Timeout) => {
                        if pending.is_empty() {
                            // The poll timer: nothing was watched, so look anyway.
                            pending.full = true;
                        }
                        break;
                    }
                }
                if self.shared.cancel.is_cancelled() {
                    return;
                }
                if immediate {
                    break;
                }
            }
            immediate = false;
            if self.shared.cancel.is_cancelled() {
                return;
            }

            // Decide what to recompute.
            let before = snapshot.as_ref().map(|s| Arc::clone(&s.status));
            let scopes = if pending.full {
                None
            } else {
                before
                    .as_deref()
                    .and_then(|status| plan_scopes(&pending.paths, status, self.options.max_scopes))
            };
            let full = scopes.is_none();
            if !full && scopes.as_ref().is_some_and(|s| s.is_empty()) {
                pending = Pending::default();
                continue;
            }
            pending = Pending::default();

            let started = Instant::now();
            let computed = match (&scopes, &before) {
                (Some(scopes), Some(before)) => compute(
                    &self.root,
                    &self.options.status,
                    &self.shared.cancel,
                    Some(scopes),
                )
                .map(|fresh| before.merged(scopes, fresh)),
                _ => compute(&self.root, &self.options.status, &self.shared.cancel, None),
            };
            let status = match computed {
                Ok(status) => status,
                Err(StatusError::Cancelled) => return,
                Err(error) => {
                    (self.sink)(TrackEvent::Failed {
                        message: error.to_string(),
                    });
                    previous = Some((Instant::now(), started.elapsed()));
                    continue;
                }
            };
            let summary = match summarize(&self.root, &status) {
                Ok(summary) => summary,
                Err(error) => {
                    (self.sink)(TrackEvent::Failed {
                        message: error.to_string(),
                    });
                    previous = Some((Instant::now(), started.elapsed()));
                    continue;
                }
            };
            previous = Some((Instant::now(), started.elapsed()));

            if let Some(watches) = watches.as_mut() {
                if !watched_once {
                    watched_once = true;
                    let git_dir = self.git_dir.clone();
                    watches.add(&git_dir, RecursiveMode::NonRecursive);
                    watches.add(&git_dir.join("refs"), RecursiveMode::Recursive);
                    watches.add(&git_dir.join("info"), RecursiveMode::NonRecursive);
                    watches.add_tree(&self.root, &self.root, &status);
                } else {
                    watches.prune(&self.root, &status);
                }
                if let (Some(reason), false) = (&watches.degraded, degraded_reported) {
                    degraded_reported = true;
                    (self.sink)(TrackEvent::Degraded {
                        reason: reason.clone(),
                    });
                }
            }

            let changed = match &snapshot {
                Some(old) => *old.status != status || *old.summary != summary,
                None => true,
            };
            if changed {
                let next = Arc::new(Snapshot {
                    revision: snapshot.as_ref().map_or(1, |s| s.revision + 1),
                    status: Arc::new(status),
                    summary: Arc::new(summary),
                });
                *self
                    .shared
                    .snapshot
                    .lock()
                    .unwrap_or_else(|e| e.into_inner()) = Some(Arc::clone(&next));
                snapshot = Some(Arc::clone(&next));
                (self.sink)(TrackEvent::Updated(next));
            }
        }
    }

    /// Sorts the paths of one notification into what they ask for.
    fn absorb(
        &self,
        paths: Vec<PathBuf>,
        pending: &mut Pending,
        status: Option<&RepoStatus>,
        mut watches: Option<&mut WatchSet>,
    ) {
        for path in paths {
            if let Ok(inside) = path.strip_prefix(&self.git_dir) {
                // Only the files that say the index, `HEAD`, a reference or the excludes changed.
                let first = inside.components().next();
                let Some(std::path::Component::Normal(name)) = first else {
                    continue;
                };
                let name = name.to_string_lossy();
                let relevant = matches!(
                    &*name,
                    "index"
                        | "HEAD"
                        | "MERGE_HEAD"
                        | "CHERRY_PICK_HEAD"
                        | "REVERT_HEAD"
                        | "BISECT_LOG"
                        | "rebase-merge"
                        | "rebase-apply"
                        | "packed-refs"
                        | "refs"
                        | "config"
                ) || (&*name == "info" && inside.ends_with("exclude"));
                if relevant && !name.ends_with(".lock") {
                    pending.full = true;
                    pending.touch();
                }
                continue;
            }
            let Ok(rel) = path.strip_prefix(&self.root) else {
                continue;
            };
            if rel.as_os_str().is_empty() {
                continue;
            }
            if rel.components().any(|c| c.as_os_str() == ".git") {
                continue;
            }
            let rel = rel_bytes(rel);
            let is_ignore_file = rel.rsplit(|&b| b == b'/').next() == Some(b".gitignore");
            if is_ignore_file {
                pending.full = true;
                pending.touch();
                continue;
            }
            if let Some(status) = status {
                if is_ignored(&self.root, &path, status) {
                    continue;
                }
                if let Some(watches) = watches.as_deref_mut() {
                    if path.is_dir() && !watches.dirs.contains(&path) {
                        watches.add_tree(&self.root, &path, status);
                    }
                }
            }
            if !pending.full {
                if pending.paths.len() >= self.options.max_scopes * 4 {
                    pending.full = true;
                } else {
                    pending.paths.insert(rel);
                }
            }
            pending.touch();
        }
    }
}

/// The places to recompute for the changed paths, or `None` for everything.
fn plan_scopes(
    paths: &BTreeSet<Vec<u8>>,
    status: &RepoStatus,
    max_scopes: usize,
) -> Option<Vec<Vec<u8>>> {
    // A staged rename is two paths, and a recompute of one cannot see the other.
    if status.has_staged_renames() {
        return None;
    }
    let mut scopes: BTreeSet<Vec<u8>> = BTreeSet::new();
    for rel in paths {
        let scope = match status.collapsed_ancestor(rel) {
            // Inside an untracked folder: recompute the folder, which may now be gone or partly
            // tracked. (Inside an ignored one nothing is watched.)
            Some((folder, _)) => folder.to_vec(),
            None => match rel.iter().rposition(|&b| b == b'/') {
                Some(at) => rel[..at].to_vec(),
                None => rel.clone(),
            },
        };
        scopes.insert(scope);
    }
    // A scope inside another is redundant.
    let all: Vec<&Vec<u8>> = scopes.iter().collect();
    let kept: Vec<Vec<u8>> = all
        .iter()
        .filter(|scope| {
            !all.iter().any(|other| {
                other != *scope && scope.starts_with(other) && scope.get(other.len()) == Some(&b'/')
            })
        })
        .map(|scope| (*scope).clone())
        .collect();
    (kept.len() <= max_scopes).then_some(kept)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::status::model::EntryStatus;

    fn status(entries: &[(&str, EntryStatus)]) -> RepoStatus {
        RepoStatus::new(
            PathBuf::from("/r"),
            entries
                .iter()
                .map(|(p, s)| (p.as_bytes().to_vec(), *s))
                .collect::<BTreeMap<_, _>>(),
        )
    }

    fn paths(list: &[&str]) -> BTreeSet<Vec<u8>> {
        list.iter().map(|p| p.as_bytes().to_vec()).collect()
    }

    fn scope_strings(scopes: Option<Vec<Vec<u8>>>) -> Option<Vec<String>> {
        scopes.map(|s| {
            s.into_iter()
                .map(|p| String::from_utf8(p).unwrap())
                .collect()
        })
    }

    #[test]
    fn a_change_recomputes_its_folder_and_a_top_level_one_itself() {
        let s = status(&[]);
        assert_eq!(
            scope_strings(plan_scopes(&paths(&["a/b/c.txt", "top.txt"]), &s, 8)),
            Some(vec!["a/b".into(), "top.txt".into()])
        );
    }

    #[test]
    fn nested_scopes_collapse_into_the_outer_one() {
        let s = status(&[]);
        assert_eq!(
            scope_strings(plan_scopes(
                &paths(&["a/b/c.txt", "a/d.txt", "ab/x"]),
                &s,
                8
            )),
            Some(vec!["a".into(), "ab".into()])
        );
    }

    #[test]
    fn a_change_inside_an_untracked_folder_recomputes_that_folder() {
        let s = status(&[("new_dir", EntryStatus::untracked())]);
        assert_eq!(
            scope_strings(plan_scopes(&paths(&["new_dir/deep/x.txt"]), &s, 8)),
            Some(vec!["new_dir".into()])
        );
    }

    #[test]
    fn too_many_places_or_a_staged_rename_means_everything() {
        let s = status(&[]);
        assert_eq!(
            plan_scopes(&paths(&["a/x", "b/x", "c/x"]), &s, 2),
            None,
            "more scopes than the limit"
        );
        let renamed = status(&[(
            "moved",
            EntryStatus {
                staged: Some(Change::Renamed),
                unstaged: None,
            },
        )]);
        assert_eq!(plan_scopes(&paths(&["a/x"]), &renamed, 8), None);
    }
}
