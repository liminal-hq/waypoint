// The SSH client's callbacks: the server's host key is checked here against the known hosts, the
// person's answer and the keys trusted for this session.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use russh::keys::{PublicKey, PublicKeyOrCertificate};
use waypoint_protocol::{Location, VfsError};
use waypoint_vfs::ConnectAnswer;

use crate::host_keys::{host_label, HostKeyCheck, KnownHosts, ServerKey};

/// Keys trusted for this run of the app only: accepted without "remember", or accepted when the
/// known-hosts file could not be written. Keyed by `known_hosts` label and fingerprint.
pub(crate) type SessionTrust = Mutex<HashSet<(String, String)>>;

/// Why a server's key was refused.
#[derive(Debug, Clone)]
enum Verdict {
    Unknown(ServerKey),
    Changed {
        recorded: ServerKey,
        offered: ServerKey,
    },
    Revoked(ServerKey),
}

/// The check of one server's key during one handshake.
pub(crate) struct HostCheck {
    known_hosts: Arc<dyn KnownHosts>,
    host: String,
    port: u16,
    answer: Option<ConnectAnswer>,
    session_trust: Arc<SessionTrust>,
    verdict: Mutex<Option<Verdict>>,
}

impl HostCheck {
    pub(crate) fn new(
        known_hosts: Arc<dyn KnownHosts>,
        host: &str,
        port: u16,
        answer: Option<&ConnectAnswer>,
        session_trust: Arc<SessionTrust>,
    ) -> Self {
        let answer = answer.filter(|answer| {
            matches!(
                answer,
                ConnectAnswer::TrustHostKey { .. } | ConnectAnswer::TrustChangedHostKey { .. }
            )
        });
        Self {
            known_hosts,
            host: host.to_owned(),
            port,
            answer: answer.cloned(),
            session_trust,
            verdict: Mutex::new(None),
        }
    }

    fn trusted_this_session(&self, key: &ServerKey) -> bool {
        let label = host_label(&self.host, self.port);
        lock(&self.session_trust).contains(&(label, key.fingerprint().to_owned()))
    }

    fn trust_this_session(&self, key: &ServerKey) {
        let label = host_label(&self.host, self.port);
        lock(&self.session_trust).insert((label, key.fingerprint().to_owned()));
    }

    /// Whether to go on with a server that offered `key`.
    pub(crate) fn decide(&self, key: &ServerKey) -> bool {
        let refuse = |verdict| {
            *lock(&self.verdict) = Some(verdict);
            false
        };
        match self.known_hosts.check(&self.host, self.port, key) {
            HostKeyCheck::Known => true,
            HostKeyCheck::Revoked => refuse(Verdict::Revoked(key.clone())),
            HostKeyCheck::Unknown => match &self.answer {
                Some(ConnectAnswer::TrustHostKey {
                    fingerprint,
                    remember,
                }) if fingerprint == key.fingerprint() => {
                    if !*remember {
                        self.trust_this_session(key);
                    } else if let Err(error) = self.known_hosts.remember(&self.host, self.port, key)
                    {
                        log::warn!("sftp: could not record a trusted host key: {error}");
                        self.trust_this_session(key);
                    }
                    true
                }
                _ if self.trusted_this_session(key) => true,
                _ => refuse(Verdict::Unknown(key.clone())),
            },
            HostKeyCheck::Changed { recorded } => {
                if self.trusted_this_session(key) {
                    return true;
                }
                match &self.answer {
                    Some(ConnectAnswer::TrustChangedHostKey {
                        recorded_fingerprint,
                        offered_fingerprint,
                    }) if recorded_fingerprint == recorded.fingerprint()
                        && offered_fingerprint == key.fingerprint() =>
                    {
                        if let Err(error) = self
                            .known_hosts
                            .replace(&self.host, self.port, &recorded, key)
                        {
                            log::warn!("sftp: could not replace a changed host key: {error}");
                            self.trust_this_session(key);
                        }
                        true
                    }
                    _ => refuse(Verdict::Changed {
                        recorded,
                        offered: key.clone(),
                    }),
                }
            }
        }
    }

