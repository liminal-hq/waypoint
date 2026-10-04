// Reports every palette colour as unavailable on platforms without a reader
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use tauri::{AppHandle, Runtime};
use tokio::sync::mpsc::UnboundedSender;

use super::models::PaletteColours;
use crate::{appearance::models::UnavailableReason, service::Readiness};

/// There is nothing to watch on an unsupported platform.
pub struct Watcher;

impl Watcher {
    pub fn take_readiness(&mut self) -> Readiness {
        Readiness::Listening
    }
}

pub async fn read<R: Runtime>(_app: &AppHandle<R>) -> PaletteColours {
    PaletteColours::unavailable(
        UnavailableReason::PlatformUnsupported,
        "the colour palette is not supported on this platform",
    )
}

pub fn watch<R: Runtime>(_app: &AppHandle<R>, _changed: UnboundedSender<()>) -> Watcher {
    Watcher
}
