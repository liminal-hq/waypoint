// A manual check of the whole route through the real `pkexec` and a running polkit agent
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

// It cannot run unattended: it needs a graphical session with a polkit authentication agent, an installed helper and policy, and a person to authenticate. Run it by hand with the helper, the policy and the helper's ready line in the environment:
//
//     ELEVATE_LIVE_HELPER=/usr/libexec/tool/helper ELEVATE_LIVE_POLICY=/usr/share/polkit-1/actions/tool.policy \
//     ELEVATE_LIVE_READY='tool ready' cargo test -p tauri-plugin-elevate --test live -- --ignored --nocapture

#![cfg(target_os = "linux")]

use tauri_plugin_elevate::{Config, Elevator};

#[test]
#[ignore = "manual: needs a polkit agent, an installed helper and a person to authenticate"]
fn live_pkexec_starts_the_helper() {
    let var = |name: &str| std::env::var(name).unwrap_or_else(|_| panic!("set {name}"));
    let elevator = Elevator::new(Config::new(
        var("ELEVATE_LIVE_HELPER"),
        var("ELEVATE_LIVE_POLICY"),
        var("ELEVATE_LIVE_READY"),
    ));
    let status = elevator.status();
    assert!(status.available, "{:?}", status.reason);
    let stream = elevator
        .launch(&|| false)
        .unwrap_or_else(|error| panic!("{error}"));
    // Closing the pipes ends the helper.
    drop(stream);
}
