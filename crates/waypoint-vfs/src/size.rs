// A folder's recursive size: a cancellable walk that stays on one volume and never follows a link.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use waypoint_path::VfsPath;
use waypoint_protocol::VfsError;

use crate::error::from_io;
use crate::local::{file_path, is_placeholder};
use crate::{CancelToken, EntryKind, FolderSizeTotals, Provider};

/// How often a walk reports its running total.
pub const REPORT_EVERY: Duration = Duration::from_millis(100);

/// How many entries a walk handles between looks at the clock and the cancel flag's report.
const CHECK_EVERY: u32 = 64;

/// How a folder-size walk ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FolderSizeRun {
    /// What was counted. After a cancel this is the partial total.
    pub totals: FolderSizeTotals,
    pub cancelled: bool,
}

/// Lowers the calling thread's CPU and disk priority, for work the person is not waiting on. It
/// lasts for the life of the thread, so call it on a thread of its own, not a pooled one.
pub fn lower_thread_priority() {
    crate::sys::lower_thread_priority();
}

struct Counter<'a> {
    totals: FolderSizeTotals,
    cancel: &'a CancelToken,
    report: &'a mut dyn FnMut(&FolderSizeTotals),
    report_every: Duration,
    last_report: Instant,
    since_check: u32,
}

impl Counter<'_> {
    /// Called once per entry. Returns `true` when the walk should stop.
    fn tick(&mut self) -> bool {
        if self.cancel.is_cancelled() {
            return true;
        }
        self.since_check += 1;
        if self.since_check >= CHECK_EVERY {
            self.since_check = 0;
            if self.last_report.elapsed() >= self.report_every {
                self.last_report = Instant::now();
                (self.report)(&self.totals);
            }
        }
        false
    }

    fn run(self, cancelled: bool) -> FolderSizeRun {
        FolderSizeRun {
            totals: self.totals,
            cancelled,
        }
    }
}

/// The total size of the folder at `path`, walking it on the local file system.
///
/// - Symlinks are never followed and never add size (`symlinks_skipped` counts them).
/// - A folder on another volume (a mount point) is not entered (`mounts_skipped`). On Windows a
///   mount point or junction is a reparse point that `std` reports as a link, so it is skipped
///   with the symlinks.
/// - A Windows cloud placeholder counts as a file of zero bytes (`placeholders`); a placeholder
///   folder is not entered, because listing it could download its contents.
/// - A file with several hard links counts once.
/// - An entry that cannot be read adds to `unreadable` and the walk goes on, so a partial total is
///   still a total.
///
/// `cancel` is checked for every entry. A cancelled walk returns what it counted, with `cancelled`
/// set. Only the folder itself being unreadable is an error.
pub(crate) fn local_folder_size(
    path: &VfsPath,
    cancel: &CancelToken,
    report: &mut dyn FnMut(&FolderSizeTotals),
) -> Result<FolderSizeRun, VfsError> {
    walk_local(path, cancel, report, REPORT_EVERY)
}

fn walk_local(
    path: &VfsPath,
    cancel: &CancelToken,
    report: &mut dyn FnMut(&FolderSizeTotals),
    report_every: Duration,
) -> Result<FolderSizeRun, VfsError> {
    let location = path.to_location();
    let root = file_path(path)?.as_path().to_path_buf();
    let root_meta = open_root(&root, &location)?;
    let mut walker = LocalWalker::new(&root_meta, cancel, report, report_every);
    let cancelled = walker
        .walk(root)
        .map_err(|error| from_io(&error, &location))?;
    Ok(walker.finish(cancelled))
}

/// Looks at the root of a walk. It may be a link to a folder: the person asked about what it
/// shows. Everything below it is looked at without following.
pub(crate) fn open_root(
    root: &std::path::Path,
    location: &waypoint_protocol::Location,
) -> Result<fs::Metadata, VfsError> {
    let root_meta = fs::metadata(root).map_err(|e| from_io(&e, location))?;
    if !root_meta.is_dir() {
        return Err(VfsError::NotADirectory {
            location: location.clone(),
        });
    }
    Ok(root_meta)
}

