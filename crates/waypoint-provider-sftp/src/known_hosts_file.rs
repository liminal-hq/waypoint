// `KnownHosts` over OpenSSH's own `known_hosts` file, shared with `ssh`.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::io;
use std::path::PathBuf;
use std::sync::Mutex;

use openssh_config::{KnownHostsFile, Lookup, RecordedKey};

use crate::host_keys::{HostKeyCheck, KnownHosts, ServerKey};

/// Server keys recorded in OpenSSH's `known_hosts`: the user's file, read and written, and the
/// system's, only read. Writes from this process are serialised.
#[derive(Debug)]
pub struct OpenSshKnownHosts {
    file: KnownHostsFile,
    writing: Mutex<()>,
}

impl OpenSshKnownHosts {
    /// `~/.ssh/known_hosts` and the system file, or `None` without a home folder.
    pub fn for_user() -> Option<Self> {
        KnownHostsFile::for_user().map(Self::from_file)
    }

    /// Only the file at `path`.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self::from_file(KnownHostsFile::new(path))
    }

    fn from_file(file: KnownHostsFile) -> Self {
        Self {
            file,
            writing: Mutex::new(()),
        }
    }
}

impl KnownHosts for OpenSshKnownHosts {
    fn check(&self, host: &str, port: u16, key: &ServerKey) -> HostKeyCheck {
        match self.file.check(host, port, key.algorithm(), key.base64()) {
            Lookup::Known => HostKeyCheck::Known,
            Lookup::Unknown => HostKeyCheck::Unknown,
            Lookup::Revoked => HostKeyCheck::Revoked,
            Lookup::Changed { recorded } => {
                match ServerKey::from_openssh(&format!(
                    "{} {}",
                    recorded.algorithm, recorded.base64
                )) {
                    Some(recorded) => HostKeyCheck::Changed { recorded },
                    // A recorded key that cannot be read cannot be shown: refuse as revoked.
                    None => HostKeyCheck::Revoked,
                }
            }
        }
    }

    fn remember(&self, host: &str, port: u16, key: &ServerKey) -> io::Result<()> {
        let _writing = self.writing.lock().unwrap_or_else(|e| e.into_inner());
        self.file.append(host, port, key.algorithm(), key.base64())
    }

    fn replace(
        &self,
        host: &str,
        port: u16,
        recorded: &ServerKey,
        key: &ServerKey,
    ) -> io::Result<()> {
        let _writing = self.writing.lock().unwrap_or_else(|e| e.into_inner());
        let recorded = RecordedKey {
            algorithm: recorded.algorithm().to_owned(),
            base64: recorded.base64().to_owned(),
        };
        self.file
            .replace(host, port, &recorded, key.algorithm(), key.base64())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_recorded_in_the_file_and_replaced_there() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("known_hosts");
        let hosts = OpenSshKnownHosts::new(&path);
        let old = ServerKey::from_openssh(
            "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIBCb8PDBXvzERLPgkKWxNbNUSsz4pysfnT/WkQUYHuQH",
        )
        .unwrap();
        let new = ServerKey::from_openssh(
            "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIH8a5MQ6ySq4iqqSL2z5fjBOnvjOZ2f6VHcrgaahwM7P",
        )
        .unwrap();
        assert_eq!(hosts.check("nas", 22, &old), HostKeyCheck::Unknown);
        hosts.remember("nas", 22, &old).unwrap();
        assert_eq!(hosts.check("nas", 22, &old), HostKeyCheck::Known);
        assert_eq!(
            hosts.check("nas", 22, &new),
            HostKeyCheck::Changed {
                recorded: old.clone()
            }
        );
        hosts.replace("nas", 22, &old, &new).unwrap();
        assert_eq!(hosts.check("nas", 22, &new), HostKeyCheck::Known);
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            format!("nas {}\n", new.to_openssh())
        );
    }
}
