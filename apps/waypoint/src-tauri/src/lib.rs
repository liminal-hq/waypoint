// Composition root: registers the Tauri plugins and starts the app
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod checksum;
mod effects;
mod ops;
mod ops_window;
mod persistence;
mod properties_window;
mod settings;
mod settings_window;
mod shelf_window;
mod storage;
mod thumbnails;
mod windows;

use std::sync::Arc;

use tauri::{Manager, RunEvent, WindowEvent};
use tauri_plugin_waypoint_session::SessionDeps;
use waypoint_protocol::WindowKind;
use waypoint_session::StorePolicy;

use persistence::{CloseFlush, Intent, Saver};
use windows::{GeometryCapture, HoldNextWindow, TauriWindowFactory};

/// The label of the tear-off ghost; `WindowKind::TearGhost` routes it to `TearGhostScreen`.
const GHOST_LABEL: &str = "tear-ghost";

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

/// The type a tab drag offers the compositor on Wayland, which every main window accepts a drop of.
const TAB_MIME: &str = "application/x-waypoint-tab";

/// The tear-off ghost: one shared window the plugin creates once the event loop runs (never
/// `tauri.conf.json`), which loads the app bundle and is routed to `TearGhostScreen` by its label.
/// No window-state plugin is registered, so the ghost needs no denylist entry; add
/// `Options::window_state_denylist` to one if it is ever added.
fn tear_off_options() -> tauri_plugin_window_tearoff::Options {
    tauri_plugin_window_tearoff::Options {
        ghost_label: GHOST_LABEL.into(),
        ghost_url: "index.html".into(),
        ghost_size: (240.0, 84.0),
        toplevel_drag_mime: TAB_MIME.into(),
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
        on_change.save(|| Some(store.to_document()), Intent::Change)
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

/// Keeps the next window the calling window makes hidden (`on`), or stops doing so; see `HoldNextWindow`.
#[tauri::command]
fn hold_next_window(
    window: tauri::WebviewWindow,
    hold: tauri::State<'_, HoldNextWindow>,
    on: bool,
) {
    hold.set(window.label(), on, std::time::Instant::now());
}

/// Shows a main window made hidden for a drag that then could not start, so the tabs in it are not lost.
#[tauri::command]
fn show_window(app: tauri::AppHandle, label: String) -> Result<(), String> {
    use tauri::Manager;
    if !label.starts_with("main-") {
        return Err(format!("`{label}` is not a main window"));
    }
    let window = app
        .get_webview_window(&label)
        .ok_or_else(|| format!("no window `{label}`"))?;
    window.show().map_err(|e| e.to_string())
}

/// A Properties window has closed: its place is free for another, and a checksum it was running stops.
fn forget_properties_window<R: tauri::Runtime>(window: &tauri::Window<R>) {
    let app = window.app_handle();
    if let Some(windows) = app.try_state::<properties_window::PropertiesWindows>() {
        windows.forget(window.label());
    }
    if let Some(runs) = app.try_state::<checksum::Checksums>() {
        runs.cancel_window(window.label());
    }
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
        .plugin(tauri_plugin_os_prefs::init())
        .plugin(tauri_plugin_system_appearance::init())
        .plugin(tauri_plugin_window_manager::init())
        .plugin(tauri_plugin_window_tearoff::init(tear_off_options()))
        .plugin(tauri_plugin_trash::init())
        .plugin(tauri_plugin_thumbnails::init())
        .plugin(tauri_plugin_volumes::init())
        .plugin(tauri_plugin_window_effects::init())
        .plugin(tauri_plugin_mime_apps::init())
        .plugin(tauri_plugin_native_dnd::init())
        .plugin(tauri_plugin_waypoint_vfs::init())
        // After the store plugin it saves through; the session reads its choices (start-up, the view
        // of a new window) from it in `setup`.
        .plugin(tauri_plugin_waypoint_settings::init_with(settings::storage))
        // After the vfs and trash plugins its adapters reach, and after the store plugin it saves through.
        .plugin(tauri_plugin_waypoint_ops::init_with(ops::deps))
        .plugin(tauri_plugin_waypoint_session::init(session_deps(&saver)))
        .manage(Arc::clone(&saver))
        .manage(HoldNextWindow::default())
        .manage(effects::Driver::default())
        .manage(properties_window::PropertiesWindows::default())
        .manage(checksum::Checksums::default())
        .manage(thumbnails::ThumbnailBridge::default())
        .invoke_handler(tauri::generate_handler![
            take_restore_notice,
            hold_next_window,
            show_window,
            ops_window::open_ops_window,
            settings_window::open_settings_window,
            shelf_window::raise_shelf_window,
            shelf_window::toggle_shelf_window,
            shelf_window::hide_shelf_window,
            shelf_window::shelf_window_visible,
            properties_window::open_properties_window,
            properties_window::properties_subject,
            properties_window::properties_set_subject,
            checksum::file_checksum,
            checksum::cancel_checksum,
            thumbnails::thumbnails_request_entries,
            thumbnails::thumbnails_request_locations,
            thumbnails::thumbnails_cancel,
            thumbnails::thumbnails_prioritise
        ])
        .setup({
            let saver = Arc::clone(&saver);
            move |app| {
                settings::wire(app.handle());
                effects::wire(app.handle());
                thumbnails::wire(app.handle());
                persistence::restore(app.handle(), &saver);
                Ok(())
            }
        })
        // A window's page starts loading once the window exists: it gets the effects the settings ask for.
        .on_page_load(|webview, payload| {
            if matches!(payload.event(), tauri::webview::PageLoadEvent::Started) {
                effects::on_page_load(webview);
            }
        })
        .on_window_event({
            let saver = Arc::clone(&saver);
            let flush = Arc::new(CloseFlush::default());
            move |window, event| {
                effects::on_window_event(window, event);
                if matches!(event, WindowEvent::Destroyed)
                    && window.label().starts_with(properties_window::LABEL_PREFIX)
                {
                    forget_properties_window(window);
                }
                let kind = WindowKind::from_label(window.label());
                // The Shelf window's place and size are kept too; closing it is not a session close.
                if kind == Some(WindowKind::Shelf) {
                    geometry.on_event(window, event);
                    return;
                }
                if kind != Some(WindowKind::Main) {
                    return;
                }
                geometry.on_event(window, event);
                if matches!(event, WindowEvent::CloseRequested { .. }) {
                    // The page reports its last hints first; the repeated request saves.
                    if flush.on_close_requested(window, event) {
                        return;
                    }
                    saver.window_closing(window.app_handle());
                    if let Some(ops) = window
                        .app_handle()
                        .try_state::<tauri_plugin_waypoint_ops::Ops<tauri::Wry>>()
                    {
                        ops.flush_journal();
                    }
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
            // The event loop is running and the first windows are on their way: the Trash sweep
            // waits for one to be shown, on its own thread, so start-up never does.
            RunEvent::Ready => ops::start_trash_sweep(app),
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
    fn the_ghost_label_is_the_one_the_front_end_routes() {
        assert_eq!(
            WindowKind::from_label(&tear_off_options().ghost_label),
            Some(WindowKind::TearGhost)
        );
        assert_eq!(GHOST_LABEL, tear_off_options().ghost_label);
    }

    #[test]
    fn debug_builds_log_everything() {
        // Tests always run with debug assertions on.
        assert_eq!(log_level(), log::LevelFilter::Trace);
    }
}
