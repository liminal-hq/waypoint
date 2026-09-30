// Shows the app's Window menu at the pointer, the nearest macOS has to a window's system menu
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use log::{info, warn};
use objc2::{rc::Retained, MainThreadMarker};
use objc2_app_kit::{NSApplication, NSView};
use objc2_foundation::NSPoint;
use tauri::{Runtime, WebviewWindow, Window};

use crate::{
    main_thread,
    models::{WindowCapabilities, WindowPosition},
};

/// A frameless macOS window has no system window menu, so the nearest equivalent is the
/// application's Window menu; it is offered only when the app has one.
/// AppKit keeps the window level on the window itself, so Tauri's read is authoritative.
pub async fn always_on_top<R: Runtime>(window: &WebviewWindow<R>) -> Option<bool> {
    window.is_always_on_top().ok()
}

/// Nothing to watch: the Window menu has no Always on Top entry, so only the app changes it.
pub fn watch_always_on_top<R: Runtime>(_window: &Window<R>) {}

pub async fn capabilities<R: Runtime>(window: &WebviewWindow<R>) -> WindowCapabilities {
    let has_window_menu = main_thread::run(window, has_window_menu)
        .await
        .unwrap_or(false);
    WindowCapabilities::macos(has_window_menu)
}

fn has_window_menu() -> bool {
    MainThreadMarker::new().is_some_and(|mtm| {
        NSApplication::sharedApplication(mtm)
            .windowsMenu()
            .is_some()
    })
}

/// Pops the application's Window menu up at `position` inside the window's content view.
///
/// This native path has not been run on macOS yet. Every step logs before it calls into AppKit, so
/// if the process crashes inside one of them the last "untested native path" line in the log names
/// it.
pub async fn show_system_window_menu<R: Runtime>(
    window: &WebviewWindow<R>,
    position: WindowPosition,
) -> bool {
    let label = window.label().to_string();
    info!(
        "macos window menu (untested native path): requested for label={label} at ({}, {})",
        position.x, position.y
    );

    let target = window.clone();
    let thread_label = label.clone();
    let shown = main_thread::run(window, move || show_menu(&target, position, &thread_label)).await;
    let Some(shown) = shown else {
        warn!("macos window menu (untested native path): cannot reach the main thread for label={label}");
        return false;
    };
    info!("macos window menu (untested native path): finished for label={label}, shown={shown}");
    shown
}

/// Must run on the main thread.
fn show_menu<R: Runtime>(window: &WebviewWindow<R>, position: WindowPosition, label: &str) -> bool {
    let Some(mtm) = MainThreadMarker::new() else {
        warn!("macos window menu (untested native path): not on the main thread for label={label}");
        return false;
    };

    info!("macos window menu (untested native path): looking up the Window menu for label={label}");
    let Some(menu) = NSApplication::sharedApplication(mtm).windowsMenu() else {
        warn!("macos window menu (untested native path): the app has no Window menu for label={label}");
        return false;
    };

    // The view is looked up here, on the main thread, rather than before the job was queued: a
    // window can close in between, and a pointer taken earlier would then dangle. Retaining it keeps
    // it alive for the whole (modal) menu call.
    let raw_view = match window.ns_view() {
        Ok(raw_view) => raw_view,
        Err(error) => {
            warn!("macos window menu (untested native path): no content view for label={label}: {error}");
            return false;
        }
    };
    // SAFETY: Tauri returns either null or the window's content view, an `NSView`, and this runs on
    // the main thread that owns it, so nothing can release it between this check and the retain.
    let Some(view) = (unsafe { Retained::retain(raw_view.cast::<NSView>()) }) else {
        warn!(
            "macos window menu (untested native path): the content view is gone for label={label}"
        );
        return false;
    };

    // AppKit views put the origin at the bottom-left unless they are flipped, while the page
    // reports coordinates from the top-left.
    let y = if view.isFlipped() {
        position.y
    } else {
        view.bounds().size.height - position.y
    };
    let location = NSPoint::new(position.x, y);

    info!(
        "macos window menu (untested native path): calling popUpMenuPositioningItem for label={label} at view ({}, {}); this is modal, so a crash logged after this line happened inside the menu",
        location.x, location.y
    );
    let shown = menu.popUpMenuPositioningItem_atLocation_inView(None, location, Some(&view));
    info!("macos window menu (untested native path): popUpMenuPositioningItem returned {shown} for label={label}");
    shown
}