/// What `LocalWalker::visit` decided about one directory entry.
pub(crate) enum Visit {
    /// Counted (or passed over); there is nothing to enter.
    Done,
    /// A folder on the same volume to walk into. It is already counted as a folder.
    Enter(PathBuf),
}

/// The local walk's primitives: how one entry is counted, which folders are entered, and the state
/// that makes a hard link count once across everything this walker visits. The folder size and the
/// directory-size scan are both built on it.
pub(crate) struct LocalWalker<'a> {
    counter: Counter<'a>,
    #[cfg(unix)]
    root_dev: u64,
    #[cfg(unix)]
    seen_links: std::collections::HashSet<(u64, u64)>,
}

impl<'a> LocalWalker<'a> {
    pub(crate) fn new(
        root_meta: &fs::Metadata,
        cancel: &'a CancelToken,
        report: &'a mut dyn FnMut(&FolderSizeTotals),
        report_every: Duration,
    ) -> Self {
        #[cfg(not(unix))]
        let _ = root_meta;
        Self {
            counter: Counter {
                totals: FolderSizeTotals {
                    allocated_bytes: cfg!(unix).then_some(0),
                    ..FolderSizeTotals::default()
                },
                cancel,
                report,
                report_every,
                last_report: Instant::now(),
                since_check: 0,
            },
            #[cfg(unix)]
            root_dev: std::os::unix::fs::MetadataExt::dev(root_meta),
            #[cfg(unix)]
            seen_links: std::collections::HashSet::new(),
        }
    }

    /// What has been counted so far.
    pub(crate) fn totals(&self) -> &FolderSizeTotals {
        &self.counter.totals
    }

    pub(crate) fn unreadable(&mut self) {
        self.counter.totals.unreadable += 1;
    }

    /// Called once per entry; `true` means the walk should stop (it was cancelled).
    pub(crate) fn tick(&mut self) -> bool {
        self.counter.tick()
    }

    pub(crate) fn finish(self, cancelled: bool) -> FolderSizeRun {
        self.counter.run(cancelled)
    }

    /// Counts one entry of a folder: a symlink is skipped, a folder on another volume or a cloud
    /// placeholder folder is passed over, a folder on this volume is counted and returned to
    /// enter, and a file adds its size (a hard-linked file once). Never follows a link.
    pub(crate) fn visit(&mut self, entry: &fs::DirEntry) -> Visit {
        // `DirEntry::metadata` does not follow a symlink, and on Windows it costs no extra call.
        let Ok(meta) = entry.metadata() else {
            self.counter.totals.unreadable += 1;
            return Visit::Done;
        };
        let file_type = meta.file_type();
        if file_type.is_symlink() {
            self.counter.totals.symlinks_skipped += 1;
        } else if file_type.is_dir() {
            #[cfg(unix)]
            if std::os::unix::fs::MetadataExt::dev(&meta) != self.root_dev {
                self.counter.totals.mounts_skipped += 1;
                return Visit::Done;
            }
            if is_placeholder(&meta) {
                self.counter.totals.placeholders += 1;
                return Visit::Done;
            }
            self.counter.totals.folders += 1;
            return Visit::Enter(entry.path());
        } else {
            self.counter.totals.files += 1;
            if is_placeholder(&meta) {
                self.counter.totals.placeholders += 1;
                return Visit::Done;
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                if file_type.is_file() {
                    let again =
                        meta.nlink() > 1 && !self.seen_links.insert((meta.dev(), meta.ino()));
                    if !again {
                        self.counter.totals.bytes += meta.len();
                        if let Some(allocated) = self.counter.totals.allocated_bytes.as_mut() {
                            *allocated += meta.blocks().saturating_mul(512);
                        }
                    }
                }
            }
            #[cfg(not(unix))]
            if file_type.is_file() {
                self.counter.totals.bytes += meta.len();
            }
        }
        Visit::Done
    }

