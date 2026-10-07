// Shows the Windows system menu for a window through `GetSystemMenu` and `TrackPopupMenu`
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use log::{info, warn};
use tauri::{Runtime, WebviewWindow, Window};
use windows::Win32::{
    Foundation::{HWND, LPARAM, POINT, WPARAM},
    Graphics::Gdi::ClientToScreen,
    UI::WindowsAndMessaging::{
        EnableMenuItem, GetSystemMenu, GetWindowLongPtrW, IsIconic, IsZoomed, PostMessageW,
        SetForegroundWindow, TrackPopupMenu, GWL_STYLE, HMENU, MF_BYCOMMAND, MF_ENABLED, MF_GRAYED,
        SC_MAXIMIZE, SC_MINIMIZE, SC_MOVE, SC_RESTORE, SC_SIZE, TPM_RETURNCMD, TPM_RIGHTBUTTON,
        WM_NULL, WM_SYSCOMMAND, WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_THICKFRAME,
    },
};

use crate::{
    main_thread,
    models::{WindowCapabilities, WindowPosition},
    system_menu::{system_menu_state, WindowFacts},
};

pub async fn capabilities<R: Runtime>(_window: &WebviewWindow<R>) -> WindowCapabilities {
    WindowCapabilities::windows()
}

/// Windows keeps the topmost state on the window itself, so Tauri's read is authoritative.
pub async fn always_on_top<R: Runtime>(window: &WebviewWindow<R>) -> Option<bool> {
    window.is_always_on_top().ok()
}

/// Nothing to watch: the Windows system menu has no Always on Top entry, so only the app changes it.
pub fn watch_always_on_top<R: Runtime>(_window: &Window<R>) {}

/// Shows the window's system menu (Restore, Move, Size, Minimise, Maximise, Close) at `position`
/// and sends the chosen command back to the window.
///
/// This native path has not been run on Windows yet. Every step logs before it calls into Win32,
/// so if the process crashes inside one of them the last "untested native path" line in the log
/// names it.
pub async fn show_system_window_menu<R: Runtime>(
    window: &WebviewWindow<R>,
    position: WindowPosition,
) -> bool {
    let label = window.label().to_string();
    info!(
        "windows system menu (untested native path): requested for label={label} at ({}, {})",
        position.x, position.y
    );

    // `TrackPopupMenu` runs a modal loop and must run on the thread that owns the window. The
    // handle is resolved inside that job, not before it is queued: a window can close in between,
    // and Windows reuses handle values, so an earlier handle could name an unrelated new window.
    let target = window.clone();
    let thread_label = label.clone();
    let shown = main_thread::run(window, move || {
        let hwnd = match target.hwnd() {
            Ok(hwnd) => hwnd,
            Err(error) => {
                warn!("windows system menu (untested native path): no window handle for label={thread_label}: {error}");
                return false;
            }
        };
        let scale = target.scale_factor().unwrap_or(1.0);
        show_menu(hwnd, position, scale, &thread_label)
    })
    .await;
    let Some(shown) = shown else {
        warn!("windows system menu (untested native path): cannot reach the main thread for label={label}");
        return false;
    };
    info!("windows system menu (untested native path): finished for label={label}, shown={shown}");
    shown
}

/// Sets the entries that depend on the window's state to match it. `TrackPopupMenu` does not give
/// the system the chance it has when the title bar is clicked, so without this a maximised window's
/// menu offers Maximise and greys Restore.
///
/// Must run on the thread that owns `hwnd`.
fn refresh_system_menu(hwnd: HWND, menu: HMENU) {
    // SAFETY: `hwnd` is the live handle of a window owned by this process, used on its own thread.
    let (maximised, minimised, style) = unsafe {
        (
            IsZoomed(hwnd).as_bool(),
            IsIconic(hwnd).as_bool(),
            GetWindowLongPtrW(hwnd, GWL_STYLE) as u32,
        )
    };
    let state = system_menu_state(WindowFacts {
        maximised,
        minimised,
        can_size: style & WS_THICKFRAME.0 != 0,
        can_minimise: style & WS_MINIMIZEBOX.0 != 0,
        can_maximise: style & WS_MAXIMIZEBOX.0 != 0,
    });
    for (command, enabled) in [
        (SC_RESTORE, state.restore),
        (SC_MOVE, state.move_window),
        (SC_SIZE, state.size),
        (SC_MINIMIZE, state.minimise),
        (SC_MAXIMIZE, state.maximise),
    ] {
        let flags = MF_BYCOMMAND | if enabled { MF_ENABLED } else { MF_GRAYED };
        // SAFETY: `menu` is this window's system menu, got from `GetSystemMenu` above.
        let _ = unsafe { EnableMenuItem(menu, command, flags) };
    }
}

/// Must run on the thread that owns `hwnd`.
fn show_menu(hwnd: HWND, position: WindowPosition, scale: f64, label: &str) -> bool {
    info!("windows system menu (untested native path): calling GetSystemMenu for label={label}");
    // SAFETY: `hwnd` is the live handle of a window owned by this process, used on its own thread.
    let menu = unsafe { GetSystemMenu(hwnd, false) };
    if menu.is_invalid() {
        warn!("windows system menu (untested native path): the window has no system menu for label={label}");
        return false;
    }

    refresh_system_menu(hwnd, menu);

    // The page reports CSS pixels from the window's top-left; Win32 wants physical screen pixels.
    let mut point = POINT {
        x: (position.x * scale).round() as i32,
        y: (position.y * scale).round() as i32,
    };
    info!(
        "windows system menu (untested native path): calling ClientToScreen for label={label} at client ({}, {})",
        point.x, point.y
    );
    // SAFETY: `hwnd` is valid (above) and `point` is a live, writable POINT.
    if !unsafe { ClientToScreen(hwnd, &mut point) }.as_bool() {
        warn!(
            "windows system menu (untested native path): ClientToScreen failed for label={label}"
        );
        return false;
    }

    info!(
        "windows system menu (untested native path): calling SetForegroundWindow for label={label}"
    );
    // SAFETY: `hwnd` is valid. A menu shown from a window that is not foreground may not dismiss
    // correctly, so this is requested first; its result is advisory.
    let _ = unsafe { SetForegroundWindow(hwnd) };

    info!(
        "windows system menu (untested native path): calling TrackPopupMenu for label={label} at screen ({}, {}); this is modal, so a crash logged after this line happened inside the menu",
        point.x, point.y
    );
    // SAFETY: `menu` and `hwnd` are valid. With `TPM_RETURNCMD` the call returns the chosen
    // command identifier, or 0 if the menu was dismissed, instead of posting it.
    let command = unsafe {
        TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_RIGHTBUTTON,
            point.x,
            point.y,
            None,
            hwnd,
            None,
        )
    }
    .0;
    info!("windows system menu (untested native path): TrackPopupMenu returned command={command} for label={label}");

    if command != 0 {
        info!("windows system menu (untested native path): posting WM_SYSCOMMAND {command} for label={label}");
        // SAFETY: `hwnd` is valid; the command identifier came from this window's own system menu.
        if let Err(error) = unsafe {
            PostMessageW(
                Some(hwnd),
                WM_SYSCOMMAND,
                WPARAM(command as usize),
                LPARAM(0),
            )
        } {
            warn!("windows system menu (untested native path): posting the command failed for label={label}: {error}");
        }
    }
    // A benign message after a popup lets the menu dismiss cleanly (see the `TrackPopupMenu` remarks).
    // SAFETY: `hwnd` is valid.
    let _ = unsafe { PostMessageW(Some(hwnd), WM_NULL, WPARAM(0), LPARAM(0)) };
    true
}
