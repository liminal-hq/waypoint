// Reports the fixed macOS titlebar layout and the configured double-click action
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::process::Command;

use tokio::sync::mpsc::UnboundedSender;

use crate::{models::Snapshot, parse};

/// Changes to the double-click preference are picked up on the next read, not pushed.
pub struct Watcher;

/// Reads `AppleActionOnDoubleClick` from the global domain; absent when never set.
fn double_click_setting() -> Option<String> {
    let output = Command::new("defaults")
        .args(["read", "NSGlobalDomain", "AppleActionOnDoubleClick"])
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub async fn read() -> Snapshot {
    let setting = tauri::async_runtime::spawn_blocking(double_click_setting)
        .await
        .ok()
        .flatten();
    let action = parse::macos_double_click(setting.as_deref());
    Snapshot::from_source(parse::macos_preferences(action), "platform")
}

pub fn watch(_changed: UnboundedSender<()>) -> Watcher {
    Watcher
}
