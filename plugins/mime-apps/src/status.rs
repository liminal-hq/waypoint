// Works out what the plugin can do on Linux from the sandbox and the display, and reports it with a typed reason for each feature
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![cfg_attr(not(target_os = "linux"), allow(dead_code))]

use std::path::{Path, PathBuf};

use crate::mimeapps::XdgEnv;
use crate::models::{
    FeatureStatus, Flavour, PluginStatus, Reason, FEATURE_APP_ICONS, FEATURE_CHOOSER,
    FEATURE_FOLDER_ICONS, FEATURE_HANDLERS, FEATURE_OPEN_DEFAULT, FEATURE_OPEN_WITH,
    FEATURE_SET_DEFAULT, FEATURE_TYPE_ICONS, FEATURE_TYPE_INFO,
};

/// What the Linux status depends on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LinuxEnv {
    /// The process runs in a Flatpak sandbox (`/.flatpak-info` exists).
    pub flatpak: bool,
    /// A display is there to draw icons for, so the icon theme is usable.
    pub display: bool,
    /// An icon theme other than the fallback `hicolor` is installed, so file and folder icons look like the desktop's.
    pub icon_theme: bool,
}

impl LinuxEnv {
    pub fn detect() -> Self {
        LinuxEnv {
            flatpak: std::path::Path::new("/.flatpak-info").exists(),
            display: std::env::var_os("WAYLAND_DISPLAY").is_some()
                || std::env::var_os("DISPLAY").is_some(),
            icon_theme: has_icon_theme(
                &icon_dirs(&XdgEnv::from_env()),
                |dir| {
                    std::fs::read_dir(dir)
                        .map(|entries| {
                            entries
                                .filter_map(|entry| entry.ok())
                                .map(|entry| entry.path())
                                .collect()
                        })
                        .unwrap_or_default()
                },
                |path| path.is_file(),
            ),
        }
    }
}

/// The folders icon themes are installed in, from the user's own to the system's.
pub fn icon_dirs(xdg: &XdgEnv) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(home) = xdg.data_home.parent().and_then(|local| local.parent()) {
        dirs.push(home.join(".icons"));
    }
    dirs.push(xdg.data_home.join("icons"));
    dirs.extend(xdg.data_dirs.iter().map(|dir| dir.join("icons")));
    dirs
}

/// Whether any folder in `dirs` holds a theme other than `hicolor` (a folder with an `index.theme`): `hicolor` is where every application's own icons go and has no file or folder icons.
pub fn has_icon_theme(
    dirs: &[PathBuf],
    list: impl Fn(&Path) -> Vec<PathBuf>,
    is_file: impl Fn(&Path) -> bool,
) -> bool {
    dirs.iter().any(|dir| {
        list(dir).iter().any(|theme| {
            theme.file_name().is_some_and(|name| name != "hicolor")
                && is_file(&theme.join("index.theme"))
        })
    })
}

/// The status of the two icon features from what the environment has.
fn icon_features(env: LinuxEnv) -> [FeatureStatus; 2] {
    let make = |name: &str| {
        if !env.display {
            FeatureStatus::unavailable(
                name,
                Reason::NoDisplay,
                "there is no display to read the icon theme from",
            )
        } else if !env.icon_theme && env.flatpak {
            FeatureStatus::unavailable(
                name,
                Reason::FlatpakSandbox,
                "the sandbox does not see an icon theme; export the host's icon folders to it to draw the desktop's file and folder icons",
            )
        } else if !env.icon_theme {
            FeatureStatus::unavailable(
                name,
                Reason::NoIconTheme,
                "no icon theme is installed besides the fallback one, so there are no file or folder icons to draw",
            )
        } else {
            FeatureStatus::available(name)
        }
    };
    [make(FEATURE_TYPE_ICONS), make(FEATURE_FOLDER_ICONS)]
}

const SANDBOX: &str = "a Flatpak sandbox sees neither the host's applications nor its defaults; the OpenURI portal opens a file in the application the person picks";

