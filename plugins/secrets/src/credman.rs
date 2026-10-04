// The parts of the Windows backend that do not need Windows: credential target names, the blob size limit and the mapping of Win32 error codes
//
// Compiled everywhere so its tests run on every platform; only `windows.rs` uses it outside them.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![cfg_attr(not(target_os = "windows"), allow(dead_code))]

use crate::error::SecretsError;
use crate::models::SecretKind;

/// The most bytes Credential Manager keeps in one credential (`CRED_MAX_CREDENTIAL_BLOB_SIZE`).
pub const MAX_BLOB: usize = 2560;

/// The longest target name Credential Manager accepts, in UTF-16 units (`CRED_MAX_GENERIC_TARGET_NAME_LENGTH`).
pub const MAX_TARGET_UNITS: usize = 32767;

pub const ERROR_NOT_FOUND: u32 = 1168;
const ERROR_INVALID_PARAMETER: u32 = 87;
const ERROR_BAD_USERNAME: u32 = 2202;
const ERROR_NO_SUCH_LOGON_SESSION: u32 = 1312;
const ERROR_INVALID_FLAGS: u32 = 1004;

/// Escapes what would make two different names collide in a target name (`/` separates the parts) or act as a wildcard in an enumeration filter (`*`), so `a/b` and `a`, `b` stay apart.
fn escape(part: &str) -> String {
    let mut escaped = String::with_capacity(part.len());
    for character in part.chars() {
        match character {
            '%' => escaped.push_str("%25"),
            '/' => escaped.push_str("%2F"),
            '*' => escaped.push_str("%2A"),
            other => escaped.push(other),
        }
    }
    escaped
}

/// The target name of a secret: `namespace/service/account/kind`, shown by Windows Credentials as is, so the namespace (the app identifier) says whose it is.
pub fn target_name(namespace: &str, service: &str, account: &str, kind: SecretKind) -> String {
    format!(
        "{}{}",
        account_prefix(namespace, service, account),
        kind.as_str()
    )
}

/// The part every kind of one account shares, ending in `/`: also the filter (with `*` after it) for deleting the account.
pub fn account_prefix(namespace: &str, service: &str, account: &str) -> String {
    format!(
        "{}/{}/{}/",
        escape(namespace),
        escape(service),
        escape(account)
    )
}

/// The filter `CredEnumerateW` takes to list every credential of an account.
pub fn account_filter(namespace: &str, service: &str, account: &str) -> String {
    format!("{}*", account_prefix(namespace, service, account))
}

/// Refuses a secret or a target name Credential Manager cannot hold, before asking it.
pub fn check_limits(target: &str, secret_len: usize) -> Result<(), SecretsError> {
    if secret_len > MAX_BLOB {
        return Err(SecretsError::Invalid {
            field: "secret (longer than Credential Manager keeps)".into(),
        });
    }
    if target.encode_utf16().count() > MAX_TARGET_UNITS {
        return Err(SecretsError::Invalid {
            field: "service or account (too long)".into(),
        });
    }
    Ok(())
}

/// The Win32 error inside an `HRESULT` that wraps one (`HRESULT_FROM_WIN32`), or the code itself when it is not wrapped.
pub fn win32_code(hresult: i32) -> u32 {
    let value = hresult as u32;
    if (value >> 16) & 0x1FFF == 7 {
        value & 0xFFFF
    } else {
        value
    }
}

/// Maps a Win32 error code from Credential Manager to a typed error. `ERROR_NOT_FOUND` is not an error to the plugin (a missing secret is `None`), so callers test for it first.
pub fn classify(code: u32) -> SecretsError {
    match code {
        ERROR_NO_SUCH_LOGON_SESSION => SecretsError::NoKeyring,
        ERROR_INVALID_PARAMETER | ERROR_BAD_USERNAME | ERROR_INVALID_FLAGS => {
            SecretsError::Invalid {
                field: "credential".into(),
            }
        }
        other => SecretsError::failed(format!("Credential Manager failed (Win32 error {other})")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_target_name_is_namespaced_and_kinds_are_apart() {
        let password = target_name("ca.liminalhq.app", "sftp", "home", SecretKind::Password);
        let passphrase = target_name("ca.liminalhq.app", "sftp", "home", SecretKind::Passphrase);
        assert_eq!(password, "ca.liminalhq.app/sftp/home/password");
        assert_ne!(password, passphrase);
        assert!(password.starts_with(&account_prefix("ca.liminalhq.app", "sftp", "home")));
    }

    #[test]
    fn separators_and_wildcards_in_names_cannot_make_names_collide() {
        let a = target_name("n", "a/b", "c", SecretKind::Key);
        let b = target_name("n", "a", "b/c", SecretKind::Key);
        assert_ne!(a, b);
        assert_eq!(a, "n/a%2Fb/c/key");
        assert_eq!(account_filter("n", "s", "x*y"), "n/s/x%2Ay/*");
        assert_ne!(escape("%2F"), escape("/"));
    }

    #[test]
    fn an_account_filter_matches_every_kind_of_that_account_only() {
        let filter = account_filter("n", "s", "a");
        let prefix = filter.trim_end_matches('*');
        for kind in [SecretKind::Password, SecretKind::Token] {
            assert!(target_name("n", "s", "a", kind).starts_with(prefix));
        }
        assert!(!target_name("n", "s", "ab", SecretKind::Password).starts_with(prefix));
    }

    #[test]
    fn limits_are_checked_before_asking_windows() {
        assert!(check_limits("n/s/a/password", MAX_BLOB).is_ok());
        assert!(matches!(
            check_limits("n/s/a/password", MAX_BLOB + 1),
            Err(SecretsError::Invalid { .. })
        ));
        assert!(matches!(
            check_limits(&"x".repeat(MAX_TARGET_UNITS + 1), 1),
            Err(SecretsError::Invalid { .. })
        ));
    }

    #[test]
    fn win32_errors_unwrap_from_an_hresult_and_map_to_typed_errors() {
        assert_eq!(win32_code(0x8007_0490_u32 as i32), 1168);
        assert_eq!(win32_code(1168), 1168);
        assert_eq!(classify(1312), SecretsError::NoKeyring);
        assert!(matches!(classify(87), SecretsError::Invalid { .. }));
        match classify(5) {
            SecretsError::Failed { message } => assert!(message.contains("Win32 error 5")),
            other => panic!("{other:?}"),
        }
    }
}
