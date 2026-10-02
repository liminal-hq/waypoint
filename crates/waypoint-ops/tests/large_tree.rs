// The engine at scale (A48 to A52): a 100,000-file tree copied, moved, deleted, undone and cancelled.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

// A disposable tree of 100,000 small files over about 1,000 folders and a few 64 MiB files goes
// through the real engine on the local provider. The test prints what each job took and checks
// nothing about speed, so it is `#[ignore]`d and run by hand:
//
//     cargo test --release -p waypoint-ops --test large_tree -- --ignored --nocapture
//
// The tree lives in `tempfile`'s directory, so `TMPDIR` chooses the volume it is measured on (on
// many Linux systems `/tmp` is a memory file system, which is not the disk).

mod common;
mod journal_support;

use std::io::Write;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use common::*;
use journal_support::*;

const FOLDERS: usize = 1_000;
const FILES_PER_FOLDER: usize = 100;
/// Each folder holds files of one of these sizes, so the tree has a few per-folder sizes.
const SIZES: [usize; 4] = [256, 1024, 4 * 1024, 16 * 1024];
const BIG_FILES: usize = 3;
const BIG_BYTES: usize = 64 * 1024 * 1024;

/// What a tree holds, for rates.
#[derive(Clone, Copy, Default)]
struct Shape {
    files: u64,
    bytes: u64,
}

fn content(len: usize, seed: u8) -> Vec<u8> {
    (0..len)
        .map(|i| (i as u8).wrapping_mul(31).wrapping_add(seed))
        .collect()
}

/// Writes the source tree with the standard library, under `root/src`, and says what it made.
fn make_tree(root: &Path) -> Shape {
    let mut shape = Shape::default();
    let pool: Vec<Vec<u8>> = SIZES.iter().map(|&n| content(n, n as u8)).collect();
    for d in 0..FOLDERS {
        let dir = root.join("src").join(format!("dir{d:04}"));
        std::fs::create_dir_all(&dir).unwrap();
        let data = &pool[d % SIZES.len()];
        for f in 0..FILES_PER_FOLDER {
            let mut file = std::fs::File::create(dir.join(format!("file{f:03}.dat"))).unwrap();
            file.write_all(data).unwrap();
            shape.files += 1;
            shape.bytes += data.len() as u64;
        }
    }
    let big_dir = root.join("src").join("big");
    std::fs::create_dir_all(&big_dir).unwrap();
    let block = content(BIG_BYTES, 7);
    for b in 0..BIG_FILES {
        std::fs::write(big_dir.join(format!("big{b}.bin")), &block).unwrap();
        shape.files += 1;
        shape.bytes += BIG_BYTES as u64;
    }
    shape
}

/// The peak and the current resident memory, in MiB, from `/proc/self/status`.
fn memory() -> String {
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let field = |name: &str| {
        status
            .lines()
            .find_map(|l| l.strip_prefix(name))
            .and_then(|v| v.trim().trim_end_matches("kB").trim().parse::<f64>().ok())
            .map(|kb| format!("{:.0} MiB", kb / 1024.0))
            .unwrap_or_else(|| "n/a".to_owned())
    };
    format!("peak {}, now {}", field("VmHWM:"), field("VmRSS:"))
}

fn rate(label: &str, took: Duration, shape: Shape) {
    let secs = took.as_secs_f64().max(1e-9);
    eprintln!(
        "{label:<34} {secs:>8.3} s {:>10.0} files/s {:>9.1} MB/s",
        shape.files as f64 / secs,
        shape.bytes as f64 / 1e6 / secs
    );
}

fn progress_events<P: Provider + 'static>(h: &JournalHarness<P>) -> usize {
    h.events
        .iter()
        .filter(|e| matches!(e, OpsEvent::JobChanged { .. }))
        .count()
}

/// Every file and folder below `dir`, and the partial files among them.
fn walk(dir: &Path, files: &mut u64, partial: &mut Vec<String>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with(".waypoint-partial-") {
            partial.push(entry.path().display().to_string());
        }
        if entry.file_type().unwrap().is_dir() {
            walk(&entry.path(), files, partial);
        } else {
            *files += 1;
        }
    }
}

fn transfer<P: Provider + 'static>(
    h: &JournalHarness<P>,
    kind: JobKind,
    sources: &[&str],
    dest: &str,
    verify: bool,
) -> JobRequest {
    let mut request = h.request(kind, sources, Some(dest), None);
    request.options.verify = Some(verify);
    request
}

