// A manual check of the whole route through a real UAC prompt on Windows
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

// It cannot run unattended: it needs a person to answer the prompt, and the status only passes for an installed copy, so both the helper and this test program must sit under Program Files. Build it with `cargo test -p tauri-plugin-elevate --test live_windows --no-run`, copy the test executable next to the installed helper, and run it there with the helper path and its ready line in the environment:
//
//     $env:ELEVATE_LIVE_HELPER='C:\Program Files\Tool\helper.exe'; $env:ELEVATE_LIVE_READY='tool ready'
//     .\live_windows.exe --ignored --nocapture

#![cfg(target_os = "windows")]

use tauri_plugin_elevate::{Config, Elevator};

#[test]
#[ignore = "manual: needs an installed helper under Program Files and a person to answer the UAC prompt"]
fn live_uac_starts_the_helper() {
    let var = |name: &str| std::env::var(name).unwrap_or_else(|_| panic!("set {name}"));
    let elevator = Elevator::new(
        Config::new(var("ELEVATE_LIVE_HELPER"), "", var("ELEVATE_LIVE_READY"))
            .with_pipe_prefix("elevate-live"),
    );
    let status = elevator.status();
    assert!(status.available, "{:?}", status.reason);
    let stream = elevator
        .launch(&|| false)
        .unwrap_or_else(|error| panic!("{error}"));
    // Closing the pipes ends the helper.
    drop(stream);
}
