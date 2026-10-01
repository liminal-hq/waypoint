// Registers the plugin's commands and Android manifest permissions
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

const COMMANDS: &[&str] = &[
    "get_status",
    "get_time_format",
    "get_animator_duration_scale",
    "open_notification_settings",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .global_api_script_path("./api-iife.js")
        .android_path("android")
        .build();

    inject_android_permissions()
        .expect("Failed to inject Android manifest permissions for os-prefs");
}

fn inject_android_permissions() -> std::io::Result<()> {
    // The plugin needs no Android permissions; the block keeps the injection mechanism in place.
    tauri_plugin::mobile::update_android_manifest(
        "tauri-plugin-os-prefs.permissions",
        "manifest",
        String::new(),
    )
    .map_err(std::io::Error::other)
}
