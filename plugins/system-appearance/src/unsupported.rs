// Reports the default titlebar preferences on platforms without a reader
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use tokio::sync::mpsc::UnboundedSender;

use crate::{
    models::{DesktopEnvironment, Snapshot},
    service::Readiness,
};

/// There is nothing to watch on an unsupported platform.
pub struct Watcher;

impl Watcher {
    /// Nothing is listened to here, so there is nothing to wait for.
    pub fn take_readiness(&mut self) -> Readiness {
        Readiness::Listening
    }
}

pub async fn read() -> Snapshot {
    Snapshot::unavailable(
        DesktopEnvironment::Unknown,
        "titlebar preferences are not supported on this platform",
    )
}

pub fn watch(_changed: UnboundedSender<()>) -> Watcher {
    Watcher
}
