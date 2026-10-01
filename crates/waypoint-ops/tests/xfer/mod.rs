// Helpers for the copy and move tests: requests, the driver, a rich description of a tree (content,
// times and modes) that can be built and read back through a provider, and the in-memory provider's
// volumes.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![allow(dead_code, unused_imports)]

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::io::{Read, Write};
use std::time::{Duration, UNIX_EPOCH};

pub use crate::common::*;
pub use waypoint_ops::testing::faulty::FaultyProvider;
pub use waypoint_ops::testing::sandbox::SandboxProvider;
pub use waypoint_ops::testing::transfer::{run_transfer, Answers, TransferOptions};
pub use waypoint_protocol::Location;
pub use waypoint_vfs::MemOp;
pub use waypoint_vfs::{
    CancelToken, EntryKind, FileTimes, Permissions, VolumeId, VolumeSpace, WriteOptions,
};

/// A chunk small enough that a few kilobytes cross many chunk boundaries.
pub const SMALL_CHUNK: usize = 1024;

pub fn small() -> TransferOptions {
    TransferOptions {
        chunk_bytes: SMALL_CHUNK,
        ..TransferOptions::default()
    }
}

/// A copy or move request over `sources` (relative to the work folder) into `dest`.
pub fn req<P: Provider + 'static>(
    h: &Harness<P>,
    kind: JobKind,
    sources: &[&str],
    dest: &str,
    policy: Option<ConflictPolicy>,
) -> JobRequest {
    let mut request = h.request(kind, sources, Some(dest), None);
    request.options.conflict = policy;
    request
}

/// Builds and runs a request with no answers to give.
pub fn go<P: Provider + 'static>(
    h: &mut Harness<P>,
    kind: JobKind,
    sources: &[&str],
    dest: &str,
    policy: Option<ConflictPolicy>,
) -> RunResult {
    let request = req(h, kind, sources, dest, policy);
    run_plain(h, request)
}

pub fn run<P: Provider + 'static>(
    h: &mut Harness<P>,
    request: JobRequest,
    answers: &mut Answers,
) -> RunResult {
    run_transfer(h, request, answers, &small(), &mut |_, _| {})
}

/// Runs with no answers to give: a clash or an error the request does not settle stops the job.
pub fn run_plain<P: Provider + 'static>(h: &mut Harness<P>, request: JobRequest) -> RunResult {
    run(h, request, &mut Answers::default())
}

pub fn done(result: &RunResult) {
    assert_eq!(
        result.state,
        JobState::Done,
        "failure: {:?}",
        result.failure.as_ref().map(|f| (&f.error, &f.item))
    );
}

/// The memory provider under the harness's wrappers.
pub fn memory(h: &Harness<MemoryProvider>) -> &MemoryProvider {
    h.provider.inner().inner()
}

/// The `file://` source for a relative path, as a `Location`.
pub fn l<P: Provider + 'static>(h: &Harness<P>, relative: &str) -> Location {
    h.loc(relative)
}

/// Every name in `tree` that is a partial or a replaced leftover.
pub fn leftovers(tree: &Tree) -> Vec<String> {
    tree.keys()
        .filter(|key| {
            key.split('/').any(|n| {
                n.starts_with(".waypoint-partial-") || n.starts_with(".waypoint-replaced-")
            })
        })
        .cloned()
        .collect()
}

pub fn ms_to_time(ms: i64) -> std::time::SystemTime {
    if ms >= 0 {
        UNIX_EPOCH + Duration::from_millis(ms as u64)
    } else {
        UNIX_EPOCH - Duration::from_millis(ms.unsigned_abs())
    }
}

/// Sets the modification time of `relative` to `ms`.
pub fn set_mtime<P: Provider + 'static>(h: &Harness<P>, relative: &str, ms: i64) {
    h.provider
        .set_times(
            &h.path(relative),
            FileTimes {
                accessed: None,
                modified: Some(ms_to_time(ms)),
            },
        )
        .unwrap();
}

pub fn mtime_of<P: Provider + 'static>(h: &Harness<P>, relative: &str) -> Option<i64> {
    h.provider.stat(&h.path(relative)).unwrap().modified_ms
}

pub fn mode_of<P: Provider + 'static>(h: &Harness<P>, relative: &str) -> Option<u32> {
    h.provider
        .permissions(&h.path(relative))
        .ok()
        .and_then(|p| p.mode)
        .map(|m| m & 0o7777)
}

pub fn set_mode<P: Provider + 'static>(h: &Harness<P>, relative: &str, mode: u32) {
    h.provider
        .set_permissions(
            &h.path(relative),
            Permissions {
                mode: Some(mode),
                readonly: mode & 0o222 == 0,
            },
        )
        .unwrap();
}

/// Writes a file of `len` bytes, each `byte`, below the work folder.
pub fn put_bytes<P: Provider + 'static>(h: &Harness<P>, relative: &str, bytes: &[u8]) {
    let mut stream = h
        .provider
        .create_write(&h.path(relative), WriteOptions::exclusive())
        .unwrap();
    stream.write_all(bytes).unwrap();
    stream.finish(false).unwrap();
}

pub fn read_bytes<P: Provider + 'static>(h: &Harness<P>, relative: &str) -> Vec<u8> {
    let mut out = Vec::new();
    h.provider
        .open_read(&h.path(relative))
        .unwrap()
        .read_to_end(&mut out)
        .unwrap();
    out
}

/// A deterministic byte pattern of `len` bytes that differs from chunk to chunk.
pub fn pattern(len: usize, seed: u8) -> Vec<u8> {
    (0..len)
        .map(|i| ((i * 31 + i / 251 + seed as usize) % 253) as u8)
        .collect()
}
