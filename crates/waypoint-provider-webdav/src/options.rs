// What a WebDAV provider is built with, and the tuning of one connection.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use waypoint_vfs::{CredentialSource, NoCredentials};

/// How a connection answers a server that asks who is calling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AuthMode {
    /// Whatever the server's challenge offers, the strongest first: Digest, then Basic, then a
    /// bearer token. Nothing is sent before a server asks.
    #[default]
    Auto,
    /// Basic, sent with the first request once a credential is known (Nextcloud and other servers
    /// that take an app password).
    Basic,
    /// Digest only; a server that offers nothing else is not sent a password.
    Digest,
    /// A bearer token, which the credential source supplies as a passphrase named "access token".
    Bearer,
}

/// The server's dialect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Preset {
    /// Nextcloud (and ownCloud) when the path runs through `remote.php/dav` or `remote.php/webdav`,
    /// plain WebDAV otherwise.
    #[default]
    Auto,
    /// Plain WebDAV (RFC 4918) whatever the path.
    Generic,
    /// Nextcloud: uploads carry `X-OC-MTime` when a modification time is known and an
    /// `OC-Checksum` the server verifies, and `checksums` can be read back.
    Nextcloud,
}

/// The tuning of one connection (A81's per-connection options).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct WebDavOptions {
    /// How long connecting, or waiting for more of a response, may go on before the call fails
    /// with `Timeout`.
    pub timeout: Duration,
    pub auth: AuthMode,
    pub preset: Preset,
    /// Requests on the wire at once for this connection; the rest wait.
    pub max_requests: usize,
    /// The entries of one batch a listing hands over.
    pub batch: usize,
    /// Whether `HTTP_PROXY`, `HTTPS_PROXY` and `NO_PROXY` apply.
    pub system_proxy: bool,
}

impl Default for WebDavOptions {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            auth: AuthMode::Auto,
            preset: Preset::Auto,
            max_requests: 6,
            batch: 2_000,
            system_proxy: true,
        }
    }
}

impl WebDavOptions {
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn with_auth(mut self, auth: AuthMode) -> Self {
        self.auth = auth;
        self
    }

    pub fn with_preset(mut self, preset: Preset) -> Self {
        self.preset = preset;
        self
    }

    /// At least one request at a time.
    pub fn with_max_requests(mut self, requests: usize) -> Self {
        self.max_requests = requests.max(1);
        self
    }

    /// At least one entry per batch.
    pub fn with_batch(mut self, batch: usize) -> Self {
        self.batch = batch.max(1);
        self
    }

    pub fn with_system_proxy(mut self, system_proxy: bool) -> Self {
        self.system_proxy = system_proxy;
        self
    }
}

/// What a `WebDavProvider` is built with: where credentials come from, the default tuning and where
/// uploads wait before they are sent.
#[derive(Clone)]
pub struct WebDavConfig {
    pub(crate) credentials: Arc<dyn CredentialSource>,
    pub(crate) options: WebDavOptions,
    pub(crate) spool_dir: Option<PathBuf>,
}

impl Default for WebDavConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl WebDavConfig {
    /// A configuration with no credential source (every login is asked for) and the default tuning.
    pub fn new() -> Self {
        Self {
            credentials: Arc::new(NoCredentials),
            options: WebDavOptions::default(),
            spool_dir: None,
        }
    }

    /// Where passwords and tokens are looked up before a login fails with `AuthRequired`.
    pub fn with_credentials(mut self, credentials: Arc<dyn CredentialSource>) -> Self {
        self.credentials = credentials;
        self
    }

    /// The tuning of a connection that has none of its own (`WebDavProvider::set_options`).
    pub fn with_options(mut self, options: WebDavOptions) -> Self {
        self.options = options;
        self
    }

    /// Where an upload waits, as a temporary file, until it is complete and can be sent with its
    /// length (the system's temporary folder by default; a folder on a roomy disk is better when
    /// that is a RAM disk). The file is removed when the upload ends.
    pub fn with_spool_dir(mut self, dir: PathBuf) -> Self {
        self.spool_dir = Some(dir);
        self
    }
}
