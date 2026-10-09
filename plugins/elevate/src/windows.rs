// Starts the helper through a UAC prompt on Windows, after the availability check passes
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The helper is started with `ShellExecuteExW` and the `runas` verb, and the app talks to it over two named pipes (one per direction, because a single synchronous pipe handle would let a waiting read block a write). The checks that make the stream trusted are in [`crate::winapi::launch`].

use std::path::Path;

use crate::launch::{ElevatedStream, LaunchError};
use crate::models::PluginStatus;
use crate::win_check::{check, HelperFacts, WinProbe};
use crate::winapi;
use crate::Config;

/// The real system.
struct SystemWinProbe;

impl WinProbe for SystemWinProbe {
    fn helper_facts(&self, path: &Path) -> Option<HelperFacts> {
        let metadata = std::fs::symlink_metadata(path).ok()?;
        Some(HelperFacts {
            is_file: metadata.is_file(),
            is_reparse_point: winapi::is_reparse_point(&metadata),
        })
    }

    fn program_files(&self) -> Vec<String> {
        winapi::program_files()
    }

    fn app_exe(&self) -> Option<String> {
        std::env::current_exe()
            .ok()
            .map(|path| path.to_string_lossy().into_owned())
    }

    fn is_elevated(&self) -> Option<bool> {
        winapi::is_elevated()
    }
}

pub struct Platform {
    config: Config,
    probe: Box<dyn WinProbe + Send + Sync>,
}

impl Platform {
    pub fn new(config: Config) -> Self {
        Platform {
            config,
            probe: Box::new(SystemWinProbe),
        }
    }

    pub fn status(&self) -> PluginStatus {
        check(&self.config, self.probe.as_ref())
    }

    /// Runs the check again (the system may have changed since the status was read) and refuses when it fails, then starts the helper through UAC.
    pub fn launch(&self, cancelled: &dyn Fn() -> bool) -> Result<ElevatedStream, LaunchError> {
        let status = self.status();
        if !status.available {
            return Err(LaunchError::Unavailable {
                reason: status
                    .reason
                    .unwrap_or_else(|| "Elevation is not available".to_string()),
            });
        }
        winapi::launch(&self.config, cancelled)
    }
}
