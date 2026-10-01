// The milestone 2 performance budgets over a 500 000-entry folder
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

// Ignored by default: it takes a while and timings need a quiet machine:
//
//     cargo test -p waypoint-vfs --release --test budgets -- --ignored --nocapture
//
// The budgets come from A18: the milestone 0 spike sorted 500 000 names in 169 to 187 ms and read
// a 256-row page in a few milliseconds, and the plan sets sort by name within 200 ms in Rust.

use std::ffi::OsString;
use std::fs::File;
use std::sync::Arc;
use std::time::{Duration, Instant};

use waypoint_path::{FilePath, VfsPath};
use waypoint_protocol::VfsError;
use waypoint_vfs::{
    CancelToken, Capabilities, Change, EntryKind, Filter, IconGroup, Listing, ListingHandle,
    ListingOptions, LocalProvider, Provider, ScannedEntry, SortKey, SortSpec,
};

const COUNT: usize = 500_000;

/// Timings from an unoptimised build mean nothing, so the budgets are only asserted in release.
const ENFORCE: bool = !cfg!(debug_assertions);

/// The budget for sorting the whole listing by name (A18, plan "Done means").
const SORT_BY_NAME: Duration = Duration::from_millis(200);
/// Other columns sort faster in the spike (20 to 26 ms); the same ceiling applies to them.
const SORT_OTHER: Duration = Duration::from_millis(200);
/// One page of 256 rows, which the spike measured in a few milliseconds end to end.
const PAGE: Duration = Duration::from_millis(5);
/// Scanning 500 000 entries took about 0.6 s in the spike; allow headroom for the index build.
const SCAN: Duration = Duration::from_millis(2000);

struct Xorshift(u64);

impl Xorshift {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
}

fn synthetic(count: usize) -> Vec<ScannedEntry> {
    const WORDS: [&str; 12] = [
        "report",
        "photo",
        "invoice",
        "draft",
        "backup",
        "notes",
        "archive",
        "screenshot",
        "data",
        "build",
        "log",
        "track",
    ];
    const EXTS: [&str; 8] = ["txt", "jpg", "pdf", "rs", "ts", "mp3", "zip", "md"];
    let mut rng = Xorshift(0x9E37_79B9_7F4A_7C15);
    (0..count)
        .map(|i| {
            let r = rng.next();
            let word = WORDS[(r % WORDS.len() as u64) as usize];
            let ext = EXTS[((r >> 8) % EXTS.len() as u64) as usize];
            let number = (r >> 16) % 1_000_000;
            let folder = r.is_multiple_of(17);
            ScannedEntry {
                name: OsString::from(format!("{word}-{number:06}-{i}.{ext}")),
                kind: if folder {
                    EntryKind::Directory
                } else {
                    EntryKind::File
                },
                link_target: None,
                link_pending: false,
                group: if folder {
                    IconGroup::Folder
                } else {
                    IconGroup::Other
                },
                size: (!folder).then_some((r >> 24) % 50_000_000),
                modified_ms: Some(1_600_000_000_000 + ((r >> 40) % 100_000_000) as i64 * 1000),
                hidden: r.is_multiple_of(29),
                trashed: None,
            }
        })
        .collect()
}

struct Synthetic(Vec<ScannedEntry>);

impl Provider for Synthetic {
    fn scheme(&self) -> &'static str {
        "file"
    }
    fn capabilities(&self) -> Capabilities {
        LocalProvider::new().capabilities()
    }
    fn stat(&self, _: &VfsPath) -> Result<ScannedEntry, VfsError> {
        unimplemented!()
    }
    fn list(
        &self,
        _: &VfsPath,
        _: &CancelToken,
        _: usize,
        progress: &mut dyn FnMut(u32),
    ) -> Result<Vec<ScannedEntry>, VfsError> {
        progress(self.0.len() as u32);
        Ok(self.0.clone())
    }
    fn resolve_link(&self, _: &VfsPath, e: &ScannedEntry) -> Result<ScannedEntry, VfsError> {
        Ok(e.clone())
    }
}

fn millis(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

fn resident_mb() -> f64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("VmRSS:"))
                .and_then(|l| l.split_whitespace().nth(1)?.parse::<f64>().ok())
        })
        .map_or(f64::NAN, |kb| kb / 1024.0)
}

fn sort(key: SortKey, descending: bool) -> SortSpec {
    SortSpec {
        key,
        descending,
        directories_first: true,
    }
}

