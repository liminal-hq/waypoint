// Where logins come from: this session's answers in memory, then the system keyring, through one `CredentialSource`
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// A provider asks the `CredentialSource` before it fails with `AuthRequired` (A82). The source the
// app gives every provider is `Credentials`: the answers given this session (kept in memory and
// never written), then what the keyring remembers through the injected `SecretStore`. A credential
// the server refused is not offered again until the person answers anew, so a stale remembered
// password asks instead of failing in a loop.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use waypoint_path::ConnectionKey;
use waypoint_protocol::AuthPrompt;
use waypoint_vfs::{Credential, CredentialSource, Secret};

/// The keyring service saved logins are filed under; the account is the `ConnectionKey`.
pub const KEYRING_SERVICE: &str = "connection";

/// What a remembered secret is, as the keyring files it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SecretKind {
    Password,
    Passphrase,
    /// An S3 secret access key.
    AccessKey,
}

impl SecretKind {
    /// The kind a prompt asks for, when one can be remembered (a keyboard-interactive round
    /// cannot: its questions change).
    pub fn of(prompt: &AuthPrompt) -> Option<Self> {
        match prompt {
            AuthPrompt::Password { .. } => Some(Self::Password),
            AuthPrompt::Passphrase { .. } => Some(Self::Passphrase),
            AuthPrompt::AccessKey { .. } => Some(Self::AccessKey),
            AuthPrompt::Challenge { .. } => None,
        }
    }

    /// The kind of secret a credential carries, when it is one secret.
    pub fn of_credential(credential: &Credential) -> Option<(Self, Secret)> {
        match credential {
            Credential::Password { password, .. } => Some((Self::Password, password.clone())),
            Credential::Passphrase(secret) => Some((Self::Passphrase, secret.clone())),
            Credential::AccessKey { secret, .. } => Some((Self::AccessKey, secret.clone())),
            Credential::Challenge(_) => None,
        }
    }
}

/// Why the keyring cannot keep a login. The page words it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum KeyringUnavailable {
    /// No keyring is running (or this system has none).
    NoKeyring,
    /// The keyring is locked and was not unlocked.
    Locked,
    /// The keyring answered but failed.
    Failed,
}

/// The system keyring as the connections see it (A81). The app implements it over the reusable
/// `secrets` plugin; tests use `MemorySecrets`. Every call may block on the desktop's own unlock
/// prompt, so callers run them off the async runtime's threads.
pub trait SecretStore: Send + Sync {
    /// Whether a secret can be stored now, without asking to unlock anything.
    fn status(&self) -> Result<(), KeyringUnavailable>;
    fn store(
        &self,
        key: &ConnectionKey,
        kind: SecretKind,
        secret: Secret,
    ) -> Result<(), KeyringUnavailable>;
    fn fetch(
        &self,
        key: &ConnectionKey,
        kind: SecretKind,
    ) -> Result<Option<Secret>, KeyringUnavailable>;
    /// Forgets every secret of a login; returns how many there were.
    fn forget(&self, key: &ConnectionKey) -> Result<usize, KeyringUnavailable>;
}

/// A keyring in memory, for tests and for an app without one.
#[derive(Default)]
pub struct MemorySecrets {
    secrets: Mutex<HashMap<(ConnectionKey, SecretKind), Secret>>,
    unavailable: Mutex<Option<KeyringUnavailable>>,
    fetches: Mutex<usize>,
}

impl MemorySecrets {
    pub fn new() -> Self {
        Self::default()
    }

    /// Makes every call fail with `why`, or work again with `None`.
    pub fn fail_with(&self, why: Option<KeyringUnavailable>) {
        *self.unavailable.lock().unwrap_or_else(|e| e.into_inner()) = why;
    }

