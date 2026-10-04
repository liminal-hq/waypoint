// Server keys and the trust in them: the key a server offered, the known-hosts check and an
// in-memory store of trusted keys.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::HashMap;
use std::io;
use std::sync::Mutex;

use russh::keys::{HashAlg, PublicKey};
use waypoint_protocol::{HostKey, HostKeyChange};

/// A server's public host key: its algorithm, its SHA-256 fingerprint and its encoded form.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ServerKey {
    algorithm: String,
    fingerprint: String,
    base64: String,
}

impl ServerKey {
    pub(crate) fn from_public(key: &PublicKey) -> Self {
        let openssh = key.to_openssh().unwrap_or_default();
        let base64 = openssh
            .split_whitespace()
            .nth(1)
            .unwrap_or_default()
            .to_owned();
        Self {
            algorithm: key.algorithm().as_str().to_owned(),
            fingerprint: key.fingerprint(HashAlg::Sha256).to_string(),
            base64,
        }
    }

    /// Reads a key as `known_hosts` and `.pub` files write it: `algorithm base64 [comment]`.
    pub fn from_openssh(text: &str) -> Option<Self> {
        let mut words = text.split_whitespace();
        let algorithm = words.next()?;
        let base64 = words.next()?;
        let key = russh::keys::parse_public_key_base64(base64).ok()?;
        (key.algorithm().as_str() == algorithm).then(|| Self::from_public(&key))
    }

    /// The key type (`ssh-ed25519`, `ecdsa-sha2-nistp256`, `ssh-rsa`).
    pub fn algorithm(&self) -> &str {
        &self.algorithm
    }

    /// `SHA256:` and the base64 digest, as `ssh-keygen -l` prints it.
    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }

    /// The key as `known_hosts` writes it: `algorithm base64`.
    pub fn to_openssh(&self) -> String {
        format!("{} {}", self.algorithm, self.base64)
    }

    /// The wire form the warning about a changed key shows: `recorded` was on record, `self` is
    /// offered now.
    pub(crate) fn to_change(&self, recorded: &ServerKey, host: &str, port: u16) -> HostKeyChange {
        HostKeyChange {
            host: host_label(host, port),
            recorded_algorithm: recorded.algorithm.clone(),
            recorded_fingerprint: recorded.fingerprint.clone(),
            offered_algorithm: self.algorithm.clone(),
            offered_fingerprint: self.fingerprint.clone(),
        }
    }

    /// The wire form the trust dialog shows, for the server at `host` and `port`.
    pub(crate) fn to_wire(&self, host: &str, port: u16) -> HostKey {
        HostKey {
            host: host_label(host, port),
            algorithm: self.algorithm.clone(),
            fingerprint: self.fingerprint.clone(),
        }
    }
}

/// How `known_hosts` writes a server: the host alone on port 22, `[host]:port` otherwise.
pub fn host_label(host: &str, port: u16) -> String {
    if port == 22 {
        host.to_owned()
    } else {
        format!("[{host}]:{port}")
    }
}

/// What the record says about a server's key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostKeyCheck {
    /// The key is recorded for this server.
    Known,
    /// Nothing is recorded for this server and this key type: the person decides.
    Unknown,
    /// Another key of the same type is recorded for this server. It never connects silently; only
    /// the explicit `TrustChangedHostKey` answer naming both keys replaces the record (D148).
    Changed { recorded: ServerKey },
    /// The key is marked revoked. It is refused whatever the answer.
    Revoked,
}

/// Where trusted server keys are recorded (OpenSSH's `known_hosts`, or a fake in tests).
pub trait KnownHosts: Send + Sync {
    /// Checks `key` as offered by the server at `host` (the name connected to, after any SSH
    /// config alias) and `port`.
    fn check(&self, host: &str, port: u16, key: &ServerKey) -> HostKeyCheck;

    /// Records `key` as trusted for `host` and `port`, after the person accepted an unknown key.
    fn remember(&self, host: &str, port: u16, key: &ServerKey) -> io::Result<()>;

