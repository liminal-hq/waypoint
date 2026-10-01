// Composition root: registers the Tauri plugins and starts the app
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::sync::Arc;

use tauri_plugin_waypoint_session::{MemoryStorage, SessionDeps, WindowError, WindowFactory};
use waypoint_session::StorePolicy;

/// The level floor applied to every log line — native Rust and forwarded
/// webview `console.*` calls alike. Verbose in a debug build, `Info` and up in
/// a release one, the same split the other Liminal HQ apps use.
fn log_level() -> log::LevelFilter {
    if cfg!(debug_assertions) {
        log::LevelFilter::Trace
    } else {
        log::LevelFilter::Info
    }
}

/// The window factory until windows can be created from Rust (milestone 3, slice 04): the app has
/// the one window `tauri.conf.json` declares, so a command that needs another one fails cleanly.
struct NoWindowFactory;

impl WindowFactory for NoWindowFactory {
    fn create(
        &self,
        _app: &tauri::AppHandle,
        _label: &str,
        _geometry: Option<&waypoint_session::Geometry>,
    ) -> Result<(), WindowError> {
        Err(WindowError::NotAvailable)
    }
}

/// The session's dependencies for now: no window creation, nothing persisted, and the milestone 2
/// rule that closing a window's last tab leaves it empty (the page puts a Home tab there). The
/// persistence wiring and the app's own policy come with slice 03.
fn session_deps() -> SessionDeps {
    SessionDeps::new(
        Arc::new(NoWindowFactory),
        Arc::new(MemoryStorage::default()),
        StorePolicy {
            close_window_on_last_tab: false,
        },
    )
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[allow(unused_mut)]
    let mut builder = tauri::Builder::default()
        // `tauri-plugin-log`'s own `plugin:log|log` command is what
        // `src/services/logger.ts` forwards the webview's `console.*` calls
        // into, tagged `webview[:file:line]`. That command re-emits through this
        // same process-global `log` logger, so forwarded webview messages land
        // in exactly the same stdout stream and log file (the plugin's default
        // `Stdout` + `LogDir` targets) as native `log::info!()` calls, with the
        // same formatting and level floor.
        .plugin(
            tauri_plugin_log::Builder::default()
                .level(log_level())
                // The debug-only MCP bridge's websocket internals are extremely
                // chatty at `Trace` (every handshake read and write poll) and
                // drown out everything else in a debug build's log.
                .level_for("tungstenite", log::LevelFilter::Warn)
                .level_for("tokio_tungstenite", log::LevelFilter::Warn)
                .format(|out, message, record| {
                    out.finish(format_args!(
                        "[{}][{}][{}] {}",
                        chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f %:z"),
                        record.level(),
                        record.target(),
                        message
                    ))
                })
                .build(),
        )
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_system_appearance::init())
        .plugin(tauri_plugin_window_manager::init())
        .plugin(tauri_plugin_waypoint_vfs::init())
        .plugin(tauri_plugin_waypoint_session::init(session_deps()))
        .plugin(tauri_plugin_window_state::Builder::default().build());

    // Lets an agent drive and screenshot the running app during development. Not on Windows, where
    // the bridge does not yet compile against Tauri's `windows` crate (see `Cargo.toml`).
    #[cfg(all(debug_assertions, not(windows)))]
    {
        builder = builder.plugin(tauri_plugin_mcp_bridge::init());
    }

    builder
        .run(tauri::generate_context!())
        .expect("error while running the Waypoint application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_builds_log_everything() {
        // Tests always run with debug assertions on.
        assert_eq!(log_level(), log::LevelFilter::Trace);
    }
}
