// Mapping between the `admin:` paths the app uses and the `file:` paths the helper acts on.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use waypoint_path::{FilePath, VfsPath};
use waypoint_protocol::{Location, VfsError};

use crate::os_name::WireOs;

/// The path the helper is asked about: only an `admin:` path is the client's to send.
pub(crate) fn to_helper(path: &VfsPath) -> Result<WireOs, VfsError> {
    match path {
        VfsPath::Elevated(elevated) => Ok(WireOs::from_os(elevated.file().as_path().as_os_str())),
        other => Err(VfsError::InvalidLocation {
            input: other.to_uri(),
        }),
    }
}

/// A path the helper named, as the `admin:` path that reaches it from here. The helper must name
/// an absolute path in the form `FilePath` keeps.
pub(crate) fn from_helper(wire: &WireOs) -> Option<VfsPath> {
    let raw = wire.to_os_string().ok()?;
    let file = FilePath::from_path(&raw).ok()?;
    VfsPath::File(file).elevated()
}

/// The location the helper reported, as the `admin:` location of the same place; one that is not
/// a `file:` location is left as it is.
pub(crate) fn to_admin(location: Location) -> Location {
    match VfsPath::from_uri(&location.uri) {
        Ok(path @ VfsPath::File(_)) => path
            .elevated()
            .map_or(location, |elevated| elevated.to_location()),
        _ => location,
    }
}

/// Calls `visit` on every location an error carries.
pub(crate) fn for_each_location(error: &mut VfsError, visit: &mut dyn FnMut(&mut Location)) {
    match error {
        VfsError::NotFound { location }
        | VfsError::PermissionDenied { location }
        | VfsError::NotADirectory { location }
        | VfsError::AlreadyExists { location }
        | VfsError::NotEmpty { location }
        | VfsError::IsADirectory { location }
        | VfsError::StorageFull { location }
        | VfsError::ReadOnly { location }
        | VfsError::InUse { location }
        | VfsError::NotText { location }
        | VfsError::Disconnected { location }
        | VfsError::Unreachable { location, .. }
        | VfsError::Timeout { location }
        | VfsError::AuthRequired { location, .. }
        | VfsError::AuthFailed { location }
        | VfsError::HostKeyUnknown { location, .. }
        | VfsError::HostKeyChanged { location, .. }
        | VfsError::CertificateUntrusted { location, .. }
        | VfsError::RateLimited { location, .. }
        | VfsError::Archived { location }
        | VfsError::ClockSkew { location, .. }
        | VfsError::Corrupt { location } => visit(location),
        VfsError::CrossesDevices { from, to } => {
            visit(from);
            visit(to);
        }
        VfsError::Io {
            location: Some(location),
            ..
        } => visit(location),
        VfsError::Io { location: None, .. }
        | VfsError::InvalidLocation { .. }
        | VfsError::StaleHandle
        | VfsError::Cancelled
        | VfsError::Unsupported { .. }
        | VfsError::ProtocolOff { .. }
        | VfsError::InvalidName { .. } => {}
    }
}

/// An error from the helper with its locations in the `admin:` form, so the person sees and
/// navigates to the place they asked about.
pub(crate) fn rewrite_error(mut error: VfsError) -> VfsError {
    for_each_location(&mut error, &mut |location| {
        *location = to_admin(location.clone());
    });
    error
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn a_file_location_becomes_an_admin_one() {
        let location = to_admin(Location::new("/etc/a b", "file:///etc/a%20b"));
        assert_eq!(location.uri, "admin:///etc/a%20b");
        assert_eq!(location.display, "/etc/a b");
        let other = Location::new("x", "sftp://h/x");
        assert_eq!(to_admin(other.clone()), other);
    }

    #[test]
    fn every_location_of_an_error_is_rewritten() {
        let error = rewrite_error(VfsError::CrossesDevices {
            from: Location::new("/a", "file:///a"),
            to: Location::new("/b", "file:///b"),
        });
        match error {
            VfsError::CrossesDevices { from, to } => {
                assert_eq!(
                    (from.uri.as_str(), to.uri.as_str()),
                    ("admin:///a", "admin:///b")
                );
            }
            other => panic!("{other:?}"),
        }
        match rewrite_error(VfsError::Io {
            message: "x".to_owned(),
            location: Some(Location::new("/a", "file:///a")),
        }) {
            VfsError::Io { location, .. } => assert_eq!(location.unwrap().uri, "admin:///a"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn only_admin_paths_go_to_the_helper() {
        let admin = VfsPath::from_uri("admin:///etc").unwrap();
        assert_eq!(to_helper(&admin).unwrap(), WireOs::Text("/etc".to_owned()));
        let file = VfsPath::from_uri("file:///etc").unwrap();
        assert!(matches!(
            to_helper(&file),
            Err(VfsError::InvalidLocation { .. })
        ));
    }

    #[test]
    fn the_helper_must_name_a_normal_absolute_path() {
        let back = from_helper(&WireOs::Text("/etc/x".to_owned())).unwrap();
        assert_eq!(back.to_uri(), "admin:///etc/x");
        assert!(from_helper(&WireOs::Text("etc".to_owned())).is_none());
        assert!(from_helper(&WireOs::Wide(vec![0x2f])).is_none());
    }
}
