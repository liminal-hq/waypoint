// Defines plugin error types for file system command failures
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use serde::ser::SerializeStruct;
use thiserror::Error;
use waypoint_connections::ConnectionsError;
use waypoint_protocol::VfsError;

/// What a command rejects with. It serialises as the tagged `VfsError` object (`{ "kind": "notFound",
/// "location": … }`), never as a string, so `isVfsError` on the JavaScript side recognises it. An
/// edit of the saved connections that is refused is `{ "kind": "connections", "error": … }`, so its
/// kinds never read as a `VfsError`'s.
#[derive(Debug, Error)]
pub enum Error {
    /// A failure the UI shows as its own state.
    #[error("{0:?}")]
    Vfs(VfsError),
    /// A saved connection that cannot be saved, or is not there.
    #[error("{0}")]
    Connections(ConnectionsError),
    /// A fault in the plugin itself (a background task that panicked); reported as an I/O error.
    #[error("internal error: {0}")]
    Internal(String),
}

impl From<VfsError> for Error {
    fn from(error: VfsError) -> Self {
        Error::Vfs(error)
    }
}

impl From<ConnectionsError> for Error {
    fn from(error: ConnectionsError) -> Self {
        Error::Connections(error)
    }
}

impl serde::Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Error::Vfs(error) => error.serialize(serializer),
            Error::Connections(error) => {
                let mut object = serializer.serialize_struct("Error", 2)?;
                object.serialize_field("kind", "connections")?;
                object.serialize_field("error", error)?;
                object.end()
            }
            Error::Internal(message) => VfsError::Io {
                message: message.clone(),
                location: None,
            }
            .serialize(serializer),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialises_as_the_tagged_object() {
        let json = serde_json::to_string(&Error::from(VfsError::StaleHandle)).unwrap();
        assert_eq!(json, r#"{"kind":"staleHandle"}"#);
    }

    #[test]
    fn a_refused_edit_is_wrapped_so_it_never_reads_as_a_vfs_error() {
        let json = serde_json::to_value(Error::from(ConnectionsError::NotFound {
            id: "c1".to_owned(),
        }))
        .unwrap();
        assert_eq!(json["kind"], "connections");
        assert_eq!(json["error"]["kind"], "notFound");
    }

    #[test]
    fn an_internal_fault_is_an_io_error() {
        let json = serde_json::to_value(Error::Internal("boom".to_owned())).unwrap();
        assert_eq!(json["kind"], "io");
        assert_eq!(json["message"], "boom");
    }
}
