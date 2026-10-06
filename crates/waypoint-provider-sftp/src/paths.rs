// From `sftp://` locations to the paths the server reads.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use waypoint_path::{RemotePath, RemoteScheme, VfsPath};
use waypoint_protocol::VfsError;

/// The SFTP location `path` is, or `Unsupported` for any other scheme.
pub(crate) fn remote(path: &VfsPath) -> Result<&RemotePath, VfsError> {
    match path {
        VfsPath::Remote(remote) if remote.scheme() == RemoteScheme::Sftp => Ok(remote),
        _ => Err(VfsError::Unsupported {
            what: format!("{} over SFTP", path.display()),
        }),
    }
}

/// The absolute path on the server. The library speaks UTF-8 paths, so a name that is not UTF-8
/// cannot be addressed and is `InvalidName`.
pub(crate) fn server_path(path: &RemotePath) -> Result<String, VfsError> {
    if path.is_root() {
        return Ok("/".to_owned());
    }
    let mut out = String::new();
    for segment in path.segments() {
        let name = std::str::from_utf8(segment).map_err(|_| VfsError::InvalidName {
            name: String::from_utf8_lossy(segment).into_owned(),
            reason: "SFTP names that are not UTF-8 are not supported".to_owned(),
        })?;
        out.push('/');
        out.push_str(name);
    }
    Ok(out)
}

/// `folder` and one name in it, as the server reads it.
pub(crate) fn child(folder: &str, name: &str) -> String {
    if folder.ends_with('/') {
        format!("{folder}{name}")
    } else {
        format!("{folder}/{name}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locations_become_absolute_server_paths() {
        let path = VfsPath::from_uri("sftp://me@h/srv/a%20b/caf%C3%A9").unwrap();
        assert_eq!(
            server_path(remote(&path).unwrap()).unwrap(),
            "/srv/a b/café"
        );
        let root = VfsPath::from_uri("sftp://h").unwrap();
        assert_eq!(server_path(remote(&root).unwrap()).unwrap(), "/");
        let odd = VfsPath::from_uri("sftp://h/%FF").unwrap();
        assert!(matches!(
            server_path(remote(&odd).unwrap()),
            Err(VfsError::InvalidName { .. })
        ));
        let local = VfsPath::from_uri(if cfg!(windows) {
            "file:///C:/tmp"
        } else {
            "file:///tmp"
        })
        .unwrap();
        assert!(matches!(remote(&local), Err(VfsError::Unsupported { .. })));
        assert_eq!(child("/", "a"), "/a");
        assert_eq!(child("/x", "a"), "/x/a");
    }
}
