// Shows the Windows system menu for a window through `GetSystemMenu` and `TrackPopupMenu`
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use log::{info, warn};
use tauri::{Runtime, WebviewWindow};
use windows::Win32::{
    Foundation::{HWND, LPARAM, POINT, WPARAM},
    Graphics::Gdi::ClientToScreen,
    UI::WindowsAndMessaging::{
        GetSystemMenu, PostMessageW, SetForegroundWindow, TrackPopupMenu, TPM_RETURNCMD,
        TPM_RIGHTBUTTON, WM_NULL, WM_SYSCOMMAND,
    },
};

use crate::{
    main_thread,
    models::{WindowCapabilities, WindowPosition},
};

pub async fn capabilities<R: Runtime>(_window: &WebviewWindow<R>) -> WindowCapabilities {
    WindowCapabilities::windows()
}

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

    let hwnd = match window.hwnd() {
        Ok(hwnd) => hwnd,
        Err(error) => {
            warn!("windows system menu (untested native path): no window handle for label={label}: {error}");
            return false;
        }
    };
    let scale = window.scale_factor().unwrap_or(1.0);
    // A raw window handle is a pointer and so is not `Send`; carry it as an integer and rebuild it
    // on the main thread, which owns the window.
    let raw_hwnd = hwnd.0 as isize;

    // `TrackPopupMenu` runs a modal loop and must run on the thread that owns the window.
    let thread_label = label.clone();
    let shown = main_thread::run(window, move || {
        let hwnd = HWND(raw_hwnd as *mut core::ffi::c_void);
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

/// Must run on the thread that owns `hwnd`.
fn show_menu(hwnd: HWND, position: WindowPosition, scale: f64, label: &str) -> bool {
    info!("windows system menu (untested native path): calling GetSystemMenu for label={label}");
    // SAFETY: `hwnd` is the live handle of a window owned by this process, used on its own thread.
    let menu = unsafe { GetSystemMenu(hwnd, false) };
    if menu.is_invalid() {
        warn!("windows system menu (untested native path): the window has no system menu for label={label}");
        return false;
    }

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
