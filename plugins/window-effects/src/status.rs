// Maps what a probe found about the session and the compositor to the plugin's status, with the reason for each feature that is missing
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::models::{
    FeatureStatus, Flavour, PluginStatus, Reason, FEATURE_ACRYLIC, FEATURE_BLUR, FEATURE_MICA,
    FEATURE_OPACITY, FEATURE_SHADOW_INSET,
};

/// The Wayland global of the standard blur protocol.
pub const EXT_BACKGROUND_EFFECT: &str = "ext_background_effect_manager_v1";
/// The Wayland global of KDE's blur protocol.
pub const KDE_BLUR: &str = "org_kde_kwin_blur_manager";

/// Windows 10 version 1809, where Acrylic arrives.
pub const BUILD_WINDOWS_10_1809: u32 = 17763;
/// The first Windows 11 build, where Mica arrives.
pub const BUILD_WINDOWS_11: u32 = 22000;

/// The display server or operating system the plugin runs on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionType {
    Wayland,
    X11,
    Windows,
    Other,
}

/// The desktop environment, which tells which compositor is behind a display server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Desktop {
    Gnome,
    Kde,
    Cinnamon,
    Other,
}

impl Desktop {
    /// Reads `XDG_CURRENT_DESKTOP`, a colon-separated list (`ubuntu:GNOME`, `X-Cinnamon`, `KDE`).
    pub fn from_xdg(value: &str) -> Self {
        let mut found = Desktop::Other;
        for entry in value.split(':') {
            match entry.trim().to_ascii_lowercase().as_str() {
                "kde" => return Desktop::Kde,
                "x-cinnamon" | "cinnamon" => return Desktop::Cinnamon,
                "gnome" | "gnome-classic" | "ubuntu" => found = Desktop::Gnome,
                _ => {}
            }
        }
        found
    }
}

/// The facts the status is worked out from. A probe fills it in; tests write it by hand.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Environment {
    pub session_type: SessionType,
    pub desktop: Desktop,
    /// Windows can be transparent: always true on Wayland and Windows, the screen's RGBA visual and a compositing manager on X11.
    pub has_composite: bool,
    /// The interfaces of the Wayland registry, with `ext_background_effect_manager_v1` left out when the compositor reported it cannot blur.
    pub wayland_globals: Vec<String>,
    /// The Windows build number, from `RtlGetVersion`.
    pub build_number: Option<u32>,
}

impl Environment {
    /// An environment on a system the plugin has no backend for.
    pub fn unsupported() -> Self {
        Environment {
            session_type: SessionType::Other,
            desktop: Desktop::Other,
            has_composite: false,
            wayland_globals: Vec::new(),
            build_number: None,
        }
    }

    pub fn has_global(&self, interface: &str) -> bool {
        self.wayland_globals.iter().any(|name| name == interface)
    }
}

/// How a blur request reaches the compositor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlurPath {
    /// `ext_background_effect_manager_v1` on the window's `wl_surface`.
    ExtBackgroundEffect,
    /// `org_kde_kwin_blur_manager` on the window's `wl_surface`.
    KdeBlur,
    /// `_KDE_NET_WM_BLUR_BEHIND_REGION` on the window's X11 window.
    X11Property,
    /// DWM, through Tauri's `set_effects`.
    Dwm,
}

/// Which way blur goes on this system, if any. The standard protocol is preferred to KDE's.
pub fn blur_path(env: &Environment) -> Option<BlurPath> {
    match env.session_type {
        SessionType::Wayland if env.has_global(EXT_BACKGROUND_EFFECT) => {
            Some(BlurPath::ExtBackgroundEffect)
        }
        SessionType::Wayland if env.has_global(KDE_BLUR) => Some(BlurPath::KdeBlur),
        SessionType::X11 if env.has_composite && env.desktop == Desktop::Kde => {
            Some(BlurPath::X11Property)
        }
        SessionType::Windows if env.build_number.is_some() => Some(BlurPath::Dwm),
        _ => None,
    }
}

