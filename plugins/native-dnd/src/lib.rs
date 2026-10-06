// Registers the native-dnd plugin: normalised inbound file drops, outbound file drags and the file clipboard
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod commands;
mod error;
pub mod hover;
pub mod inbound;
mod main_thread;
pub mod models;
pub mod outbound;
pub mod status;
pub mod uri;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux as platform_impl;

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
mod windows_hover;
#[cfg(target_os = "windows")]
use windows as platform_impl;

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
mod unsupported;
#[cfg(not(any(target_os = "linux", target_os = "windows")))]
use unsupported as platform_impl;

/// The platform the plugin was built for, behind one name.
mod platform {
    pub use super::platform_impl::*;

    /// What the platform knows about a drag that the runtime's own event does not carry.
    #[derive(Debug, Clone, Default)]
    pub struct InboundExtras {
        /// The drag's `text/uri-list` read losslessly, normalised.
        pub raw_uris: Option<Vec<String>>,
        /// The platform's sign that the drag began in this process.
        pub source_is_ours: bool,
        /// The modifiers at the drag's last motion, where the platform recorded them.
        pub modifiers: Option<crate::models::Modifiers>,
        /// The action negotiated at the drag's last motion, where the platform reports one.
        pub action: Option<crate::models::DragAction>,
    }
}

use std::{
    sync::{Arc, Mutex},
    time::Instant,
};

use log::warn;
use serde::Serialize;
use tauri::{
    plugin::{Builder, TauriPlugin},
    AppHandle, DragDropEvent, Emitter, Manager, RunEvent, Runtime, WebviewEvent, WebviewWindow,
    WindowEvent,
};

pub use error::{Error, Result};
pub use models::*;

use inbound::{Env, RawEvent, Translated};
use outbound::{Begun, DragRequest, Driver, EndSink, Finisher, Outbound};

/// Extensions to [`tauri::App`], [`tauri::AppHandle`] and [`tauri::Window`] to access the native-dnd APIs.
pub trait NativeDndExt<R: Runtime> {
    fn native_dnd(&self) -> &NativeDnd<R>;
}

impl<R: Runtime, T: Manager<R>> NativeDndExt<R> for T {
    fn native_dnd(&self) -> &NativeDnd<R> {
        self.state::<NativeDnd<R>>().inner()
    }
}

/// The plugin's handle: the same operations the commands run, for Rust callers.
pub struct NativeDnd<R: Runtime> {
    app: AppHandle<R>,
    outbound: Arc<Outbound>,
    /// Where an outbound drag has been reported to the windows under it, where the platform holds its own events back (Windows).
    hover: Arc<Mutex<hover::Hover>>,
}

/// Starts a drag through the platform module.
struct NativeDriver<'a, R: Runtime> {
    app: &'a AppHandle<R>,
    window: &'a WebviewWindow<R>,
}

impl<R: Runtime> Driver for NativeDriver<'_, R> {
    fn available(&self) -> Result<()> {
        match platform::display_server() {
            DisplayServer::None => Err(Error::Unsupported(platform::unavailable_reason())),
            _ => Ok(()),
        }
    }

    fn primary_button_down(&self) -> bool {
        platform::primary_button_down(self.window)
    }

    fn begin(&self, id: u32, request: &DragRequest, finisher: Finisher) -> Result<Begun> {
        platform::begin_drag(self.app, self.window, id, request, finisher)
    }

    fn cancel(&self, id: u32) {
        platform::cancel_drag(id)
    }
}

impl<R: Runtime> NativeDnd<R> {
    fn new(app: AppHandle<R>) -> Self {
        NativeDnd {
            app,
            outbound: Arc::new(Outbound::default()),
            hover: Arc::new(Mutex::new(hover::Hover::default())),
        }
    }

    /// Which features work on this system, and why the others do not.
    pub async fn status(&self) -> PluginStatus {
        let probed = main_thread::run(&self.app, || {
            (platform::display_server(), platform::unavailable_reason())
        })
        .await;
        let (server, reason) = probed.unwrap_or((
            DisplayServer::None,
            "the event loop is not running".to_string(),
        ));
        status::status_for(server, &reason)
    }

    /// Starts an outbound drag from `window` on the main thread. See the `start_drag` command.
    pub async fn start_drag(
        &self,
        window: &WebviewWindow<R>,
        request: StartDragRequest,
    ) -> Result<StartDragReport> {
        let outbound = self.outbound.clone();
        let app = self.app.clone();
        let window = window.clone();
        main_thread::run(&self.app, move || {
            let label = window.label().to_string();
            let emit: EndSink = {
                let app = app.clone();
                Arc::new(move |ended| {
                    if let Err(error) = app.emit_to(label.as_str(), DRAG_ENDED_EVENT, &ended) {
                        warn!("native-dnd: cannot tell `{label}` the drag ended: {error}");
                    }
                })
            };
            let driver = NativeDriver {
                app: &app,
                window: &window,
            };
            outbound::start(&outbound, &driver, &request, emit)
        })
        .await
        .unwrap_or_else(|| Err(Error::Failed("the event loop is not running".into())))
    }

