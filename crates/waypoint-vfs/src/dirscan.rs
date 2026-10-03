// The directory-size scan: how much each top-level folder of a root takes, one folder at a time
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The engine behind Overview's "Biggest folders in Home". It walks a root's top-level folders one
// after another with the same primitives as the folder size (`size::LocalWalker`: no symlink is
// followed, no mount point is crossed, a hard link counts once, a cloud placeholder counts as zero
// and is never entered), and after each folder sends a partial result: every row so far with its
// share of what has been scanned. Loose files and hidden items at the top level are one remainder
// row, so the rows always add up to the total.
//
// The scan is a plain function over a cancel token. Running it on a thread of its own at low
// priority (A70: never on the operations pool) is the caller's job, as it is for the folder size.

use std::cell::{Cell, RefCell};
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use waypoint_path::{FilePath, VfsPath};
use waypoint_protocol::{Location, VfsError};

use crate::error::from_io;
use crate::local::{file_path, is_hidden};
use crate::size::{open_root, LocalWalker, Visit, REPORT_EVERY};
use crate::{CancelToken, FolderSizeTotals};

/// The remainder row's name: what is left at the top level once the visible folders are counted.
pub const REMAINDER_NAME: &str = "Other files and folders, including hidden";

/// How a scan measures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct DirScanOptions {
    /// Count what files take on disk (where the platform reports it cheaply) instead of their
    /// `lstat` sizes.
    pub allocated: bool,
}

/// What a row stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum DirSizeRowKind {
    /// A visible top-level folder.
    Folder,
    /// Loose files and hidden items at the top level, together.
    Other,
}

/// One row of the result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct DirSizeRow {
    /// The folder's name, or `REMAINDER_NAME` for the remainder row.
    pub name: String,
    pub kind: DirSizeRowKind,
    /// Where the folder is, to open it; `None` for the remainder.
    pub location: Option<Location>,
    /// The row's size in the scan's measure.
    #[ts(type = "number")]
    pub bytes: u64,
    /// Files counted in the row, cloud placeholders included.
    #[ts(type = "number")]
    pub files: u64,
    /// `bytes` as a fraction (0 to 1) of the rows scanned so far.
    pub share: f64,
}

/// The rows so far, or all of them, with the totals.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct DirScanResult {
    pub root: Location,
    /// Folders largest first (ties by name), then the remainder row last.
    pub rows: Vec<DirSizeRow>,
    /// The sum of the rows.
    #[ts(type = "number")]
    pub total_bytes: u64,
    #[ts(type = "number")]
    pub total_files: u64,
    /// Whether `bytes` is what files take on disk rather than their sizes.
    pub allocated: bool,
    /// Top-level folders measured so far, and how many there are (visible and hidden).
    #[ts(type = "number")]
    pub folders_scanned: u64,
    #[ts(type = "number")]
    pub folders_total: u64,
    /// Symlinks, other-volume folders, cloud placeholders and unreadable entries passed over.
    #[ts(type = "number")]
    pub symlinks_skipped: u64,
    #[ts(type = "number")]
    pub mounts_skipped: u64,
    #[ts(type = "number")]
    pub placeholders: u64,
    #[ts(type = "number")]
    pub unreadable: u64,
    /// When this result was produced, in milliseconds since the Unix epoch.
    #[ts(type = "number")]
    pub measured_at_ms: u64,
}

/// What a scan sends. Exactly one of `done`, `cancelled` and `failed` ends the stream.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(
    export,
    export_to = "../../../packages/protocol/src/generated/",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum DirScanEvent {
    /// About every 100 ms inside a folder: which one, and the bytes measured so far overall.
    Progress {
        current: String,
        #[ts(type = "number")]
        scanned_bytes: u64,
    },
    /// A top-level folder finished (or the remainder grew): the result so far.
    Partial { result: DirScanResult },
    /// Every folder was measured.
    Done { result: DirScanResult },
    /// Cancelled; the result covers the folders that finished before it.
    Cancelled { result: DirScanResult },
    /// The root itself could not be read.
    Failed { error: VfsError },
}

#[derive(Default, Clone, Copy)]
struct Amount {
    bytes: u64,
    files: u64,
}

fn measure(totals: &FolderSizeTotals, allocated: bool) -> Amount {
    Amount {
        bytes: if allocated {
            totals.allocated_bytes.unwrap_or(totals.bytes)
        } else {
            totals.bytes
        },
        files: totals.files,
    }
}

