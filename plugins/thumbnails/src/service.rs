// The Rust API of the plugin: the `Thumbnails` handle behind `app.thumbnails()`
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::sync::Arc;

use crate::cache::{CacheKey, Store};
use crate::engine::{Engine, Limits};
use crate::memcache::MemCache;
use crate::models::{PluginStatus, ThumbRequest, Ticket};
use crate::queue::Sink;

/// The thumbnails of this system: ask for them in batches, cancel what scrolled away, and read what was made through the `thumb://` scheme.
pub struct Thumbnails {
    engine: Engine,
    store: Arc<Store>,
    mem: Arc<MemCache>,
    limits: Arc<Limits>,
    status: PluginStatus,
}

impl Thumbnails {
    pub(crate) fn new(
        engine: Engine,
        store: Arc<Store>,
        mem: Arc<MemCache>,
        limits: Arc<Limits>,
        status: PluginStatus,
    ) -> Self {
        Thumbnails {
            engine,
            store,
            mem,
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
        if let Some(hit) = self.mem.get(&key) {
            return Some(hit);
        }
        let bytes = Arc::new(self.store.read(&key)?);
        self.mem.insert(key, Arc::clone(&bytes));
        Some(bytes)
    }
}
