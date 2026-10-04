// Typed errors for what can go wrong inside an archive.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::io;

use waypoint_protocol::{AuthPrompt, Location, VfsError};

/// The archive cannot be read as one: damaged, cut short or not what it claims to be.
pub(crate) fn corrupt(container: &Location) -> VfsError {
    VfsError::Corrupt {
        location: container.clone(),
    }
}

/// An I/O failure while reading the archive: data that does not decode is `Corrupt`; anything else
/// is what the provider underneath says (a typed error it injected survives).
pub(crate) fn from_io(error: &io::Error, container: &Location) -> VfsError {
    match error.kind() {
        io::ErrorKind::InvalidData | io::ErrorKind::UnexpectedEof
            if error.get_ref().is_none_or(|inner| {
                inner
                    .downcast_ref::<waypoint_vfs::InjectedError>()
                    .is_none()
            }) =>
        {
            log::debug!("archive: {} does not decode: {error}", container.uri);
            corrupt(container)
        }
        _ => waypoint_vfs::from_io(error, container),
    }
}

/// A password is needed to read `container`.
pub(crate) fn password_required(container: &Location) -> VfsError {
    VfsError::AuthRequired {
        location: container.clone(),
        prompt: Box::new(AuthPrompt::Passphrase {
            subject: container.display.clone(),
        }),
    }
}

pub(crate) fn password_refused(container: &Location) -> VfsError {
    VfsError::AuthFailed {
        location: container.clone(),
    }
}

/// A format feature this provider does not read.
pub(crate) fn unsupported(what: impl Into<String>) -> VfsError {
    VfsError::Unsupported { what: what.into() }
}

/// Whether a read failure is the data's (a decoder that choked, in whatever words its library uses)
/// and not the disk's (an operating system code) or the provider's (a typed `InjectedError`).
pub(crate) fn is_bad_data(error: &io::Error) -> bool {
    error.raw_os_error().is_none()
        && !error.get_ref().is_some_and(|inner| {
            inner
                .downcast_ref::<waypoint_vfs::InjectedError>()
                .is_some()
        })
}

/// A decoder whose own failures are all `InvalidData`, so `from_io` reports them as `Corrupt`.
pub(crate) struct DecodeErrors<R>(pub R);

impl<R: io::Read> io::Read for DecodeErrors<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.read(buf).map_err(|error| {
            if is_bad_data(&error) && error.kind() != io::ErrorKind::InvalidData {
                io::Error::new(io::ErrorKind::InvalidData, error.to_string())
            } else {
                error
            }
        })
    }
}
