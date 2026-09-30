// Runs a job on the thread that owns the window and awaits its result
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use tauri::{Runtime, WebviewWindow};
use tokio::sync::oneshot;

/// Runs `job` on the main thread and awaits its result; `None` if the event loop is gone. GTK,
/// Win32 menus and AppKit all require the thread that owns the window.
pub async fn run<R: Runtime, T: Send + 'static>(
    window: &WebviewWindow<R>,
    job: impl FnOnce() -> T + Send + 'static,
) -> Option<T> {
    let (tx, rx) = oneshot::channel();
    window
        .run_on_main_thread(move || {
            let _ = tx.send(job());
        })
        .ok()?;
    rx.await.ok()
}
