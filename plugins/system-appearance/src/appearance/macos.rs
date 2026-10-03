// Reports every appearance preference as unavailable on macOS, which this plugin does not read yet
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use tokio::sync::mpsc::UnboundedSender;

use super::resolve::{unsupported, Resolution};
use crate::service::Readiness;

/// There is nothing to watch: no preference is read.
pub struct Watcher;

impl Watcher {
    pub fn take_readiness(&mut self) -> Readiness {
        Readiness::Listening
    }
}

pub async fn read() -> Resolution {
    unsupported("appearance preferences are not read on macOS yet")
}

pub fn watch(_changed: UnboundedSender<()>) -> Watcher {
    Watcher
}
