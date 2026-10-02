// The last directory-size result for each root, kept in a small JSON file so Overview can show it
// at once as "as of <time>" while a fresh scan runs
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The file is a cache, never a source of truth: anything unreadable, corrupt or written by another
// schema version reads as "nothing cached", and the next completed scan replaces it. It is bounded
// twice: `CACHE_MAX_ROOTS` roots (least recently measured dropped first) and `CACHE_MAX_ROWS`
// folder rows per root (the largest kept; the remainder row always stays, so the totals still add
// up). Only a finished scan is stored, so a cancelled one never replaces a good result.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use waypoint_protocol::{Location, VfsError};

use crate::dirscan::{DirScanResult, DirSizeRowKind};

/// The schema this build reads and writes.
pub const CACHE_VERSION: u32 = 1;

/// The file's name inside the app's data directory.
pub const CACHE_FILE: &str = "dir-size-cache.json";

/// Roots kept.
pub const CACHE_MAX_ROOTS: usize = 8;

/// Folder rows kept per root.
pub const CACHE_MAX_ROWS: usize = 200;

#[derive(Serialize, Deserialize)]
struct Document {
    version: u32,
    /// Most recently measured first.
    results: Vec<DirScanResult>,
}

/// The cache file of one app data directory.
#[derive(Debug, Clone)]
pub struct DirScanCache {
    path: PathBuf,
}

impl DirScanCache {
    /// A cache stored as `CACHE_FILE` inside `dir`, which need not exist yet.
    pub fn in_dir(dir: &Path) -> Self {
        Self {
            path: dir.join(CACHE_FILE),
        }
    }

    fn read(&self) -> Vec<DirScanResult> {
        let Ok(text) = fs::read_to_string(&self.path) else {
            return Vec::new();
        };
        let Ok(value) = serde_json::from_str::<Value>(&text) else {
            return Vec::new();
        };
        if value.get("version").and_then(Value::as_u64) != Some(u64::from(CACHE_VERSION)) {
            return Vec::new();
        }
        serde_json::from_value::<Document>(value)
            .map(|document| document.results)
            .unwrap_or_default()
    }

    /// The last finished result for `root`, with the time it was measured in `measured_at_ms`.
    pub fn load(&self, root: &Location) -> Option<DirScanResult> {
        self.read().into_iter().find(|r| r.root.uri == root.uri)
    }

    /// Remembers a finished result for its root, replacing the one before and dropping the oldest
    /// roots past the bound. The file is replaced atomically.
    pub fn store(&self, result: &DirScanResult) -> Result<(), VfsError> {
        let mut result = result.clone();
        let remainder = result
            .rows
            .iter()
            .position(|r| r.kind == DirSizeRowKind::Other)
            .map(|at| result.rows.remove(at));
        result.rows.truncate(CACHE_MAX_ROWS);
        result.rows.extend(remainder);

        let mut results = self.read();
        results.retain(|r| r.root.uri != result.root.uri);
        results.insert(0, result);
        results.truncate(CACHE_MAX_ROOTS);
        let text = serde_json::to_string(&Document {
            version: CACHE_VERSION,
            results,
        })
        .map_err(|e| io_error(&e.to_string()))?;
        self.write(&text).map_err(|e| io_error(&e.to_string()))
    }

    fn write(&self, text: &str) -> std::io::Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut temporary = self.path.clone().into_os_string();
        temporary.push(".tmp");
        let temporary = PathBuf::from(temporary);
        fs::write(&temporary, text)?;
        fs::rename(&temporary, &self.path)
    }
}

