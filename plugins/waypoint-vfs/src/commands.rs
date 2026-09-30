// Implements the IPC commands exposed by the file system plugin
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use waypoint_protocol::PluginStatus;

use crate::error::Error;

/// Reports whether the plugin can do anything yet. Until the provider and listing commands land,
/// it is honest about that, so the Services panel never claims a feature that is not there.
#[tauri::command]
pub async fn get_status() -> Result<PluginStatus, Error> {
    Ok(PluginStatus::unavailable(
        "the file system commands have not been built yet",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_unavailable_until_commands_exist() {
        let status = tauri::async_runtime::block_on(get_status()).unwrap();
        assert!(!status.available);
        assert!(status.reason.is_some());
    }
}
