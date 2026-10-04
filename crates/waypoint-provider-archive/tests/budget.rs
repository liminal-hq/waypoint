// The listing budget: a 10 000-entry zip browses within the milestone 0 listing budget (A18), and
// a very large archive is listed without extracting anything.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod support;

use std::time::{Duration, Instant};

use support::*;
use waypoint_vfs::{CancelToken, Provider};

fn many_entries(count: usize) -> Vec<u8> {
    let mut zip = RawZip::new();
    for n in 0..count {
        zip = zip.file(&format!("folder_{}/file_{n}.dat", n % 100), b"x");
    }
    zip.build()
}

#[test]
fn a_10_000_entry_zip_lists_within_the_budget() {
    let dir = scratch();
    let path = dir.path().join("many.zip");
    std::fs::write(&path, many_entries(10_000)).unwrap();
    let p = provider();
    let root = top(&path);
    // Cold: the central directory is read and the tree built; then each folder lists from it.
    let started = Instant::now();
    let top_level = p.list(&root, &CancelToken::new(), 0, &mut |_| {}).unwrap();
    let cold = started.elapsed();
    assert_eq!(top_level.len(), 100);
    let started = Instant::now();
    let mut total = 0;
    for entry in &top_level {
        let folder = root.join(&entry.name).unwrap();
        total += p
            .list(&folder, &CancelToken::new(), 0, &mut |_| {})
            .unwrap()
            .len();
    }
    let warm = started.elapsed();
    assert_eq!(total, 10_000);
    eprintln!("10 000-entry zip: cold {cold:?}, warm listing of every folder {warm:?}");
    // A18: 500 000 entries scanned in about 0.6 s, so 10 000 is well under 50 ms in a release
    // build; a debug build gets a looser bound.
    let (cold_limit, warm_limit) = if cfg!(debug_assertions) {
        (Duration::from_millis(1500), Duration::from_millis(1500))
    } else {
        (Duration::from_millis(100), Duration::from_millis(100))
    };
    assert!(cold < cold_limit, "cold {cold:?}");
    assert!(warm < warm_limit, "warm {warm:?}");
}

#[test]
fn one_big_folder_of_10_000_entries_streams_in_batches() {
    let dir = scratch();
    let path = dir.path().join("flat.zip");
    let mut zip = RawZip::new();
    for n in 0..10_000 {
        zip = zip.file(&format!("file_{n}.dat"), b"x");
    }
    std::fs::write(&path, zip.build()).unwrap();
    let p = provider();
    let mut batches = Vec::new();
    p.list_batches(&top(&path), &CancelToken::new(), 0, &mut |batch| {
        batches.push(batch.len())
    })
    .unwrap();
    assert_eq!(batches.iter().sum::<usize>(), 10_000);
    assert_eq!(batches[0], 2_000);
    assert_eq!(batches.len(), 5);
}

/// 200 000 entries in a plain tar and a gzip tar; not run by default.
///
///   WAYPOINT_TEST_TMP=/a/disk/folder cargo test --release -p waypoint-provider-archive \
///     --test budget -- --ignored --nocapture
#[test]
#[ignore = "a benchmark: run it with --ignored and --release"]
fn a_200_000_entry_tar_lists() {
    let dir = scratch();
    let mut builder = tar::Builder::new(Vec::new());
    for n in 0..200_000 {
        let mut header = tar::Header::new_gnu();
        header.set_size(0);
        header.set_mode(0o644);
        header.set_cksum();
        builder
            .append_data(&mut header, format!("d{}/f{n}", n % 500), std::io::empty())
            .unwrap();
    }
    let bytes = builder.into_inner().unwrap();
    let plain = dir.path().join("many.tar");
    std::fs::write(&plain, &bytes).unwrap();
    let zipped = dir.path().join("many.tar.gz");
    {
        use std::io::Write;
        let mut out = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        out.write_all(&bytes).unwrap();
        std::fs::write(&zipped, out.finish().unwrap()).unwrap();
    }
    let p = provider();
    for path in [plain, zipped] {
        let started = Instant::now();
        let top_level = p
            .list(&top(&path), &CancelToken::new(), 0, &mut |_| {})
            .unwrap();
        eprintln!(
            "{}: {} folders in {:?}",
            path.display(),
            top_level.len(),
            started.elapsed()
        );
        assert_eq!(top_level.len(), 500);
    }
}
