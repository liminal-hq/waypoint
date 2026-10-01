// Generates plugin metadata and permission manifests for commands
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

const COMMANDS: &[&str] = &["get_status", "trash", "list", "restore", "delete", "empty"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .global_api_script_path("./api-iife.js")
        .build();
    common_controls_manifest();
}

/// The tests build a mock Tauri app, which links `wry` and `muda`; their imports from `comctl32` (`TaskDialogIndirect`) exist only in version 6, which a binary gets only from a manifest. Without one the test executable does not start (`STATUS_ENTRYPOINT_NOT_FOUND`).
fn common_controls_manifest() {
    let target = |key: &str| std::env::var(format!("CARGO_CFG_TARGET_{key}")).unwrap_or_default();
    if target("OS") != "windows" || target("ENV") != "msvc" {
        return;
    }
    println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
    println!(
        "cargo:rustc-link-arg=/MANIFESTDEPENDENCY:type='win32' name='Microsoft.Windows.Common-Controls' version='6.0.0.0' processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'"
    );
}
