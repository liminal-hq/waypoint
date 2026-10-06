// Reports every feature unavailable on targets without native drag and drop support, such as macOS, Android and iOS
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use tauri::{AppHandle, Runtime, Webview, WebviewWindow};

use crate::{
    error::{Error, Result},
    inbound::PositionUnit,
    models::{ClipboardFiles, DisplayServer, Modifiers},
    outbound::{Begun, DragRequest, Finisher},
    platform::InboundExtras,
};

pub const POSITION_UNIT: PositionUnit = PositionUnit::Logical;

pub const UNAVAILABLE: &str = "native drag and drop is not supported on this platform";

pub fn display_server() -> DisplayServer {
    DisplayServer::None
}

pub fn unavailable_reason() -> String {
    UNAVAILABLE.to_string()
}

pub fn primary_button_down<R: Runtime>(_window: &WebviewWindow<R>) -> bool {
    false
}

pub fn modifiers_now() -> Modifiers {
    Modifiers::default()
}

pub fn begin_drag<R: Runtime>(
    _app: &AppHandle<R>,
    _window: &WebviewWindow<R>,
    _id: u32,
    _request: &DragRequest,
    _finisher: Finisher,
) -> Result<Begun> {
    Err(Error::Unsupported(UNAVAILABLE.into()))
}

pub fn cancel_drag(_id: u32) {}

pub fn set_files(_files: &ClipboardFiles) -> Result<()> {
    Err(Error::Unsupported(UNAVAILABLE.into()))
}

pub fn get_files() -> Result<Option<ClipboardFiles>> {
    Err(Error::Unsupported(UNAVAILABLE.into()))
}

pub fn on_webview_ready<R: Runtime>(_webview: &Webview<R>) {}

pub fn on_ready<R: Runtime>(_app: &AppHandle<R>) {}

pub fn on_exit<R: Runtime>(_app: &AppHandle<R>) {}

pub fn inbound_extras(_label: &str, _consume: bool) -> InboundExtras {
    InboundExtras::default()
}
