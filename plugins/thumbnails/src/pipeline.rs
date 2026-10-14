// The steps every platform shares around its generator: the cache lookup before, and storing the result or the failure after
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::fs::Metadata;
use std::path::Path;
use std::sync::Arc;
use std::time::SystemTime;

use crate::builtin::Rendered;
use crate::cache::{self, CacheKey, Lookup, Meta, Store};
use crate::engine::Outcome;
use crate::memcache::MemCache;
use crate::models::ThumbRequest;
use crate::scheme::url_for;

/// What the cache says about a request.
pub enum Start {
    /// Nothing to make: the answer is already known.
    Done(Outcome),
    /// Make it, then record it under `key`.
    Make {
        key: CacheKey,
        uri: String,
        secs: i64,
    },
}

/// How long after its modified time a file is taken to be still changing. A failure to read a file modified this recently is reported but not remembered: the writer may not have finished.
pub const SETTLE_MS: i64 = 2_000;

/// What a stat says about a file, to tell whether it changed while its thumbnail was being made.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileState {
    pub len: u64,
    pub modified: Option<SystemTime>,
}

impl FileState {
    pub fn of(metadata: &Metadata) -> Self {
        FileState {
            len: metadata.len(),
            modified: metadata.modified().ok(),
        }
    }

    /// The state of the file at `path` now; `None` if it cannot be read (gone, or not accessible).
    fn now(path: &Path) -> Option<Self> {
        std::fs::metadata(path).ok().map(|m| FileState::of(&m))
    }
}

/// Looks the request up in the cache for the file as it is (`size` bytes): a fresh thumbnail is `Ready`, a recorded failure for this version of the file is `Failed`, and anything else is to be made.
pub fn begin(store: &Store, mem: &MemCache, request: &ThumbRequest, size: u64) -> Start {
    let (key, uri) = store.key_for(request.size, Path::new(&request.path));
    let secs = cache::mtime_secs(request.mtime_ms);
    match store.lookup(&key, &uri, secs, size) {
        Lookup::Fresh => Start::Done(Outcome::Ready {
            url: url_for(&key, request.mtime_ms, size),
        }),
        Lookup::Failed => Start::Done(Outcome::Failed {
            reason: "a thumbnail could not be made before and the file has not changed".to_string(),
        }),
        Lookup::Stale => {
            mem.remove(&key);
            Start::Make { key, uri, secs }
        }
        Lookup::Miss => Start::Make { key, uri, secs },
    }
}

/// Encodes the pixels with the standard's metadata, writes them to the cache (atomically), keeps them in memory and clears any recorded failure. `mtime_ms` and the size in `meta` make the address the page loads.
pub fn finish(
    store: &Store,
    mem: &MemCache,
    key: &CacheKey,
    meta: &Meta,
    mtime_ms: i64,
    rendered: &Rendered,
) -> Outcome {
    let png = match cache::encode_png(
        rendered.width,
        rendered.height,
        rendered.color,
        &rendered.pixels,
        meta,
    ) {
        Ok(png) => png,
        Err(error) => {
            return Outcome::Failed {
                reason: error.to_string(),
            }
        }
    };
    if let Err(error) = store.store(key, &png) {
        return Outcome::Failed {
            reason: format!("the thumbnail cache cannot be written: {error}"),
        };
    }
    store.clear_failure(key);
    mem.insert(key.clone(), Arc::new(png));
    Outcome::Ready {
        url: url_for(key, mtime_ms, meta.file_size.unwrap_or(0)),
    }
}

/// Whether the file changed since `before` was read (or cannot be read now).
fn changed_since(path: &Path, before: &FileState) -> bool {
    FileState::now(path) != Some(*before)
}

/// Whether a file modified at `mtime_ms` may still be being written at `now_ms`. A modified time in the future counts, and `0` (not known) does not.
fn recently_modified(now_ms: i64, mtime_ms: i64) -> bool {
    mtime_ms != 0 && now_ms.saturating_sub(mtime_ms) < SETTLE_MS
}

/// Like [`finish`], but only when the file is the one that was read: if its size or modified time changed while the thumbnail was being made, the pixels may be of a half-written file, so nothing is cached and the request fails (the next request for the changed file makes it again).
pub fn finish_if_unchanged(
    store: &Store,
    mem: &MemCache,
    key: &CacheKey,
    meta: &Meta,
    request: &ThumbRequest,
    before: &FileState,
    rendered: &Rendered,
) -> Outcome {
    if changed_since(Path::new(&request.path), before) {
        return Outcome::Failed {
            reason: "the file changed while its thumbnail was being made".to_string(),
        };
    }
    finish(store, mem, key, meta, request.mtime_ms, rendered)
}

/// Records that the thumbnail could not be made, so it is not tried again until the file changes, and reports it. A file that changed while it was read, or was modified in the last [`SETTLE_MS`], may simply not be finished: the failure is reported for this request but not recorded, so the next request tries again.
pub fn fail(
    store: &Store,
    key: &CacheKey,
    meta: &Meta,
    request: &ThumbRequest,
    before: &FileState,
    reason: String,
) -> Outcome {
    let unsettled = recently_modified(store.now_ms(), request.mtime_ms)
        || changed_since(Path::new(&request.path), before);
    if !unsettled {
        if let Err(error) = store.store_failure(key, meta) {
            log::debug!("could not record the thumbnail failure: {error}");
        }
    }
    Outcome::Failed { reason }
}
