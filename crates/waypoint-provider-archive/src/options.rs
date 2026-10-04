// What an archive provider is configured with, and what it tells the app.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::sync::Arc;

use waypoint_path::VfsPath;
use waypoint_protocol::{Location, VfsError};
use waypoint_vfs::{Provider, ProviderRegistry};

use crate::format::ArchiveFormat;

/// Where an archive provider finds the provider that holds an archive file: the local provider for
/// a file on disk, SFTP for one on a server. An archive inside an archive is served by the archive
/// provider itself, so a source never needs to name it.
pub trait ContainerSource: Send + Sync {
    /// The provider of `container`, or `Unsupported` naming what cannot hold an archive.
    fn provider_for(&self, container: &VfsPath) -> Result<Arc<dyn Provider>, VfsError>;
}

impl ContainerSource for ProviderRegistry {
    fn provider_for(&self, container: &VfsPath) -> Result<Arc<dyn Provider>, VfsError> {
        self.for_path(container)
    }
}

impl<F> ContainerSource for F
where
    F: Fn(&VfsPath) -> Result<Arc<dyn Provider>, VfsError> + Send + Sync,
{
    fn provider_for(&self, container: &VfsPath) -> Result<Arc<dyn Provider>, VfsError> {
        self(container)
    }
}

/// The limits an archive provider keeps to. Every one is a default the app can change.
#[derive(Debug, Clone)]
pub struct ArchiveOptions {
    /// The most entries an archive may list. A bigger one fails with `Unsupported` naming the
    /// limit, rather than showing part of it as if it were all.
    pub max_entries: usize,
    /// A tar archive at least this big (in bytes, compressed) that has to be read from the start to
    /// be listed raises `ArchiveNotice::SlowListing` before the scan, so the tab can say why it is
    /// waiting.
    pub slow_scan_bytes: u64,
    /// The biggest archive inside an archive that is copied to a scratch file so it can be read
    /// with seeks (a zip, a plain tar or a 7z inside another archive).
    pub max_nested_bytes: u64,
    /// Where scratch copies of nested archives go; the system's temporary folder when `None`.
    pub scratch_dir: Option<std::path::PathBuf>,
    /// Archives whose listings are kept (the index of each is in memory).
    pub cached_archives: usize,
    /// Entries per batch of `list_batches`.
    pub batch: usize,
}

impl Default for ArchiveOptions {
    fn default() -> Self {
        Self {
            max_entries: 2_000_000,
            slow_scan_bytes: 64 * 1024 * 1024,
            max_nested_bytes: 2 * 1024 * 1024 * 1024,
            scratch_dir: None,
            cached_archives: 8,
            batch: 2_000,
        }
    }
}

/// Something the app may want to tell the person while a listing is being made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArchiveNotice {
    /// The archive has to be read from its start to be listed (a compressed tar has no index), so
    /// opening it will take a while. `bytes` is its size on disk.
    SlowListing {
        container: Location,
        format: ArchiveFormat,
        bytes: u64,
    },
}

/// Where notices go. It may be called from any thread.
pub type NoticeSink = Arc<dyn Fn(ArchiveNotice) + Send + Sync>;
