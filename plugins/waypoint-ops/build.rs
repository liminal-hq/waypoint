// Declares the plugin's commands so Tauri generates their permissions
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

const COMMANDS: &[&str] = &[
    "get_status",
    "get_snapshot",
    "plan",
    "preview_batch_rename",
    "submit",
    "pause",
    "resume",
    "cancel",
    "retry",
    "dismiss",
    "dismiss_finished",
    "reorder",
    "resolve",
    "resolve_error",
    "undo",
    "redo",
    "journal_summaries",
    "journal_entry_of",
    "subscribe_progress",
    "unsubscribe_progress",
    "set_clipboard",
    "set_clipboard_from_selection",
    "get_clipboard",
    "jobs_targeting",
    "get_settings",
    "set_settings",
    "take_recovery_report",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .global_api_script_path("./api-iife.js")
        .build();
}
