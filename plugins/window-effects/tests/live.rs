// Reads the real system: ignored tests to run by hand on a desktop (Linux) or in the Windows virtual machine
//
// Run with `cargo test -p tauri-plugin-window-effects --test live -- --ignored --nocapture`. They only read: nothing here changes a window or the desktop's settings.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

/// Prints what the plugin finds on this desktop session. The GTK display is opened here, so a Wayland or X11 session must be running.
#[cfg(target_os = "linux")]
#[test]
#[ignore = "reads the real desktop session"]
fn live_status() {
    gtk::init().expect("GTK needs a display: run this inside a desktop session");
    let env = tauri_plugin_window_effects::probe_environment();
    println!("{env:#?}");
    let status = tauri_plugin_window_effects::status_for(&env);
    println!("{}", serde_json::to_string_pretty(&status).unwrap());
    assert!(status.has("opacity") || !env.has_composite);
}

/// Reads the build number and checks the status agrees with it: Mica from build 22000.
#[cfg(target_os = "windows")]
#[test]
#[ignore = "reads the real Windows version"]
fn live_mica() {
    let build = tauri_plugin_window_effects::build_number().expect("RtlGetVersion answers");
    let env = tauri_plugin_window_effects::probe_environment();
    let status = tauri_plugin_window_effects::status_for(&env);
    println!("build {build}: {status:#?}");
    assert_eq!(status.has("mica"), build >= 22000);
    assert_eq!(status.has("acrylic"), build >= 17763);
    assert!(status.has("blur") && status.has("opacity"));
}
