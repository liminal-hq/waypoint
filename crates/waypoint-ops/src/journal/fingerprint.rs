// Fingerprints: what an entry looked like when a job finished, and whether it still does.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use waypoint_path::VfsPath;
use waypoint_protocol::VfsError;
use waypoint_vfs::{CancelToken, EntryKind, Provider, ScannedEntry};

use super::model::{Fingerprint, StaleReason};

/// FNV-1a over bytes: a fixed, dependency-free hash, so a stored digest means the same thing in
/// every build. It guards against edits, not against an adversary.
struct Fnv(u64);

impl Fnv {
    fn new() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }

    fn bytes(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.0 ^= u64::from(*b);
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }

    fn number(&mut self, n: u64) {
        self.bytes(&n.to_le_bytes());
    }

    fn done(&self) -> String {
        format!("{:016x}", self.0)
    }
}

fn kind_code(kind: EntryKind) -> u64 {
    match kind {
        EntryKind::File => 1,
        EntryKind::Directory => 2,
        EntryKind::Symlink => 3,
        EntryKind::Other => 4,
    }
}

/// Reads a fingerprint of the entry at `path`, walking a folder (never following links).
pub fn fingerprint(provider: &dyn Provider, path: &VfsPath) -> Result<Fingerprint, VfsError> {
    let entry = provider.stat(path)?;
    describe(provider, path, &entry)
}

fn describe(
    provider: &dyn Provider,
    path: &VfsPath,
    entry: &ScannedEntry,
) -> Result<Fingerprint, VfsError> {
    match entry.kind {
        EntryKind::Directory => {
            // (path below the folder, kind, size, file time), sorted so the hash does not depend
            // on the order the provider lists in.
            let mut rows: Vec<(String, u64, u64, i64)> = Vec::new();
            let mut stack = vec![(path.clone(), String::new())];
            let cancel = CancelToken::new();
            while let Some((folder, prefix)) = stack.pop() {
                for child in provider.list(&folder, &cancel, 0, &mut |_| {})? {
                    let name = child.name.to_string_lossy().into_owned();
                    let key = format!("{prefix}{name}");
                    let is_file = child.kind == EntryKind::File;
                    rows.push((
                        key.clone(),
                        kind_code(child.kind),
                        if is_file { child.size.unwrap_or(0) } else { 0 },
                        if is_file {
                            child.modified_ms.unwrap_or(0)
                        } else {
                            0
                        },
                    ));
                    if child.kind == EntryKind::Directory {
                        let below =
                            folder
                                .join(&child.name)
                                .map_err(|_| VfsError::InvalidName {
                                    name: name.clone(),
                                    reason: "not a usable name".to_owned(),
                                })?;
                        stack.push((below, format!("{key}/")));
                    }
                }
            }
            rows.sort();
            let mut hash = Fnv::new();
            for (key, kind, size, modified) in &rows {
                hash.bytes(key.as_bytes());
                hash.number(*kind);
                hash.number(*size);
                hash.number(*modified as u64);
            }
            Ok(Fingerprint {
                is_dir: true,
                size: None,
                modified_ms: entry.modified_ms,
                entry_count: Some(rows.len() as u64),
                digest: Some(hash.done()),
            })
        }
        EntryKind::Symlink => {
            let mut hash = Fnv::new();
            hash.bytes(provider.read_link(path)?.to_string_lossy().as_bytes());
            Ok(Fingerprint {
                is_dir: false,
                size: None,
                modified_ms: None,
                entry_count: None,
                digest: Some(hash.done()),
            })
        }
        EntryKind::File | EntryKind::Other => Ok(Fingerprint {
            is_dir: false,
            size: entry.size,
            modified_ms: entry.modified_ms,
            entry_count: None,
            digest: None,
        }),
    }
}

/// Checks the entry at `path` against `expected`, reading it as it is now.
pub fn verify(
    provider: &dyn Provider,
    path: &VfsPath,
    expected: Option<&Fingerprint>,
) -> Result<(), StaleReason> {
    let Some(expected) = expected else {
        return Err(StaleReason::Unverified);
    };
    match provider.stat(path) {
        Ok(_) => {}
        Err(VfsError::NotFound { .. } | VfsError::NotADirectory { .. }) => {
            return Err(StaleReason::Missing)
        }
        Err(_) => return Err(StaleReason::Changed),
    }
    let now = fingerprint(provider, path).map_err(|_| StaleReason::Changed)?;
    if same(expected, &now) {
        Ok(())
    } else {
        Err(StaleReason::Changed)
    }
}

/// Whether two fingerprints describe the same entry. A folder's own time is left out.
pub fn same(a: &Fingerprint, b: &Fingerprint) -> bool {
    a.is_dir == b.is_dir
        && a.size == b.size
        && a.entry_count == b.entry_count
        && a.digest == b.digest
        && (a.is_dir || a.modified_ms == b.modified_ms)
}
