// Reports no capabilities on targets without a window manager integration, such as Android and iOS
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use tauri::{Runtime, WebviewWindow, Window};

use crate::models::{WindowCapabilities, WindowPosition};

pub async fn capabilities<R: Runtime>(_window: &WebviewWindow<R>) -> WindowCapabilities {
    WindowCapabilities::unsupported()
}

pub async fn show_system_window_menu<R: Runtime>(
    _window: &WebviewWindow<R>,
    _position: WindowPosition,
) -> bool {
    false
}

pub async fn always_on_top<R: Runtime>(_window: &WebviewWindow<R>) -> Option<bool> {
    None
}

pub fn watch_always_on_top<R: Runtime>(_window: &Window<R>) {}
