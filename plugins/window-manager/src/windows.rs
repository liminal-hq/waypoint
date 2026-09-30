// Reports Windows window manager capabilities; the system window menu is a follow-up
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use tauri::{Runtime, WebviewWindow};

use crate::models::{Session, WindowCapabilities, WindowPosition};

pub async fn capabilities<R: Runtime>(_window: &WebviewWindow<R>) -> WindowCapabilities {
    WindowCapabilities::new(Session::Windows, true, false)
}

pub async fn show_system_window_menu<R: Runtime>(
    _window: &WebviewWindow<R>,
    _position: WindowPosition,
) -> bool {
    false
}
