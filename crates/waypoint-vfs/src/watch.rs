// Local folder watching: `notify` events, coalesced into by-name changes, with a polling fallback.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::{HashMap, HashSet, VecDeque};
use std::ffi::{OsStr, OsString};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use notify::event::{ModifyKind, RenameMode};
use notify::{EventKind, RecursiveMode, Watcher};
use waypoint_protocol::{Location, VfsError};

use crate::error::from_io;
use crate::local::{list_folder, stat_child};
use crate::provider::{Change, RescanReason, ScannedEntry, Watch, WatchEvent, WatchSink};
use crate::CancelToken;

/// Which mechanism a watch uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WatchMode {
    /// The operating system's notifications (inotify, `ReadDirectoryChangesW`), falling back to
    /// polling, with a reported reason, when they cannot be set up.
    #[default]
    Auto,
    /// Only the operating system's notifications; failing to set them up is an error.
    Native,
    /// Only polling. Used where notifications are known not to work, and by tests.
    Poll,
}

/// The timings of a local watch.
#[derive(Debug, Clone, Copy)]
pub struct WatchOptions {
    pub mode: WatchMode,
    /// How long events must go quiet before a batch is sent.
    pub debounce: Duration,
    /// The longest a steady stream of events may delay a batch.
    pub max_wait: Duration,
    /// How long an unmatched "renamed from" waits for its "renamed to" before it counts as a removal.
    pub rename_grace: Duration,
    /// How often the polling fallback re-reads the folder.
    pub poll_interval: Duration,
}

impl Default for WatchOptions {
    fn default() -> Self {
        Self {
            mode: WatchMode::Auto,
            debounce: Duration::from_millis(50),
            max_wait: Duration::from_millis(500),
            rename_grace: Duration::from_millis(50),
            poll_interval: Duration::from_secs(2),
        }
    }
}

/// An operating system event reduced to what coalescing needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Raw {
    Created(PathBuf),
    Removed(PathBuf),
    Modified(PathBuf),
    RenameFrom {
        path: PathBuf,
        tracker: Option<usize>,
    },
    RenameTo {
        path: PathBuf,
        tracker: Option<usize>,
    },
    RenameBoth {
        from: PathBuf,
        to: PathBuf,
        tracker: Option<usize>,
    },
    /// The system dropped events.
    Overflow,
    /// The backend reported an error while running.
    Failure(String),
}

/// Reduces a `notify` event to the raw events the coalescer understands.
pub(crate) fn translate(event: notify::Event) -> Vec<Raw> {
    if event.need_rescan() {
        return vec![Raw::Overflow];
    }
    let tracker = event.tracker();
    let mut paths = event.paths.into_iter();
    match event.kind {
        EventKind::Create(_) => paths.map(Raw::Created).collect(),
        EventKind::Remove(_) => paths.map(Raw::Removed).collect(),
        EventKind::Modify(ModifyKind::Name(RenameMode::From)) => paths
            .map(|path| Raw::RenameFrom { path, tracker })
            .collect(),
        EventKind::Modify(ModifyKind::Name(RenameMode::To)) => {
            paths.map(|path| Raw::RenameTo { path, tracker }).collect()
        }
        EventKind::Modify(ModifyKind::Name(RenameMode::Both)) => {
            match (paths.next(), paths.next()) {
                (Some(from), Some(to)) => vec![Raw::RenameBoth { from, to, tracker }],
                (Some(only), None) => vec![Raw::Modified(only)],
                _ => Vec::new(),
            }
        }
        EventKind::Modify(_) | EventKind::Any | EventKind::Other => {
            paths.map(Raw::Modified).collect()
        }
        EventKind::Access(_) => Vec::new(),
    }
}

struct PendingFrom {
    name: OsString,
    tracker: Option<usize>,
    at: Instant,
}

/// Collects raw events over a short window and turns them into one batch of by-name changes.
///
/// Coalescing works by name, not by event: every touched name is looked at once, when the batch is
/// flushed, and reported as it is then (present, or gone). That makes create-then-delete, bursts of
/// writes and out-of-order events collapse correctly. Renames are the exception, because the file
/// system can tell us "this became that" and the listing can then keep the entry's `EntryId`: a
/// "renamed from" and "renamed to" with the same tracker (or, on backends without trackers,
/// back to back) are paired, and chains such as `a` to `b` to `c` collapse into one.
pub(crate) struct Coalescer {
    folder: PathBuf,
    location: Location,
    touched: Vec<OsString>,
    seen: HashSet<OsString>,
    renames: Vec<(OsString, OsString)>,
    pending_from: Vec<PendingFrom>,
    paired: VecDeque<usize>,
    overflow: Option<RescanReason>,
    folder_touched: bool,
    first: Option<Instant>,
    last: Option<Instant>,
}

