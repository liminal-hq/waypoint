// The seam between the plugin's logic and the operating system: what a platform backend has to provide
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::error::Result;
use crate::models::{Passphrase, PluginStatus, Volume};

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Called by a backend, from any thread, whenever something may have changed. The plugin debounces the calls and then asks for the list again, so a backend may call it freely.
pub type Notify = Arc<dyn Fn() + Send + Sync>;

/// A platform's volumes. The real ones are in `linux.rs` and `windows.rs`; tests inject a fake. None of the methods measures free space: the plugin does that, with a timeout.
pub trait Backend: Send + Sync + 'static {
    /// What works on this system. Probing may touch the system bus, so this is asked when a status is wanted rather than once.
    fn status(&self) -> BoxFuture<'_, PluginStatus>;

    /// Every volume now, without free space.
    fn volumes(&self) -> BoxFuture<'_, Result<Vec<Volume>>>;

    /// Mounts a volume and returns where it is mounted.
    fn mount(&self, id: String) -> BoxFuture<'_, Result<String>>;

    fn unmount(&self, id: String) -> BoxFuture<'_, Result<()>>;

    /// Unmounts what is on the volume's drive, then ejects it (and powers it off when the drive allows).
    fn eject(&self, id: String) -> BoxFuture<'_, Result<()>>;

    /// Unlocks an encrypted volume and returns the id of the volume that appears. The passphrase is used for this one call and not kept.
    fn unlock(&self, id: String, passphrase: Passphrase) -> BoxFuture<'_, Result<String>>;

    /// Starts watching for changes and returns once the watch is running. Everything it starts lives as long as the backend.
    fn watch(&self, notify: Notify) -> BoxFuture<'_, Result<()>>;
}
