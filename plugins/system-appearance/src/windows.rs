// Reports the fixed Windows titlebar layout and actions
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use tokio::sync::mpsc::UnboundedSender;

use crate::{models::Snapshot, parse};

/// Windows has no user-configurable titlebar, so there is nothing to watch.
pub struct Watcher;

impl Watcher {
    /// Nothing is listened to here, so there is nothing to wait for.
    pub fn take_ready(&mut self) -> Option<tokio::sync::oneshot::Receiver<()>> {
        None
    }
}

pub async fn read() -> Snapshot {
    Snapshot::from_source(parse::windows_preferences(), "platform")
}

pub fn watch(_changed: UnboundedSender<()>) -> Watcher {
    Watcher
}
