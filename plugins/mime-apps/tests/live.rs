// Prints what this machine answers for a few types; ignored by default, and read-only
//
// Run with `cargo test -p tauri-plugin-mime-apps --test live -- --ignored --nocapture`. Neither test starts an application, opens a dialog or changes a default: they only ask.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![cfg(any(target_os = "linux", target_os = "windows"))]

use tauri_plugin_mime_apps::{target, Backend};

#[cfg(target_os = "linux")]
use tauri_plugin_mime_apps::linux::Platform;
#[cfg(target_os = "windows")]
use tauri_plugin_mime_apps::windows::Platform;

fn print(platform: &Platform, label: &str, location: &str) {
    let target = target::parse(location).expect("a valid location");
    let info = platform.type_info(&target, false).expect("type info");
    println!("== {label}: {} ({})", info.mime, info.description);
    println!("   icon: {:?}", info.icon);
    match platform.handlers(&[target]) {
        Ok(handlers) => {
            let names = |apps: &[tauri_plugin_mime_apps::App]| {
                apps.iter()
                    .map(|app| format!("{} [{}]", app.name, app.id))
                    .collect::<Vec<_>>()
            };
            println!(
                "   default: {:?}",
                handlers
                    .default
                    .map(|app| format!("{} [{}]", app.name, app.id))
            );
            println!("   recommended: {:?}", names(&handlers.recommended));
            println!(
                "   others: {} apps, first few {:?}",
                handlers.others.len(),
                &names(&handlers.others)[..handlers.others.len().min(5)]
            );
        }
        Err(error) => println!("   handlers: {error}"),
    }
}

#[test]
#[ignore = "reads this machine's associations"]
#[cfg(target_os = "linux")]
fn live_handlers() {
    let platform = Platform::new();
    let status = platform.status();
    println!(
        "flavour {:?}, available {}",
        status.flavour, status.available
    );
    for feature in &status.features {
        println!(
            "   {} -> {} {:?}",
            feature.name, feature.available, feature.reason
        );
    }
    println!(
        "mimeapps.list files read, highest priority first: {:#?}",
        status.association_files
    );
    print(&platform, "image/png", "/nonexistent/picture.png");
    print(&platform, "text/plain", "/nonexistent/notes.txt");
    print(&platform, "inode/directory", "/tmp");
}

#[test]
#[ignore = "reads this machine's associations"]
#[cfg(target_os = "windows")]
fn live_handlers_windows() {
    let platform = Platform::new();
    let status = platform.status();
    println!(
        "flavour {:?}, available {}",
        status.flavour, status.available
    );
    print(&platform, ".png", r"C:\nonexistent\picture.png");
    print(&platform, ".txt", r"C:\nonexistent\notes.txt");
    print(&platform, "directory", r"C:\Windows\");
}

/// Draws file and folder icons through the real GTK icon theme. It needs a display and an icon theme, and skips with a message when there is none; it never fails for a missing system theme, only for a picture that is not a PNG. Run with `-- --nocapture` to see what it found.
#[test]
#[cfg(target_os = "linux")]
fn live_type_icons() {
    use tauri_plugin_mime_apps::typeicons::{FolderKind, IconKind, IconRequest};

    if gtk::init().is_err() {
        println!("skipped: GTK cannot start here (no display)");
        return;
    }
    let platform = Platform::new();
    let status = platform.status();
    if !status.has("typeIcons") || !status.has("folderIcons") {
        println!("skipped: the status says icons are unavailable: {status:?}");
        return;
    }
    println!("icon theme in force: {:?}", platform.type_icon_theme());
    let kinds = [
        IconKind::Folder(FolderKind::Plain),
        IconKind::Folder(FolderKind::Downloads),
        IconKind::Extension("png".into()),
        IconKind::Extension("pdf".into()),
        IconKind::Extension("zip".into()),
        IconKind::Mime("text/plain".into()),
    ];
    let mut drawn = 0;
    for kind in kinds {
        let request = IconRequest::new(kind.clone(), 32, 2, None);
        match platform.type_icon(&request) {
            Some(png) => {
                assert!(png.starts_with(b"\x89PNG"), "{kind:?} is not a PNG");
                println!("{kind:?}: {} bytes at {} px", png.len(), request.pixels());
                drawn += 1;
            }
            None => println!("{kind:?}: this theme has none"),
        }
    }
    println!("{drawn} of 6 drawn");
    // A theme named in the request draws from that theme, whatever the system is set to.
    for theme in ["Adwaita", "breeze"] {
        let request = IconRequest::new(
            IconKind::Folder(FolderKind::Plain),
            32,
            1,
            Some(theme.into()),
        );
        match platform.type_icon(&request) {
            Some(png) => {
                assert!(png.starts_with(b"\x89PNG"));
                println!("folder in {theme}: {} bytes", png.len());
            }
            None => println!("folder in {theme}: not installed or none"),
        }
    }
}
