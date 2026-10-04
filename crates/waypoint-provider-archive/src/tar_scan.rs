// Listing a tar archive, plain or compressed, by reading its headers.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::io::{self, Read, Seek};

use tar::{Archive, EntryType};
use waypoint_vfs::{CancelToken, EntryKind, InjectedError};

use crate::index::{ArchiveIndex, Locator, NewEntry};
use crate::zip_scan::ScanError;

fn convert(error: io::Error) -> ScanError {
    let injected = error
        .get_ref()
        .and_then(|inner| inner.downcast_ref::<InjectedError>());
    match injected {
        Some(injected) if injected.0 == waypoint_protocol::VfsError::Cancelled => {
            ScanError::Cancelled
        }
        Some(_) => ScanError::Io(error),
        // The decoders and the tar reader describe a bad or cut-short stream in different ways;
        // anything that is not the disk's or the provider's is the archive's.
        None if error.raw_os_error().is_none() => ScanError::Corrupt("the tar headers do not read"),
        None => ScanError::Io(error),
    }
}

/// Lists a tar stream that can only be read forward (a compressed one).
pub(crate) fn scan_stream(
    reader: impl Read,
    index: &mut ArchiveIndex,
    max_entries: usize,
    cancel: &CancelToken,
    report: &mut dyn FnMut(u32),
) -> Result<(), ScanError> {
    let mut archive = Archive::new(reader);
    let entries = archive.entries().map_err(convert)?;
    record(entries, index, max_entries, None, cancel, report)
}

/// Lists an uncompressed tar by seeking from header to header, so only the headers are read.
pub(crate) fn scan_seek(
    reader: impl Read + Seek,
    length: u64,
    index: &mut ArchiveIndex,
    max_entries: usize,
    cancel: &CancelToken,
    report: &mut dyn FnMut(u32),
) -> Result<(), ScanError> {
    let mut archive = Archive::new(reader);
    let entries = archive.entries_with_seek().map_err(convert)?;
    record(entries, index, max_entries, Some(length), cancel, report)
}

fn is_link_type(kind: EntryType) -> bool {
    matches!(kind, EntryType::Link | EntryType::Symlink)
}

fn record<R: Read>(
    entries: tar::Entries<'_, R>,
    index: &mut ArchiveIndex,
    max_entries: usize,
    length: Option<u64>,
    cancel: &CancelToken,
    report: &mut dyn FnMut(u32),
) -> Result<(), ScanError> {
    for (ordinal, entry) in entries.enumerate() {
        let entry = entry.map_err(convert)?;
        if ordinal >= max_entries {
            return Err(ScanError::TooMany(max_entries));
        }
        if ordinal.is_multiple_of(256) {
            if cancel.is_cancelled() {
                return Err(ScanError::Cancelled);
            }
            report(ordinal as u32);
        }
        let this = ordinal;
        let header = entry.header();
        let kind_type = header.entry_type();
        let (kind, sparse) = match kind_type {
            EntryType::Regular | EntryType::Continuous => (EntryKind::File, false),
            EntryType::GNUSparse => (EntryKind::File, true),
            EntryType::Directory => (EntryKind::Directory, false),
            EntryType::Symlink => (EntryKind::Symlink, false),
            // A hard link is a file that shares another entry's data.
            EntryType::Link => (EntryKind::File, false),
            EntryType::XGlobalHeader => continue,
            EntryType::Char | EntryType::Block | EntryType::Fifo => (EntryKind::Other, false),
            _ => (EntryKind::Other, false),
        };
        let name = entry.path_bytes().into_owned();
        let link = entry.link_name_bytes().map(|bytes| bytes.into_owned());
        let size = entry.size();
        let mode = header.mode().ok().map(|mode| mode & 0o7777);
        let modified_ms = header
            .mtime()
            .ok()
            .and_then(|seconds| i64::try_from(seconds).ok())
            .map(|seconds| seconds * 1000);
        let data_offset = entry.raw_file_position();
        // Seeking from header to header never reads the data, so a file cut short inside an entry
        // is only seen by comparing where the data should end with where the file does.
        if length.is_some_and(|length| {
            kind != EntryKind::Directory
                && !sparse
                && !is_link_type(kind_type)
                && data_offset.saturating_add(size) > length
        }) {
            return Err(ScanError::Corrupt("the tar archive ends inside an entry"));
        }
        let is_link = kind_type == EntryType::Link;
        index.insert(
            &name,
            false,
            NewEntry {
                kind: Some(kind),
                size: (kind != EntryKind::Directory && !is_link).then_some(size),
                compressed: None,
                modified_ms,
                mode,
                link: if kind == EntryKind::Symlink {
                    link.clone()
                } else {
                    None
                },
                hardlink: if is_link { link } else { None },
                encrypted: false,
                sparse,
                locator: Some(Locator::Tar {
                    ordinal: this,
                    data_offset,
                }),
            },
        );
    }
    Ok(())
}