    /// Replaces the `recorded` key of `host` and `port` with `key`, after the person explicitly
    /// trusted a changed key.
    fn replace(
        &self,
        host: &str,
        port: u16,
        recorded: &ServerKey,
        key: &ServerKey,
    ) -> io::Result<()>;
}

/// Trusted keys kept in memory: for tests, and for an app that keeps no file.
#[derive(Debug, Default)]
pub struct MemoryKnownHosts {
    keys: Mutex<HashMap<String, Vec<ServerKey>>>,
}

impl MemoryKnownHosts {
    pub fn new() -> Self {
        Self::default()
    }
}

impl KnownHosts for MemoryKnownHosts {
    fn check(&self, host: &str, port: u16, key: &ServerKey) -> HostKeyCheck {
        let keys = self.keys.lock().unwrap_or_else(|e| e.into_inner());
        let recorded = keys.get(&host_label(host, port)).map(Vec::as_slice);
        compare(recorded.unwrap_or_default().iter(), key)
    }

    fn remember(&self, host: &str, port: u16, key: &ServerKey) -> io::Result<()> {
        let mut keys = self.keys.lock().unwrap_or_else(|e| e.into_inner());
        let entry = keys.entry(host_label(host, port)).or_default();
        entry.retain(|old| old.algorithm != key.algorithm);
        entry.push(key.clone());
        Ok(())
    }

    fn replace(
        &self,
        host: &str,
        port: u16,
        recorded: &ServerKey,
        key: &ServerKey,
    ) -> io::Result<()> {
        let mut keys = self.keys.lock().unwrap_or_else(|e| e.into_inner());
        let entry = keys.entry(host_label(host, port)).or_default();
        entry.retain(|old| old != recorded);
        entry.push(key.clone());
        Ok(())
    }
}

/// The verdict on `key` given the keys recorded for its server: known when one matches, changed
/// when another of the same type is recorded, unknown otherwise (as OpenSSH, a server may have a
/// key of each type).
pub(crate) fn compare<'a>(
    recorded: impl Iterator<Item = &'a ServerKey>,
    key: &ServerKey,
) -> HostKeyCheck {
    let mut changed = None;
    for old in recorded {
        if old.base64 == key.base64 {
            return HostKeyCheck::Known;
        }
        if old.algorithm == key.algorithm && changed.is_none() {
            changed = Some(old.clone());
        }
    }
    match changed {
        Some(recorded) => HostKeyCheck::Changed { recorded },
        None => HostKeyCheck::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ED: &str =
        "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIBCb8PDBXvzERLPgkKWxNbNUSsz4pysfnT/WkQUYHuQH";
    const ED2: &str =
        "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIH8a5MQ6ySq4iqqSL2z5fjBOnvjOZ2f6VHcrgaahwM7P";

    #[test]
    fn a_key_reads_with_its_fingerprint() {
        let key = ServerKey::from_openssh(ED).unwrap();
        assert_eq!(key.algorithm(), "ssh-ed25519");
        assert!(key.fingerprint().starts_with("SHA256:"));
        assert_eq!(key.to_openssh(), ED);
        assert!(ServerKey::from_openssh("ssh-rsa AAAA").is_none());
    }

    #[test]
    fn the_memory_store_tells_known_unknown_and_changed_apart() {
        let store = MemoryKnownHosts::new();
        let key = ServerKey::from_openssh(ED).unwrap();
        let other = ServerKey::from_openssh(ED2).unwrap();
        assert_eq!(store.check("h", 22, &key), HostKeyCheck::Unknown);
        store.remember("h", 22, &key).unwrap();
        assert_eq!(store.check("h", 22, &key), HostKeyCheck::Known);
        assert_eq!(
            store.check("h", 22, &other),
            HostKeyCheck::Changed {
                recorded: key.clone()
            }
        );
        store.replace("h", 22, &key, &other).unwrap();
        assert_eq!(store.check("h", 22, &other), HostKeyCheck::Known);
        assert_eq!(store.check("h", 2222, &key), HostKeyCheck::Unknown);
        assert_eq!(key.to_wire("h", 2222).host, "[h]:2222");
    }
}
