// Runs an extraction: the plan's items are copied out of the archives through the copy engine, so
// conflicts, partial files, verification, progress, cancelling and the undo journal are the ones a
// copy has, with the archive provider wrapped in a guard that holds the line against what an archive
// can be made to do (A92).
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::HashSet;
use std::io::{self, Read};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use waypoint_path::{VfsPath, ARCHIVE_SCHEME};
use waypoint_protocol::{Location, VfsError};
use waypoint_vfs::{
    CancelToken, Capabilities, EntryKind, InjectedError, ListingLayout, Permissions, Provider,
    ReadStream, ScannedEntry,
};

use super::copy_job;
use super::{ExecEnv, ExecFailure, ExecReport, ExecSink, RunOptions};
use crate::model::{JobId, JobKind, OpsError};
use crate::plan::Plan;

/// Entries the archive lists that the extraction leaves out are not there as far as the copy can
/// tell, what an entry yields never exceeds what it declared, and permissions are applied the way an
/// extraction may (no setuid, setgid or sticky bits, and a folder the owner can use).
struct Guard {
    inner: Arc<dyn Provider>,
    skip: HashSet<VfsPath>,
    /// Bytes read from all entries so far.
    total: Arc<AtomicU64>,
    /// What the archives declared in all: reading more than this is a bomb whatever the entries say.
    cap: u64,
}

fn lie(what: &str) -> io::Error {
    InjectedError(VfsError::Io {
        message: format!("{what} holds more than the archive says it does"),
        location: None,
    })
    .into_io()
}

/// A stream cut at the size its entry declared, failing if the entry goes on past it.
struct Limited {
    inner: ReadStream,
    remaining: u64,
    total: Arc<AtomicU64>,
    cap: u64,
    name: String,
    /// The entry, named by the error of one that ends short.
    location: Location,
}

impl Read for Limited {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        if self.remaining == 0 {
            let mut probe = [0u8; 1];
            return match self.inner.read(&mut probe)? {
                0 => Ok(0),
                _ => Err(lie(&self.name)),
            };
        }
        let want = buf
            .len()
            .min(usize::try_from(self.remaining).unwrap_or(usize::MAX));
        let read = self.inner.read(&mut buf[..want])?;
        if read == 0 {
            // The archive says there is more: it is damaged, or it lied about the size.
            return Err(InjectedError(VfsError::Corrupt {
                location: self.location.clone(),
            })
            .into_io());
        }
        self.remaining -= read as u64;
        let all = self.total.fetch_add(read as u64, Ordering::Relaxed) + read as u64;
        if all > self.cap {
            return Err(lie("the archive"));
        }
        Ok(read)
    }
}

impl Guard {
    fn hidden(&self, path: &VfsPath) -> Result<(), VfsError> {
        if self.skip.contains(path) {
            Err(VfsError::NotFound {
                location: path.to_location(),
            })
        } else {
            Ok(())
        }
    }
}

