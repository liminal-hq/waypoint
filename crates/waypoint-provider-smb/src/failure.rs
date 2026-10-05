// What goes wrong with an SMB server, as the states the UI tells apart, and how each reaches the
// page through the contract's typed errors.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use waypoint_protocol::{AuthPrompt, Location, UnreachableReason, VfsError};

/// The ways an SMB server turns a person away or cannot be used, before they become a `VfsError`.
/// A provider names the failure where it knows the cause (a refused login, a share that is not
/// there) and `into_error` words it for the page, so a server's own message never travels.
///
/// The contract has no variant of its own for signing, encryption or the protocol version, so those
/// become `AuthRequired` (a guest session cannot sign) or `Unsupported` with the reason in
/// `what`, which the Connect dialog shows as it is (A99).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SmbFailure {
    /// The login is fine and the account may not do this (`STATUS_ACCESS_DENIED`).
    AccessDenied,
    /// The server wants a login and none was offered.
    CredentialsNeeded { user: Option<String> },
    /// The server refused the user name and password (a wrong password, a disabled or locked
    /// account, an expired password).
    BadCredentials,
    /// The server has no such share.
    ShareNotFound,
    /// Nothing answered at the address.
    HostUnreachable(UnreachableReason),
    /// The server requires signed messages, which a guest session cannot have: a real login is
    /// needed.
    SigningRequired { user: Option<String> },
    /// The server requires a protection (encryption, or signing for this account) that this
    /// connection could not set up, so it refuses the session of a valid account.
    EncryptionRequired,
    /// The server and Waypoint agree on no SMB dialect: Waypoint speaks SMB 2.0.2 and newer, SMB 1
    /// is not used, and a server that cannot answer in SMB 2 closes the connection.
    NoCommonDialect,
    /// The login needs a method the library does not have.
    Unsupported(&'static str),
}

impl SmbFailure {
    pub fn into_error(self, location: &Location) -> VfsError {
        let location = location.clone();
        let password = |user| VfsError::AuthRequired {
            location: location.clone(),
            prompt: Box::new(AuthPrompt::Password { user }),
        };
        match self {
            SmbFailure::AccessDenied => VfsError::PermissionDenied { location },
            SmbFailure::CredentialsNeeded { user } | SmbFailure::SigningRequired { user } => {
                password(user)
            }
            SmbFailure::BadCredentials => VfsError::AuthFailed { location },
            SmbFailure::ShareNotFound => VfsError::NotFound { location },
            SmbFailure::HostUnreachable(reason) => VfsError::Unreachable { location, reason },
            SmbFailure::EncryptionRequired => VfsError::Unsupported {
                what: "this server requires encryption or signing that could not be set up with it"
                    .to_owned(),
            },
            SmbFailure::NoCommonDialect => VfsError::Unsupported {
                what: "this server does not agree on an SMB version: Waypoint speaks SMB 2 and 3, \
                       so a server that only speaks SMB 1 cannot be used"
                    .to_owned(),
            },
            SmbFailure::Unsupported(what) => VfsError::Unsupported {
                what: what.to_owned(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_failure_is_a_distinct_typed_error_and_none_carries_a_secret() {
        let at = Location::new("smb://nas/Share", "smb://nas/Share");
        let kinds: Vec<(SmbFailure, &str)> = vec![
            (SmbFailure::AccessDenied, "permissionDenied"),
            (
                SmbFailure::CredentialsNeeded {
                    user: Some("WORK;me".into()),
                },
                "authRequired",
            ),
            (SmbFailure::BadCredentials, "authFailed"),
            (SmbFailure::ShareNotFound, "notFound"),
            (
                SmbFailure::HostUnreachable(UnreachableReason::Refused),
                "unreachable",
            ),
            (SmbFailure::SigningRequired { user: None }, "authRequired"),
            (SmbFailure::EncryptionRequired, "unsupported"),
            (SmbFailure::NoCommonDialect, "unsupported"),
            (SmbFailure::Unsupported("Kerberos"), "unsupported"),
        ];
        for (failure, kind) in kinds {
            let error = failure.clone().into_error(&at);
            let value = serde_json::to_value(&error).unwrap();
            assert_eq!(value["kind"], kind, "{failure:?}");
        }
    }

    #[test]
    fn the_dialect_and_encryption_reasons_are_worded_for_people() {
        let at = Location::new("smb://nas/", "smb://nas/");
        let VfsError::Unsupported { what } = SmbFailure::NoCommonDialect.into_error(&at) else {
            panic!()
        };
        assert!(what.contains("SMB 2"));
        assert!(what.contains("SMB 1"));
        let VfsError::Unsupported { what } = SmbFailure::EncryptionRequired.into_error(&at) else {
            panic!()
        };
        assert!(what.contains("encryption"));
    }
}
