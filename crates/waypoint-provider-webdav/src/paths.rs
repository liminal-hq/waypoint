// From `dav://` and `davs://` locations to the URLs a server reads, and from the hrefs a server
// answers with back to names.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::ffi::OsString;

use percent_encoding::{percent_decode, utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};
use waypoint_path::{Host, RemotePath, RemoteScheme, VfsPath};
use waypoint_protocol::VfsError;

/// Everything but the unreserved characters of RFC 3986 is escaped, so a name never reads as
/// syntax (`#`, `?`, `;`, `%`) to a server or a proxy in front of it.
const SEGMENT: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');

/// The location `path` is on `scheme`, or `Unsupported` for any other scheme.
pub(crate) fn remote(path: &VfsPath, scheme: RemoteScheme) -> Result<&RemotePath, VfsError> {
    match path {
        VfsPath::Remote(remote) if remote.scheme() == scheme => Ok(remote),
        _ => Err(VfsError::Unsupported {
            what: format!("{} over {scheme}://", path.display()),
        }),
    }
}

/// `https://host[:port]` or `http://host[:port]`, without a path.
pub(crate) fn origin(path: &RemotePath) -> String {
    let secure = path.scheme() == RemoteScheme::Davs;
    let authority = path.authority();
    let mut out = String::from(if secure { "https://" } else { "http://" });
    match &authority.host {
        Host::Name(name) => out.push_str(name),
        Host::Ipv4(addr) => out.push_str(&addr.to_string()),
        // A zone is local to this computer and has no place in a URL.
        Host::Ipv6 { addr, .. } => {
            out.push('[');
            out.push_str(&addr.to_string());
            out.push(']');
        }
    }
    if let Some(port) = authority.port {
        out.push(':');
        out.push_str(&port.to_string());
    }
    out
}

/// The escaped path of a list of names: `/a/b%20c`, or `/` for none. A folder's has a trailing
/// slash when `folder` is set.
pub(crate) fn encode_path(segments: &[Vec<u8>], folder: bool) -> String {
    let mut out = String::new();
    for segment in segments {
        out.push('/');
        out.extend(utf8_percent_encode_bytes(segment));
    }
    if out.is_empty() || (folder && !out.ends_with('/')) {
        out.push('/');
    }
    out
}

fn utf8_percent_encode_bytes(bytes: &[u8]) -> impl Iterator<Item = String> + '_ {
    // A name that is not UTF-8 is escaped byte by byte, so it reaches the server as it is.
    let mut rest = bytes;
    std::iter::from_fn(move || {
        if rest.is_empty() {
            return None;
        }
        match std::str::from_utf8(rest) {
            Ok(text) => {
                rest = &[];
                Some(utf8_percent_encode(text, SEGMENT).to_string())
            }
            Err(error) => {
                let valid = error.valid_up_to();
                if valid > 0 {
                    let text = std::str::from_utf8(&rest[..valid]).unwrap_or_default();
                    rest = &rest[valid..];
                    Some(utf8_percent_encode(text, SEGMENT).to_string())
                } else {
                    let byte = rest[0];
                    rest = &rest[1..];
                    Some(format!("%{byte:02X}"))
                }
            }
        }
    })
}

/// The URL of `path`. Folders end in a slash.
pub(crate) fn url(path: &RemotePath, folder: bool) -> String {
    format!("{}{}", origin(path), encode_path(path.segments(), folder))
}

/// The names in the path of an `href` (an absolute URL, or an absolute or relative path), decoded.
pub(crate) fn href_segments(href: &str) -> Vec<Vec<u8>> {
    let href = href.trim();
    let path = match href.find("://") {
        Some(at) => {
            let after = &href[at + 3..];
            after.find('/').map_or("", |slash| &after[slash..])
        }
        None => href,
    };
    let path = path.split(['?', '#']).next().unwrap_or("");
    path.split('/')
        .filter(|segment| !segment.is_empty())
        .map(|segment| percent_decode(segment.as_bytes()).collect())
        .collect()
}

/// A name as an `OsString`: bytes as they are on Unix, lossy where names are text.
pub(crate) fn os_name(bytes: &[u8]) -> OsString {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        OsString::from_vec(bytes.to_vec())
    }
    #[cfg(not(unix))]
    {
        OsString::from(String::from_utf8_lossy(bytes).into_owned())
    }
}

/// Whether the path runs through Nextcloud's (or ownCloud's) `remote.php` WebDAV endpoint.
pub(crate) fn is_nextcloud(segments: &[Vec<u8>]) -> bool {
    segments
        .windows(2)
        .any(|pair| pair[0] == b"remote.php" && (pair[1] == b"dav" || pair[1] == b"webdav"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(uri: &str) -> RemotePath {
        RemotePath::from_uri(uri).unwrap()
    }

    #[test]
    fn names_are_escaped_so_none_reads_as_syntax() {
        let p = path("davs://me@h:8443/a%20b/100%25/%23hash/caf%C3%A9/semi%3Bcolon%2Ccomma");
        assert_eq!(
            url(&p, false),
            "https://h:8443/a%20b/100%25/%23hash/caf%C3%A9/semi%3Bcolon%2Ccomma"
        );
        assert_eq!(url(&path("dav://h"), false), "http://h/");
        assert_eq!(url(&path("dav://h/x"), true), "http://h/x/");
        assert_eq!(url(&path("dav://h/x/"), false), "http://h/x");
    }

    #[test]
    fn names_that_are_not_utf8_reach_the_server_byte_for_byte() {
        let p = path("dav://h/a%FFb%C3%A9");
        assert_eq!(url(&p, false), "http://h/a%FFb%C3%A9");
    }

    #[test]
    fn ipv6_hosts_are_bracketed() {
        assert_eq!(
            url(&path("dav://[::1]:8080/x"), false),
            "http://[::1]:8080/x"
        );
    }

    #[test]
    fn hrefs_decode_whatever_shape_a_server_uses() {
        let wanted = vec![
            b"dav".to_vec(),
            b"a b".to_vec(),
            "caf\u{e9}".as_bytes().to_vec(),
        ];
        for href in [
            "/dav/a%20b/caf%C3%A9/",
            "/dav/a%20b/caf%c3%a9",
            "http://h:80/dav/a%20b/caf%C3%A9",
            "dav/a%20b/caf%C3%A9?x=1",
        ] {
            assert_eq!(href_segments(href), wanted, "{href}");
        }
        assert!(href_segments("/").is_empty());
        assert!(href_segments("http://h").is_empty());
    }

    #[test]
    fn nextcloud_paths_are_recognised() {
        let p = path("davs://h/nextcloud/remote.php/dav/files/me/x");
        assert!(is_nextcloud(p.segments()));
        assert!(is_nextcloud(path("davs://h/remote.php/webdav/").segments()));
        assert!(!is_nextcloud(path("davs://h/dav/files").segments()));
    }
}
