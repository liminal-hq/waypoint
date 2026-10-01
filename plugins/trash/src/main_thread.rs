// Runs a job on the main thread, which owns the apartment the shell's COM objects need, and awaits its result
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use tauri::{AppHandle, Runtime};
use tokio::sync::oneshot;

/// Runs `job` on the main thread and awaits its result; `None` if the event loop is gone. The shell's file operations are COM objects that want the single-threaded apartment the event loop's thread runs.
pub async fn run<R: Runtime, T: Send + 'static>(
    app: &AppHandle<R>,
    job: impl FnOnce() -> T + Send + 'static,
) -> Option<T> {
    let (tx, rx) = oneshot::channel();
    app.run_on_main_thread(move || {
        let _ = tx.send(job());
    })
    .ok()?;
    rx.await.ok()
}
