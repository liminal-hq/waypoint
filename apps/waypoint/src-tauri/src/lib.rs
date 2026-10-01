// Composition root: registers the Tauri plugins and starts the app
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod persistence;
mod storage;
mod windows;

use std::sync::Arc;

use tauri::{Manager, RunEvent, WindowEvent};
use tauri_plugin_waypoint_session::SessionDeps;
use waypoint_protocol::WindowKind;
use waypoint_session::StorePolicy;

use persistence::Saver;
use windows::{GeometryCapture, TauriWindowFactory};

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

/// The session's dependencies: windows made by `TauriWindowFactory`, the document saved by
/// `saver` a second after each change and once more as the app quits, closing a window's last tab
/// closes the window (D91), and closing the last window quits the app after a final save.
fn session_deps(saver: &Arc<Saver>) -> SessionDeps {
    let mut deps = SessionDeps::new(
        Arc::new(TauriWindowFactory),
        saver.storage(),
        StorePolicy {
            close_window_on_last_tab: true,
        },
    );
    let on_change = Arc::clone(saver);
    deps.on_change = Some(Arc::new(move |store| {
        on_change.save(|| Some(store.to_document()), false)
    }));
    let on_last = Arc::clone(saver);
    deps.on_last_window_closed = Some(Arc::new(move |app| {
        on_last.finish(app);
        app.exit(0);
    }));
    deps
}

/// Hands the page the sentence to show when the last session could not be restored, once.
#[tauri::command]
fn take_restore_notice(saver: tauri::State<'_, Arc<Saver>>) -> Option<String> {
    saver.take_notice()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let saver = Arc::new(Saver::new());
    let geometry = GeometryCapture::default();

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
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_system_appearance::init())
        .plugin(tauri_plugin_window_manager::init())
        .plugin(tauri_plugin_waypoint_vfs::init())
        .plugin(tauri_plugin_waypoint_session::init(session_deps(&saver)))
        .manage(Arc::clone(&saver))
        .invoke_handler(tauri::generate_handler![take_restore_notice])
        .setup({
            let saver = Arc::clone(&saver);
            move |app| {
                persistence::restore(app.handle(), &saver);
                Ok(())
            }
        })
        .on_window_event({
            let saver = Arc::clone(&saver);
            move |window, event| {
                if WindowKind::from_label(window.label()) != Some(WindowKind::Main) {
                    return;
                }
                geometry.on_event(window, event);
                if matches!(event, WindowEvent::CloseRequested { .. }) {
                    saver.window_closing(window.app_handle());
                }
            }
        });

    // Lets an agent drive and screenshot the running app during development. Not on Windows, where
    // the bridge does not yet compile against Tauri's `windows` crate (see `Cargo.toml`).
    #[cfg(all(debug_assertions, not(windows)))]
    {
        builder = builder.plugin(tauri_plugin_mcp_bridge::init());
    }

    builder
        .build(tauri::generate_context!())
        .expect("error while building the Waypoint application")
        .run(move |app, event| match event {
            RunEvent::ExitRequested { code, api, .. } => {
                if code.is_none() {
                    // Every window is gone. The session plugin is still closing the last one's
                    // session, so wait for it and quit from `finish`, which saves what is left.
                    api.prevent_exit();
                    saver.quit_when_settled(app);
                } else {
                    saver.finish(app);
                }
            }
            RunEvent::Exit => saver.finish(app),
            _ => {}
        });
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
