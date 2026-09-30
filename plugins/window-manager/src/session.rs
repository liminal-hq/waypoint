// Detects the Linux windowing system from environment variables and the GDK backend name
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::models::Session;

/// Decides the Linux session from the environment, as strings so the rule is testable.
///
/// The rule, in order:
/// 1. `XDG_SESSION_TYPE` of `wayland` or `x11` (case-insensitive) decides. It is set by the login manager and is the most reliable hint.
/// 2. Otherwise, a non-empty `WAYLAND_DISPLAY` means Wayland. GTK prefers Wayland whenever a compositor socket is available, even when `DISPLAY` is also set for XWayland.
/// 3. Otherwise, a non-empty `DISPLAY` means X11.
/// 4. Otherwise the session is unknown (a bare tty or an unrecognised `XDG_SESSION_TYPE`).
///
/// Empty strings count as unset.
pub fn from_env(
    xdg_session_type: Option<&str>,
    wayland_display: Option<&str>,
    display: Option<&str>,
) -> Session {
    let set = |value: Option<&str>| value.is_some_and(|v| !v.trim().is_empty());
    match xdg_session_type
        .map(|v| v.trim().to_ascii_lowercase())
        .as_deref()
    {
        Some("wayland") => Session::Wayland,
        Some("x11") => Session::X11,
        _ if set(wayland_display) => Session::Wayland,
        _ if set(display) => Session::X11,
        _ => Session::Unknown,
    }
}

/// Maps a GDK display type name to a session, or `None` for other backends.
pub fn from_gdk_display_type(type_name: &str) -> Option<Session> {
    match type_name {
        "GdkWaylandDisplay" => Some(Session::Wayland),
        "GdkX11Display" => Some(Session::X11),
        _ => None,
    }
}

/// Reads the three variables from the process environment.
pub fn from_process_env() -> Session {
    let get = |name: &str| std::env::var(name).ok();
    from_env(
        get("XDG_SESSION_TYPE").as_deref(),
        get("WAYLAND_DISPLAY").as_deref(),
        get("DISPLAY").as_deref(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_rule_table() {
        type Case = (
            Option<&'static str>,
            Option<&'static str>,
            Option<&'static str>,
            Session,
        );
        let cases: &[Case] = &[
            (
                Some("wayland"),
                Some("wayland-0"),
                Some(":0"),
                Session::Wayland,
            ),
            (Some("wayland"), None, None, Session::Wayland),
            (Some("Wayland"), None, Some(":0"), Session::Wayland),
            (Some("x11"), None, Some(":0"), Session::X11),
            (Some("x11"), Some("wayland-0"), Some(":0"), Session::X11),
            (Some("X11"), None, None, Session::X11),
            (None, Some("wayland-0"), None, Session::Wayland),
            (None, Some("wayland-0"), Some(":0"), Session::Wayland),
            (Some("tty"), Some("wayland-0"), None, Session::Wayland),
            (Some(""), None, Some(":1"), Session::X11),
            (None, None, Some(":0"), Session::X11),
            (Some("tty"), None, None, Session::Unknown),
            (None, Some(""), Some(""), Session::Unknown),
            (None, None, None, Session::Unknown),
        ];
        for (xdg, wayland, display, expected) in cases {
            assert_eq!(
                from_env(*xdg, *wayland, *display),
                *expected,
                "xdg={xdg:?} wayland={wayland:?} display={display:?}"
            );
        }
    }

    #[test]
    fn gdk_display_names() {
        assert_eq!(
            from_gdk_display_type("GdkWaylandDisplay"),
            Some(Session::Wayland)
        );
        assert_eq!(from_gdk_display_type("GdkX11Display"), Some(Session::X11));
        assert_eq!(from_gdk_display_type("GdkBroadwayDisplay"), None);
    }
}
