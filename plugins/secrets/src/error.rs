// Defines the typed errors the secrets plugin reports for one action on one secret
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Why one action on a secret failed. Serialised as `{ "kind": "locked" }`, so the front end can branch on `kind`. No variant carries a secret, and `Failed` carries only a library's own description of what went wrong.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub enum SecretsError {
    /// No keyring is running: nothing owns `org.freedesktop.secrets` on the session bus, there is no session bus, or the Secret portal refused.
    #[error("no keyring is running")]
    NoKeyring,
    /// The keyring is locked and could not be unlocked.
    #[error("the keyring is locked")]
    Locked,
    /// The person dismissed the keyring's unlock prompt.
    #[error("the unlock prompt was dismissed")]
    Dismissed,
    /// This system has no keyring support in the plugin.
    #[error("not supported")]
    Unsupported,
    /// A name was empty, too long or held a NUL character; `field` says which.
    #[error("invalid {field}")]
    Invalid { field: String },
    /// Anything else the keyring reported.
    #[error("{message}")]
    Failed { message: String },
}

impl SecretsError {
    pub fn failed(message: impl Into<String>) -> Self {
        SecretsError::Failed {
            message: message.into(),
        }
    }

    fn invalid(field: &str) -> Self {
        SecretsError::Invalid {
            field: field.to_string(),
        }
    }

    /// Checks a service or account name: not empty, at most [`MAX_NAME`] bytes and no NUL character.
    pub fn check_name(field: &str, value: &str) -> Result<()> {
        if value.is_empty() || value.len() > MAX_NAME || value.contains('\0') {
            Err(SecretsError::invalid(field))
        } else {
            Ok(())
        }
    }
}

/// The longest service or account name, in bytes.
pub const MAX_NAME: usize = 1024;

pub type Result<T> = std::result::Result<T, SecretsError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_checked() {
        assert!(SecretsError::check_name("service", "sftp").is_ok());
        for bad in ["", "a\0b", &"x".repeat(MAX_NAME + 1)] {
            assert_eq!(
                SecretsError::check_name("account", bad),
                Err(SecretsError::Invalid {
                    field: "account".into()
                })
            );
        }
        assert!(SecretsError::check_name("account", &"x".repeat(MAX_NAME)).is_ok());
    }

    #[test]
    fn errors_serialise_with_a_kind() {
        let json = serde_json::to_value(SecretsError::Locked).unwrap();
        assert_eq!(json, serde_json::json!({ "kind": "locked" }));
    }
}
