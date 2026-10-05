// From `smb://` locations to the UNC paths and Win32 errors of Windows' own SMB client. Pure
// functions over strings and numbers, so they are unit-tested on Linux too.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![cfg_attr(not(windows), allow(dead_code))]

use waypoint_path::{Host, RemotePath};
use waypoint_protocol::{UnreachableReason, VfsError};

use crate::failure::SmbFailure;
use crate::paths::target;
use crate::paths::Target;

/// The host as a UNC path writes it. An IPv6 address is not valid in a UNC path, so Windows
/// reads it as a name under `ipv6-literal.net`: colons become dashes and a zone's `%` an `s`.
pub(crate) fn unc_host(host: &Host) -> String {
    match host {
        Host::Name(name) => name.clone(),
        Host::Ipv4(addr) => addr.to_string(),
        Host::Ipv6 { addr, zone } => {
            let mut name = addr.to_string().replace(':', "-");
            if let Some(zone) = zone {
                name.push('s');
                name.push_str(zone);
            }
            name.push_str(".ipv6-literal.net");
            name
        }
    }
}

/// The server's own part: `\\host`.
pub(crate) fn unc_server(path: &RemotePath) -> Result<String, VfsError> {
    if path.authority().port.is_some() {
        return Err(VfsError::Unsupported {
            what: "Windows' SMB client connects to port 445 only".to_owned(),
        });
    }
    Ok(format!(r"\\{}", unc_host(&path.authority().host)))
}

/// The UNC path of a location inside a share: `\\host\share\folder\file`. The server's root (the
/// share browser) has none.
pub(crate) fn unc_path(path: &RemotePath) -> Result<String, VfsError> {
    let server = unc_server(path)?;
    match target(path)? {
        Target::Shares => Err(VfsError::Unsupported {
            what: "the list of shares is not a folder on disk".to_owned(),
        }),
        Target::Inside { share, inner } => {
            let mut out = format!(r"{server}\{share}");
            if !inner.is_empty() {
                out.push('\\');
                out.push_str(&inner.replace('/', r"\"));
            }
            Ok(out)
        }
    }
}

/// What a Win32 error from the network provider means, or `None` when it means the connection is
/// already there (`ERROR_SESSION_CREDENTIAL_CONFLICT`: this server is already connected under
/// other credentials, which Explorer shares too). `offered` says whether a password was sent.
pub(crate) fn failure_from_win32(
    code: u32,
    user: Option<String>,
    offered: bool,
) -> Option<SmbFailure> {
    Some(match code {
        1219 => return None,
        5 => SmbFailure::AccessDenied,
        // Logon failure, account restrictions, a locked or disabled account, a bad user name.
        1326 | 1327 | 1328 | 1329 | 1330 | 1331 | 1907 | 2202 => {
            if offered {
                SmbFailure::BadCredentials
            } else {
                SmbFailure::CredentialsNeeded { user }
            }
        }
        53 => SmbFailure::HostUnreachable(UnreachableReason::NameNotResolved),
        67 => SmbFailure::ShareNotFound,
        1225 => SmbFailure::HostUnreachable(UnreachableReason::Refused),
        1231 | 1232 => SmbFailure::HostUnreachable(UnreachableReason::NoRoute),
        1222 => SmbFailure::HostUnreachable(UnreachableReason::Offline),
        _ => SmbFailure::Unsupported("Windows could not connect to this server"),
    })
}

#[cfg(test)]
mod tests {
    use waypoint_path::VfsPath;

    use super::*;

    fn remote(uri: &str) -> RemotePath {
        match VfsPath::from_uri(uri).unwrap() {
            VfsPath::Remote(remote) => remote,
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_location_becomes_the_unc_path_explorer_would_open() {
        assert_eq!(
            unc_path(&remote("smb://NAS.lan/Media/a%20b/c")).unwrap(),
            r"\\nas.lan\Media\a b\c"
        );
        assert_eq!(
            unc_path(&remote("smb://nas/Media")).unwrap(),
            r"\\nas\Media"
        );
        assert_eq!(
            unc_path(&remote("smb://WORK;me@10.0.0.2/x")).unwrap(),
            r"\\10.0.0.2\x"
        );
        assert_eq!(
            unc_path(&remote("smb://[fe80::1%25eth0]/x")).unwrap(),
            r"\\fe80--1seth0.ipv6-literal.net\x"
        );
    }

    #[test]
    fn the_share_browser_is_not_a_path_and_a_port_cannot_be_chosen() {
        assert!(matches!(
            unc_path(&remote("smb://nas/")),
            Err(VfsError::Unsupported { .. })
        ));
        assert!(matches!(
            unc_path(&remote("smb://nas:4445/x")),
            Err(VfsError::Unsupported { .. })
        ));
        assert_eq!(unc_server(&remote("smb://nas/")).unwrap(), r"\\nas");
    }

    #[test]
    fn win32_errors_become_the_same_failures_as_the_library_s() {
        let user = Some("WORK;me".to_owned());
        assert_eq!(failure_from_win32(1219, None, false), None);
        assert_eq!(
            failure_from_win32(5, None, true),
            Some(SmbFailure::AccessDenied)
        );
        assert_eq!(
            failure_from_win32(1326, user.clone(), true),
            Some(SmbFailure::BadCredentials)
        );
        assert_eq!(
            failure_from_win32(1326, user.clone(), false),
            Some(SmbFailure::CredentialsNeeded { user })
        );
        assert_eq!(
            failure_from_win32(67, None, false),
            Some(SmbFailure::ShareNotFound)
        );
        assert_eq!(
            failure_from_win32(1225, None, false),
            Some(SmbFailure::HostUnreachable(UnreachableReason::Refused))
        );
    }
}
