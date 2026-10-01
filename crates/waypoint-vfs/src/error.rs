// Turns operating system errors into the typed errors the UI shows as distinct states.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::io::{self, ErrorKind};

use waypoint_protocol::{Location, VfsError};

/// Maps an I/O error on `location` to a `VfsError`. Anything without its own state keeps the
/// operating system's message.
pub fn from_io(error: &io::Error, location: &Location) -> VfsError {
    match error.kind() {
        ErrorKind::NotFound => VfsError::NotFound {
            location: location.clone(),
        },
        ErrorKind::PermissionDenied => VfsError::PermissionDenied {
            location: location.clone(),
        },
        ErrorKind::NotADirectory => VfsError::NotADirectory {
            location: location.clone(),
        },
        _ => VfsError::Io {
            message: error.to_string(),
            location: Some(location.clone()),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn here() -> Location {
        Location::new("/x", "file:///x")
    }

    #[test]
    fn maps_the_kinds_that_have_their_own_state() {
        for (kind, expected) in [
            (ErrorKind::NotFound, VfsError::NotFound { location: here() }),
            (
                ErrorKind::PermissionDenied,
                VfsError::PermissionDenied { location: here() },
            ),
            (
                ErrorKind::NotADirectory,
                VfsError::NotADirectory { location: here() },
            ),
        ] {
            assert_eq!(from_io(&io::Error::from(kind), &here()), expected);
        }
    }

    #[test]
    fn keeps_the_message_for_everything_else() {
        let error = from_io(&io::Error::other("the disk is on fire"), &here());
        assert_eq!(
            error,
            VfsError::Io {
                message: "the disk is on fire".to_owned(),
                location: Some(here())
            }
        );
    }
}
