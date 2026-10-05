// Listing and reading 7z archives through `sevenz-rust2`.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::io::{Read, Seek};

use sevenz_rust2::{Archive, ArchiveReader, Error, NtTime, Password};
use waypoint_protocol::{Location, VfsError};
use waypoint_vfs::{CancelToken, EntryKind, Secret};

use crate::errors::{
    corrupt, from_io, is_bad_data, password_refused, password_required, unsupported,
};
use crate::index::{ArchiveIndex, Locator, NewEntry};
use crate::stream::Pump;
use crate::zip_scan::ScanError;

/// The method id of 7z's AES-256 coder.
const AES_ID: [u8; 4] = [0x06, 0xf1, 0x07, 0x01];
const UNIX_EXTENSION: u32 = 0x8000;
const S_IFMT: u32 = 0o170_000;
const S_IFDIR: u32 = 0o040_000;
const S_IFLNK: u32 = 0o120_000;
const S_IFREG: u32 = 0o100_000;
/// 100 ns ticks between 1601-01-01 and 1970-01-01, in milliseconds.
const NT_EPOCH_OFFSET_MS: i64 = 11_644_473_600_000;

pub(crate) fn password_of(secret: Option<&Secret>) -> Password {
    match secret {
        Some(secret) => match secret.expose_str() {
            Some(text) => Password::new(text),
            None => Password::from_raw(secret.expose()),
        },
        None => Password::empty(),
    }
}

/// What a 7z failure means, for the archive at `container`. `data` says the failure came while
/// decoding file data of an encrypted entry, where bad data usually means a wrong password.
pub(crate) fn map_error(error: &Error, container: &Location, encrypted: bool) -> VfsError {
    match error {
        Error::PasswordRequired => password_required(container),
        Error::MaybeBadPassword(_) => password_refused(container),
        Error::Io(io, _) | Error::FileOpen(io, _) => {
            if encrypted && is_bad_data(io) {
                password_refused(container)
            } else {
                from_io(io, container)
            }
        }
        Error::ChecksumVerificationFailed if encrypted => password_refused(container),
        Error::UnsupportedCompressionMethod(method) => {
            unsupported(format!("7z compression method {method}"))
        }
        Error::MaxMemLimited { .. } => {
            unsupported("a 7z stream that needs more memory than allowed")
        }
        Error::Unsupported(what) => unsupported(what.to_string()),
        Error::ExternalUnsupported => unsupported("7z external headers"),
        Error::UnsupportedVersion { .. } => unsupported("this version of the 7z format"),
        _ => {
            log::debug!("archive: 7z error at {}: {error}", container.uri);
            corrupt(container)
        }
    }
}

fn modified_ms(time: NtTime, present: bool) -> Option<i64> {
    if !present {
        return None;
    }
    let ticks = u64::from(time);
    i64::try_from(ticks / 10_000)
        .ok()
        .map(|ms| ms - NT_EPOCH_OFFSET_MS)
}

pub(crate) enum SevenScan {
    Done,
    /// The password is missing or wrong (the header is encrypted).
    Password(Error),
}

