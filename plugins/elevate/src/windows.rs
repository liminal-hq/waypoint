// Reports elevation unavailable on Windows until its launcher (ShellExecuteExW with the runas verb) is written
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A compiling stub: Windows support (a UAC prompt, a pipe only the invoking user can reach) is not implemented yet, so the status says so and `launch` refuses.

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
