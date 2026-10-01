// Windows specifics: the platform and the mouse button state
//
// The cursor itself comes from `tao`, which reads `GetCursorPos`, and the ghost stays out of the way through `focusable(false)`, which `tao` maps to `WS_EX_NOACTIVATE`. This module has not been run on Windows.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VIRTUAL_KEY, VK_LBUTTON, VK_RBUTTON,
};

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
