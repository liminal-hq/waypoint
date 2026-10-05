// The remote listing budget of spike #278, measured through the S3 provider.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! 100,000 keys in one prefix, listed on loopback: first rows within three round trips plus 50 ms,
//! and the whole listing in 0.5 s on a LAN (a transport budget; `remote-locations.md`). Not run by
//! default:
//!
//!   WAYPOINT_TEST_TMP=/some/disk/folder cargo nextest run --release -p waypoint-provider-s3 \
//!     --run-ignored only listing_budget --no-capture
//!
//! It runs against the in-memory server (the SDK and the provider alone, so the budget is asserted
//! in release builds) and against `rclone serve s3` and MinIO where they are installed (printed, not
//! asserted: rclone re-reads the folder for every page, which spike #278 measured, and a page is
//! 1,000 keys on every service, so 100,000 keys are at least 100 round trips; the budget holds only
//! where a page costs under 5 ms). `WAYPOINT_S3_BENCH_ENTRIES` changes the number of keys.

mod support;

use std::time::{Duration, Instant};

use support::fake_s3::FakeS3;
use support::{FixedKey, Kind, Server, KEY_ID, SECRET};
use waypoint_path::VfsPath;
use waypoint_provider_s3::{S3Config, S3Provider};
use waypoint_vfs::{CancelToken, Provider, WriteOptions};

fn resident_mb() -> f64 {
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    status
        .lines()
        .find(|line| line.starts_with("VmRSS"))
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|kb| kb.parse::<f64>().ok())
        .map_or(0.0, |kb| kb / 1024.0)
}

/// Lists `folder`, prints what it cost, and returns the total time.
fn measure(name: &str, provider: &S3Provider, folder: &VfsPath, entries: usize) -> Duration {
    provider.stat(folder).unwrap();
    let before = resident_mb();
    let start = Instant::now();
    let mut first = None;
    let mut batches = 0;
    let mut kept = Vec::with_capacity(entries);
    provider
        .list_batches(folder, &CancelToken::new(), 0, &mut |batch| {
            first.get_or_insert_with(|| start.elapsed());
            batches += 1;
            kept.extend(batch);
        })
        .unwrap();
    let total = start.elapsed();
    assert_eq!(kept.len(), entries);
    println!(
        "{name:>8}: {entries} keys in {:.0} ms (first batch {:.1} ms, {batches} batches, {:.1} MB more resident; budget {:.0} ms)",
        total.as_secs_f64() * 1e3,
        first.unwrap_or_default().as_secs_f64() * 1e3,
        resident_mb() - before,
        0.5 * entries as f64 / 100_000.0 * 1e3,
    );
    total
}

#[test]
#[ignore = "a benchmark: run it with --run-ignored and --release"]
fn listing_budget() {
    let entries: usize = std::env::var("WAYPOINT_S3_BENCH_ENTRIES")
        .ok()
        .and_then(|n| n.parse().ok())
        .unwrap_or(100_000);
    let budget = 0.5 * entries as f64 / 100_000.0;

    let fake = FakeS3::start();
    fake.bucket("bench");
    for n in 0..entries {
        fake.put("bench", &format!("many/file_{n}"), b"", "STANDARD");
    }
    let provider = S3Provider::new(S3Config::new(std::sync::Arc::new(FixedKey {
        key_id: KEY_ID.to_owned(),
        secret: SECRET.to_owned(),
    })));
    let folder = VfsPath::from_uri(&format!(
        "s3://bench/many?endpoint=http%3A%2F%2F127.0.0.1%3A{}",
        fake.port
    ))
    .unwrap();
    let total = measure("in-memory", &provider, &folder, entries);
    if !cfg!(debug_assertions) {
        assert!(total.as_secs_f64() <= budget, "over the listing budget");
    }
    drop(fake);

    for server in Server::start_all() {
        let bucket = server.bucket("bench");
        let provider = server.provider();
        let path = bucket.join("many").unwrap();
        match server.kind {
            // Files on disk are keys: write them there, not one request at a time.
            Kind::Rclone => {
                let folder = server.data.join("bench/many");
                std::fs::create_dir_all(&folder).unwrap();
                for n in 0..entries {
                    std::fs::File::create(folder.join(format!("file_{n}"))).unwrap();
                }
            }
            Kind::Minio => {
                for n in 0..entries {
                    let stream = provider
                        .create_write(
                            &path.join(format!("file_{n}")).unwrap(),
                            WriteOptions::truncate(),
                        )
                        .unwrap();
                    stream.finish(false).unwrap();
                }
            }
        }
        measure(server.name(), &provider, &path, entries);
    }
}
