// Reports elevation unavailable on systems with no elevation support
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::launch::{ElevatedStream, LaunchError};
use crate::models::{Flavour, PluginStatus};
use crate::Config;

pub struct Platform;

impl Platform {
    pub fn new(_config: Config) -> Self {
        Platform
    }

    pub fn status(&self) -> PluginStatus {
        PluginStatus::unavailable(Flavour::Unsupported, crate::REASON_NOT_IMPLEMENTED)
    }

    pub fn launch(&self, _cancelled: &dyn Fn() -> bool) -> Result<ElevatedStream, LaunchError> {
        Err(LaunchError::Unavailable {
            reason: crate::REASON_NOT_IMPLEMENTED.to_string(),
        })
    }
}