/// What a flush needs to know about the file system.
pub(crate) trait Probe {
    fn stat(&self, name: &OsStr) -> io::Result<ScannedEntry>;
    /// Whether the watched folder can still be read.
    fn folder_health(&self) -> io::Result<()>;
}

/// How many paired rename trackers are remembered for a trailing `RenameBoth`.
const PAIRED_MEMORY: usize = 64;

/// Whether a failure to read the watched folder means it is gone for good. Anything else
/// (`EMFILE`, `EINTR`, a permission flap) is transient and the watch carries on.
fn is_fatal(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::NotFound | io::ErrorKind::PermissionDenied
    )
}

impl Coalescer {
    pub fn new(folder: PathBuf, location: Location) -> Self {
        Self {
            folder,
            location,
            touched: Vec::new(),
            seen: HashSet::new(),
            renames: Vec::new(),
            pending_from: Vec::new(),
            paired: VecDeque::new(),
            overflow: None,
            folder_touched: false,
            first: None,
            last: None,
        }
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.first.is_none()
    }

    fn child_name(&self, path: &Path) -> Option<OsString> {
        if path.parent() == Some(self.folder.as_path()) {
            path.file_name().map(OsStr::to_owned)
        } else {
            None
        }
    }

    fn touch(&mut self, path: &Path) {
        match self.child_name(path) {
            Some(name) => {
                if self.seen.insert(name.clone()) {
                    self.touched.push(name);
                }
            }
            None if path == self.folder => self.folder_touched = true,
            // Something deeper, or outside: not part of this listing.
            None => {}
        }
    }

    fn add_rename(&mut self, from: OsString, to: OsString) {
        let at = match self.renames.iter().position(|(_, target)| *target == from) {
            Some(at) => {
                self.renames[at].1 = to;
                at
            }
            None => {
                self.renames.push((from, to));
                self.renames.len() - 1
            }
        };
        let (origin, end) = self.renames[at].clone();
        if origin == end {
            // Renamed away and back: nothing moved, but the name may have new contents.
            self.renames.remove(at);
            self.touch_name(origin);
            return;
        }
        // A swap or longer cycle (`a` to `tmp`, `b` to `a`, `tmp` to `b`) cannot be replayed by
        // name, so ask for a rescan instead of guessing.
        let mut current = end;
        for _ in 0..=self.renames.len() {
            match self.renames.iter().find(|(f, _)| *f == current) {
                Some((_, next)) => current = next.clone(),
                None => return,
            }
            if current == origin {
                self.overflow
                    .get_or_insert(RescanReason::Unknown("a rename cycle".to_owned()));
                return;
            }
        }
    }

    fn touch_name(&mut self, name: OsString) {
        if self.seen.insert(name.clone()) {
            self.touched.push(name);
        }
    }

    pub fn push(&mut self, raw: Raw, now: Instant) {
        self.first.get_or_insert(now);
        self.last = Some(now);
        match raw {
            Raw::Created(path) | Raw::Removed(path) | Raw::Modified(path) => self.touch(&path),
            Raw::RenameFrom { path, tracker } => match self.child_name(&path) {
                Some(name) => self.pending_from.push(PendingFrom {
                    name,
                    tracker,
                    at: now,
                }),
                None => self.touch(&path),
            },
            Raw::RenameTo { path, tracker } => {
                let Some(to) = self.child_name(&path) else {
                    return self.touch(&path);
                };
                let matching = self.pending_from.iter().rposition(|p| match tracker {
                    Some(_) => p.tracker == tracker,
                    // Without trackers the pair arrives back to back.
                    None => p.tracker.is_none(),
                });
                match matching {
                    Some(at) => {
                        let from = self.pending_from.remove(at);
                        if let Some(tracker) = tracker {
                            // Remembered past a flush, for the trailing `RenameBoth`.
                            self.paired.push_back(tracker);
                            if self.paired.len() > PAIRED_MEMORY {
                                self.paired.pop_front();
                            }
                        }
                        self.add_rename(from.name, to);
                    }
                    None => self.touch(&path),
                }
            }
            Raw::RenameBoth { from, to, tracker } => {
                // `notify` reports an inotify pair as From, To and then Both; the first two have
                // already paired it.
                if let Some(at) = tracker.and_then(|t| self.paired.iter().position(|p| *p == t)) {
                    self.paired.remove(at);
                    return;
                }
                match (self.child_name(&from), self.child_name(&to)) {
                    (Some(from), Some(to)) => self.add_rename(from, to),
                    _ => {
                        self.touch(&from);
                        self.touch(&to);
                    }
                }
            }
            Raw::Overflow => self.overflow = Some(RescanReason::Overflow),
            Raw::Failure(message) => self.overflow = Some(RescanReason::Unknown(message)),
        }
    }

