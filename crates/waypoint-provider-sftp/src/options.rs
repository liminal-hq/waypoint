// What an SFTP provider is built with, and the tuning of one connection.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use waypoint_vfs::{CredentialSource, NoCredentials};

use crate::host_keys::KnownHosts;
use crate::ssh_config::SshConfigSource;

/// The tuning of one connection (A81's per-connection options). The defaults are the ones spike
/// #278 measured: 64 `readdir` requests in flight, 64 reads of 32 KiB, and an 8 MiB SSH window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct SftpOptions {
    /// How long a request, or connecting, may go unanswered before it fails with `Timeout`.
    pub timeout: Duration,
    /// `readdir` requests a listing keeps in flight. A server that refuses them makes the listing
    /// fall back to one at a time.
    pub listing_requests: usize,
    /// Read requests a stream keeps in flight.
    pub read_requests: usize,
    /// Write requests a stream keeps in flight.
    pub write_requests: usize,
    /// The bytes asked for by one read request, and sent by one write request.
    pub chunk: u32,
    /// The SSH channel window, which caps the bytes in flight per round trip.
    pub window_size: u32,
}

impl Default for SftpOptions {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            listing_requests: 64,
            read_requests: 64,
            write_requests: 64,
            chunk: 32 * 1024,
            window_size: 8 * 1024 * 1024,
        }
    }
}

impl SftpOptions {
    /// The defaults with another timeout.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// The defaults with another number of `readdir` requests in flight (at least one).
    pub fn with_listing_requests(mut self, requests: usize) -> Self {
        self.listing_requests = requests.max(1);
        self
    }
}

/// Where the SSH agent is found.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum AgentSource {
    /// `SSH_AUTH_SOCK` on Linux; the OpenSSH agent's pipe, then Pageant, on Windows.
    #[default]
    System,
    /// This socket (Linux) or named pipe (Windows).
    Path(PathBuf),
    /// No agent.
    None,
}

/// What an `SftpProvider` is built with: where credentials and trusted host keys come from, the
/// agent, the key files to try, and the default tuning.
#[derive(Clone)]
pub struct SftpConfig {
    pub(crate) credentials: Arc<dyn CredentialSource>,
    pub(crate) known_hosts: Arc<dyn KnownHosts>,
    pub(crate) agent: AgentSource,
    pub(crate) identity_files: Option<Vec<PathBuf>>,
    pub(crate) ssh_config: Option<Arc<dyn SshConfigSource>>,
    pub(crate) options: SftpOptions,
    pub(crate) plain_protocol: bool,
}

impl SftpConfig {
    /// A configuration that checks server keys against `known_hosts`, asks no credential source
    /// (every password is asked for), reads no SSH config, and uses the system's agent and the
    /// default key files.
    pub fn new(known_hosts: Arc<dyn KnownHosts>) -> Self {
        Self {
            credentials: Arc::new(NoCredentials),
            known_hosts,
            agent: AgentSource::System,
            identity_files: None,
            ssh_config: None,
            options: SftpOptions::default(),
            plain_protocol: false,
        }
    }

    /// Where passwords, passphrases and keyboard-interactive answers are looked up before a login
    /// fails with `AuthRequired`.
    pub fn with_credentials(mut self, credentials: Arc<dyn CredentialSource>) -> Self {
        self.credentials = credentials;
        self
    }

    pub fn with_agent(mut self, agent: AgentSource) -> Self {
        self.agent = agent;
        self
    }

    /// Where host names find their address, user, port, key files and jump hosts
    /// (`openssh_config::SshConfig::for_user()` for `~/.ssh/config`). Without one a location is
    /// taken as written.
    pub fn with_ssh_config(mut self, source: Arc<dyn SshConfigSource>) -> Self {
        self.ssh_config = Some(source);
        self
    }

    /// The private key files to try, in order, instead of the configuration's `IdentityFile`s or
    /// `~/.ssh/id_ed25519`, `id_ecdsa` and `id_rsa`.
    pub fn with_identity_files(mut self, files: Vec<PathBuf>) -> Self {
        self.identity_files = Some(files);
        self
    }

    /// Uses none of OpenSSH's protocol extensions even where the server has them, as against a
    /// server that only speaks version 3 of the draft. For tests of those paths.
    #[doc(hidden)]
    pub fn with_plain_protocol(mut self) -> Self {
        self.plain_protocol = true;
        self
    }

    /// The tuning of a connection that has none of its own (`SftpProvider::set_options`).
    pub fn with_options(mut self, options: SftpOptions) -> Self {
        self.options = options;
        self
    }
}

/// The key files OpenSSH tries when none is configured, that exist.
pub(crate) fn default_identity_files() -> Vec<PathBuf> {
    let Some(home) = home_dir() else {
        return Vec::new();
    };
    ["id_ed25519", "id_ecdsa", "id_rsa"]
        .iter()
        .map(|name| home.join(".ssh").join(name))
        .filter(|path| path.is_file())
        .collect()
}

pub(crate) fn home_dir() -> Option<PathBuf> {
    std::env::home_dir()
}

/// The local account's name, the user of a location that names none.
pub(crate) fn local_user() -> String {
    ["USER", "USERNAME", "LOGNAME"]
        .iter()
        .find_map(|name| std::env::var(name).ok().filter(|v| !v.is_empty()))
        .unwrap_or_else(|| "root".to_owned())
}
