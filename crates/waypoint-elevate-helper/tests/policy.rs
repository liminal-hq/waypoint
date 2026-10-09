// The polkit policy shipped for the helper: the action, its message, its defaults and the one program it admits.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::PathBuf;

fn policy() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../apps/waypoint/src-tauri/packaging/linux/ca.liminalhq.waypoint.admin.policy");
    std::fs::read_to_string(path).unwrap()
}

/// The text of the first `<tag ...>text</tag>` after `from`.
fn element<'a>(text: &'a str, tag: &str) -> &'a str {
    let open = text.find(&format!("<{tag}")).unwrap();
    let start = open + text[open..].find('>').unwrap() + 1;
    let end = start + text[start..].find(&format!("</{tag}>")).unwrap();
    &text[start..end]
}

fn annotation(text: &str, key: &str) -> String {
    let at = text.find(&format!("<annotate key=\"{key}\">")).unwrap();
    element(&text[at..], "annotate").to_string()
}

#[test]
fn the_policy_declares_one_action_with_the_agreed_id_and_message() {
    let text = policy();
    assert!(text.contains("<!DOCTYPE policyconfig PUBLIC"));
    assert_eq!(text.matches("<action ").count(), 1);
    assert!(text.contains("<action id=\"ca.liminalhq.waypoint.admin\">"));
    let message = "Waypoint wants to read and change files as an administrator";
    assert_eq!(element(&text, "description"), message);
    assert_eq!(element(&text, "message"), message);
}

#[test]
fn only_an_active_local_administrator_can_authenticate_and_it_is_remembered() {
    let text = policy();
    assert_eq!(element(&text, "allow_any"), "no");
    assert_eq!(element(&text, "allow_inactive"), "no");
    assert_eq!(element(&text, "allow_active"), "auth_admin_keep");
}

#[test]
fn the_action_is_bound_to_the_helper_path_and_has_no_gui() {
    let text = policy();
    assert_eq!(
        annotation(&text, "org.freedesktop.policykit.exec.path"),
        "/usr/libexec/waypoint/waypoint-elevate-helper"
    );
    assert_eq!(
        annotation(&text, "org.freedesktop.policykit.exec.allow_gui"),
        "false"
    );
}
