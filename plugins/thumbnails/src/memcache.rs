// An in-memory least-recently-used cache of encoded thumbnails, bounded by bytes
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::sync::{Arc, Mutex};

use lru::LruCache;

use crate::cache::CacheKey;

struct Inner {
    entries: LruCache<CacheKey, Arc<Vec<u8>>>,
    bytes: u64,
}

/// Keeps recently served thumbnails in memory so a scroll back over them reads no files. Evicts the least recently used entry when the total of the encoded sizes passes the limit.
pub struct MemCache {
    inner: Mutex<Inner>,
    limit: u64,
}

impl MemCache {
    pub fn new(limit_bytes: u64) -> Self {
        MemCache {
            inner: Mutex::new(Inner {
                entries: LruCache::unbounded(),
                bytes: 0,
            }),
            limit: limit_bytes,
        }
    }

    pub fn get(&self, key: &CacheKey) -> Option<Arc<Vec<u8>>> {
        self.inner.lock().ok()?.entries.get(key).cloned()
    }

    /// Adds (or replaces) an entry. One bigger than the whole limit is not kept.
    pub fn insert(&self, key: CacheKey, bytes: Arc<Vec<u8>>) {
        let size = bytes.len() as u64;
        if size > self.limit {
            return;
        }
        let Ok(mut inner) = self.inner.lock() else {
            return;
        };
        if let Some(old) = inner.entries.push(key, bytes).map(|(_, old)| old) {
            inner.bytes -= old.len() as u64;
        }
        inner.bytes += size;
        while inner.bytes > self.limit {
            match inner.entries.pop_lru() {
                Some((_, evicted)) => inner.bytes -= evicted.len() as u64,
                None => break,
            }
        }
    }

    /// Forgets an entry whose file was replaced.
    pub fn remove(&self, key: &CacheKey) {
        if let Ok(mut inner) = self.inner.lock() {
            if let Some(old) = inner.entries.pop(key) {
                inner.bytes -= old.len() as u64;
            }
        }
    }

    pub fn bytes(&self) -> u64 {
        self.inner.lock().map_or(0, |inner| inner.bytes)
    }

    pub fn len(&self) -> usize {
        self.inner.lock().map_or(0, |inner| inner.entries.len())
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests;
