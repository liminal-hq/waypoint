// Tests the byte-bounded LRU
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;
use crate::models::ThumbSize;

fn key(n: u32) -> CacheKey {
    CacheKey::new(ThumbSize::Normal, &format!("file:///{n}"))
}

fn blob(n: usize) -> Arc<Vec<u8>> {
    Arc::new(vec![0; n])
}

#[test]
fn the_least_recently_used_entry_goes_first() {
    let cache = MemCache::new(100);
    cache.insert(key(1), blob(40));
    cache.insert(key(2), blob(40));
    assert!(cache.get(&key(1)).is_some()); // 1 is now more recent than 2
    cache.insert(key(3), blob(40));
    assert!(cache.get(&key(2)).is_none());
    assert!(cache.get(&key(1)).is_some());
    assert!(cache.get(&key(3)).is_some());
    assert_eq!(cache.bytes(), 80);
}

#[test]
fn replacing_an_entry_keeps_the_total_right() {
    let cache = MemCache::new(100);
    cache.insert(key(1), blob(30));
    cache.insert(key(1), blob(50));
    assert_eq!((cache.len(), cache.bytes()), (1, 50));
    cache.remove(&key(1));
    assert_eq!((cache.len(), cache.bytes()), (0, 0));
    assert!(cache.is_empty());
}

#[test]
fn an_entry_larger_than_the_limit_is_not_kept() {
    let cache = MemCache::new(10);
    cache.insert(key(1), blob(11));
    assert!(cache.get(&key(1)).is_none());
    assert_eq!(cache.bytes(), 0);
}
