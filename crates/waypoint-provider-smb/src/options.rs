// What an SMB provider is built with, and the tuning of its connections.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::sync::Arc;
use std::time::Duration;

use waypoint_vfs::{CredentialSource, NoCredentials};

/// The tuning of one connection (A81's per-connection options).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct SmbOptions {
    /// How long connecting, or a request, may go unanswered before it fails with `Timeout`.
    pub timeout: Duration,
    /// The entries handed over at a time while a listing is delivered.
    pub listing_batch: usize,
    /// Read requests a stream keeps in flight.
    pub read_requests: usize,
    /// The bytes asked for by one read request, and held back before one write request.
    pub chunk: usize,
}

impl Default for SmbOptions {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            listing_batch: 2_000,
            read_requests: 8,
            chunk: 1024 * 1024,
        }
    }
}

impl SmbOptions {
    /// The defaults with another number of entries per batch (at least one).
    pub fn with_listing_batch(mut self, entries: usize) -> Self {
        self.listing_batch = entries.max(1);
        self
    }

    /// The defaults with another timeout.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

/// What an `SmbProvider` is built with: where credentials come from and the default tuning.
#[derive(Clone)]
pub struct SmbConfig {
    pub(crate) credentials: Arc<dyn CredentialSource>,
    pub(crate) options: SmbOptions,
}

impl Default for SmbConfig {
    fn default() -> Self {
        Self {
            credentials: Arc::new(NoCredentials),
            options: SmbOptions::default(),
        }
    }
}

impl SmbConfig {
    pub fn new() -> Self {
        Self::default()
    }

    /// Where a password is looked up before a login fails with `AuthRequired`.
    pub fn with_credentials(mut self, credentials: Arc<dyn CredentialSource>) -> Self {
        self.credentials = credentials;
        self
    }

    /// The tuning of a connection that has none of its own.
    pub fn with_options(mut self, options: SmbOptions) -> Self {
        self.options = options;
        self
    }
}