    /// When the batch is due: after the quiet period, never later than `max_wait` after its first
    /// event, and not before an unmatched "renamed from" has had `rename_grace` to find its partner.
    pub fn due_at(&self, options: &WatchOptions) -> Option<Instant> {
        let (first, last) = (self.first?, self.last?);
        let mut due = (last + options.debounce).min(first + options.max_wait);
        if let Some(oldest) = self.pending_from.iter().map(|p| p.at).min() {
            due = due.max(oldest + options.rename_grace);
        }
        Some(due)
    }

    /// Turns everything collected into events and starts over.
    pub fn flush(&mut self, probe: &dyn Probe) -> Vec<WatchEvent> {
        // A "renamed from" that never found its partner is a removal or a move out; `stat` tells
        // which.
        for unmatched in std::mem::take(&mut self.pending_from) {
            if self.seen.insert(unmatched.name.clone()) {
                self.touched.push(unmatched.name);
            }
        }
        let touched = std::mem::take(&mut self.touched);
        let renames = std::mem::take(&mut self.renames);
        let overflow = self.overflow.take();
        let folder_touched = std::mem::take(&mut self.folder_touched);
        self.seen.clear();
        self.first = None;
        self.last = None;

        if folder_touched {
            if let Err(error) = probe.folder_health() {
                if is_fatal(&error) {
                    return vec![WatchEvent::Lost(from_io(&error, &self.location))];
                }
            }
        }
        if let Some(reason) = overflow {
            return vec![WatchEvent::Rescan(reason)];
        }

        let mut changes = Vec::new();
        let targets: HashSet<&OsString> = renames.iter().map(|(_, to)| to).collect();
        for (from, to) in &renames {
            match probe.stat(to) {
                Ok(entry) => changes.push(Change::Rename {
                    from: from.clone(),
                    to: entry,
                }),
                Err(_) => changes.push(Change::Remove(from.clone())),
            }
        }
        for name in &touched {
            if targets.contains(name) {
                continue;
            }
            match probe.stat(name) {
                Ok(entry) => changes.push(Change::Upsert(entry)),
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    changes.push(Change::Remove(name.clone()));
                }
                // Unreadable right now: leave the entry as it was.
                Err(_) => {}
            }
        }
        if changes.is_empty() {
            Vec::new()
        } else {
            vec![WatchEvent::Changes(changes)]
        }
    }
}

struct LocalProbe<'a> {
    folder: &'a Path,
}

impl Probe for LocalProbe<'_> {
    fn stat(&self, name: &OsStr) -> io::Result<ScannedEntry> {
        stat_child(self.folder, name)
    }

    fn folder_health(&self) -> io::Result<()> {
        std::fs::read_dir(self.folder).map(|_| ())
    }
}

/// How starting a native watch failed.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum StartFailure {
    /// The folder cannot be watched because it cannot be used at all.
    Fatal(VfsError),
    /// Notifications are unavailable; poll instead and say why.
    Fallback(String),
}

pub(crate) fn classify_start_error(error: &notify::Error, location: &Location) -> StartFailure {
    match &error.kind {
        notify::ErrorKind::PathNotFound => StartFailure::Fatal(VfsError::NotFound {
            location: location.clone(),
        }),
        notify::ErrorKind::MaxFilesWatch => StartFailure::Fallback(
            "the system limit on inotify watches was reached (raise `fs.inotify.max_user_watches`)"
                .to_owned(),
        ),
        notify::ErrorKind::Io(io) => match io.kind() {
            io::ErrorKind::NotFound | io::ErrorKind::PermissionDenied => {
                StartFailure::Fatal(from_io(io, location))
            }
            _ => match io.raw_os_error() {
                // ENOSPC: the watch limit. EMFILE: the limit on inotify instances.
                #[cfg(target_os = "linux")]
                Some(28) => StartFailure::Fallback(
                    "the system limit on inotify watches was reached (raise `fs.inotify.max_user_watches`)"
                        .to_owned(),
                ),
                #[cfg(target_os = "linux")]
                Some(24) => StartFailure::Fallback(
                    "the system limit on inotify instances was reached (raise `fs.inotify.max_user_instances`)"
                        .to_owned(),
                ),
                _ => StartFailure::Fallback(format!("native file watching failed: {io}")),
            },
        },
        other => StartFailure::Fallback(format!("native file watching failed: {other:?}")),
    }
}

