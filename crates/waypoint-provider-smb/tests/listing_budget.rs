// The SMB listing and read budget of spike #278, measured through the provider against a real
// Samba server.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The remote listing budget of spike #278, measured through the provider: 100,000 entries on a
//! LAN, at 10 ms and at 100 ms round trip, and the speed of reading one large file. Not run by
//! default:
//!
//!   WAYPOINT_TEST_TMP=/some/disk/folder cargo nextest run --release -p waypoint-provider-smb \
//!     --run-ignored only listing_budget --no-capture
//!
//! `WAYPOINT_SMB_BENCH_ENTRIES` changes the number of entries (default 100,000). The fixture goes
//! in a temporary folder under `WAYPOINT_TEST_TMP` (use a disk, not a small tmpfs) and is deleted
//! afterwards. The budget is asserted in release builds only. The library's directory queries are
//! not pipelined, so the 100 ms case is expected to miss it (the spike measured 19.7 s), which is
//! why the listing's first rows wait for the whole folder (A99).

#![cfg(all(feature = "client", not(windows)))]

mod support;

use std::fs;
use std::io::Read;
use std::time::{Duration, Instant};

use support::{Proxy, Smbd};
use waypoint_vfs::{CancelToken, Provider};

#[test]
#[ignore = "a benchmark: run it with --run-ignored and --release"]
fn listing_budget() {
    let Some(server) = Smbd::start() else { return };
    let entries: usize = std::env::var("WAYPOINT_SMB_BENCH_ENTRIES")
        .ok()
        .and_then(|n| n.parse().ok())
        .unwrap_or(100_000);
    let folder = server.data.join("many");
    fs::create_dir(&folder).unwrap();
    for n in 0..entries {
        fs::File::create(folder.join(format!("file_{n}"))).unwrap();
    }
    let big = vec![7u8; 256 * 1024 * 1024];
    fs::write(server.data.join("big.bin"), &big).unwrap();
    drop(big);
    let scale = entries as f64 / 100_000.0;
    let conditions = [
        ("LAN", Duration::ZERO, 1.5),
        ("10 ms", Duration::from_millis(10), 4.0),
    ];
    let mut over = Vec::new();
    for (name, round_trip, budget) in conditions {
        let proxy = (round_trip > Duration::ZERO).then(|| Proxy::start(server.port, round_trip));
        let port = proxy.as_ref().map_or(server.port, |proxy| proxy.port);
        let provider = server.provider();
        let location = server.location_at(port, "data/many");
        // Connect and list once first, as the spike did: the clock covers a warm listing only.
        provider
            .list_batches(&location, &CancelToken::new(), 0, &mut |_| {})
            .unwrap();
        let start = Instant::now();
        let mut kept = Vec::with_capacity(entries);
        provider
            .list_batches(&location, &CancelToken::new(), 0, &mut |batch| {
                kept.extend(batch)
            })
            .unwrap();
        let total = start.elapsed();
        assert_eq!(kept.len(), entries);
        println!(
            "{name:>6}: {entries} entries in {:.0} ms (budget {:.0} ms)",
            total.as_secs_f64() * 1e3,
            budget * scale * 1e3,
        );
        if total.as_secs_f64() > budget * scale {
            over.push(name);
        }
    }
    // Reading 256 MiB from the LAN.
    let provider = server.provider();
    let file = server.location_at(server.port, "data/big.bin");
    provider.stat(&file).unwrap();
    let start = Instant::now();
    let mut stream = provider.open_read(&file).unwrap();
    let mut buf = vec![0u8; 1 << 20];
    let mut total = 0u64;
    loop {
        let n = stream.read(&mut buf).unwrap();
        if n == 0 {
            break;
        }
        total += n as u64;
    }
    let seconds = start.elapsed().as_secs_f64();
    println!(
        "  read: {} MiB in {seconds:.2} s = {:.0} MiB/s",
        total >> 20,
        (total >> 20) as f64 / seconds
    );
    if !cfg!(debug_assertions) {
        assert!(over.is_empty(), "over the listing budget: {over:?}");
    }
}
