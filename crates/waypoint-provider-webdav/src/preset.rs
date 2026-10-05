// The Nextcloud preset: from the address a person knows to the location of their files.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use percent_encoding::{utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};
use waypoint_path::VfsPath;
use waypoint_protocol::VfsError;

const COMPONENT: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');

/// The root of `user`'s files on the Nextcloud (or ownCloud) at `server`, which may be a bare host
/// name (`cloud.example.com`), an address as people copy it (`https://cloud.example.com/nextcloud/`)
/// or a `davs://` location. The files are at `remote.php/dav/files/<user>/` below the server's own
/// path (`remote.php/webdav/` is the older spelling, and reaches the same files). `user` is the
/// account's id, which is not always the name it logs in with (a directory account has an internal
/// id); the login itself is an app password, made in the account's security settings, and arrives
/// through the `CredentialSource` like any password.
pub fn nextcloud_root(server: &str, user: &str) -> Result<VfsPath, VfsError> {
    let invalid = || VfsError::InvalidLocation {
        input: server.to_owned(),
    };
    let server = server.trim();
    let (scheme, rest) = match server.split_once("://") {
        Some((scheme, rest)) => match scheme.to_ascii_lowercase().as_str() {
            "https" | "davs" | "webdavs" => ("davs", rest),
            "http" | "dav" | "webdav" => ("dav", rest),
            _ => return Err(invalid()),
        },
        None => ("davs", server),
    };
    let (host, base) = rest.split_once('/').unwrap_or((rest, ""));
    if host.is_empty() || user.is_empty() || host.contains('@') {
        return Err(invalid());
    }
    let mut uri = format!("{scheme}://{}@{host}", utf8_percent_encode(user, COMPONENT));
    for segment in base.split('/').filter(|segment| !segment.is_empty()) {
        // The server's own path is kept as written, so a name already escaped stays so.
        uri.push('/');
        uri.push_str(segment);
    }
    for segment in ["remote.php", "dav", "files"] {
        uri.push('/');
        uri.push_str(segment);
    }
    uri.push('/');
    uri.push_str(&utf8_percent_encode(user, COMPONENT).to_string());
    VfsPath::from_uri(&uri).map_err(|_| invalid())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_address_people_know_becomes_the_files_root() {
        for (server, user, uri) in [
            (
                "cloud.example.com",
                "alice",
                "davs://alice@cloud.example.com/remote.php/dav/files/alice",
            ),
            (
                "https://cloud.example.com/nextcloud/",
                "alice",
                "davs://alice@cloud.example.com/nextcloud/remote.php/dav/files/alice",
            ),
            (
                "http://nas.lan:8080",
                "bob smith",
                "dav://bob%20smith@nas.lan:8080/remote.php/dav/files/bob%20smith",
            ),
            (
                "davs://cloud.example.com/",
                "me",
                "davs://me@cloud.example.com/remote.php/dav/files/me",
            ),
        ] {
            assert_eq!(
                nextcloud_root(server, user).unwrap().to_uri(),
                uri,
                "{server}"
            );
        }
    }

    #[test]
    fn nonsense_is_refused() {
        assert!(nextcloud_root("", "me").is_err());
        assert!(nextcloud_root("ftp://h", "me").is_err());
        assert!(nextcloud_root("h", "").is_err());
        assert!(nextcloud_root("me@h", "me").is_err());
    }
}
