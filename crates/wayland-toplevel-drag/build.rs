// Compiles the C interposer that records GDK's `xdg_toplevel` and `wl_data_device` proxies
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

fn main() {
    println!("cargo:rerun-if-changed=shim.c");
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("linux") {
        return;
    }
    cc::Build::new()
        .file("shim.c")
        .warnings(true)
        .compile("wayland_toplevel_drag_shim");
}