impl Provider for Guard {
    fn scheme(&self) -> &'static str {
        self.inner.scheme()
    }

    fn capabilities(&self) -> Capabilities {
        self.inner.capabilities()
    }

    fn read_only(&self) -> bool {
        true
    }

    fn layout(&self) -> ListingLayout {
        self.inner.layout()
    }

    fn stat(&self, path: &VfsPath) -> Result<ScannedEntry, VfsError> {
        self.hidden(path)?;
        self.inner.stat(path)
    }

    fn list(
        &self,
        path: &VfsPath,
        cancel: &CancelToken,
        inline_link_budget: usize,
        progress: &mut dyn FnMut(u32),
    ) -> Result<Vec<ScannedEntry>, VfsError> {
        self.hidden(path)?;
        let mut entries = self
            .inner
            .list(path, cancel, inline_link_budget, progress)?;
        if !self.skip.is_empty() {
            entries.retain(|entry| {
                path.join(&entry.name)
                    .map_or(true, |child| !self.skip.contains(&child))
            });
        }
        Ok(entries)
    }

    fn resolve_link(
        &self,
        folder: &VfsPath,
        entry: &ScannedEntry,
    ) -> Result<ScannedEntry, VfsError> {
        self.inner.resolve_link(folder, entry)
    }

    fn connection_key(&self, path: &VfsPath) -> Option<waypoint_path::ConnectionKey> {
        self.inner.connection_key(path)
    }

    fn open_read(&self, path: &VfsPath) -> Result<ReadStream, VfsError> {
        self.open_read_at(path, 0)
    }

    fn open_read_at(&self, path: &VfsPath, start: u64) -> Result<ReadStream, VfsError> {
        self.hidden(path)?;
        let entry = self.inner.stat(path)?;
        let inner = self.inner.open_read_at(path, start)?;
        let name = entry.name.to_string_lossy().into_owned();
        // An entry whose size the archive does not give may not be read at all.
        let declared = entry.size.unwrap_or(0);
        Ok(Box::new(Limited {
            inner,
            remaining: declared.saturating_sub(start),
            total: self.total.clone(),
            cap: self.cap,
            name,
            location: path.to_location(),
        }))
    }

    fn permissions(&self, path: &VfsPath) -> Result<Permissions, VfsError> {
        self.hidden(path)?;
        let found = self.inner.permissions(path)?;
        let folder = self
            .inner
            .stat(path)
            .is_ok_and(|entry| entry.kind == EntryKind::Directory);
        let mode = found.mode.map(|mode| {
            let kept = mode & 0o777;
            if folder {
                kept | 0o700
            } else {
                kept | 0o400
            }
        });
        Ok(Permissions {
            mode,
            readonly: mode.is_some_and(|mode| mode & 0o200 == 0),
        })
    }

    fn read_link(&self, path: &VfsPath) -> Result<std::ffi::OsString, VfsError> {
        self.hidden(path)?;
        self.inner.read_link(path)
    }
}

/// The archive files whose items were placed, in order and without repeats.
fn archive_files(plan: &Plan, placed: &[Location]) -> Vec<Location> {
    let Some(extract) = &plan.extract else {
        return Vec::new();
    };
    let mut files: Vec<Location> = Vec::new();
    for location in placed {
        let archive = plan
            .items
            .iter()
            .position(|item| {
                item.source.as_ref().map(VfsPath::to_location).as_ref() == Some(location)
            })
            .and_then(|at| extract.item_archives.get(at));
        if let Some(file) = archive.map(VfsPath::to_location) {
            if !files.contains(&file) {
                files.push(file);
            }
        }
    }
    files
}

pub(super) fn run(
    env: &ExecEnv,
    job: JobId,
    plan: &Plan,
    cancel: &CancelToken,
    sink: &mut dyn ExecSink,
    options: RunOptions,
) -> Result<ExecReport, Box<ExecFailure>> {
    let Some(extract) = &plan.extract else {
        return Err(Box::new(ExecFailure {
            error: OpsError::Unsupported {
                what: "an extraction without its plan".to_owned(),
            },
            item: None,
            done: 0,
            report: ExecReport::default(),
        }));
    };
    let mut providers = env.providers.clone();
    if let Some(inner) = providers.get(ARCHIVE_SCHEME) {
        providers.register(Arc::new(Guard {
            inner,
            skip: extract.skip.clone(),
            total: Arc::new(AtomicU64::new(0)),
            cap: extract.declared_bytes,
        }));
    }
    let guarded = ExecEnv {
        providers,
        ..env.clone()
    };
    // From here on it is a copy of the archives' entries.
    let mut copy = plan.clone();
    copy.kind = JobKind::Copy;
    let fix = |report: &mut ExecReport| {
        let placed = std::mem::take(&mut report.transfer.placed_sources);
        report.transfer.placed_sources = archive_files(plan, &placed);
    };
    match copy_job::run(&guarded, job, &copy, cancel, sink, options) {
        Ok(mut report) => {
            fix(&mut report);
            Ok(report)
        }
        Err(mut failure) => {
            fix(&mut failure.report);
            Err(failure)
        }
    }
}
