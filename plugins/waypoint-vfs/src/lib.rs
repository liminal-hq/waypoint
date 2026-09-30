// Registers the file system plugin. Commands for listings and places arrive in later slices.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod commands;
mod error;

use tauri::{
    plugin::{Builder, TauriPlugin},
    Runtime,
};

pub use error::Error;

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("waypoint-vfs")
        .invoke_handler(tauri::generate_handler![commands::get_status])
        .build()
}