pub fn linux_status(env: LinuxEnv) -> PluginStatus {
    let [type_icons, folder_icons] = icon_features(env);
    if env.flatpak {
        let sandboxed =
            |name: &str| FeatureStatus::unavailable(name, Reason::FlatpakSandbox, SANDBOX);
        return PluginStatus::build(
            Flavour::Portal,
            vec![
                FeatureStatus::available(FEATURE_TYPE_INFO),
                sandboxed(FEATURE_HANDLERS),
                sandboxed(FEATURE_OPEN_WITH),
                FeatureStatus::available(FEATURE_OPEN_DEFAULT),
                sandboxed(FEATURE_SET_DEFAULT),
                FeatureStatus::available(FEATURE_CHOOSER),
                sandboxed(FEATURE_APP_ICONS),
                type_icons,
                folder_icons,
            ],
        );
    }
    PluginStatus::build(
        Flavour::Gio,
        vec![
            FeatureStatus::available(FEATURE_TYPE_INFO),
            FeatureStatus::available(FEATURE_HANDLERS),
            FeatureStatus::available(FEATURE_OPEN_WITH),
            FeatureStatus::available(FEATURE_OPEN_DEFAULT),
            FeatureStatus::available(FEATURE_SET_DEFAULT),
            FeatureStatus::unavailable(
                FEATURE_CHOOSER,
                Reason::NoSystemChooser,
                "this system has no application chooser to call; draw a list from the handlers",
            ),
            if env.display {
                FeatureStatus::available(FEATURE_APP_ICONS)
            } else {
                FeatureStatus::unavailable(
                    FEATURE_APP_ICONS,
                    Reason::NoDisplay,
                    "there is no display to read the icon theme from",
                )
            },
            type_icons,
            folder_icons,
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::FEATURES;

    fn names(status: &PluginStatus) -> Vec<&str> {
        status.features.iter().map(|f| f.name.as_str()).collect()
    }

    #[test]
    fn a_desktop_session_can_do_everything_but_ask_the_system_to_choose() {
        let status = linux_status(LinuxEnv {
            flatpak: false,
            display: true,
            icon_theme: true,
        });
        assert_eq!(names(&status), FEATURES);
        assert_eq!(status.flavour, Flavour::Gio);
        assert!(status.available);
        for feature in &status.features {
            assert_eq!(
                feature.available,
                feature.name != FEATURE_CHOOSER,
                "{}",
                feature.name
            );
        }
        let chooser = status
            .features
            .iter()
            .find(|f| f.name == FEATURE_CHOOSER)
            .unwrap();
        assert_eq!(chooser.reason, Some(Reason::NoSystemChooser));
        assert_eq!(status.reason, Some(Reason::NoSystemChooser));
    }

    #[test]
    fn without_a_display_there_are_no_icons() {
        let status = linux_status(LinuxEnv {
            flatpak: false,
            display: false,
            icon_theme: true,
        });
        let icons = status
            .features
            .iter()
            .find(|f| f.name == FEATURE_APP_ICONS)
            .unwrap();
        assert_eq!(icons.reason, Some(Reason::NoDisplay));
        assert!(status.has(FEATURE_HANDLERS));
    }

    #[test]
    fn in_a_flatpak_the_portal_opens_and_asks_but_the_rest_is_unavailable_with_the_reason() {
        let status = linux_status(LinuxEnv {
            flatpak: true,
            display: true,
            icon_theme: true,
        });
        assert_eq!(status.flavour, Flavour::Portal);
        assert!(status.has(FEATURE_OPEN_DEFAULT) && status.has(FEATURE_CHOOSER));
        assert!(status.has(FEATURE_TYPE_INFO));
        // The sandbox sees the runtime's icon theme, so file and folder icons work.
        assert!(status.has(FEATURE_TYPE_ICONS) && status.has(FEATURE_FOLDER_ICONS));
        for name in [
            FEATURE_HANDLERS,
            FEATURE_OPEN_WITH,
            FEATURE_SET_DEFAULT,
            FEATURE_APP_ICONS,
        ] {
            let feature = status.features.iter().find(|f| f.name == name).unwrap();
            assert!(!feature.available, "{name}");
            assert_eq!(feature.reason, Some(Reason::FlatpakSandbox), "{name}");
            assert!(feature.message.is_some());
        }
    }

    fn icon_feature(status: &PluginStatus, name: &str) -> FeatureStatus {
        status
            .features
            .iter()
            .find(|f| f.name == name)
            .cloned()
            .unwrap()
    }

    #[test]
    fn file_and_folder_icons_say_why_they_are_missing() {
        let env = |flatpak, display, icon_theme| LinuxEnv {
            flatpak,
            display,
            icon_theme,
        };
        for name in [FEATURE_TYPE_ICONS, FEATURE_FOLDER_ICONS] {
            let no_theme = linux_status(env(false, true, false));
            let feature = icon_feature(&no_theme, name);
            assert!(!feature.available);
            assert_eq!(feature.reason, Some(Reason::NoIconTheme), "{name}");
            assert!(feature.message.is_some());
            let headless = linux_status(env(false, false, true));
            assert_eq!(
                icon_feature(&headless, name).reason,
                Some(Reason::NoDisplay),
                "{name}"
            );
            let sandbox = linux_status(env(true, true, false));
            assert_eq!(
                icon_feature(&sandbox, name).reason,
                Some(Reason::FlatpakSandbox),
                "{name}"
            );
            let fine = linux_status(env(false, true, true));
            assert!(icon_feature(&fine, name).available, "{name}");
        }
    }

    #[test]
    fn a_theme_besides_hicolor_is_an_icon_theme() {
        let dirs = vec![PathBuf::from("/a/icons"), PathBuf::from("/b/icons")];
        let list = |dir: &Path| match dir.to_str().unwrap() {
            "/a/icons" => vec![PathBuf::from("/a/icons/hicolor")],
            "/b/icons" => vec![
                PathBuf::from("/b/icons/default"),
                PathBuf::from("/b/icons/Adwaita"),
            ],
            _ => vec![],
        };
        let with_index = |path: &Path| {
            matches!(
                path.to_str().unwrap(),
                "/a/icons/hicolor/index.theme" | "/b/icons/Adwaita/index.theme"
            )
        };
        assert!(has_icon_theme(&dirs, list, with_index));
        // Only the fallback theme, or a folder with no `index.theme`, is not a theme to draw from.
        assert!(!has_icon_theme(&dirs[..1], list, with_index));
        assert!(!has_icon_theme(&dirs, list, |path| path
            .to_str()
            .unwrap()
            .starts_with("/a/")));
        assert!(!has_icon_theme(&[], list, with_index));
    }

    #[test]
    fn icon_theme_folders_start_with_the_users_own() {
        let xdg = XdgEnv::from_vars(
            |name| (name == "XDG_DATA_DIRS").then(|| "/usr/share:/opt/share".to_string()),
            Some(PathBuf::from("/home/u")),
        );
        assert_eq!(
            icon_dirs(&xdg),
            [
                PathBuf::from("/home/u/.icons"),
                PathBuf::from("/home/u/.local/share/icons"),
                PathBuf::from("/usr/share/icons"),
                PathBuf::from("/opt/share/icons"),
            ]
        );
    }
}