fn since(now: Amount, before: Amount) -> Amount {
    Amount {
        bytes: now.bytes - before.bytes,
        files: now.files - before.files,
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// Scans the top-level folders of the local folder at `root`, sending events to `emit` as it goes.
///
/// A cancel is looked at for every entry, so it stops within one entry's work. The root itself
/// being unreadable sends `failed`; anything below it that cannot be read is counted and passed.
pub fn scan_dir_sizes(
    root: &Location,
    options: DirScanOptions,
    cancel: &CancelToken,
    emit: &mut dyn FnMut(DirScanEvent),
) {
    scan(root, options, cancel, emit, REPORT_EVERY);
}

fn scan(
    root: &Location,
    options: DirScanOptions,
    cancel: &CancelToken,
    emit: &mut dyn FnMut(DirScanEvent),
    report_every: Duration,
) {
    let emit = RefCell::new(emit);
    if let Err(error) = scan_inner(root, options, cancel, &emit, report_every) {
        (emit.borrow_mut())(DirScanEvent::Failed { error });
    }
}

struct Folder {
    name: String,
    path: PathBuf,
}

fn scan_inner(
    root: &Location,
    options: DirScanOptions,
    cancel: &CancelToken,
    emit: &RefCell<&mut dyn FnMut(DirScanEvent)>,
    report_every: Duration,
) -> Result<(), VfsError> {
    let vfs_path = VfsPath::from_location(root).map_err(|_| VfsError::InvalidLocation {
        input: root.uri.clone(),
    })?;
    let root_path = file_path(&vfs_path)?.as_path().to_path_buf();
    let root_meta = open_root(&root_path, root)?;
    let entries = fs::read_dir(&root_path).map_err(|e| from_io(&e, root))?;

    let current = RefCell::new(String::new());
    let scanned = Cell::new(0u64);
    let mut report = |totals: &FolderSizeTotals| {
        let so_far = scanned.get() + measure(totals, options.allocated).bytes;
        (emit.borrow_mut())(DirScanEvent::Progress {
            current: current.borrow().clone(),
            scanned_bytes: so_far,
        });
    };
    let mut walker = LocalWalker::new(&root_meta, cancel, &mut report, report_every);
    let allocated = options.allocated && walker.totals().allocated_bytes.is_some();
    let measured = |walker: &LocalWalker| measure(walker.totals(), allocated);

    // The top level: loose files are counted now, folders are queued.
    let mut visible: Vec<Folder> = Vec::new();
    let mut hidden: Vec<Folder> = Vec::new();
    let mut other = Amount::default();
    for entry in entries {
        if walker.tick() {
            break;
        }
        let Ok(entry) = entry else {
            walker.unreadable();
            continue;
        };
        let name = entry.file_name();
        let is_hidden = is_hidden(&name, entry.metadata().ok().as_ref());
        let before = measured(&walker);
        match walker.visit(&entry) {
            Visit::Done => {
                let delta = since(measured(&walker), before);
                other.bytes += delta.bytes;
                other.files += delta.files;
            }
            Visit::Enter(path) => {
                let folder = Folder {
                    name: name.to_string_lossy().into_owned(),
                    path,
                };
                if is_hidden {
                    hidden.push(folder);
                } else {
                    visible.push(folder);
                }
            }
        }
    }
    visible.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.name.cmp(&b.name))
    });

    let folders_total = (visible.len() + hidden.len()) as u64;
    let mut rows: Vec<DirSizeRow> = Vec::new();
    let mut folders_scanned = 0u64;
    let mut cancelled = cancel.is_cancelled();

    let snapshot = |rows: &[DirSizeRow],
                    other: Amount,
                    folders_scanned: u64,
                    walker: &LocalWalker|
     -> DirScanResult {
        let totals = walker.totals();
        build_result(
            root,
            rows,
            other,
            allocated,
            (folders_scanned, folders_total),
            totals,
        )
    };

    // Visible folders one by one, then the hidden ones into the remainder.
    let queue = visible
        .into_iter()
        .map(|f| (f, false))
        .chain(hidden.into_iter().map(|f| (f, true)));
    for (folder, into_other) in queue {
        if cancelled {
            break;
        }
        *current.borrow_mut() = folder.name.clone();
        let before = measured(&walker);
        match walker.walk(folder.path.clone()) {
            Ok(true) => {
                cancelled = true;
                break;
            }
            Ok(false) => {}
            Err(_) => walker.unreadable(),
        }
        let delta = since(measured(&walker), before);
        folders_scanned += 1;
        if into_other {
            other.bytes += delta.bytes;
            other.files += delta.files;
        } else {
            rows.push(DirSizeRow {
                name: folder.name,
                kind: DirSizeRowKind::Folder,
                location: FilePath::from_path(&folder.path)
                    .ok()
                    .map(|p| p.to_location()),
                bytes: delta.bytes,
                files: delta.files,
                share: 0.0,
            });
        }
        scanned.set(rows.iter().map(|r| r.bytes).sum::<u64>() + other.bytes);
        let result = snapshot(&rows, other, folders_scanned, &walker);
        (emit.borrow_mut())(DirScanEvent::Partial { result });
    }

    let result = snapshot(&rows, other, folders_scanned, &walker);
    (emit.borrow_mut())(if cancelled {
        DirScanEvent::Cancelled { result }
    } else {
        DirScanEvent::Done { result }
    });
    Ok(())
}

