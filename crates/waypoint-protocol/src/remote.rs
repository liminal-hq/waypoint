// The wire types of remote locations: what a login asks for, the keys and certificates a person is
// asked to trust, why a server cannot be reached, and a connection's state.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::VfsError;

/// Why nothing answered at a server's address.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum UnreachableReason {
    /// The host name does not resolve.
    NameNotResolved,
    /// The host answered and refused the connection (nothing listens on the port).
    Refused,
    /// There is no route to the host.
    NoRoute,
    /// This computer has no network connection.
    Offline,
}

/// One question of a keyboard-interactive login (a one-time code, a second factor).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct ChallengePrompt {
    /// The server's question, shown as the field's label.
    pub text: String,
    /// Whether the answer may be shown as it is typed (a code), or is hidden (a password).
    pub echo: bool,
}

/// What a login needs from the person. It names what to ask for, never a secret.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum AuthPrompt {
    /// A password for `user` (the dialog asks for the user too when it is `None`).
    Password { user: Option<String> },
    /// The passphrase of a key file, or the password of an encrypted archive. `subject` is what
    /// it unlocks, as people read it (`~/.ssh/id_ed25519`, `photos.zip`).
    Passphrase { subject: String },
    /// A keyboard-interactive round: the server's name and instructions, and its questions.
    Challenge {
        name: String,
        instructions: String,
        prompts: Vec<ChallengePrompt>,
    },
    /// An access key id and its secret (S3).
    AccessKey { key_id: Option<String> },
}

/// An SSH server's public key, as the trust dialog shows it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct HostKey {
    /// The host and port the key was offered for (`nas.lan`, `[nas.lan]:2222`).
    pub host: String,
    /// The key type (`ssh-ed25519`, `ecdsa-sha2-nistp256`, `rsa-sha2-512`).
    pub algorithm: String,
    /// `SHA256:` and the base64 digest, as `ssh-keygen -l` prints it.
    pub fingerprint: String,
}

/// An SSH server whose key differs from the one recorded for it in `known_hosts`: the host and
/// both keys, so the warning can show the old and the new fingerprint side by side.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct HostKeyChange {
    /// The host and port the key was offered for (`nas.lan`, `[nas.lan]:2222`).
    pub host: String,
    /// The key type recorded in `known_hosts`.
    pub recorded_algorithm: String,
    /// The recorded key's `SHA256:` fingerprint.
    pub recorded_fingerprint: String,
    /// The key type the server offers now.
    pub offered_algorithm: String,
    /// The offered key's `SHA256:` fingerprint.
    pub offered_fingerprint: String,
}

/// A TLS certificate the system does not trust, as the trust dialog shows it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct Certificate {
    pub subject: String,
    pub issuer: String,
    /// The SHA-256 digest of the certificate, in colon-separated hex.
    pub fingerprint: String,
    /// Why it is not trusted (self-signed, expired, issued for another name), in words.
    pub reason: String,
}

/// The state of one connection, as the Network section and a remote tab show it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum ConnectionState {
    /// No session is open; the next use opens one.
    Idle,
    Connecting,
    Connected,
    /// The last attempt failed with this error.
    Failed {
        error: VfsError,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompts_and_states_carry_a_kind_tag() {
        let json = serde_json::to_string(&AuthPrompt::AccessKey { key_id: None }).unwrap();
        assert_eq!(json, r#"{"kind":"accessKey","keyId":null}"#);
        let json = serde_json::to_string(&ConnectionState::Connected).unwrap();
        assert_eq!(json, r#"{"kind":"connected"}"#);
        let json = serde_json::to_string(&UnreachableReason::NameNotResolved).unwrap();
        assert_eq!(json, r#""nameNotResolved""#);
    }
}