/// Plans the request on its own, then runs it whole (which plans again), and prints both.
fn timed<P: Provider + 'static>(
    label: &str,
    h: &mut JournalHarness<P>,
    request: JobRequest,
    shape: Shape,
) -> JournalRun {
    let started = Instant::now();
    let planned = h.plan(&request).expect("the request plans");
    let planning = started.elapsed();
    drop(planned);
    let events_before = progress_events(h);
    let started = Instant::now();
    let run = h.run_journalled(request);
    let whole = started.elapsed();
    assert_eq!(
        run.state,
        JobState::Done,
        "{label}: {:?}",
        run.failure.as_ref().map(|f| (&f.error, &f.item))
    );
    rate(&format!("{label} (planning)"), planning, shape);
    rate(&format!("{label} (whole job)"), whole, shape);
    eprintln!(
        "{label:<34} {} progress events; memory {}",
        progress_events(h) - events_before,
        memory()
    );
    run
}

#[test]
#[ignore = "builds a 100,000-file tree and writes about 1.5 GB; run by hand"]
fn the_engine_on_a_100_000_file_tree() {
    let dir = tempfile::tempdir().unwrap();
    let root = FilePath::from_path(dir.path()).unwrap();
    eprintln!("tree under {}", dir.path().display());

    let h = &mut JournalHarness::new(LocalProvider::new(), VfsPath::File(root));
    let work = h.work.clone();
    let work_dir = match &work {
        VfsPath::File(p) => p.as_path().to_path_buf(),
        other => panic!("not a file path: {other:?}"),
    };
    for name in ["dst1", "dst2", "dst3", "dst4", "mv"] {
        std::fs::create_dir(work_dir.join(name)).unwrap();
    }

    let started = Instant::now();
    let shape = make_tree(&work_dir);
    eprintln!(
        "built {} files, {:.0} MB in {:.1} s ({} folders besides)",
        shape.files,
        shape.bytes as f64 / 1e6,
        started.elapsed().as_secs_f64(),
        FOLDERS + 1
    );
    h.provider.reset();

    // Copy, then Undo of the copy.
    let copy = timed(
        "copy",
        h,
        transfer(h, JobKind::Copy, &["src"], "dst1", false),
        shape,
    );
    let entry = copy.entry.expect("the copy is in the journal");
    let copy_whole = {
        // The cancel run below is timed against a copy of this length.
        let started = Instant::now();
        let request = transfer(h, JobKind::Copy, &["src"], "dst4", false);
        let run = h.run_journalled(request);
        assert_eq!(run.state, JobState::Done);
        let took = started.elapsed();
        let delete = h.request(JobKind::Delete, &["dst4"], None, None);
        let cleaned = h.run_journalled(delete);
        assert_eq!(cleaned.state, JobState::Done);
        let _ = std::fs::create_dir(work_dir.join("dst4"));
        took
    };
    let started = Instant::now();
    let undo = h.undo(entry);
    assert_eq!(undo.state, JobState::Done, "{:?}", undo.undo);
    rate("undo of the copy (whole job)", started.elapsed(), shape);
    eprintln!("memory {}", memory());

    // Verified copy, then a permanent delete of what it made.
    timed(
        "copy, verify on (BLAKE3)",
        h,
        transfer(h, JobKind::Copy, &["src"], "dst2", true),
        shape,
    );
    timed(
        "delete (permanent)",
        h,
        h.request(JobKind::Delete, &["dst2/src"], None, None),
        shape,
    );

    // Move on one volume.
    timed(
        "move (same volume)",
        h,
        transfer(h, JobKind::Move, &["src"], "mv", false),
        shape,
    );

    // Cancel a copy part of the way through, from another thread.
    let delay = copy_whole.mul_f64(0.4);
    let cancelled_at: Arc<Mutex<Option<Instant>>> = Arc::new(Mutex::new(None));
    let request = transfer(h, JobKind::Copy, &["mv/src"], "dst3", false);
    let slot = cancelled_at.clone();
    let events_before = progress_events(h);
    let run = h.run_journalled_hooked(request, &mut |_, token| {
        let token = token.clone();
        let slot = slot.clone();
        std::thread::spawn(move || {
            std::thread::sleep(delay);
            *slot.lock().unwrap() = Some(Instant::now());
            token.cancel();
        });
    });
    let ended = Instant::now();
    let cancelled = cancelled_at.lock().unwrap().expect("the cancel fired");
    let mut files = 0;
    let mut partial = Vec::new();
    walk(&work_dir.join("dst3"), &mut files, &mut partial);
    eprintln!(
        "cancel after {:.3} s of a {:.3} s copy: job ended {:.1} ms later in state {:?}; {} files had landed; {} partial files left; {} progress events",
        delay.as_secs_f64(),
        copy_whole.as_secs_f64(),
        ended.saturating_duration_since(cancelled).as_secs_f64() * 1e3,
        run.state,
        files,
        partial.len(),
        progress_events(h) - events_before
    );
    eprintln!("memory {}", memory());
    assert!(partial.is_empty(), "partial files left: {partial:?}");
    assert_eq!(run.state, JobState::Cancelled, "the cancel took effect");
    // The disposable tree goes with `dir`.
}
