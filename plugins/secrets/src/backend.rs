// The seam between the plugin's logic and the operating system: what a keyring backend has to provide
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::future::Future;
use std::pin::Pin;

use crate::error::Result;
use crate::models::{PluginStatus, Secret, SecretId};

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// A platform's keyring. The real ones are in `linux.rs` and `windows.rs`; tests use [`crate::MemoryBackend`] or their own fake. Ids reaching a backend are already validated, and a backend must never log or put a secret in an error.
pub trait Backend: Send + Sync + 'static {
    /// What works on this system. Probing may touch the session bus, so this is asked when a status is wanted rather than once. It never prompts the person.
    fn status(&self) -> BoxFuture<'_, PluginStatus>;

    /// Stores the secret, replacing one with the same id. `label` is the name the keyring's own manager shows.
    fn store(
        &self,
        id: SecretId,
        label: Option<String>,
        secret: Secret,
    ) -> BoxFuture<'_, Result<()>>;

    /// The secret, or `None` when there is none with that id.
    fn fetch(&self, id: SecretId) -> BoxFuture<'_, Result<Option<Secret>>>;

    /// Whether a secret with that id exists, without reading it.
    fn exists(&self, id: SecretId) -> BoxFuture<'_, Result<bool>>;

    /// Deletes the secret; true when there was one.
    fn delete(&self, id: SecretId) -> BoxFuture<'_, Result<bool>>;

    /// Deletes every secret of the service and account, whatever its kind, and returns how many there were.
    fn delete_account(&self, service: String, account: String) -> BoxFuture<'_, Result<usize>>;
}