/// The plugin's status on a system.
pub fn status_for(env: &Environment) -> PluginStatus {
    match env.session_type {
        SessionType::Wayland | SessionType::X11 => linux(env),
        SessionType::Windows => windows(env),
        SessionType::Other => PluginStatus::all_unavailable(
            Flavour::Unsupported,
            Reason::UnsupportedPlatform,
            "this system has no window effects",
        ),
    }
}

fn linux(env: &Environment) -> PluginStatus {
    let wayland = env.session_type == SessionType::Wayland;
    let flavour = if wayland {
        Flavour::Wayland
    } else {
        Flavour::X11
    };
    let no_compositor = || {
        if wayland {
            (
                Reason::ProbeFailed,
                "the Wayland compositor did not say whether windows can be transparent",
            )
        } else {
            (
                Reason::X11NoCompositor,
                "no compositing manager is running, so windows cannot be transparent",
            )
        }
    };

    let opacity = if env.has_composite {
        FeatureStatus::available(FEATURE_OPACITY)
    } else {
        let (reason, message) = no_compositor();
        FeatureStatus::unavailable(FEATURE_OPACITY, reason, message)
    };

    let blur = match blur_path(env) {
        Some(_) => FeatureStatus::available(FEATURE_BLUR),
        None if !wayland && !env.has_composite => {
            let (reason, message) = no_compositor();
            FeatureStatus::unavailable(FEATURE_BLUR, reason, message)
        }
        None => match env.desktop {
            Desktop::Gnome => FeatureStatus::unavailable(
                FEATURE_BLUR,
                Reason::CompositorHasNoBlur,
                "GNOME does not let apps blur behind their windows",
            ),
            Desktop::Cinnamon => FeatureStatus::unavailable(
                FEATURE_BLUR,
                Reason::CompositorHasNoBlur,
                "Cinnamon does not let apps blur behind their windows",
            ),
            _ if wayland => FeatureStatus::unavailable(
                FEATURE_BLUR,
                Reason::NoBlurProtocol,
                "the compositor offers neither ext_background_effect_manager_v1 nor org_kde_kwin_blur_manager",
            ),
            _ => FeatureStatus::unavailable(
                FEATURE_BLUR,
                Reason::UnknownCompositor,
                "this desktop's compositor is not known to blur behind windows when asked through _KDE_NET_WM_BLUR_BEHIND_REGION",
            ),
        },
    };

    let shadow_inset = if env.has_composite {
        FeatureStatus::available(FEATURE_SHADOW_INSET)
    } else {
        let (reason, message) = no_compositor();
        FeatureStatus::unavailable(FEATURE_SHADOW_INSET, reason, message)
    };

    PluginStatus::build(
        flavour,
        vec![
            opacity,
            blur,
            FeatureStatus::unavailable(
                FEATURE_MICA,
                Reason::WindowsOnly,
                "Mica is a Windows 11 material",
            ),
            FeatureStatus::unavailable(
                FEATURE_ACRYLIC,
                Reason::WindowsOnly,
                "Acrylic is a Windows material",
            ),
            shadow_inset,
        ],
    )
}

