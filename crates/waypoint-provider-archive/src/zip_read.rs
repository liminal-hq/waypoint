// Reading one zip entry through the `zip` crate, which does the decompression and the decryption.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use waypoint_protocol::{Location, VfsError};
use waypoint_vfs::{CancelToken, Secret};
use zip::read::ZipReadOptions;
use zip::result::ZipError;
use zip::ZipArchive;

use crate::errors::{corrupt, from_io, password_refused, password_required, unsupported};
use crate::sevenz::RemapInvalid;
use crate::source::SourceSpec;
use crate::stream::Pump;

pub(crate) fn map_error(error: ZipError, container: &Location) -> VfsError {
    match error {
        ZipError::Io(io) => from_io(&io, container),
        ZipError::InvalidPassword => password_refused(container),
        ZipError::UnsupportedArchive(what) if what == ZipError::PASSWORD_REQUIRED => {
            password_required(container)
        }
        ZipError::UnsupportedArchive(what) => unsupported(what),
        ZipError::CompressionMethodNotSupported(method) => {
            unsupported(format!("zip compression method {method}"))
        }
        ZipError::FileNotFound => VfsError::NotFound {
            location: container.clone(),
        },
        ZipError::InvalidArchive(why) => {
            log::debug!("archive: zip invalid at {}: {why}", container.uri);
            corrupt(container)
        }
        _ => corrupt(container),
    }
}

/// The work of a reading thread for one zip entry.
#[allow(clippy::too_many_arguments)]
pub(crate) fn read_job(
    pump: &mut Pump,
    spec: &SourceSpec,
    container: &Location,
    ordinal: usize,
    header_start: u64,
    secret: Option<Secret>,
    encrypted: bool,
    skip: u64,
    cancel: &CancelToken,
) {
    let source = match spec.open_seek() {
        Ok(source) => source,
        Err(error) => return pump.fail(from_io(&error, container)),
    };
    let mut archive = match ZipArchive::new(source) {
        Ok(archive) => archive,
        Err(error) => return pump.fail(map_error(error, container)),
    };
    let options = ZipReadOptions::new().password(secret.as_ref().map(Secret::expose));
    let mut file = match archive.by_index_with_options(ordinal, options) {
        Ok(file) => file,
        Err(error) => return pump.fail(map_error(error, container)),
    };
    if file.header_start() != header_start {
        // The library numbered the entries differently from the scan: never read the wrong one.
        log::debug!("archive: zip entry {ordinal} is not where the scan found it");
        return pump.fail(corrupt(container));
    }
    pump.ready();
    let mut data = RemapInvalid {
        inner: crate::errors::DecodeErrors(&mut file),
        when: encrypted.then(|| password_refused(container)),
    };
    pump.copy_from(&mut data, skip, None, Some(cancel));
}
