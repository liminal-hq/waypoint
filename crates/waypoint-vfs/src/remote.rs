// What a remote provider is given to log in with: secrets that never print, credentials, the
// person's answers to a connection's questions, and the source the app supplies credentials from.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::fmt;

use waypoint_path::ConnectionKey;
use waypoint_protocol::AuthPrompt;

/// A password, passphrase or secret key (A82). It cannot be printed (`Debug` is redacted), is not
/// `Serialize`, and its bytes are overwritten when it is dropped, so it never reaches a log, an
/// error, an event or a file by accident.
#[derive(Clone, PartialEq, Eq)]
pub struct Secret(Vec<u8>);

impl Secret {
    pub fn new(bytes: impl Into<Vec<u8>>) -> Self {
        Self(bytes.into())
    }

    /// The secret, for the one call that hands it to a server or a keyring.
    pub fn expose(&self) -> &[u8] {
        &self.0
    }

    /// The secret as text, when it is valid UTF-8.
    pub fn expose_str(&self) -> Option<&str> {
        std::str::from_utf8(&self.0).ok()
    }
}

impl From<&str> for Secret {
    fn from(text: &str) -> Self {
        Self::new(text.as_bytes())
    }
}

impl From<String> for Secret {
    fn from(text: String) -> Self {
        Self::new(text.into_bytes())
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(<redacted>)")
    }
}

impl Drop for Secret {
    fn drop(&mut self) {
        for byte in self.0.iter_mut() {
            // A volatile write, so the clearing is not optimised away as a dead store.
            // SAFETY: `byte` is a valid, aligned, exclusive reference into the vector.
            unsafe { std::ptr::write_volatile(byte, 0) };
        }
        std::sync::atomic::compiler_fence(std::sync::atomic::Ordering::SeqCst);
    }
}

/// What a login is answered with. Each kind answers the `AuthPrompt` of the same name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Credential {
    Password {
        user: Option<String>,
        password: Secret,
    },
    /// A key file's passphrase, or an encrypted archive's password.
    Passphrase(Secret),
    /// The answers to a keyboard-interactive round, in the order of its prompts.
    Challenge(Vec<Secret>),
    /// An access key id and its secret (S3).
    AccessKey { key_id: String, secret: Secret },
}

/// The person's answer to the question a connection's last attempt asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectAnswer {
    /// A credential for an `AuthRequired`.
    Credential(Credential),
    /// Trust an unknown SSH host key, named by its fingerprint so an answer cannot be applied to a
    /// key the person was not shown. `remember` adds it to `known_hosts`. It never answers a
    /// changed key.
    TrustHostKey { fingerprint: String, remember: bool },
    /// The explicit "Trust the new key" action on a changed SSH host key (D148), naming both the
    /// recorded and the offered fingerprint the warning showed. It replaces the recorded entry in
    /// `known_hosts`; nothing else ever does.
    TrustChangedHostKey {
        recorded_fingerprint: String,
        offered_fingerprint: String,
    },
    /// Trust a TLS certificate, named by its fingerprint. `remember` pins it in the saved
    /// connection; otherwise it is trusted for this session.
    TrustCertificate { fingerprint: String, remember: bool },
}

/// Where a provider finds a credential it does not have (A82). The app implements it over a cache
/// of this session's logins and the system keyring; a provider asks before it fails with
/// `AuthRequired`, so a remembered login connects without a question.
pub trait CredentialSource: Send + Sync {
    /// The credential for `key` that answers `prompt`, or `None` when there is none.
    fn credential(&self, key: &ConnectionKey, prompt: &AuthPrompt) -> Option<Credential>;

    /// The server refused what `credential` returned for `key`, so it should not be offered again
    /// without asking. Default: nothing to forget.
    fn rejected(&self, key: &ConnectionKey, prompt: &AuthPrompt) {
        let _ = (key, prompt);
    }
}

/// A source that never has a credential: every login that needs one asks.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoCredentials;

impl CredentialSource for NoCredentials {
    fn credential(&self, _: &ConnectionKey, _: &AuthPrompt) -> Option<Credential> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_secret_never_prints() {
        let secret = Secret::from("hunter2");
        assert_eq!(format!("{secret:?}"), "Secret(<redacted>)");
        let credential = Credential::Password {
            user: Some("me".into()),
            password: secret.clone(),
        };
        assert!(!format!("{credential:?}").contains("hunter2"));
        let answer = ConnectAnswer::Credential(Credential::AccessKey {
            key_id: "AKIA".into(),
            secret: Secret::from("wJalr"),
        });
        assert!(!format!("{answer:?}").contains("wJalr"));
        assert_eq!(secret.expose_str(), Some("hunter2"));
    }
}
