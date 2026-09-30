// Declares the plugin's commands so Tauri generates their permissions
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

const COMMANDS: &[&str] = &[
    "get_status",
    "open_listing",
    "get_range",
    "set_sort",
    "set_filter",
    "close_listing",
    "get_home",
    "list_places",
    "add_favourite",
    "remove_favourite",
    "rename_favourite",
    "move_favourite",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .global_api_script_path("./api-iife.js")
        .build();
}