fn windows(env: &Environment) -> PluginStatus {
    let build = env.build_number;
    let unknown = || {
        FeatureStatus::unavailable(
            FEATURE_MICA,
            Reason::ProbeFailed,
            "the Windows build number could not be read",
        )
    };
    let mica = match build {
        Some(build) if build >= BUILD_WINDOWS_11 => FeatureStatus::available(FEATURE_MICA),
        Some(_) => FeatureStatus::unavailable(
            FEATURE_MICA,
            Reason::NeedsWindows11,
            "Mica needs Windows 11",
        ),
        None => unknown(),
    };
    let acrylic = match build {
        Some(build) if build >= BUILD_WINDOWS_10_1809 => FeatureStatus::available(FEATURE_ACRYLIC),
        Some(_) => FeatureStatus::unavailable(
            FEATURE_ACRYLIC,
            Reason::NeedsWindows10,
            "Acrylic needs Windows 10 version 1809 or later",
        ),
        None => FeatureStatus {
            name: FEATURE_ACRYLIC.to_string(),
            ..unknown()
        },
    };
    let blur = match build {
        Some(_) => FeatureStatus::available(FEATURE_BLUR),
        None => FeatureStatus {
            name: FEATURE_BLUR.to_string(),
            ..unknown()
        },
    };
    PluginStatus::build(
        Flavour::Windows,
        vec![
            FeatureStatus::available(FEATURE_OPACITY),
            blur,
            mica,
            acrylic,
            FeatureStatus::unavailable(
                FEATURE_SHADOW_INSET,
                Reason::GtkOnly,
                "the shadow inset is a GTK feature; Windows draws the shadow itself",
            ),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(
        session_type: SessionType,
        desktop: Desktop,
        has_composite: bool,
        globals: &[&str],
    ) -> Environment {
        Environment {
            session_type,
            desktop,
            has_composite,
            wayland_globals: globals.iter().map(|name| name.to_string()).collect(),
            build_number: None,
        }
    }

    fn windows_env(build: Option<u32>) -> Environment {
        Environment {
            session_type: SessionType::Windows,
            desktop: Desktop::Other,
            has_composite: true,
            wayland_globals: Vec::new(),
            build_number: build,
        }
    }

    fn reason(status: &PluginStatus, name: &str) -> Option<Reason> {
        status.feature(name).and_then(|feature| feature.reason)
    }

    #[test]
    fn the_desktop_is_read_from_the_colon_separated_list() {
        assert_eq!(Desktop::from_xdg("ubuntu:GNOME"), Desktop::Gnome);
        assert_eq!(Desktop::from_xdg("KDE"), Desktop::Kde);
        assert_eq!(Desktop::from_xdg("X-Cinnamon"), Desktop::Cinnamon);
        assert_eq!(Desktop::from_xdg("sway"), Desktop::Other);
        assert_eq!(Desktop::from_xdg(""), Desktop::Other);
    }

    #[test]
    fn gnome_wayland_can_be_transparent_and_inset_but_not_blur() {
        let status = status_for(&env(
            SessionType::Wayland,
            Desktop::Gnome,
            true,
            &["wl_compositor", "xdg_wm_base"],
        ));
        assert_eq!(status.flavour, Flavour::Wayland);
        assert!(status.has(FEATURE_OPACITY));
        assert!(status.has(FEATURE_SHADOW_INSET));
        assert!(!status.has(FEATURE_BLUR));
        assert_eq!(
            reason(&status, FEATURE_BLUR),
            Some(Reason::CompositorHasNoBlur)
        );
        assert_eq!(status.reason, Some(Reason::CompositorHasNoBlur));
        assert!(status.available);
        assert_eq!(reason(&status, FEATURE_MICA), Some(Reason::WindowsOnly));
    }

    #[test]
    fn kde_wayland_blurs_through_its_own_protocol() {
        let environment = env(SessionType::Wayland, Desktop::Kde, true, &[KDE_BLUR]);
        let status = status_for(&environment);
        assert!(status.has(FEATURE_BLUR));
        assert_eq!(blur_path(&environment), Some(BlurPath::KdeBlur));
        // Everything that applies works, so the status has nothing to explain.
        assert_eq!(status.reason, None);
    }

    #[test]
    fn the_standard_protocol_is_preferred_to_kdes() {
        let environment = env(
            SessionType::Wayland,
            Desktop::Kde,
            true,
            &[KDE_BLUR, EXT_BACKGROUND_EFFECT],
        );
        assert_eq!(blur_path(&environment), Some(BlurPath::ExtBackgroundEffect));
    }

    #[test]
    fn another_wayland_compositor_with_no_protocol_says_so() {
        let status = status_for(&env(SessionType::Wayland, Desktop::Other, true, &[]));
        assert_eq!(reason(&status, FEATURE_BLUR), Some(Reason::NoBlurProtocol));
    }

    #[test]
    fn kde_x11_sets_the_blur_property() {
        let environment = env(SessionType::X11, Desktop::Kde, true, &[]);
        let status = status_for(&environment);
        assert_eq!(status.flavour, Flavour::X11);
        assert!(status.has(FEATURE_BLUR));
        assert_eq!(blur_path(&environment), Some(BlurPath::X11Property));
    }

    #[test]
    fn x11_without_a_compositor_has_no_transparency_blur_or_inset() {
        let status = status_for(&env(SessionType::X11, Desktop::Kde, false, &[]));
        for name in [FEATURE_OPACITY, FEATURE_BLUR, FEATURE_SHADOW_INSET] {
            assert_eq!(
                reason(&status, name),
                Some(Reason::X11NoCompositor),
                "{name}"
            );
        }
        assert!(!status.available);
    }

    #[test]
    fn cinnamon_cannot_blur_and_an_unknown_x11_desktop_is_not_assumed_to() {
        let cinnamon = status_for(&env(SessionType::X11, Desktop::Cinnamon, true, &[]));
        assert_eq!(
            reason(&cinnamon, FEATURE_BLUR),
            Some(Reason::CompositorHasNoBlur)
        );
        assert!(cinnamon
            .feature(FEATURE_BLUR)
            .and_then(|feature| feature.message.as_deref())
            .is_some_and(|message| message.contains("Cinnamon")));
        let other = status_for(&env(SessionType::X11, Desktop::Other, true, &[]));
        assert_eq!(
            reason(&other, FEATURE_BLUR),
            Some(Reason::UnknownCompositor)
        );
    }

    #[test]
    fn windows_10_has_acrylic_and_blur_but_not_mica() {
        let status = status_for(&windows_env(Some(19045)));
        assert_eq!(status.flavour, Flavour::Windows);
        assert!(status.has(FEATURE_OPACITY));
        assert!(status.has(FEATURE_BLUR));
        assert!(status.has(FEATURE_ACRYLIC));
        assert_eq!(reason(&status, FEATURE_MICA), Some(Reason::NeedsWindows11));
        assert_eq!(status.reason, Some(Reason::NeedsWindows11));
    }

    #[test]
    fn windows_11_has_mica_and_nothing_to_explain() {
        let status = status_for(&windows_env(Some(22631)));
        assert!(status.has(FEATURE_MICA));
        assert!(status.has(FEATURE_ACRYLIC));
        assert_eq!(reason(&status, FEATURE_SHADOW_INSET), Some(Reason::GtkOnly));
        assert_eq!(status.reason, None);
        assert_eq!(blur_path(&windows_env(Some(22631))), Some(BlurPath::Dwm));
    }

    #[test]
    fn the_build_thresholds_are_exact() {
        let at = |build| status_for(&windows_env(Some(build)));
        assert!(!at(BUILD_WINDOWS_11 - 1).has(FEATURE_MICA));
        assert!(at(BUILD_WINDOWS_11).has(FEATURE_MICA));
        assert!(!at(BUILD_WINDOWS_10_1809 - 1).has(FEATURE_ACRYLIC));
        assert!(at(BUILD_WINDOWS_10_1809).has(FEATURE_ACRYLIC));
    }

    #[test]
    fn an_unreadable_windows_build_reports_the_probe_failure() {
        let status = status_for(&windows_env(None));
        assert_eq!(reason(&status, FEATURE_MICA), Some(Reason::ProbeFailed));
        assert_eq!(reason(&status, FEATURE_BLUR), Some(Reason::ProbeFailed));
        assert!(status.has(FEATURE_OPACITY));
    }

    #[test]
    fn an_unsupported_system_reports_every_feature_unavailable() {
        let status = status_for(&Environment::unsupported());
        assert!(!status.available);
        assert_eq!(status.flavour, Flavour::Unsupported);
        assert_eq!(status.features.len(), 5);
        assert!(status.features.iter().all(|feature| !feature.available
            && feature.reason == Some(Reason::UnsupportedPlatform)
            && feature.message.is_some()));
        assert_eq!(status.reason, Some(Reason::UnsupportedPlatform));
    }

    #[test]
    fn the_features_come_in_the_documented_order() {
        let status = status_for(&windows_env(Some(22631)));
        let names: Vec<_> = status.features.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, ["opacity", "blur", "mica", "acrylic", "shadowInset"]);
    }
}
