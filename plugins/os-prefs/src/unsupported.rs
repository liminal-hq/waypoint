// Reports the default time format on platforms without a reader
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use tokio::sync::mpsc::UnboundedSender;

use crate::{
    models::{TimeFormat, TimeFormatSource},
    service::{Readiness, Reading},
};

/// There is nothing to watch on an unsupported platform.
pub struct Watcher;

impl Watcher {
    pub fn take_readiness(&mut self) -> Readiness {
        Readiness::Unavailable("this platform is not supported".to_string())
    }
}

pub async fn read() -> Reading {
    Reading {
        format: TimeFormat {
            is_24_hour: false,
            source: TimeFormatSource::Default,
        },
        note: Some("the time format cannot be read on this platform".to_string()),
    }
}

pub fn watch(_changed: UnboundedSender<()>) -> Watcher {
    Watcher
}
