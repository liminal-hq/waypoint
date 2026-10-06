// Zip times across time zones: DOS times are local wall clocks, extended timestamps are instants.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

// Windows reads its zone from the system, not from `TZ`, so the zones are only switched on Unix.
#![cfg(unix)]

mod support;

use std::io::Read;
use std::process::Command;

use support::*;
use waypoint_protocol::Location;
use waypoint_vfs::{
    ArchiveKind, ArchiveWriters, EntryAttrs, LocalProvider, Provider, WriteOptions,
};

/// 2024-01-01 12:00:00 UTC.
const INSTANT_MS: i64 = 1_704_110_400_000;

/// Zones given as POSIX rules, so no zone database is needed: (TZ, hours its wall clock is ahead of UTC).
const ZONES: [(&str, f64); 4] = [
    ("UTC0", 0.0),
    ("EST5EDT,M3.2.0,M11.1.0", -5.0),
    ("<+0530>-5:30", 5.5),
    ("NZST-12NZDT,M9.5.0,M4.1.0/3", 13.0),
];

/// Runs the checks below in a process of its own with `TZ` set: the zone is read once per process
/// by the system's time functions, so one process cannot switch between them reliably.
#[test]
fn zip_times_hold_across_time_zones() {
    for (zone, _) in ZONES {
        let out = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "in_this_zone", "--ignored", "--nocapture"])
            .env("TZ", zone)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "in TZ={zone}: {}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

fn offset_hours() -> f64 {
    let tz = std::env::var("TZ").unwrap();
    ZONES.iter().find(|(zone, _)| *zone == tz).unwrap().1
}

/// The DOS time and date of the first local header of a zip, as (hour, minute, day).
fn first_dos_clock(bytes: &[u8]) -> (u16, u16, u16) {
    let time = u16::from_le_bytes([bytes[10], bytes[11]]);
    let date = u16::from_le_bytes([bytes[12], bytes[13]]);
    (time >> 11, (time >> 5) & 0x3f, date & 0x1f)
}

#[test]
#[ignore = "run by zip_times_hold_across_time_zones with TZ set"]
fn in_this_zone() {
    let dir = scratch();
    let p = provider();

    // 1. A zip that has only a DOS time (Python's `zipfile`, `RawZip`): 2024-01-01 12:00:00 on
    //    the machine's clock, which is a different instant in each zone.
    let dos = RawZip::new()
        .file("a.txt", b"a")
        .write(&dir.path().join("dos.zip"));
    let expected_ms = INSTANT_MS - (offset_hours() * 3_600_000.0) as i64;
    // (Daylight time is in force in neither January zone rule above except New Zealand's NZDT, whose
    // offset is the 13 hours listed.)
    let stat = p.stat(&at(&top(&dos), "a.txt")).unwrap();
    assert_eq!(
        stat.modified_ms,
        Some(expected_ms),
        "a DOS time is a local wall clock"
    );

    // 2. A zip written by the provider keeps the instant: the DOS fields are the local clock and
    //    the extended timestamp holds the instant, which is what reads back.
    let path = dir.path().join("made.zip");
    let out = LocalProvider::new()
        .create_write(&file(&path), WriteOptions::exclusive())
        .unwrap();
    let mut builder = p
        .begin(ArchiveKind::Zip, out, Location::new("out", "file:///out"))
        .unwrap();
    builder
        .add_file(
            b"b.txt",
            1,
            EntryAttrs {
                mode: Some(0o644),
                modified_ms: Some(INSTANT_MS),
            },
            &mut &b"b"[..],
        )
        .unwrap();
    builder.finish(true).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    let (hour, minute, _) = first_dos_clock(&bytes);
    let local_minutes = (12.0 * 60.0 + offset_hours() * 60.0).rem_euclid(24.0 * 60.0);
    assert_eq!(
        (hour as f64 * 60.0 + minute as f64),
        local_minutes,
        "the DOS time is the local clock"
    );
    assert!(
        bytes.windows(2).any(|w| w == [0x55, 0x54]),
        "the extended timestamp is written"
    );
    let stat = p.stat(&at(&top(&path), "b.txt")).unwrap();
    assert_eq!(stat.modified_ms, Some(INSTANT_MS));

    // 3. A time-zone-free reader sees the same thing: the file decompresses and keeps its name.
    let mut data = Vec::new();
    p.open_read(&at(&top(&path), "b.txt"))
        .unwrap()
        .read_to_end(&mut data)
        .unwrap();
    assert_eq!(data, b"b");
}