/// A running local watch. Dropping it stops the worker thread and the operating system watch.
pub(crate) struct LocalWatch {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    // Dropped before the thread is joined, which disconnects the event channel.
    watcher: Option<notify::RecommendedWatcher>,
}

impl Watch for LocalWatch {}

impl Drop for LocalWatch {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.watcher.take();
        // The worker is detached, not joined: it may be inside an uncancellable scan of a huge
        // folder, and closing a listing must not wait for that. It checks `stop` at least every
        // `TICK`, and its sink is gated on `stop`, so it delivers nothing after this and exits
        // as soon as its current step finishes.
        drop(self.thread.take());
    }
}

/// The longest a worker sleeps before checking whether it was asked to stop.
const TICK: Duration = Duration::from_millis(50);

/// How often an idle native watch checks that its folder can still be read, for the platforms and
/// cases (a folder moved or deleted) where the operating system says nothing.
const HEALTH_EVERY: Duration = Duration::from_secs(2);

/// Starts watching `folder`, choosing the mechanism from `options`.
pub(crate) fn start(
    folder: PathBuf,
    location: Location,
    options: WatchOptions,
    sink: WatchSink,
) -> Result<Box<dyn Watch>, VfsError> {
    let stop = Arc::new(AtomicBool::new(false));
    let sink: WatchSink = {
        let (stop, sink) = (stop.clone(), sink);
        Arc::new(move |event| {
            if !stop.load(Ordering::Relaxed) {
                sink(event);
            }
        })
    };
    let mut fallback_reason = None;

    if options.mode != WatchMode::Poll {
        let (tx, rx) = mpsc::channel::<notify::Result<notify::Event>>();
        let started = notify::recommended_watcher(move |result| {
            let _ = tx.send(result);
        })
        .and_then(|mut watcher| {
            watcher
                .watch(&folder, RecursiveMode::NonRecursive)
                .map(|()| watcher)
        });
        match started {
            Ok(watcher) => {
                let thread = {
                    let error_location = location.clone();
                    let (stop, folder, location, sink) =
                        (stop.clone(), folder.clone(), location.clone(), sink.clone());
                    thread::Builder::new()
                        .name("waypoint-watch".to_owned())
                        .spawn(move || run_native(rx, folder, location, options, sink, stop))
                        .map_err(|e| from_io(&e, &error_location))?
                };
                return Ok(Box::new(LocalWatch {
                    stop,
                    thread: Some(thread),
                    watcher: Some(watcher),
                }));
            }
            Err(error) => match classify_start_error(&error, &location) {
                StartFailure::Fatal(error) => return Err(error),
                StartFailure::Fallback(reason) if options.mode == WatchMode::Auto => {
                    fallback_reason = Some(reason);
                }
                StartFailure::Fallback(reason) => {
                    return Err(VfsError::Unsupported { what: reason });
                }
            },
        }
    }

    // The first snapshot is taken before `start` returns, so a change made right after cannot be
    // missed by a worker that starts late.
    let before = snapshot(&folder, &location)?;
    let reason = fallback_reason.unwrap_or_else(|| "polling was requested".to_owned());
    sink(WatchEvent::Degraded {
        reason: format!(
            "{reason}; checking for changes every {:?}",
            options.poll_interval
        ),
    });
    let thread = {
        let stop = stop.clone();
        thread::Builder::new()
            .name("waypoint-poll".to_owned())
            .spawn(move || run_poll(folder, location, before, options, sink, stop))
            .map_err(|e| VfsError::Io {
                message: e.to_string(),
                location: None,
            })?
    };
    Ok(Box::new(LocalWatch {
        stop,
        thread: Some(thread),
        watcher: None,
    }))
}

