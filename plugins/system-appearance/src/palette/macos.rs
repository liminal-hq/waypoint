// Reports every palette colour as unavailable on macOS, which this plugin does not read yet
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use tauri::{AppHandle, Runtime};
use tokio::sync::mpsc::UnboundedSender;

use super::models::PaletteColours;
use crate::{appearance::models::UnavailableReason, service::Readiness};

/// There is nothing to watch: no colour is read.
pub struct Watcher;

impl Watcher {
    pub fn take_readiness(&mut self) -> Readiness {
        Readiness::Listening
    }
}

pub async fn read<R: Runtime>(_app: &AppHandle<R>) -> PaletteColours {
    PaletteColours::unavailable(
        UnavailableReason::PlatformUnsupported,
        "the colour palette is not read on macOS yet",
    )
}

pub fn watch<R: Runtime>(_app: &AppHandle<R>, _changed: UnboundedSender<()>) -> Watcher {
    Watcher
}
