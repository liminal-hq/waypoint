// Windows specifics: the platform and the mouse button state
//
// The cursor itself comes from `tao`, which reads `GetCursorPos`. The ghost is `focusable(false)` (`WS_EX_NOACTIVATE`) and every show asks for `SW_SHOWNOACTIVATE`, because `tao` only does so for the first show of a window. Run on Windows 11 in the milestone 3 verification: the ghost, its hit-test and its click-through behave, and with this show the source window keeps focus.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use tauri::{Runtime, WebviewWindow};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VIRTUAL_KEY, VK_LBUTTON, VK_RBUTTON,
};
use windows::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_SHOWNOACTIVATE};

use crate::status::Platform;

pub fn platform() -> Platform {
    Platform::Windows
}

/// Whether the left or right mouse button is held. `GetAsyncKeyState` reports the state now, with the high bit set while the button is down.
pub fn buttons_held() -> bool {
    // SAFETY: `GetAsyncKeyState` takes a virtual-key code and reads global input state.
    let state = |key: VIRTUAL_KEY| unsafe { GetAsyncKeyState(i32::from(key.0)) } < 0;
    state(VK_LBUTTON) || state(VK_RBUTTON)
}

/// Shows the ghost with `SW_SHOWNOACTIVATE`, so the window being dragged from keeps activation and keyboard focus. Falls back to a plain `show` when the window has no handle.
pub fn show_without_activating<R: Runtime>(ghost: &WebviewWindow<R>) {
    match ghost.hwnd() {
        // SAFETY: `ShowWindow` takes the handle of a live window of this process and a show command.
        Ok(handle) => unsafe {
            let _ = ShowWindow(HWND(handle.0 as _), SW_SHOWNOACTIVATE);
        },
        Err(_) => {
            let _ = ghost.show();
        }
    }
}