    /// The error that says why the last handshake refused the server's key, if it did.
    pub(crate) fn refusal(&self, location: &Location) -> Option<VfsError> {
        let verdict = lock(&self.verdict).clone()?;
        let location = location.clone();
        Some(match verdict {
            Verdict::Unknown(key) => VfsError::HostKeyUnknown {
                location,
                key: Box::new(key.to_wire(&self.host, self.port)),
            },
            Verdict::Changed { recorded, offered } => VfsError::HostKeyChanged {
                location,
                change: Box::new(offered.to_change(&recorded, &self.host, self.port)),
            },
            Verdict::Revoked(key) => VfsError::HostKeyChanged {
                location,
                change: Box::new(key.to_change(&key, &self.host, self.port)),
            },
        })
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

/// The `russh` client handler of one SSH session.
pub(crate) struct Client {
    pub(crate) check: Arc<HostCheck>,
}

impl russh::client::Handler for Client {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &PublicKeyOrCertificate,
    ) -> Result<bool, Self::Error> {
        let key = match server_public_key {
            PublicKeyOrCertificate::PublicKey { key, .. } => key.clone(),
            // A host certificate is checked as its plain key: `@cert-authority` lines are not
            // read, so a certified host is trusted like any other.
            PublicKeyOrCertificate::Certificate(certificate) => {
                PublicKey::new(certificate.public_key().clone(), "")
            }
        };
        Ok(self.check.decide(&ServerKey::from_public(&key)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host_keys::MemoryKnownHosts;

    const OLD: &str =
        "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIBCb8PDBXvzERLPgkKWxNbNUSsz4pysfnT/WkQUYHuQH";
    const NEW: &str =
        "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIH8a5MQ6ySq4iqqSL2z5fjBOnvjOZ2f6VHcrgaahwM7P";

    fn check(store: &Arc<MemoryKnownHosts>, answer: Option<ConnectAnswer>) -> HostCheck {
        HostCheck::new(
            store.clone(),
            "nas.lan",
            22,
            answer.as_ref(),
            Arc::new(SessionTrust::default()),
        )
    }

    #[test]
    fn an_unknown_key_is_refused_until_its_fingerprint_is_trusted() {
        let store = Arc::new(MemoryKnownHosts::new());
        let key = ServerKey::from_openssh(OLD).unwrap();
        let at = Location::new("sftp://nas.lan/", "sftp://nas.lan/");
        let plain = check(&store, None);
        assert!(!plain.decide(&key));
        assert!(matches!(
            plain.refusal(&at),
            Some(VfsError::HostKeyUnknown { .. })
        ));
        let wrong = check(
            &store,
            Some(ConnectAnswer::TrustHostKey {
                fingerprint: "SHA256:else".into(),
                remember: true,
            }),
        );
        assert!(!wrong.decide(&key));
        let trusted = check(
            &store,
            Some(ConnectAnswer::TrustHostKey {
                fingerprint: key.fingerprint().into(),
                remember: true,
            }),
        );
        assert!(trusted.decide(&key));
        assert_eq!(store.check("nas.lan", 22, &key), HostKeyCheck::Known);
    }

    #[test]
    fn a_changed_key_needs_the_explicit_answer_naming_both_keys() {
        let store = Arc::new(MemoryKnownHosts::new());
        let old = ServerKey::from_openssh(OLD).unwrap();
        let new = ServerKey::from_openssh(NEW).unwrap();
        store.remember("nas.lan", 22, &old).unwrap();
        let at = Location::new("sftp://nas.lan/", "sftp://nas.lan/");
        let plain_trust = check(
            &store,
            Some(ConnectAnswer::TrustHostKey {
                fingerprint: new.fingerprint().into(),
                remember: true,
            }),
        );
        assert!(!plain_trust.decide(&new));
        let Some(VfsError::HostKeyChanged { change, .. }) = plain_trust.refusal(&at) else {
            panic!("a changed key is reported with both keys");
        };
        assert_eq!(change.recorded_fingerprint, old.fingerprint());
        assert_eq!(change.offered_fingerprint, new.fingerprint());
        let explicit = check(
            &store,
            Some(ConnectAnswer::TrustChangedHostKey {
                recorded_fingerprint: old.fingerprint().into(),
                offered_fingerprint: new.fingerprint().into(),
            }),
        );
        assert!(explicit.decide(&new));
        assert_eq!(store.check("nas.lan", 22, &new), HostKeyCheck::Known);
    }
}