    /// Walks `start` and everything below it on this volume, iteratively (no recursion depth to
    /// overflow). Returns whether the walk was cancelled. Only `start` itself being unreadable is
    /// an error; a folder below it that cannot be read is counted in `unreadable`.
    pub(crate) fn walk(&mut self, start: PathBuf) -> Result<bool, std::io::Error> {
        let mut pending: Vec<PathBuf> = vec![start];
        let mut first = true;
        while let Some(folder) = pending.pop() {
            let entries = match fs::read_dir(&folder) {
                Ok(entries) => entries,
                Err(error) if first => return Err(error),
                Err(_) => {
                    self.counter.totals.unreadable += 1;
                    continue;
                }
            };
            first = false;
            for entry in entries {
                if self.counter.tick() {
                    return Ok(true);
                }
                let Ok(entry) = entry else {
                    self.counter.totals.unreadable += 1;
                    continue;
                };
                if let Visit::Enter(path) = self.visit(&entry) {
                    pending.push(path);
                }
            }
        }
        Ok(false)
    }
}

/// The folder total for a provider that can only list and stat: it descends through `list` and
/// sums the sizes it reports. Links are skipped, and where the provider names a volume
/// (`volume_id`) another one is not entered. `allocated_bytes` is `None`: such a provider does not
/// know it.
pub(crate) fn listed_folder_size<P: Provider + ?Sized>(
    provider: &P,
    path: &VfsPath,
    cancel: &CancelToken,
    report: &mut dyn FnMut(&FolderSizeTotals),
) -> Result<FolderSizeRun, VfsError> {
    let report_every = REPORT_EVERY;
    let root = provider.stat(path)?;
    let is_folder = root.kind == EntryKind::Directory
        || (root.kind == EntryKind::Symlink && root.link_target == Some(EntryKind::Directory));
    if !is_folder {
        return Err(VfsError::NotADirectory {
            location: path.to_location(),
        });
    }
    let root_volume = provider.volume_id(path);
    let rule = provider.capabilities().case_rule;
    let mut counter = Counter {
        totals: FolderSizeTotals::default(),
        cancel,
        report,
        report_every,
        last_report: Instant::now(),
        since_check: 0,
    };
    let mut pending = vec![path.clone()];
    let mut first = true;
    while let Some(folder) = pending.pop() {
        let listed = provider.list(&folder, cancel, 0, &mut |_| {});
        let entries = match listed {
            Ok(entries) => entries,
            Err(VfsError::Cancelled) => return Ok(counter.run(true)),
            Err(error) if first => return Err(error),
            Err(_) => {
                counter.totals.unreadable += 1;
                continue;
            }
        };
        first = false;
        for entry in entries {
            if counter.tick() {
                return Ok(counter.run(true));
            }
            match entry.kind {
                EntryKind::Symlink => counter.totals.symlinks_skipped += 1,
                EntryKind::Directory => {
                    let Ok(child) = crate::names::child_path(&folder, &entry.name, rule) else {
                        counter.totals.unreadable += 1;
                        continue;
                    };
                    if root_volume.is_some() && provider.volume_id(&child) != root_volume {
                        counter.totals.mounts_skipped += 1;
                        continue;
                    }
                    counter.totals.folders += 1;
                    pending.push(child);
                }
                _ => {
                    counter.totals.files += 1;
                    counter.totals.bytes += entry.size.unwrap_or(0);
                }
            }
        }
    }
    Ok(counter.run(false))
}

#[cfg(all(test, unix))]
mod tests {
    use std::cell::Cell;

    use waypoint_path::FilePath;

    use super::*;

    #[test]
    fn a_cancel_stops_the_walk_within_one_check_interval() {
        let dir = tempfile::tempdir().unwrap();
        for f in 0..2000 {
            fs::write(dir.path().join(format!("f{f}")), b"x").unwrap();
        }
        let path = VfsPath::File(FilePath::from_path(dir.path()).unwrap());
        let cancel = CancelToken::new();
        let at_cancel = Cell::new(0u64);
        let run = walk_local(
            &path,
            &cancel,
            &mut |totals| {
                // Reports come every `CHECK_EVERY` entries with a zero interval.
                if at_cancel.get() == 0 {
                    at_cancel.set(totals.files);
                    cancel.cancel();
                }
            },
            Duration::ZERO,
        )
        .unwrap();
        assert!(run.cancelled);
        assert!(at_cancel.get() > 0);
        // The entry after the cancel sees the flag, so nothing more than that one is counted.
        assert!(run.totals.files <= at_cancel.get() + 1, "{:?}", run.totals);
        assert!(run.totals.files < 2000);
    }
}