#[test]
#[ignore = "a 500 000-entry benchmark; run with --release and --ignored"]
fn sorting_paging_and_patching_a_500_000_entry_listing_stay_inside_the_budgets() {
    let entries = synthetic(COUNT);
    let rss_before = resident_mb();
    let clone_started = Instant::now();
    drop(entries.clone());
    let clone_time = clone_started.elapsed();

    let started = Instant::now();
    let listing = Listing::open(
        ListingHandle(1),
        VfsPath::File(FilePath::parse("/synthetic").unwrap()),
        Arc::new(Synthetic(entries)),
        SortSpec::default(),
        Filter::default(),
        ListingOptions::default(),
        Arc::new(|_| {}),
    )
    .unwrap();
    let opened = started.elapsed();
    let rss_after = resident_mb();
    println!(
        "open (cloning entries {:.0} ms of it): {:.0} ms; {} visible of {COUNT}; about {:.0} MB held per listing",
        millis(clone_time),
        millis(opened),
        listing.snapshot().count,
        rss_after - rss_before
    );

    let mut worst_name = Duration::ZERO;
    for (label, spec, budget) in [
        (
            "size asc (to leave the default order)",
            sort(SortKey::Size, false),
            SORT_OTHER,
        ),
        ("name asc", sort(SortKey::Name, false), SORT_BY_NAME),
        ("name desc", sort(SortKey::Name, true), SORT_BY_NAME),
        ("size asc", sort(SortKey::Size, false), SORT_OTHER),
        ("size desc", sort(SortKey::Size, true), SORT_OTHER),
        ("modified asc", sort(SortKey::Modified, false), SORT_OTHER),
        ("modified desc", sort(SortKey::Modified, true), SORT_OTHER),
        ("kind asc", sort(SortKey::Kind, false), SORT_OTHER),
        (
            "name asc from kind order",
            sort(SortKey::Name, false),
            SORT_BY_NAME,
        ),
    ] {
        let started = Instant::now();
        listing.set_sort(spec);
        let took = started.elapsed();
        println!(
            "sort {label}: {:.0} ms (budget {:.0})",
            millis(took),
            millis(budget)
        );
        if spec.key == SortKey::Name {
            worst_name = worst_name.max(took);
        }
        assert!(!ENFORCE || took <= budget, "sort {label} took {took:?}");
    }

    let started = Instant::now();
    listing.set_filter(Filter {
        show_hidden: true,
        only: None,
    });
    println!(
        "show hidden (filter and sort): {:.0} ms",
        millis(started.elapsed())
    );

    let mut rng = Xorshift(7);
    let mut pages = Vec::new();
    for _ in 0..200 {
        let start = (rng.next() % (COUNT as u64 - 256)) as u32;
        let started = Instant::now();
        assert_eq!(listing.get_range(start, 256).len(), 256);
        pages.push(started.elapsed());
    }
    pages.sort();
    let p50 = pages[pages.len() / 2];
    let worst = *pages.last().unwrap();
    println!(
        "256-row page: p50 {:.3} ms, worst {:.3} ms",
        millis(p50),
        millis(worst)
    );
    assert!(!ENFORCE || worst <= PAGE, "a page took {worst:?}");

    // One file appears, one changes and one is removed in a folder this size.
    let started = Instant::now();
    let ops = listing.apply_changes(vec![
        Change::Upsert(ScannedEntry {
            name: "zzz-new.txt".into(),
            kind: EntryKind::File,
            link_target: None,
            link_pending: false,
            group: IconGroup::Document,
            size: Some(1),
            modified_ms: Some(0),
            hidden: false,
            trashed: None,
        }),
        Change::Remove(OsString::from("report-000000-0.txt")),
    ]);
    println!(
        "apply 2 changes: {:.1} ms, {} ops",
        millis(started.elapsed()),
        ops.len()
    );
    // The first batch builds the name map; a second shows the steady state.
    let started = Instant::now();
    listing.apply_changes(vec![Change::Remove(OsString::from("zzz-new.txt"))]);
    println!("apply 1 more change: {:.1} ms", millis(started.elapsed()));
}

#[test]
#[ignore = "creates 500 000 files; run with --release and --ignored"]
fn scanning_a_real_500_000_entry_directory_stays_inside_the_budget() {
    let dir = tempfile::TempDir::new().unwrap();
    let made = Instant::now();
    for i in 0..COUNT {
        File::create(dir.path().join(format!("file-{i:06}.txt"))).unwrap();
    }
    println!(
        "created {COUNT} files in {:.1} s",
        made.elapsed().as_secs_f64()
    );

    let started = Instant::now();
    let listing = Listing::open(
        ListingHandle(1),
        VfsPath::File(FilePath::from_path(dir.path()).unwrap()),
        Arc::new(LocalProvider::new()),
        SortSpec::default(),
        Filter::default(),
        ListingOptions {
            watch: false,
            ..ListingOptions::default()
        },
        Arc::new(|_| {}),
    )
    .unwrap();
    let took = started.elapsed();
    println!(
        "scan and index {COUNT} real entries: {:.0} ms (budget {:.0})",
        millis(took),
        millis(SCAN)
    );
    assert_eq!(listing.snapshot().count as usize, COUNT);
    assert!(!ENFORCE || took <= SCAN, "the scan took {took:?}");

    let started = Instant::now();
    listing.set_sort(sort(SortKey::Name, true));
    println!(
        "sort by name descending: {:.0} ms",
        millis(started.elapsed())
    );
}