    /// Puts `files` on the clipboard.
    pub async fn set_files(&self, files: ClipboardFiles) -> Result<()> {
        let files = ClipboardFiles {
            uris: files
                .uris
                .iter()
                .map(|uri| {
                    uri::normalise(uri.as_bytes())
                        .ok_or_else(|| Error::Invalid(format!("not a file URI: {uri}")))
                })
                .collect::<Result<_>>()?,
            cut: files.cut,
        };
        if files.uris.is_empty() {
            return Err(Error::Invalid(
                "there are no files to put on the clipboard".into(),
            ));
        }
        main_thread::run(&self.app, move || platform::set_files(&files))
            .await
            .unwrap_or_else(|| Err(Error::Failed("the event loop is not running".into())))
    }

    /// The files on the clipboard, or `None`.
    pub async fn get_files(&self) -> Result<Option<ClipboardFiles>> {
        main_thread::run(&self.app, platform::get_files)
            .await
            .unwrap_or_else(|| Err(Error::Failed("the event loop is not running".into())))
    }

    /// The record of where an outbound drag has been reported, for the platform glue.
    #[cfg(target_os = "windows")]
    pub(crate) fn hover(&self) -> Arc<Mutex<hover::Hover>> {
        self.hover.clone()
    }

    /// Whether an outbound drag started by this process is running.
    pub fn outbound_active(&self) -> bool {
        self.outbound.is_active()
    }
}

fn emit<R: Runtime, S: Serialize + Clone>(
    app: &AppHandle<R>,
    label: &str,
    event: &str,
    payload: &S,
) {
    if let Err(error) = app.emit_to(label, event, payload) {
        warn!("native-dnd: cannot send `{event}` to `{label}`: {error}");
    }
}

/// Normalises a runtime drag-drop event for the window `label` and sends it to that window. Runs on the main thread.
fn on_drag_drop<R: Runtime>(app: &AppHandle<R>, label: &str, event: &DragDropEvent) {
    let Some(state) = app.try_state::<NativeDnd<R>>() else {
        return;
    };
    let raw = match event {
        DragDropEvent::Enter { paths, position } => RawEvent::Enter {
            paths: paths.clone(),
            position: (position.x, position.y),
        },
        DragDropEvent::Over { position } => RawEvent::Over {
            position: (position.x, position.y),
        },
        DragDropEvent::Drop { paths, position } => RawEvent::Drop {
            paths: paths.clone(),
            position: (position.x, position.y),
        },
        DragDropEvent::Leave => RawEvent::Leave,
        _ => return,
    };
    // A window that was already told about this drag while the platform held its events back must not hear them again; its drop it does hear.
    let kind = match raw {
        RawEvent::Enter { .. } => hover::Real::Enter,
        RawEvent::Over { .. } => hover::Real::Over,
        RawEvent::Drop { .. } => hover::Real::Drop,
        RawEvent::Leave => hover::Real::Leave,
    };
    let delivered = state
        .hover
        .lock()
        .map_or(true, |mut hover| hover.deliver(kind, label, Instant::now()));
    if !delivered {
        return;
    }
    let scale_factor = app
        .get_webview_window(label)
        .and_then(|window| window.scale_factor().ok())
        .unwrap_or(1.0);
    // The raw list and the source sign are per drag: a drop and a leave end it.
    let ends = matches!(raw, RawEvent::Drop { .. } | RawEvent::Leave);
    let extras = platform::inbound_extras(label, ends);
    let env = Env {
        unit: platform::POSITION_UNIT,
        scale_factor,
        modifiers: extras.modifiers.unwrap_or_else(platform::modifiers_now),
        action: extras.action,
        raw_uris: extras.raw_uris,
        source_is_ours: extras.source_is_ours,
        now: Instant::now(),
    };
    match inbound::translate(label, raw, &env, &state.outbound) {
        Translated::Enter(event) => emit(app, label, ENTER_EVENT, &event),
        Translated::Over(event) => emit(app, label, OVER_EVENT, &event),
        Translated::Drop(event) => emit(app, label, DROP_EVENT, &event),
        Translated::Leave(event) => emit(app, label, LEAVE_EVENT, &event),
    }
}

/// Registers the plugin. It adds no drop target of its own: it normalises the drag-drop events of windows whose handler is on (a window is built with `disable_drag_drop_handler()` to turn that off), and starts outbound drags and the file clipboard on request.
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("native-dnd")
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::start_drag,
            commands::set_files,
            commands::get_files,
        ])
        .setup(|app, _api| {
            app.manage(NativeDnd::new(app.clone()));
            Ok(())
        })
        .on_webview_ready(|webview| platform::on_webview_ready(&webview))
        .on_event(|app, event| match event {
            RunEvent::Ready => platform::on_ready(app),
            RunEvent::Exit => platform::on_exit(app),
            RunEvent::WindowEvent {
                label,
                event: WindowEvent::DragDrop(event),
                ..
            }
            | RunEvent::WebviewEvent {
                label,
                event: WebviewEvent::DragDrop(event),
                ..
            } => on_drag_drop(app, label, event),
            _ => {}
        })
        .build()
}

#[cfg(test)]
mod tests;
