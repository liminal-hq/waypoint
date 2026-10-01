// Reports every feature unavailable on targets without tear-off support, such as macOS, Android and iOS
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::status::Platform;

pub fn platform() -> Platform {
    Platform::Unsupported
}

pub fn buttons_held() -> bool {
    false
}

/// Only Windows shows the ghost without activating it (`Platform::needs_show_without_activating`); elsewhere this is a plain show.
pub fn show_without_activating<R: tauri::Runtime>(ghost: &tauri::WebviewWindow<R>) {
    let _ = ghost.show();
}