    pub fn len(&self) -> usize {
        self.secrets.lock().unwrap_or_else(|e| e.into_inner()).len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// How many times `fetch` reached the keyring.
    pub fn fetches(&self) -> usize {
        *self.fetches.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn check(&self) -> Result<(), KeyringUnavailable> {
        match *self.unavailable.lock().unwrap_or_else(|e| e.into_inner()) {
            Some(why) => Err(why),
            None => Ok(()),
        }
    }
}

impl SecretStore for MemorySecrets {
    fn status(&self) -> Result<(), KeyringUnavailable> {
        self.check()
    }

    fn store(
        &self,
        key: &ConnectionKey,
        kind: SecretKind,
        secret: Secret,
    ) -> Result<(), KeyringUnavailable> {
        self.check()?;
        self.secrets
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert((key.clone(), kind), secret);
        Ok(())
    }

    fn fetch(
        &self,
        key: &ConnectionKey,
        kind: SecretKind,
    ) -> Result<Option<Secret>, KeyringUnavailable> {
        *self.fetches.lock().unwrap_or_else(|e| e.into_inner()) += 1;
        self.check()?;
        Ok(self
            .secrets
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&(key.clone(), kind))
            .cloned())
    }

    fn forget(&self, key: &ConnectionKey) -> Result<usize, KeyringUnavailable> {
        self.check()?;
        let mut secrets = self.secrets.lock().unwrap_or_else(|e| e.into_inner());
        let before = secrets.len();
        secrets.retain(|(k, _), _| k != key);
        Ok(before - secrets.len())
    }
}

#[derive(Default)]
struct Cache {
    /// This session's answers. Never written anywhere.
    answers: HashMap<(ConnectionKey, SecretKind), Secret>,
    /// Logins the keyring was asked about and had nothing for, so it is not asked again.
    missing: HashSet<(ConnectionKey, SecretKind)>,
    /// Logins whose last credential the server refused: nothing is offered until a new answer.
    refused: HashSet<(ConnectionKey, SecretKind)>,
}

/// The app's `CredentialSource`: this session's answers, then the keyring.
pub struct Credentials {
    cache: Mutex<Cache>,
    keyring: Arc<dyn SecretStore>,
}

impl Credentials {
    pub fn new(keyring: Arc<dyn SecretStore>) -> Self {
        Self {
            cache: Mutex::new(Cache::default()),
            keyring,
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Cache> {
        self.cache.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Keeps the person's answer for this session (it replaces any earlier one and lifts a refusal).
    pub fn answer(&self, key: &ConnectionKey, kind: SecretKind, secret: Secret) {
        let mut cache = self.lock();
        let slot = (key.clone(), kind);
        cache.refused.remove(&slot);
        cache.missing.remove(&slot);
        cache.answers.insert(slot, secret);
    }

    /// Forgets everything this session holds for a login (its answers and what the keyring was
    /// found to lack), so the next connect asks the keyring again.
    pub fn forget_session(&self, key: &ConnectionKey) {
        let mut cache = self.lock();
        cache.answers.retain(|(k, _), _| k != key);
        cache.missing.retain(|(k, _)| k != key);
        cache.refused.retain(|(k, _)| k != key);
    }

    /// The server refused the answer of `kind`: it is dropped and nothing of that kind is offered
    /// until the person answers again.
    pub fn refuse(&self, key: &ConnectionKey, kind: SecretKind) {
        let mut cache = self.lock();
        let slot = (key.clone(), kind);
        cache.answers.remove(&slot);
        cache.refused.insert(slot);
    }

    /// Whether this session holds an answer for a login.
    pub fn has_answer(&self, key: &ConnectionKey, kind: SecretKind) -> bool {
        self.lock().answers.contains_key(&(key.clone(), kind))
    }

    pub fn keyring(&self) -> &Arc<dyn SecretStore> {
        &self.keyring
    }
}

impl CredentialSource for Credentials {
    fn credential(&self, key: &ConnectionKey, prompt: &AuthPrompt) -> Option<Credential> {
        let kind = SecretKind::of(prompt)?;
        let slot = (key.clone(), kind);
        let secret = {
            let cache = self.lock();
            if cache.refused.contains(&slot) {
                return None;
            }
            cache.answers.get(&slot).cloned()
        };
        let secret = match secret {
            Some(secret) => secret,
            None => {
                if self.lock().missing.contains(&slot) {
                    return None;
                }
                match self.keyring.fetch(key, kind) {
                    Ok(Some(secret)) => {
                        // Kept for the session, so the keyring is asked once per login.
                        self.lock().answers.insert(slot, secret.clone());
                        secret
                    }
                    Ok(None) => {
                        self.lock().missing.insert(slot);
                        return None;
                    }
                    Err(why) => {
                        log::debug!("the keyring could not be read for a login: {why:?}");
                        return None;
                    }
                }
            }
        };
        Some(match prompt {
            AuthPrompt::Password { user } => Credential::Password {
                user: user.clone(),
                password: secret,
            },
            AuthPrompt::Passphrase { .. } => Credential::Passphrase(secret),
            AuthPrompt::AccessKey { key_id } => Credential::AccessKey {
                key_id: key_id.clone()?,
                secret,
            },
            AuthPrompt::Challenge { .. } => return None,
        })
    }

    fn rejected(&self, key: &ConnectionKey, prompt: &AuthPrompt) {
        if let Some(kind) = SecretKind::of(prompt) {
            self.refuse(key, kind);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::root_of;

    fn key(text: &str) -> ConnectionKey {
        root_of(text).unwrap().connection_key()
    }

    fn password() -> AuthPrompt {
        AuthPrompt::Password {
            user: Some("me".into()),
        }
    }

    #[test]
    fn a_session_answer_comes_first_then_the_keyring_once() {
        let keyring = Arc::new(MemorySecrets::new());
        let k = key("sftp://me@nas.lan");
        keyring
            .store(&k, SecretKind::Password, Secret::from("kept"))
            .unwrap();
        let source = Credentials::new(keyring.clone());
        let Some(Credential::Password { user, password: p }) = source.credential(&k, &password())
        else {
            panic!("the keyring answers");
        };
        assert_eq!(
            (user.as_deref(), p.expose_str()),
            (Some("me"), Some("kept"))
        );
        source.credential(&k, &password()).unwrap();
        assert_eq!(keyring.fetches(), 1, "the keyring is read once per login");

        source.answer(&k, SecretKind::Password, Secret::from("typed"));
        let Some(Credential::Password { password: p, .. }) = source.credential(&k, &password())
        else {
            panic!("the answer is used");
        };
        assert_eq!(p.expose_str(), Some("typed"));
    }

    #[test]
    fn a_refused_credential_is_not_offered_until_a_new_answer() {
        let keyring = Arc::new(MemorySecrets::new());
        let k = key("sftp://me@nas.lan");
        keyring
            .store(&k, SecretKind::Password, Secret::from("stale"))
            .unwrap();
        let source = Credentials::new(keyring.clone());
        assert!(source.credential(&k, &password()).is_some());
        source.rejected(&k, &password());
        assert!(source.credential(&k, &password()).is_none());
        source.answer(&k, SecretKind::Password, Secret::from("new"));
        assert!(source.credential(&k, &password()).is_some());
    }

    #[test]
    fn nothing_in_the_keyring_is_remembered_as_nothing() {
        let keyring = Arc::new(MemorySecrets::new());
        let k = key("sftp://me@nas.lan");
        let source = Credentials::new(keyring.clone());
        assert!(source.credential(&k, &password()).is_none());
        assert!(source.credential(&k, &password()).is_none());
        assert_eq!(keyring.fetches(), 1);
        source.forget_session(&k);
        assert!(source.credential(&k, &password()).is_none());
        assert_eq!(keyring.fetches(), 2);
        // A locked keyring is no credential, not a failure.
        keyring.fail_with(Some(KeyringUnavailable::Locked));
        source.forget_session(&k);
        assert!(source.credential(&k, &password()).is_none());
    }

    #[test]
    fn keyboard_interactive_rounds_are_never_answered_from_memory() {
        let source = Credentials::new(Arc::new(MemorySecrets::new()));
        let prompt = AuthPrompt::Challenge {
            name: String::new(),
            instructions: String::new(),
            prompts: Vec::new(),
        };
        assert!(source
            .credential(&key("sftp://me@nas.lan"), &prompt)
            .is_none());
    }
}