fn build_result(
    root: &Location,
    rows: &[DirSizeRow],
    other: Amount,
    allocated: bool,
    (folders_scanned, folders_total): (u64, u64),
    totals: &FolderSizeTotals,
) -> DirScanResult {
    let mut rows: Vec<DirSizeRow> = rows.to_vec();
    rows.sort_by(|a, b| b.bytes.cmp(&a.bytes).then_with(|| a.name.cmp(&b.name)));
    if other.bytes > 0 || other.files > 0 {
        rows.push(DirSizeRow {
            name: REMAINDER_NAME.to_owned(),
            kind: DirSizeRowKind::Other,
            location: None,
            bytes: other.bytes,
            files: other.files,
            share: 0.0,
        });
    }
    let total_bytes: u64 = rows.iter().map(|r| r.bytes).sum();
    let total_files: u64 = rows.iter().map(|r| r.files).sum();
    for row in &mut rows {
        row.share = if total_bytes == 0 {
            0.0
        } else {
            row.bytes as f64 / total_bytes as f64
        };
    }
    DirScanResult {
        root: root.clone(),
        rows,
        total_bytes,
        total_files,
        allocated,
        folders_scanned,
        folders_total,
        symlinks_skipped: totals.symlinks_skipped,
        mounts_skipped: totals.mounts_skipped,
        placeholders: totals.placeholders,
        unreadable: totals.unreadable,
        measured_at_ms: now_ms(),
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::os::unix::fs::symlink;
    use std::time::Instant;

    use super::*;

    fn location(path: &std::path::Path) -> Location {
        FilePath::from_path(path).unwrap().to_location()
    }

    fn file(path: std::path::PathBuf, bytes: usize) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, vec![b'x'; bytes]).unwrap();
    }

    fn run(root: &std::path::Path, options: DirScanOptions) -> Vec<DirScanEvent> {
        let mut events = Vec::new();
        scan_dir_sizes(&location(root), options, &CancelToken::new(), &mut |e| {
            events.push(e)
        });
        events
    }

    fn partials(events: &[DirScanEvent]) -> Vec<&DirScanResult> {
        events
            .iter()
            .filter_map(|e| match e {
                DirScanEvent::Partial { result } => Some(result),
                _ => None,
            })
            .collect()
    }

    fn done(events: &[DirScanEvent]) -> &DirScanResult {
        match events.last() {
            Some(DirScanEvent::Done { result }) => result,
            other => panic!("expected done, got {other:?}"),
        }
    }

    fn sample() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path();
        file(p.join("Documents/a.txt"), 100);
        file(p.join("Documents/deep/b.txt"), 200);
        file(p.join("Pictures/c.jpg"), 5000);
        file(p.join("Music/d.mp3"), 50);
        file(p.join(".cache/e.bin"), 1000);
        file(p.join("loose.txt"), 7);
        dir
    }

    #[test]
    fn events_are_tagged_and_camel_cased_on_the_wire() {
        let event = DirScanEvent::Progress {
            current: "Music".to_owned(),
            scanned_bytes: 5,
        };
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            r#"{"kind":"progress","current":"Music","scannedBytes":5}"#
        );
    }

    #[test]
    fn folders_are_scanned_in_name_order_with_a_partial_after_each() {
        let dir = sample();
        let events = run(dir.path(), DirScanOptions::default());
        let partials = partials(&events);
        // Three visible folders, then the one hidden folder.
        assert_eq!(partials.len(), 4);
        assert_eq!(
            partials
                .iter()
                .map(|p| p.folders_scanned)
                .collect::<Vec<_>>(),
            [1, 2, 3, 4]
        );
        assert!(partials.iter().all(|p| p.folders_total == 4));
        // The first partial holds only Documents (name order), the second adds Music.
        assert_eq!(partials[0].rows[0].name, "Documents");
        assert_eq!(partials[0].rows[0].bytes, 300);
        let second: Vec<_> = partials[1].rows.iter().map(|r| r.name.as_str()).collect();
        assert!(second.contains(&"Documents") && second.contains(&"Music"));
        // The folder rows come largest first.
        let last = done(&events);
        let names: Vec<_> = last.rows.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["Pictures", "Documents", "Music", REMAINDER_NAME]);
    }

    #[test]
    fn shares_are_of_the_scanned_total_so_far_and_end_adding_to_one() {
        let dir = sample();
        let events = run(dir.path(), DirScanOptions::default());
        let first = partials(&events)[0];
        // Documents (300) and the loose file (7) are all that has been scanned.
        assert!((first.rows[0].share - 300.0 / 307.0).abs() < 1e-9);
        let last = done(&events);
        let sum: f64 = last.rows.iter().map(|r| r.share).sum();
        assert!((sum - 1.0).abs() < 1e-9);
        assert_eq!(
            last.total_bytes,
            last.rows.iter().map(|r| r.bytes).sum::<u64>()
        );
    }

    #[test]
    fn the_remainder_row_holds_loose_files_and_hidden_items() {
        let dir = sample();
        let events = run(dir.path(), DirScanOptions::default());
        let last = done(&events);
        let rest = last.rows.last().unwrap();
        assert_eq!(rest.kind, DirSizeRowKind::Other);
        assert_eq!(rest.name, "Other files and folders, including hidden");
        assert_eq!((rest.bytes, rest.files), (1007, 2));
        assert!(rest.location.is_none());
        assert_eq!(last.total_bytes, 300 + 5000 + 50 + 1007);
        assert_eq!(last.total_files, 6);
        // Folder rows say where they are.
        assert!(last.rows[0].location.is_some());
    }

    #[test]
    fn an_empty_remainder_is_left_out() {
        let dir = tempfile::tempdir().unwrap();
        file(dir.path().join("A/x"), 10);
        let events = run(dir.path(), DirScanOptions::default());
        assert_eq!(done(&events).rows.len(), 1);
    }

    #[test]
    fn symlinks_are_skipped_and_hard_links_count_once() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path();
        file(p.join("A/real"), 1000);
        symlink(p.join("A"), p.join("A/loop")).unwrap();
        symlink(p.join("A/real"), p.join("link-to-file")).unwrap();
        symlink(p.join("A"), p.join("link-to-folder")).unwrap();
        fs::hard_link(p.join("A/real"), p.join("A/again")).unwrap();
        // The second link is in another top-level folder: still one count in all.
        fs::create_dir(p.join("B")).unwrap();
        fs::hard_link(p.join("A/real"), p.join("B/third")).unwrap();
        let events = run(p, DirScanOptions::default());
        let last = done(&events);
        assert_eq!(last.total_bytes, 1000);
        assert_eq!(last.symlinks_skipped, 3);
        assert_eq!(last.mounts_skipped, 0);
    }

    #[test]
    fn a_folder_on_another_volume_is_not_entered() {
        // `/dev/shm` is usually its own tmpfs; where it is not there is nothing to test.
        let shm = std::path::Path::new("/dev/shm");
        let dir = tempfile::tempdir().unwrap();
        let mounted = |a: &std::path::Path, b: &std::path::Path| {
            use std::os::unix::fs::MetadataExt;
            fs::metadata(a).unwrap().dev() != fs::metadata(b).unwrap().dev()
        };
        if !shm.exists() || !mounted(shm, dir.path()) {
            return;
        }
        // `/dev` holds the mount point `/dev/shm`.
        let mut events = Vec::new();
        let cancel = CancelToken::new();
        scan_dir_sizes(
            &location(std::path::Path::new("/dev")),
            DirScanOptions::default(),
            &cancel,
            &mut |e| events.push(e),
        );
        let last = done(&events);
        assert!(last.mounts_skipped >= 1, "{last:?}");
        assert!(last.rows.iter().all(|r| r.name != "shm"));
    }

    #[test]
    fn the_allocated_option_counts_blocks() {
        let dir = tempfile::tempdir().unwrap();
        file(dir.path().join("A/tiny"), 1);
        let plain = run(dir.path(), DirScanOptions::default());
        assert_eq!(done(&plain).total_bytes, 1);
        assert!(!done(&plain).allocated);
        let blocks = run(dir.path(), DirScanOptions { allocated: true });
        assert!(done(&blocks).allocated);
        assert!(done(&blocks).total_bytes >= 512);
    }

    #[test]
    fn an_unreadable_root_fails_and_a_file_is_not_a_folder() {
        let dir = tempfile::tempdir().unwrap();
        let events = run(&dir.path().join("missing"), DirScanOptions::default());
        assert!(matches!(
            events.as_slice(),
            [DirScanEvent::Failed {
                error: VfsError::NotFound { .. }
            }]
        ));
        file(dir.path().join("f"), 1);
        let events = run(&dir.path().join("f"), DirScanOptions::default());
        assert!(matches!(
            events.as_slice(),
            [DirScanEvent::Failed {
                error: VfsError::NotADirectory { .. }
            }]
        ));
    }

    #[test]
    fn a_cancel_before_the_start_ends_cancelled_with_no_rows() {
        let dir = sample();
        let cancel = CancelToken::new();
        cancel.cancel();
        let mut events = Vec::new();
        scan_dir_sizes(
            &location(dir.path()),
            DirScanOptions::default(),
            &cancel,
            &mut |e| events.push(e),
        );
        match events.as_slice() {
            [DirScanEvent::Cancelled { result }] => assert!(result.rows.is_empty()),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_cancel_after_a_folder_keeps_the_finished_rows_and_stops_within_100_ms() {
        let dir = tempfile::tempdir().unwrap();
        file(dir.path().join("A/small"), 10);
        for f in 0..30_000 {
            file(dir.path().join(format!("B/{}/f{f}", f % 50)), 1);
        }
        let cancel = CancelToken::new();
        let cancelled_at: Cell<Option<Instant>> = Cell::new(None);
        let mut events: Vec<DirScanEvent> = Vec::new();
        let mut ended_at = None;
        scan(
            &location(dir.path()),
            DirScanOptions::default(),
            &cancel,
            &mut |e| {
                // Cancel once the first folder is done, while B is being walked.
                if matches!(e, DirScanEvent::Partial { .. }) && cancelled_at.get().is_none() {
                    cancelled_at.set(Some(Instant::now()));
                    cancel.cancel();
                }
                if matches!(e, DirScanEvent::Cancelled { .. }) {
                    ended_at = Some(Instant::now());
                }
                events.push(e);
            },
            Duration::ZERO,
        );
        match events.last() {
            Some(DirScanEvent::Cancelled { result }) => {
                assert_eq!(result.rows.len(), 1);
                assert_eq!(result.rows[0].name, "A");
            }
            other => panic!("{other:?}"),
        }
        let latency = ended_at.unwrap() - cancelled_at.get().unwrap();
        assert!(latency < Duration::from_millis(100), "{latency:?}");
    }

    #[test]
    fn a_cancel_inside_a_big_folder_returns_within_100_ms() {
        let dir = tempfile::tempdir().unwrap();
        for f in 0..30_000 {
            file(dir.path().join(format!("B/{}/f{f}", f % 50)), 1);
        }
        let cancel = CancelToken::new();
        let at: Cell<Option<Instant>> = Cell::new(None);
        let mut ended = None;
        scan(
            &location(dir.path()),
            DirScanOptions::default(),
            &cancel,
            &mut |e| match e {
                DirScanEvent::Progress { .. } if at.get().is_none() => {
                    at.set(Some(Instant::now()));
                    cancel.cancel();
                }
                DirScanEvent::Cancelled { result } => {
                    assert!(result.rows.is_empty());
                    ended = Some(Instant::now());
                }
                _ => {}
            },
            Duration::ZERO,
        );
        let latency = ended.expect("cancelled") - at.get().expect("progress came");
        assert!(latency < Duration::from_millis(100), "{latency:?}");
    }

    /// Records the time for 100,000 files (run with `--release -- --ignored --nocapture`).
    #[test]
    #[ignore = "timing run on a synthetic tree"]
    fn timing_for_a_hundred_thousand_files() {
        let dir = tempfile::tempdir().unwrap();
        for folder in 0..100 {
            for sub in 0..10 {
                for f in 0..100 {
                    file(dir.path().join(format!("d{folder}/s{sub}/f{f}")), 16);
                }
            }
        }
        let started = Instant::now();
        let events = run(dir.path(), DirScanOptions::default());
        let elapsed = started.elapsed();
        let last = done(&events);
        assert_eq!(last.total_files, 100_000);
        eprintln!("scanned 100000 files in {elapsed:?}");
    }
}