fn run_native(
    rx: mpsc::Receiver<notify::Result<notify::Event>>,
    folder: PathBuf,
    location: Location,
    options: WatchOptions,
    sink: WatchSink,
    stop: Arc<AtomicBool>,
) {
    let mut coalescer = Coalescer::new(folder.clone(), location);
    let probe = LocalProbe { folder: &folder };
    let mut last_health = Instant::now();
    loop {
        if stop.load(Ordering::Relaxed) {
            return;
        }
        let now = Instant::now();
        if now.duration_since(last_health) >= HEALTH_EVERY {
            last_health = now;
            if let Err(error) = probe.folder_health() {
                if is_fatal(&error) {
                    return sink(WatchEvent::Lost(from_io(&error, &coalescer.location)));
                }
            }
        }
        let wait = match coalescer.due_at(&options) {
            Some(due) if due <= now => {
                for event in coalescer.flush(&probe) {
                    let done = matches!(event, WatchEvent::Lost(_));
                    sink(event);
                    if done {
                        return;
                    }
                }
                continue;
            }
            Some(due) => (due - now).min(TICK),
            None => TICK,
        };
        match rx.recv_timeout(wait) {
            Ok(Ok(event)) => {
                let now = Instant::now();
                for raw in translate(event) {
                    coalescer.push(raw, now);
                }
            }
            Ok(Err(error)) => coalescer.push(Raw::Failure(error.to_string()), Instant::now()),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}

/// Compares two folder snapshots by name. Polling cannot tell a rename from a removal plus an
/// addition, so a renamed entry gets a new `EntryId` under it.
pub(crate) fn diff_snapshots(
    before: &HashMap<OsString, ScannedEntry>,
    after: &HashMap<OsString, ScannedEntry>,
) -> Vec<Change> {
    let mut changes = Vec::new();
    for name in before.keys() {
        if !after.contains_key(name) {
            changes.push(Change::Remove(name.clone()));
        }
    }
    for (name, entry) in after {
        if before.get(name) != Some(entry) {
            changes.push(Change::Upsert(entry.clone()));
        }
    }
    changes
}

fn snapshot(
    folder: &Path,
    location: &Location,
) -> Result<HashMap<OsString, ScannedEntry>, VfsError> {
    let entries = list_folder(
        folder,
        location,
        &CancelToken::new(),
        usize::MAX,
        &mut |_| {},
    )?;
    Ok(entries.into_iter().map(|e| (e.name.clone(), e)).collect())
}

fn run_poll(
    folder: PathBuf,
    location: Location,
    mut before: HashMap<OsString, ScannedEntry>,
    options: WatchOptions,
    sink: WatchSink,
    stop: Arc<AtomicBool>,
) {
    let mut next = Instant::now() + options.poll_interval;
    loop {
        if stop.load(Ordering::Relaxed) {
            return;
        }
        let now = Instant::now();
        if now < next {
            thread::sleep((next - now).min(TICK));
            continue;
        }
        next = now + options.poll_interval;
        let taken = snapshot(&folder, &location);
        if stop.load(Ordering::Relaxed) {
            return;
        }
        match taken {
            Ok(after) => {
                let changes = diff_snapshots(&before, &after);
                before = after;
                if !changes.is_empty() {
                    sink(WatchEvent::Changes(changes));
                }
            }
            Err(error @ (VfsError::NotFound { .. } | VfsError::PermissionDenied { .. })) => {
                return sink(WatchEvent::Lost(error));
            }
            // A transient failure (`EMFILE`, `EINTR`): try again at the next interval.
            Err(_) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{EntryKind, IconGroup};
    use std::cell::RefCell;

    fn entry(name: &str) -> ScannedEntry {
        ScannedEntry {
            name: name.into(),
            kind: EntryKind::File,
            link_target: None,
            link_pending: false,
            special: None,
            group: IconGroup::Other,
            size: Some(1),
            modified_ms: Some(0),
            hidden: false,
            trashed: None,
            attributes: None,
        }
    }

    /// A fake folder: the names that exist, and whether the folder itself is readable.
    struct Fake {
        present: RefCell<HashSet<String>>,
        health: RefCell<Option<io::ErrorKind>>,
    }

    impl Fake {
        fn with(names: &[&str]) -> Self {
            Self {
                present: RefCell::new(names.iter().map(|n| (*n).to_owned()).collect()),
                health: RefCell::new(None),
            }
        }
    }

    impl Probe for Fake {
        fn stat(&self, name: &OsStr) -> io::Result<ScannedEntry> {
            let name = name.to_string_lossy();
            if self.present.borrow().contains(&*name) {
                Ok(entry(&name))
            } else {
                Err(io::ErrorKind::NotFound.into())
            }
        }

        fn folder_health(&self) -> io::Result<()> {
            match *self.health.borrow() {
                Some(kind) => Err(kind.into()),
                None => Ok(()),
            }
        }
    }

    fn coalescer() -> Coalescer {
        Coalescer::new("/d".into(), Location::new("/d", "file:///d"))
    }

    fn p(name: &str) -> PathBuf {
        PathBuf::from("/d").join(name)
    }

    fn changes(events: Vec<WatchEvent>) -> Vec<Change> {
        match events.as_slice() {
            [WatchEvent::Changes(changes)] => changes.clone(),
            [] => Vec::new(),
            other => panic!("expected changes, got {other:?}"),
        }
    }

    #[test]
    fn a_burst_of_events_on_one_name_is_one_change() {
        let now = Instant::now();
        let mut c = coalescer();
        for raw in [
            Raw::Created(p("a")),
            Raw::Modified(p("a")),
            Raw::Modified(p("a")),
        ] {
            c.push(raw, now);
        }
        let fake = Fake::with(&["a"]);
        assert_eq!(changes(c.flush(&fake)), [Change::Upsert(entry("a"))]);
        assert!(c.is_empty());
    }

    #[test]
    fn created_then_deleted_in_one_window_reports_a_removal_of_nothing() {
        let now = Instant::now();
        let mut c = coalescer();
        c.push(Raw::Created(p("tmp")), now);
        c.push(Raw::Removed(p("tmp")), now);
        assert_eq!(
            changes(c.flush(&Fake::with(&[]))),
            [Change::Remove("tmp".into())]
        );
    }

    #[test]
    fn a_rename_is_paired_by_tracker_and_the_both_event_is_ignored() {
        let now = Instant::now();
        let mut c = coalescer();
        c.push(
            Raw::RenameFrom {
                path: p("old"),
                tracker: Some(7),
            },
            now,
        );
        c.push(
            Raw::RenameTo {
                path: p("new"),
                tracker: Some(7),
            },
            now,
        );
        c.push(
            Raw::RenameBoth {
                from: p("old"),
                to: p("new"),
                tracker: Some(7),
            },
            now,
        );
        assert_eq!(
            changes(c.flush(&Fake::with(&["new"]))),
            [Change::Rename {
                from: "old".into(),
                to: entry("new")
            }]
        );
    }

    #[test]
    fn renames_pair_back_to_back_when_the_backend_has_no_trackers() {
        let now = Instant::now();
        let mut c = coalescer();
        c.push(
            Raw::RenameFrom {
                path: p("old"),
                tracker: None,
            },
            now,
        );
        c.push(
            Raw::RenameTo {
                path: p("new"),
                tracker: None,
            },
            now,
        );
        assert_eq!(
            changes(c.flush(&Fake::with(&["new"]))),
            [Change::Rename {
                from: "old".into(),
                to: entry("new")
            }]
        );
    }

    #[test]
    fn a_chain_of_renames_collapses_to_one() {
        let now = Instant::now();
        let mut c = coalescer();
        for (tracker, from, to) in [(1, "a", "b"), (2, "b", "c")] {
            c.push(
                Raw::RenameFrom {
                    path: p(from),
                    tracker: Some(tracker),
                },
                now,
            );
            c.push(
                Raw::RenameTo {
                    path: p(to),
                    tracker: Some(tracker),
                },
                now,
            );
        }
        assert_eq!(
            changes(c.flush(&Fake::with(&["c"]))),
            [Change::Rename {
                from: "a".into(),
                to: entry("c")
            }]
        );
    }

    #[test]
    fn a_rename_whose_target_vanished_is_a_removal() {
        let now = Instant::now();
        let mut c = coalescer();
        c.push(
            Raw::RenameFrom {
                path: p("a"),
                tracker: Some(1),
            },
            now,
        );
        c.push(
            Raw::RenameTo {
                path: p("b"),
                tracker: Some(1),
            },
            now,
        );
        assert_eq!(
            changes(c.flush(&Fake::with(&[]))),
            [Change::Remove("a".into())]
        );
    }

    #[test]
    fn an_unmatched_from_is_a_move_out_and_an_unmatched_to_is_a_move_in() {
        let now = Instant::now();
        let mut c = coalescer();
        c.push(
            Raw::RenameFrom {
                path: p("leaving"),
                tracker: Some(1),
            },
            now,
        );
        c.push(
            Raw::RenameTo {
                path: p("arriving"),
                tracker: Some(2),
            },
            now,
        );
        assert_eq!(
            changes(c.flush(&Fake::with(&["arriving"]))),
            [
                Change::Upsert(entry("arriving")),
                Change::Remove("leaving".into()),
            ]
        );
    }

    #[test]
    fn events_for_other_folders_are_ignored() {
        let now = Instant::now();
        let mut c = coalescer();
        c.push(Raw::Created(PathBuf::from("/d/sub/deep")), now);
        c.push(Raw::Created(PathBuf::from("/elsewhere/x")), now);
        assert!(changes(c.flush(&Fake::with(&["deep", "x"]))).is_empty());
    }

    #[test]
    fn an_overflow_asks_for_a_rescan_instead_of_guessing() {
        let now = Instant::now();
        let mut c = coalescer();
        c.push(Raw::Created(p("a")), now);
        c.push(Raw::Overflow, now);
        assert_eq!(
            c.flush(&Fake::with(&["a"])),
            [WatchEvent::Rescan(RescanReason::Overflow)]
        );
        c.push(Raw::Failure("boom".to_owned()), now);
        assert_eq!(
            c.flush(&Fake::with(&[])),
            [WatchEvent::Rescan(RescanReason::Unknown("boom".to_owned()))]
        );
    }

    #[test]
    fn the_folder_going_away_or_unreadable_is_lost() {
        let now = Instant::now();
        let fake = Fake::with(&[]);
        *fake.health.borrow_mut() = Some(io::ErrorKind::NotFound);
        let mut c = coalescer();
        c.push(Raw::Removed(PathBuf::from("/d")), now);
        assert!(matches!(
            c.flush(&fake).as_slice(),
            [WatchEvent::Lost(VfsError::NotFound { .. })]
        ));
        *fake.health.borrow_mut() = Some(io::ErrorKind::PermissionDenied);
        c.push(Raw::Modified(PathBuf::from("/d")), now);
        assert!(matches!(
            c.flush(&fake).as_slice(),
            [WatchEvent::Lost(VfsError::PermissionDenied { .. })]
        ));
        // A touch of the folder that leaves it readable is nothing.
        *fake.health.borrow_mut() = None;
        c.push(Raw::Modified(PathBuf::from("/d")), now);
        assert!(c.flush(&fake).is_empty());
    }

    #[test]
    fn a_transient_folder_read_failure_is_not_a_loss() {
        let now = Instant::now();
        let fake = Fake::with(&[]);
        *fake.health.borrow_mut() = Some(io::ErrorKind::Interrupted);
        let mut c = coalescer();
        c.push(Raw::Modified(PathBuf::from("/d")), now);
        assert!(c.flush(&fake).is_empty());
        assert!(is_fatal(&io::ErrorKind::NotFound.into()));
        assert!(!is_fatal(&io::Error::from_raw_os_error(24)));
    }

    fn rename(c: &mut Coalescer, from: &str, to: &str, tracker: usize, now: Instant) {
        c.push(
            Raw::RenameFrom {
                path: p(from),
                tracker: Some(tracker),
            },
            now,
        );
        c.push(
            Raw::RenameTo {
                path: p(to),
                tracker: Some(tracker),
            },
            now,
        );
    }

    #[test]
    fn a_swap_through_a_temporary_name_asks_for_a_rescan() {
        let now = Instant::now();
        let mut c = coalescer();
        rename(&mut c, "a", "tmp", 1, now);
        rename(&mut c, "b", "a", 2, now);
        rename(&mut c, "tmp", "b", 3, now);
        assert_eq!(
            c.flush(&Fake::with(&["a", "b"])),
            [WatchEvent::Rescan(RescanReason::Unknown(
                "a rename cycle".to_owned()
            ))]
        );
    }

    #[test]
    fn a_rename_away_and_back_is_a_refresh_not_a_rename() {
        let now = Instant::now();
        let mut c = coalescer();
        rename(&mut c, "a", "tmp", 1, now);
        rename(&mut c, "tmp", "a", 2, now);
        assert_eq!(
            changes(c.flush(&Fake::with(&["a"]))),
            [Change::Upsert(entry("a"))]
        );
    }

    #[test]
    fn a_trailing_both_after_a_flush_is_still_a_duplicate() {
        let now = Instant::now();
        let mut c = coalescer();
        rename(&mut c, "a", "b", 7, now);
        assert_eq!(changes(c.flush(&Fake::with(&["b"]))).len(), 1);
        c.push(
            Raw::RenameBoth {
                from: p("a"),
                to: p("b"),
                tracker: Some(7),
            },
            now,
        );
        assert!(changes(c.flush(&Fake::with(&["b"]))).is_empty());
    }

    #[test]
    fn a_batch_is_due_after_the_quiet_period_but_not_later_than_the_maximum() {
        let options = WatchOptions::default();
        let start = Instant::now();
        let mut c = coalescer();
        assert_eq!(c.due_at(&options), None);
        c.push(Raw::Created(p("a")), start);
        assert_eq!(c.due_at(&options), Some(start + options.debounce));
        // A steady stream keeps pushing the quiet period back, up to the maximum.
        let later = start + Duration::from_millis(480);
        c.push(Raw::Modified(p("a")), later);
        assert_eq!(c.due_at(&options), Some(start + options.max_wait));
    }

    #[test]
    fn an_unmatched_from_holds_the_batch_for_the_rename_grace() {
        let options = WatchOptions::default();
        let start = Instant::now();
        let mut c = coalescer();
        c.push(
            Raw::RenameFrom {
                path: p("a"),
                tracker: Some(1),
            },
            start,
        );
        assert_eq!(c.due_at(&options), Some(start + options.rename_grace));
    }

    #[test]
    fn translates_notify_events() {
        use notify::event::{CreateKind, Event};
        let created = Event::new(EventKind::Create(CreateKind::File)).add_path(p("a"));
        assert_eq!(translate(created), [Raw::Created(p("a"))]);
        let rescan = Event::new(EventKind::Other).set_flag(notify::event::Flag::Rescan);
        assert_eq!(translate(rescan), [Raw::Overflow]);
        let access = Event::new(EventKind::Access(notify::event::AccessKind::Any)).add_path(p("a"));
        assert!(translate(access).is_empty());
        let from = Event::new(EventKind::Modify(ModifyKind::Name(RenameMode::From)))
            .add_path(p("a"))
            .set_tracker(9);
        assert_eq!(
            translate(from),
            [Raw::RenameFrom {
                path: p("a"),
                tracker: Some(9)
            }]
        );
    }

    #[test]
    fn the_inotify_limits_fall_back_to_polling_with_a_reason() {
        let location = Location::new("/d", "file:///d");
        let limit = notify::Error::new(notify::ErrorKind::MaxFilesWatch);
        assert!(matches!(
            classify_start_error(&limit, &location),
            StartFailure::Fallback(reason) if reason.contains("max_user_watches")
        ));
        let gone = notify::Error::path_not_found();
        assert!(matches!(
            classify_start_error(&gone, &location),
            StartFailure::Fatal(VfsError::NotFound { .. })
        ));
        let denied = notify::Error::io(io::Error::from(io::ErrorKind::PermissionDenied));
        assert!(matches!(
            classify_start_error(&denied, &location),
            StartFailure::Fatal(VfsError::PermissionDenied { .. })
        ));
        let odd = notify::Error::generic("who knows");
        assert!(matches!(
            classify_start_error(&odd, &location),
            StartFailure::Fallback(_)
        ));
        #[cfg(target_os = "linux")]
        {
            let enospc = notify::Error::io(io::Error::from_raw_os_error(28));
            assert!(matches!(
                classify_start_error(&enospc, &location),
                StartFailure::Fallback(reason) if reason.contains("max_user_watches")
            ));
            let emfile = notify::Error::io(io::Error::from_raw_os_error(24));
            assert!(matches!(
                classify_start_error(&emfile, &location),
                StartFailure::Fallback(reason) if reason.contains("max_user_instances")
            ));
        }
    }

    #[test]
    fn polling_snapshots_diff_into_changes() {
        let snap = |names: &[&str]| -> HashMap<OsString, ScannedEntry> {
            names
                .iter()
                .map(|n| (OsString::from(n), entry(n)))
                .collect()
        };
        let before = snap(&["a", "b"]);
        let mut after = snap(&["b", "c"]);
        after.get_mut(OsStr::new("b")).unwrap().size = Some(99);
        let mut diff = diff_snapshots(&before, &after);
        diff.sort_by_key(|c| format!("{c:?}"));
        assert_eq!(diff.len(), 3);
        assert!(diff.contains(&Change::Remove("a".into())));
        assert!(diff.contains(&Change::Upsert(entry("c"))));
        assert!(diff
            .iter()
            .any(|c| matches!(c, Change::Upsert(e) if e.name == "b" && e.size == Some(99))));
        assert!(diff_snapshots(&before, &before).is_empty());
    }
}