/// Lists a 7z archive into `index`.
pub(crate) fn scan(
    source: &mut (impl Read + Seek),
    secret: Option<&Secret>,
    index: &mut ArchiveIndex,
    max_entries: usize,
    cancel: &CancelToken,
    report: &mut dyn FnMut(u32),
) -> Result<SevenScan, ScanError> {
    let archive = match Archive::read(source, &password_of(secret)) {
        Ok(archive) => archive,
        Err(error @ (Error::PasswordRequired | Error::MaybeBadPassword(_))) => {
            return Ok(SevenScan::Password(error))
        }
        Err(Error::Io(io, _))
            if secret.is_some() && io.kind() == std::io::ErrorKind::InvalidData =>
        {
            // An encrypted header read with the wrong key decodes to garbage.
            return Ok(SevenScan::Password(Error::MaybeBadPassword(io)));
        }
        Err(Error::Io(io, _)) => return Err(ScanError::from(io)),
        Err(_) => return Err(ScanError::Corrupt("the 7z header does not read")),
    };
    if archive.files.len() > max_entries {
        return Err(ScanError::TooMany(max_entries));
    }
    let aes_blocks: Vec<bool> = archive
        .blocks
        .iter()
        .map(|block| {
            block
                .coders
                .iter()
                .any(|coder| coder.encoder_method_id() == AES_ID)
        })
        .collect();
    for (at, file) in archive.files.iter().enumerate() {
        if at % 256 == 0 {
            if cancel.is_cancelled() {
                return Err(ScanError::Cancelled);
            }
            report(at as u32);
        }
        if file.is_anti_item() {
            continue;
        }
        let attributes = file
            .has_windows_attributes
            .then_some(file.windows_attributes);
        let mode = attributes
            .filter(|attributes| attributes & UNIX_EXTENSION != 0)
            .map(|attributes| attributes >> 16);
        let kind = if file.is_directory() {
            EntryKind::Directory
        } else {
            match mode.map(|mode| mode & S_IFMT) {
                Some(S_IFDIR) => EntryKind::Directory,
                Some(S_IFLNK) => EntryKind::Symlink,
                Some(S_IFREG) | Some(0) | None => EntryKind::File,
                Some(_) => EntryKind::Other,
            }
        };
        let encrypted = archive
            .stream_map
            .file_block_index
            .get(at)
            .copied()
            .flatten()
            .and_then(|block| aes_blocks.get(block).copied())
            .unwrap_or(false);
        index.insert(
            file.name().as_bytes(),
            false,
            NewEntry {
                kind: Some(kind),
                size: Some(file.size()),
                compressed: (file.compressed_size > 0).then_some(file.compressed_size),
                modified_ms: modified_ms(file.last_modified_date(), file.has_last_modified_date),
                mode: mode.map(|mode| mode & 0o7777),
                link: None,
                hardlink: None,
                encrypted,
                sparse: false,
                locator: Some(Locator::SevenZ { file_index: at }),
            },
        );
    }
    Ok(SevenScan::Done)
}

/// The work of a reading thread for one 7z entry.
#[allow(clippy::too_many_arguments)]
pub(crate) fn read_job(
    pump: &mut Pump,
    source: crate::source::SeekSource,
    container: &Location,
    secret: Option<Secret>,
    name: &str,
    encrypted: bool,
    skip: u64,
    cancel: &CancelToken,
) {
    let mut reader = match ArchiveReader::new(source, password_of(secret.as_ref())) {
        Ok(reader) => reader,
        Err(error) => return pump.fail(map_error(&error, container, encrypted)),
    };
    reader.set_thread_count(1);
    // Inside a solid block each entry's data has to be read for the next one to be reached; the
    // library does not skip it, so the entries before the one wanted are drained.
    let solid = reader.archive().is_solid;
    let mut found = false;
    let result = reader.for_each_entries(|entry, data| {
        if entry.name() != name || entry.is_directory() {
            if solid {
                let mut drained = RemapInvalid {
                    inner: data,
                    when: None,
                };
                std::io::copy(&mut drained, &mut std::io::sink())
                    .map_err(|error| Error::Io(error, "reading past an entry".into()))?;
            }
            return Ok(true);
        }
        found = true;
        pump.ready();
        let mut data = RemapInvalid {
            inner: data,
            when: encrypted.then(|| password_refused(container)),
        };
        pump.copy_from(
            &mut data,
            skip,
            Some(entry.size().saturating_sub(skip)),
            Some(cancel),
        );
        Ok(false)
    });
    if let Err(error) = result {
        if !found {
            return pump.fail(map_error(&error, container, encrypted));
        }
    }
    if !found {
        pump.fail(VfsError::NotFound {
            location: container.clone(),
        });
    }
}

/// Turns the failure of a decoder that was given the wrong key into a typed one.
pub(crate) struct RemapInvalid<R> {
    pub inner: R,
    pub when: Option<VfsError>,
}

impl<R: Read> Read for RemapInvalid<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.inner.read(buf).map_err(|error| match &self.when {
            // A decoder given the wrong key fails in whatever way its data happens to break; only
            // the provider underneath (typed errors) and the disk (an OS code) say otherwise.
            Some(typed) if is_bad_data(&error) => {
                waypoint_vfs::InjectedError(typed.clone()).into_io()
            }
            _ => error,
        })
    }
}
