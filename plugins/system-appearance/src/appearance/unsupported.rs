// Reports every appearance preference as unavailable on platforms without a reader
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use tokio::sync::mpsc::UnboundedSender;

use super::resolve::{unsupported, Resolution};
use crate::service::Readiness;

/// There is nothing to watch on an unsupported platform.
pub struct Watcher;

impl Watcher {
    pub fn take_readiness(&mut self) -> Readiness {
        Readiness::Listening
    }
}

pub async fn read() -> Resolution {
    unsupported("appearance preferences are not supported on this platform")
}

pub fn watch(_changed: UnboundedSender<()>) -> Watcher {
    Watcher
}
