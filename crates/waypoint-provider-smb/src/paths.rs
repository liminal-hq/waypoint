// From `smb://` locations to what a server reads: the host and port to dial, who logs in, the share
// and the path inside it.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

// Without an engine (no `client` feature and not Windows) nothing reads locations.
#![cfg_attr(not(any(windows, feature = "client")), allow(dead_code))]

#[cfg(all(feature = "client", not(windows)))]
use waypoint_path::Host;
use waypoint_path::{RemotePath, RemoteScheme, VfsPath};
use waypoint_protocol::VfsError;

/// The SMB location `path` is, or `Unsupported` for any other scheme.
pub(crate) fn remote(path: &VfsPath) -> Result<&RemotePath, VfsError> {
    match path {
        VfsPath::Remote(remote) if remote.scheme() == RemoteScheme::Smb => Ok(remote),
        _ => Err(VfsError::Unsupported {
            what: format!("{} over SMB", path.display()),
        }),
    }
}

/// What an SMB location points at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Target {
    /// `smb://host/`: the share browser, a folder of the server's shares.
    Shares,
    /// A share, or something inside it. `inner` uses `/` between names and is empty at the share's
    /// root.
    Inside { share: String, inner: String },
}

fn text(segment: &[u8]) -> Result<&str, VfsError> {
    std::str::from_utf8(segment).map_err(|_| VfsError::InvalidName {
        name: String::from_utf8_lossy(segment).into_owned(),
        reason: "SMB names that are not UTF-8 are not supported".to_owned(),
    })
}

/// Splits a location into its share and the path inside it. The library speaks UTF-8 paths, so a
/// name that is not UTF-8 cannot be addressed and is `InvalidName`.
pub(crate) fn target(path: &RemotePath) -> Result<Target, VfsError> {
    let Some((share, rest)) = path.segments().split_first() else {
        return Ok(Target::Shares);
    };
    let inner = rest
        .iter()
        .map(|segment| text(segment))
        .collect::<Result<Vec<_>, _>>()?
        .join("/");
    Ok(Target::Inside {
        share: text(share)?.to_owned(),
        inner,
    })
}

/// A host as a socket address reads it.
#[cfg(all(feature = "client", not(windows)))]
pub(crate) fn host_name(host: &Host) -> String {
    match host {
        Host::Name(name) => name.clone(),
        Host::Ipv4(addr) => addr.to_string(),
        Host::Ipv6 { addr, zone: None } => format!("[{addr}]"),
        Host::Ipv6 {
            addr,
            zone: Some(zone),
        } => format!("[{addr}%{zone}]"),
    }
}

/// The `host:port` to dial.
#[cfg(all(feature = "client", not(windows)))]
pub(crate) fn socket_address(path: &RemotePath) -> String {
    let authority = path.authority();
    let port = authority
        .port
        .or_else(|| path.scheme().default_port())
        .unwrap_or(445);
    format!("{}:{port}", host_name(&authority.host))
}

/// Who logs in: the domain (empty for a local account) and the user name, from `domain;user`,
/// `domain\user` or a plain `user`.
pub(crate) fn split_login(user: &str) -> (String, String) {
    match user.split_once(';').or_else(|| user.split_once('\\')) {
        Some((domain, name)) => (domain.to_owned(), name.to_owned()),
        None => (String::new(), user.to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn remote_of(uri: &str) -> RemotePath {
        match VfsPath::from_uri(uri).unwrap() {
            VfsPath::Remote(remote) => remote,
            other => panic!("not remote: {other:?}"),
        }
    }

    #[test]
    fn a_location_is_the_share_browser_a_share_or_a_path_inside_one() {
        assert_eq!(target(&remote_of("smb://nas/")).unwrap(), Target::Shares);
        assert_eq!(
            target(&remote_of("smb://nas/Media")).unwrap(),
            Target::Inside {
                share: "Media".into(),
                inner: String::new()
            }
        );
        assert_eq!(
            target(&remote_of("smb://nas/Media/a%20b/caf%C3%A9")).unwrap(),
            Target::Inside {
                share: "Media".into(),
                inner: "a b/café".into()
            }
        );
        assert!(matches!(
            target(&remote_of("smb://nas/Media/%FF")),
            Err(VfsError::InvalidName { .. })
        ));
    }

    #[cfg(all(feature = "client", not(windows)))]
    #[test]
    fn the_address_is_the_host_and_the_port() {
        assert_eq!(socket_address(&remote_of("smb://NAS.lan/x")), "nas.lan:445");
        assert_eq!(
            socket_address(&remote_of("smb://me@10.0.0.2:4445/x")),
            "10.0.0.2:4445"
        );
        assert_eq!(socket_address(&remote_of("smb://[::1]/x")), "[::1]:445");
    }

    #[test]
    fn a_login_splits_into_domain_and_user() {
        assert_eq!(split_login("WORK;me"), ("WORK".into(), "me".into()));
        assert_eq!(split_login("WORK\\me"), ("WORK".into(), "me".into()));
        assert_eq!(split_login("me"), (String::new(), "me".into()));
    }

    #[test]
    fn other_schemes_are_not_smb() {
        let path = VfsPath::from_uri("sftp://h/x").unwrap();
        assert!(matches!(remote(&path), Err(VfsError::Unsupported { .. })));
    }
}
