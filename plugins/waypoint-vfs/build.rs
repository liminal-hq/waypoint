// Declares the plugin's commands so Tauri generates their permissions
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

const COMMANDS: &[&str] = &[
    "get_status",
    "get_trash_info",
    "open_listing",
    "get_range",
    "set_sort",
    "set_filter",
    "close_listing",
    "refresh_listing",
    "get_home",
    "parse_location",
    "parse_location_text",
    "describe_location",
    "entry_location",
    "summarise_selection",
    "get_free_space",
    "check_folder",
    "open_entry",
    "entry_details",
    "folder_size",
    "cancel_folder_size",
    "scan_dir_sizes",
    "cancel_dir_scan",
    "get_cached_dir_scan",
    "read_text_head",
    "list_places",
    "add_favourite",
    "remove_favourite",
    "rename_favourite",
    "move_favourite",
    "list_connections",
    "connection_support",
    "suggested_servers",
    "parse_address_text",
    "add_connection",
    "update_connection",
    "duplicate_connection",
    "remove_connection",
    "move_connection",
    "forget_recent_server",
    "forget_login",
    "connect",
    "cancel_connect",
    "test_connection",
    "disconnect",
    "connection_state",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .global_api_script_path("./api-iife.js")
        .build();
}
