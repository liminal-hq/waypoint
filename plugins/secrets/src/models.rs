// Defines the models of the secrets plugin: what a secret is called, the secret itself and the status
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use zeroize::Zeroizing;

/// What sort of secret this is. It is part of the identity of a secret, so one account can hold a password and the passphrase of its key side by side.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub enum SecretKind {
    /// A login password.
    Password,
    /// The passphrase of a key file or of an encrypted volume.
    Passphrase,
    /// An access token or an API secret.
    Token,
    /// A secret key (an S3 secret access key, say).
    Key,
}

impl SecretKind {
    /// The value stored in the keyring's `kind` attribute.
    pub fn as_str(self) -> &'static str {
        match self {
            SecretKind::Password => "password",
            SecretKind::Passphrase => "passphrase",
            SecretKind::Token => "token",
            SecretKind::Key => "key",
        }
    }
}

/// Names one secret: what it is for (`service`: `sftp`, `volume`), whose it is (`account`: a connection or a device) and its kind. Names are not secret, but they are not logged either: an account can be a user name.
#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct SecretId {
    pub service: String,
    pub account: String,
    pub kind: SecretKind,
}

impl SecretId {
    pub fn new(service: impl Into<String>, account: impl Into<String>, kind: SecretKind) -> Self {
        SecretId {
            service: service.into(),
            account: account.into(),
            kind,
        }
    }

    /// Checks both names.
    pub fn validate(&self) -> crate::error::Result<()> {
        crate::error::SecretsError::check_name("service", &self.service)?;
        crate::error::SecretsError::check_name("account", &self.account)
    }
}

impl std::fmt::Debug for SecretId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "SecretId({}, …, {})",
            self.service,
            self.kind.as_str()
        )
    }
}

/// A secret value. Its `Debug` is redacted, it has no `Display` and no `Serialize`, and its buffer is zeroed when it is dropped (so is every clone's). Text and bytes are both allowed.
#[derive(Clone, PartialEq, Eq)]
pub struct Secret(Zeroizing<Vec<u8>>);

impl Secret {
    pub fn from_text(text: String) -> Self {
        Secret(Zeroizing::new(text.into_bytes()))
    }

    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        Secret(Zeroizing::new(bytes))
    }

    /// The bytes, for handing to the keyring or to the program that asked for them.
    pub fn expose_bytes(&self) -> &[u8] {
        &self.0
    }

    /// The text, when the secret is valid UTF-8.
    pub fn expose_text(&self) -> Option<&str> {
        std::str::from_utf8(&self.0).ok()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Secret(…)")
    }
}

/// Which implementation is behind the plugin on this system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub enum Flavour {
    /// The Secret Service on the session bus (GNOME Keyring, KWallet's bridge, KeePassXC).
    SecretService,
    /// The Secret portal, in a Flatpak: the secrets live in an encrypted file the portal unlocks.
    SecretPortal,
    /// Windows Credential Manager, the signed-in user's vault.
    CredentialManager,
    /// Secrets held in memory only, for tests.
    Memory,
    /// No keyring support on this system.
    Unsupported,
}

/// Why a feature is unavailable. A code the front end can branch on; `message` beside it is a sentence for people.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub enum Reason {
    /// No keyring is running (or there is no session bus, or the portal is missing).
    NoKeyring,
    /// The keyring is locked and the person did not unlock it.
    Locked,
    /// The keyring answered but failed.
    Failed,
    /// This operating system has no keyring support in the plugin.
    UnsupportedPlatform,
}

pub const FEATURE_STORE: &str = "store";
pub const FEATURE_FETCH: &str = "fetch";
pub const FEATURE_DELETE: &str = "delete";

/// Every feature, in the order `get_status` lists them.
pub const FEATURES: [&str; 3] = [FEATURE_STORE, FEATURE_FETCH, FEATURE_DELETE];

/// Whether one feature of the plugin works on this system.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct FeatureStatus {
    /// One of `store`, `fetch` or `delete`.
    pub name: String,
    pub available: bool,
    /// Why the feature is unavailable; absent when it works.
    pub reason: Option<Reason>,
    /// A sentence that explains the reason; absent when the feature works.
    pub message: Option<String>,
}

impl FeatureStatus {
    pub fn available(name: &str) -> Self {
        FeatureStatus {
            name: name.to_string(),
            available: true,
            reason: None,
            message: None,
        }
    }

    pub fn unavailable(name: &str, reason: Reason, message: impl Into<String>) -> Self {
        FeatureStatus {
            name: name.to_string(),
            available: false,
            reason: Some(reason),
            message: Some(message.into()),
        }
    }
}

/// What the plugin can do on the running system.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../guest-js/bindings/")]
pub struct PluginStatus {
    /// True when at least one feature is available.
    pub available: bool,
    /// Why nothing is available; absent when everything works.
    pub reason: Option<Reason>,
    pub message: Option<String>,
    pub flavour: Flavour,
    pub features: Vec<FeatureStatus>,
}

impl PluginStatus {
    /// A status in which every feature works.
    pub fn all_available(flavour: Flavour) -> Self {
        PluginStatus {
            available: true,
            reason: None,
            message: None,
            flavour,
            features: FEATURES
                .iter()
                .map(|name| FeatureStatus::available(name))
                .collect(),
        }
    }

    /// A status in which every feature is unavailable for the same reason.
    pub fn all_unavailable(flavour: Flavour, reason: Reason, message: &str) -> Self {
        PluginStatus {
            available: false,
            reason: Some(reason),
            message: Some(message.to_string()),
            flavour,
            features: FEATURES
                .iter()
                .map(|name| FeatureStatus::unavailable(name, reason, message))
                .collect(),
        }
    }

    /// Whether the named feature is available.
    pub fn has(&self, name: &str) -> bool {
        self.features
            .iter()
            .any(|feature| feature.name == name && feature.available)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_secret_never_shows_in_debug_output() {
        let secret = Secret::from_text("hunter2".into());
        assert_eq!(format!("{secret:?}"), "Secret(…)");
        assert_eq!(format!("{:#?}", Some(&secret)), "Some(\n    Secret(…),\n)");
        assert_eq!(secret.expose_text(), Some("hunter2"));
    }

    #[test]
    fn an_id_hides_its_account_in_debug_output() {
        let id = SecretId::new("sftp", "alice@example.org", SecretKind::Password);
        let shown = format!("{id:?}");
        assert!(!shown.contains("alice"));
        assert!(shown.contains("sftp") && shown.contains("password"));
    }

    #[test]
    fn bytes_that_are_not_text_have_no_text() {
        let secret = Secret::from_bytes(vec![0xff, 0xfe]);
        assert_eq!(secret.expose_text(), None);
        assert_eq!(secret.len(), 2);
    }

    #[test]
    fn the_kinds_have_stable_attribute_values() {
        let all = [
            SecretKind::Password,
            SecretKind::Passphrase,
            SecretKind::Token,
            SecretKind::Key,
        ];
        let names: Vec<_> = all.iter().map(|kind| kind.as_str()).collect();
        assert_eq!(names, ["password", "passphrase", "token", "key"]);
    }

    #[test]
    fn an_unavailable_status_gives_every_feature_the_reason() {
        let status = PluginStatus::all_unavailable(Flavour::Unsupported, Reason::NoKeyring, "x");
        assert!(!status.available);
        assert!(status
            .features
            .iter()
            .all(|f| !f.available && f.reason == Some(Reason::NoKeyring)));
        assert!(!status.has(FEATURE_STORE));
        assert!(PluginStatus::all_available(Flavour::Memory).has(FEATURE_FETCH));
    }
}
