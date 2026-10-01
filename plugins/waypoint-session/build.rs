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
    "pin_tab",
    "set_tab_colour",
    "set_tab_hints",
    "reopen_tab",
    "create_group",
    "add_to_group",
    "remove_from_group",
    "rename_group",
    "set_group_colour",
    "set_group_collapsed",
    "collapse_other_groups",
    "sort_group",
    "duplicate_group",
    "move_group",
    "ungroup",
    "close_group",
    "save_group_as_workspace",
    "rename_workspace",
    "delete_workspace",
    "set_active_workspace",
    "set_workspace_locations",
    "join_pair",
    "separate_pair",
    "set_pair_layout",
    "set_pair_sizes",
    "swap_panes",
    "toggle_split",
    "open_window",
    "close_window",
    "set_geometry",
    "set_view",
    "move_tabs",
    "list_windows",
    "get_status",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .global_api_script_path("./api-iife.js")
        .build();
}
