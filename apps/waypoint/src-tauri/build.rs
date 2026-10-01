// Runs the Tauri build step that generates the app context and schemas, and exports the Wayland interposer
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

fn main() {
    tauri_build::build();

    // `wayland-toplevel-drag` records GTK's `xdg_toplevel` and `wl_data_device` by defining
    // `wl_proxy_marshal_flags` (the call GDK's inlined protocol code makes) and forwarding to
    // libwayland. For that definition to win symbol resolution over libwayland's own, the
    // executable has to export it: `-rdynamic` puts the executable's symbols in its dynamic table,
    // and `--undefined` keeps the linker from dropping the otherwise unreferenced definition. A
    // library crate cannot add link arguments downstream, so they are set here, for the binary
    // only. Without them the app still runs, and the plugin reports the real-window drag
    // unavailable ("not linked with the Wayland proxy interposer").
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("linux") {
        println!("cargo:rustc-link-arg-bins=-rdynamic");
        println!("cargo:rustc-link-arg-bins=-Wl,--undefined=wl_proxy_marshal_flags");
    }
}
