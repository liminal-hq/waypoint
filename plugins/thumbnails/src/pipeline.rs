// The steps every platform shares around its generator: the cache lookup before, and storing the result or the failure after
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::Path;
use std::sync::Arc;

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

/// Looks the request up in the cache: a fresh thumbnail is `Ready`, a recorded failure for this version of the file is `Failed`, and anything else is to be made.
pub fn begin(store: &Store, mem: &MemCache, request: &ThumbRequest) -> Start {
    let (key, uri) = store.key_for(request.size, Path::new(&request.path));
    let secs = cache::mtime_secs(request.mtime_ms);
    match store.lookup(&key, &uri, secs) {
        Lookup::Fresh => Start::Done(Outcome::Ready {
            url: url_for(&key, secs),
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

/// Encodes the pixels with the standard's metadata, writes them to the cache (atomically), keeps them in memory and clears any recorded failure.
pub fn finish(
    store: &Store,
    mem: &MemCache,
    key: &CacheKey,
    meta: &Meta,
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
        url: url_for(key, meta.mtime_secs),
    }
}

/// Records that the thumbnail could not be made, so it is not tried again until the file changes, and reports it.
pub fn fail(store: &Store, key: &CacheKey, meta: &Meta, reason: String) -> Outcome {
    if let Err(error) = store.store_failure(key, meta) {
        log::debug!("could not record the thumbnail failure: {error}");
    }
    Outcome::Failed { reason }
}
