// Declares the plugin's commands so Tauri generates their permissions
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

const COMMANDS: &[&str] = &[
    "get_snapshot",
    "open_tab",
    "close_tab",
    "activate_tab",
    "move_tab",
    "navigate",
    "back",
    "forward",
    "get_status",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .global_api_script_path("./api-iife.js")
        .build();
}