fn io_error(message: &str) -> VfsError {
    VfsError::Io {
        message: format!("could not save the directory-size cache: {message}"),
        location: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dirscan::DirSizeRow;

    fn result(root: &str, folders: usize, measured_at_ms: u64) -> DirScanResult {
        let mut rows: Vec<DirSizeRow> = (0..folders)
            .map(|n| DirSizeRow {
                name: format!("f{n}"),
                kind: DirSizeRowKind::Folder,
                location: None,
                bytes: (folders - n) as u64,
                files: 1,
                share: 0.0,
            })
            .collect();
        rows.push(DirSizeRow {
            name: crate::dirscan::REMAINDER_NAME.to_owned(),
            kind: DirSizeRowKind::Other,
            location: None,
            bytes: 1,
            files: 1,
            share: 0.0,
        });
        DirScanResult {
            root: Location::new(root, format!("file://{root}")),
            rows,
            total_bytes: 10,
            total_files: 3,
            allocated: false,
            folders_scanned: folders as u64,
            folders_total: folders as u64,
            symlinks_skipped: 0,
            mounts_skipped: 0,
            placeholders: 0,
            unreadable: 0,
            measured_at_ms,
        }
    }

    #[test]
    fn a_stored_result_reads_back_with_its_time_and_a_missing_file_is_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let cache = DirScanCache::in_dir(&dir.path().join("not-yet"));
        let stored = result("/home/a", 3, 1234);
        assert_eq!(cache.load(&stored.root), None);
        cache.store(&stored).unwrap();
        assert_eq!(cache.load(&stored.root), Some(stored.clone()));
        assert_eq!(cache.load(&stored.root).unwrap().measured_at_ms, 1234);
        assert_eq!(cache.load(&Location::new("/x", "file:///x")), None);
    }

    #[test]
    fn a_new_result_for_a_root_replaces_the_old() {
        let dir = tempfile::tempdir().unwrap();
        let cache = DirScanCache::in_dir(dir.path());
        cache.store(&result("/home/a", 1, 1)).unwrap();
        let newer = result("/home/a", 2, 2);
        cache.store(&newer).unwrap();
        assert_eq!(cache.read().len(), 1);
        assert_eq!(cache.load(&newer.root), Some(newer));
    }

    #[test]
    fn another_schema_version_or_a_corrupt_file_reads_as_nothing_and_is_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let cache = DirScanCache::in_dir(dir.path());
        let stored = result("/home/a", 1, 1);
        let mut value = serde_json::to_value(Document {
            version: CACHE_VERSION,
            results: vec![stored.clone()],
        })
        .unwrap();
        value["version"] = (CACHE_VERSION + 1).into();
        fs::write(dir.path().join(CACHE_FILE), value.to_string()).unwrap();
        assert_eq!(cache.load(&stored.root), None);
        fs::write(dir.path().join(CACHE_FILE), "{ not json").unwrap();
        assert_eq!(cache.load(&stored.root), None);
        // Right version, wrong shape.
        fs::write(
            dir.path().join(CACHE_FILE),
            format!(r#"{{"version":{CACHE_VERSION},"results":[{{"x":1}}]}}"#),
        )
        .unwrap();
        assert_eq!(cache.load(&stored.root), None);
        cache.store(&stored).unwrap();
        assert_eq!(cache.load(&stored.root), Some(stored));
    }

    #[test]
    fn the_cache_is_bounded_in_roots_and_rows() {
        let dir = tempfile::tempdir().unwrap();
        let cache = DirScanCache::in_dir(dir.path());
        for n in 0..(CACHE_MAX_ROOTS + 3) {
            cache
                .store(&result(&format!("/r{n}"), 1, n as u64))
                .unwrap();
        }
        let kept = cache.read();
        assert_eq!(kept.len(), CACHE_MAX_ROOTS);
        // Newest first; the oldest roots went.
        assert_eq!(kept[0].root.display, format!("/r{}", CACHE_MAX_ROOTS + 2));
        assert_eq!(cache.load(&Location::new("/r0", "file:///r0")), None);

        let big = result("/big", CACHE_MAX_ROWS + 50, 9);
        cache.store(&big).unwrap();
        let back = cache.load(&big.root).unwrap();
        assert_eq!(back.rows.len(), CACHE_MAX_ROWS + 1);
        assert_eq!(back.rows[0].name, "f0");
        assert_eq!(back.rows.last().unwrap().kind, DirSizeRowKind::Other);
    }
}
