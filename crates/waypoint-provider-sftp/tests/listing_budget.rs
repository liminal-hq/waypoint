// The SFTP listing budget of spike #278, measured through the provider against the in-process
// server and a real OpenSSH server.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The remote listing budget of spike #278, measured through the provider: 100,000 entries on a
//! LAN, at 10 ms and at 100 ms round trip. `listing_budget::in_process` runs against the
//! in-process server on any platform (its tree is in memory, and its `readdir` answers 100
//! entries at a time as OpenSSH's does); `listing_budget::openssh` runs against a real OpenSSH
//! server and skips without `sshd`. Not run by default:
//!
//!   WAYPOINT_TEST_TMP=/some/disk/folder cargo nextest run --release -p waypoint-provider-sftp \
//!     --run-ignored only listing_budget --no-capture
//!
//! `WAYPOINT_SFTP_BENCH_ENTRIES` changes the number of entries (default 100,000). The OpenSSH
//! fixture goes in a temporary folder under `WAYPOINT_TEST_TMP` (use a disk, not a small tmpfs)
//! and is deleted afterwards. The budget is asserted in release builds only. The in-process
//! server and the client share the machine's cores, so its numbers are a check of the provider's
//! own work (framing, pipelining, entry building) and not of OpenSSH's.

#[macro_use]
mod support;

use std::fs;
use std::time::{Duration, Instant};

use support::Backend;
use waypoint_provider_sftp::SftpOptions;
use waypoint_vfs::{CancelToken, Provider};

/// Resident memory now, from `/proc` (Linux only; zero elsewhere).
fn resident_mb() -> f64 {
    let status = fs::read_to_string("/proc/self/status").unwrap_or_default();
    status
        .lines()
        .find(|line| line.starts_with("VmRSS"))
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|kb| kb.parse::<f64>().ok())
        .map_or(0.0, |kb| kb / 1024.0)
}

fn listing_budget(server: &dyn Backend) {
    let entries: usize = std::env::var("WAYPOINT_SFTP_BENCH_ENTRIES")
        .ok()
        .and_then(|n| n.parse().ok())
        .unwrap_or(100_000);
    server.mkdir("many");
    for n in 0..entries {
        server.put(&format!("many/file_{n}"), b"");
    }
    // Spike #278's budget for 100,000 entries, scaled to the count.
    let scale = entries as f64 / 100_000.0;
    let conditions = [
        ("LAN", Duration::ZERO, 0.5),
        ("10 ms", Duration::from_millis(10), 1.0),
        ("100 ms", Duration::from_millis(100), 5.0),
    ];
    let mut over = Vec::new();
    for (name, round_trip, budget) in conditions {
        let proxy = (round_trip > Duration::ZERO).then(|| server.proxy(round_trip));
        let port = proxy.as_ref().map_or(server.port(), |proxy| proxy.port);
        let provider = server.provider_with(port, SftpOptions::default());
        let location = server.location(port, "many");
        // Connect first, as the spike did: the clock covers the listing only.
        provider.stat(&location).unwrap();
        let before = resident_mb();
        let start = Instant::now();
        let mut first = None;
        let mut batches = 0;
        let mut kept = Vec::with_capacity(entries);
        provider
            .list_batches(&location, &CancelToken::new(), 0, &mut |batch| {
                first.get_or_insert_with(|| start.elapsed());
                batches += 1;
                kept.extend(batch);
            })
            .unwrap();
        let total = start.elapsed();
        let grown = resident_mb() - before;
        assert_eq!(kept.len(), entries);
        println!(
            "{} {name:>6}: {entries} entries in {:.0} ms (first batch {:.1} ms, {batches} batches, \
             {grown:.1} MB more resident; budget {:.0} ms)",
            server.name(),
            total.as_secs_f64() * 1e3,
            first.unwrap_or_default().as_secs_f64() * 1e3,
            budget * scale * 1e3,
        );
        if total.as_secs_f64() > budget * scale {
            over.push(name);
        }
        drop(kept);
    }
    if !cfg!(debug_assertions) {
        assert!(over.is_empty(), "over the listing budget: {over:?}");
    }
}

#[test]
#[ignore = "a benchmark: run it with --run-ignored and --release"]
fn in_process() {
    listing_budget(&support::FakeSftp::start());
}

#[test]
#[ignore = "a benchmark: run it with --run-ignored and --release"]
fn openssh() {
    let Some(server) = support::Sshd::start() else {
        return;
    };
    listing_budget(&server);
}
