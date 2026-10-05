// The Rust API of the plugin: the `Thumbnails` handle behind `app.thumbnails()`
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::sync::Arc;

use crate::builtin::{generate_from_bytes, GenError};
use crate::cache::{encode_png, meta_of_bytes, mtime_secs, CacheKey, Meta, Store};
use crate::engine::{Engine, Limits};
use crate::memcache::MemCache;
use crate::models::{PluginStatus, SkipWhy, ThumbEvent, ThumbRequest, ThumbSize, Ticket};
use crate::queue::Sink;
use crate::scheme::url_for;

/// The thumbnails of this system: ask for them in batches, cancel what scrolled away, and read what was made through the `thumb://` scheme.
pub struct Thumbnails {
    engine: Engine,
    store: Arc<Store>,
    mem: Arc<MemCache>,
    /// Thumbnails made from bytes the caller read ([`Thumbnails::from_bytes`]): memory only, with a bound of their own.
    bytes_mem: Arc<MemCache>,
    limits: Arc<Limits>,
    status: PluginStatus,
}

impl Thumbnails {
    pub(crate) fn new(
        engine: Engine,
        store: Arc<Store>,
        mem: Arc<MemCache>,
        bytes_mem: Arc<MemCache>,
        limits: Arc<Limits>,
        status: PluginStatus,
    ) -> Self {
        Thumbnails {
            engine,
            store,
            mem,
            bytes_mem,
            limits,
            status,
        }
    }

    /// What works on this system, and why anything does not.
    pub fn get_status(&self) -> PluginStatus {
        self.status.clone()
    }

    /// Queues the items; each result is sent to `sink` as it is ready, newest request first. Items with the same key are one job.
    pub fn request(&self, items: Vec<ThumbRequest>, sink: Sink) -> Ticket {
        self.engine.request(items, sink)
    }

    /// Withdraws a request: nothing more is sent to it, work nobody else wants is dropped and a running external thumbnailer is killed. False if the ticket was not known (or was already finished).
    pub fn cancel(&self, ticket: Ticket) -> bool {
        self.engine.cancel(ticket)
    }

    /// Moves the request's pending items for `keys` to the front of the queue, in the order given.
    pub fn prioritise(&self, ticket: Ticket, keys: &[String]) {
        self.engine.prioritise(ticket, keys);
    }

    /// Changes the size above which the built-in generator does not decode a file.
    pub fn set_max_file_bytes(&self, bytes: u64) {
        self.limits.set_max_file_bytes(bytes);
    }

    /// The jobs waiting and running now.
    pub fn load(&self) -> (usize, usize) {
        self.engine.load()
    }

    /// The bytes of a cache entry named `/{size}/{md5}.png` (the path of a `thumb://` address), from memory or from the cache folder. `None` for any text that is not exactly such a name and for an entry that does not exist.
    pub fn entry_bytes(&self, path: &str) -> Option<Arc<Vec<u8>>> {
        let key = CacheKey::parse(path)?;
        if let Some(hit) = self.bytes_mem.get(&key) {
            return Some(hit);
        }
        if let Some(hit) = self.mem.get(&key) {
            return Some(hit);
        }
        let bytes = Arc::new(self.store.read(&key)?);
        self.mem.insert(key, Arc::clone(&bytes));
        Some(bytes)
    }

    /// A thumbnail already made from bytes for `uri` as it was at `mtime_ms`, answered under `key`;
    /// `None` when there is none, or it was made from another version. A caller asks before it
    /// reads anything, so a file whose thumbnail is known is not read again.
    pub fn cached_from_bytes(
        &self,
        key: &str,
        uri: &str,
        mtime_ms: i64,
        size: ThumbSize,
    ) -> Option<ThumbEvent> {
        let cache_key = CacheKey::new(size, uri);
        let png = self.bytes_mem.get(&cache_key)?;
        let meta = meta_of_bytes(&png)?;
        if meta.uri != uri || meta.mtime_secs != mtime_secs(mtime_ms) {
            self.bytes_mem.remove(&cache_key);
            return None;
        }
        Some(ThumbEvent::Ready {
            key: key.to_owned(),
            url: url_for(&cache_key, meta.mtime_secs),
        })
    }

    /// Makes a thumbnail from `bytes` the caller read itself, for a file this plugin cannot open
    /// (one on a server, read through the caller's own access), and answers under `key`. `uri`
    /// names the file and, with `mtime_ms`, keys the result, which is kept in a memory cache of its
    /// own (`Config::bytes_cache_bytes`) and never written to the shared cache folder. The bytes may
    /// be a whole image or an image embedded in a file (a photo's own small preview).
    pub fn from_bytes(
        &self,
        key: &str,
        uri: &str,
        mtime_ms: i64,
        size: ThumbSize,
        bytes: &[u8],
    ) -> ThumbEvent {
        let key = key.to_owned();
        if bytes.len() as u64 > self.limits.max_file_bytes() {
            return ThumbEvent::Skipped {
                key,
                why: SkipWhy::TooLarge,
            };
        }
        let rendered = match generate_from_bytes(bytes, size.pixels()) {
            Ok(rendered) => rendered,
            Err(GenError::TooLarge) => {
                return ThumbEvent::Skipped {
                    key,
                    why: SkipWhy::TooLarge,
                }
            }
            Err(GenError::Unsupported) => {
                return ThumbEvent::Skipped {
                    key,
                    why: SkipWhy::NoGenerator,
                }
            }
            Err(GenError::Failed(reason)) => return ThumbEvent::Failed { key, reason },
        };
        let meta = Meta {
            uri: uri.to_owned(),
            mtime_secs: mtime_secs(mtime_ms),
            file_size: None,
        };
        match encode_png(
            rendered.width,
            rendered.height,
            rendered.color,
            &rendered.pixels,
            &meta,
        ) {
            Ok(png) => {
                let cache_key = CacheKey::new(size, uri);
                let url = url_for(&cache_key, meta.mtime_secs);
                self.bytes_mem.insert(cache_key, Arc::new(png));
                ThumbEvent::Ready { key, url }
            }
            Err(error) => ThumbEvent::Failed {
                key,
                reason: error.to_string(),
            },
        }
    }
}
