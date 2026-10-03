// Recognises the user's own standard folders (Home, Documents, Downloads, …) so an icon set can mark them.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The match is by path, against a short list built once. `SpecialDirs::from_env` takes a
//! `PlacesEnv`, so tests inject their own directories and never read the real home folder;
//! `SpecialDirs::current` is the real one for the current user, read on first use.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::places::{parse_user_dirs, user_folder, PlaceKind, PlacesEnv, KINDS};

/// Which of the user's standard folders a folder is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum SpecialFolder {
    Home,
    Desktop,
    Documents,
    Downloads,
    Pictures,
    Music,
    Videos,
    Templates,
    Public,
    Projects,
}

impl SpecialFolder {
    /// Every standard folder, in a fixed order.
    pub const ALL: [SpecialFolder; 10] = [
        SpecialFolder::Home,
        SpecialFolder::Desktop,
        SpecialFolder::Documents,
        SpecialFolder::Downloads,
        SpecialFolder::Pictures,
        SpecialFolder::Music,
        SpecialFolder::Videos,
        SpecialFolder::Templates,
        SpecialFolder::Public,
        SpecialFolder::Projects,
    ];
}

/// The folders to recognise, each with the path it has on this system.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SpecialDirs {
    dirs: Vec<(PathBuf, SpecialFolder)>,
}

impl SpecialDirs {
    /// The standard folders of `env`: the freedesktop user directories on Linux (with their usual
    /// names under home when `user-dirs.dirs` does not say), the Known Folders on Windows. There is
    /// no standard for a projects folder, so `~/Projects` and `~/projects` both count.
    pub fn from_env(env: &PlacesEnv) -> Self {
        let xdg = if cfg!(windows) {
            HashMap::new()
        } else {
            fs::read_to_string(&env.user_dirs_file)
                .map(|text| parse_user_dirs(&text, &env.home))
                .unwrap_or_default()
        };
        let mut dirs = vec![(env.home.clone(), SpecialFolder::Home)];
        for (index, (kind, ..)) in KINDS.iter().enumerate() {
            let folder = match kind {
                PlaceKind::Desktop => SpecialFolder::Desktop,
                PlaceKind::Documents => SpecialFolder::Documents,
                PlaceKind::Downloads => SpecialFolder::Downloads,
                PlaceKind::Pictures => SpecialFolder::Pictures,
                PlaceKind::Music => SpecialFolder::Music,
                PlaceKind::Videos => SpecialFolder::Videos,
                _ => continue,
            };
            dirs.push((user_folder(env, &xdg, index), folder));
        }
        dirs.push((
            extra(&xdg, "XDG_TEMPLATES_DIR", &env.home, "Templates"),
            SpecialFolder::Templates,
        ));
        dirs.push((
            extra(&xdg, "XDG_PUBLICSHARE_DIR", &env.home, "Public"),
            SpecialFolder::Public,
        ));
        dirs.push((env.home.join("Projects"), SpecialFolder::Projects));
        dirs.push((env.home.join("projects"), SpecialFolder::Projects));
        Self { dirs }
    }

    /// The standard folders of the current user, read once. Empty when the home folder is unknown.
    pub fn current() -> &'static SpecialDirs {
        static CURRENT: OnceLock<SpecialDirs> = OnceLock::new();
        CURRENT.get_or_init(|| {
            PlacesEnv::detect()
                .map(|env| SpecialDirs::from_env(&env))
                .unwrap_or_default()
        })
    }

    /// Which standard folder `path` is, if it is one. Only the path itself matches, never a folder
    /// inside it.
    pub fn lookup(&self, path: &Path) -> Option<SpecialFolder> {
        self.dirs
            .iter()
            .find(|(dir, _)| dir == path)
            .map(|(_, folder)| *folder)
    }
}

#[cfg(not(windows))]
fn extra(xdg: &HashMap<String, PathBuf>, key: &str, home: &Path, fallback: &str) -> PathBuf {
    xdg.get(key).cloned().unwrap_or_else(|| home.join(fallback))
}

#[cfg(windows)]
fn extra(_xdg: &HashMap<String, PathBuf>, key: &str, home: &Path, fallback: &str) -> PathBuf {
    let known = match key {
        "XDG_TEMPLATES_DIR" => dirs::template_dir(),
        _ => dirs::public_dir(),
    };
    known.unwrap_or_else(|| home.join(fallback))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(user_dirs: &str) -> (tempfile::TempDir, PlacesEnv) {
        let root = tempfile::tempdir().unwrap();
        let home = root.path().join("home");
        fs::create_dir_all(&home).unwrap();
        let user_dirs_file = root.path().join("user-dirs.dirs");
        fs::write(&user_dirs_file, user_dirs).unwrap();
        let env = PlacesEnv {
            bookmarks_file: root.path().join("bookmarks"),
            user_dirs_file,
            home,
        };
        (root, env)
    }

    #[cfg(not(windows))]
    #[test]
    fn recognises_the_default_folders_under_home() {
        let (_root, env) = env("");
        let dirs = SpecialDirs::from_env(&env);
        let at = |name: &str| dirs.lookup(&env.home.join(name));
        assert_eq!(dirs.lookup(&env.home), Some(SpecialFolder::Home));
        assert_eq!(at("Desktop"), Some(SpecialFolder::Desktop));
        assert_eq!(at("Documents"), Some(SpecialFolder::Documents));
        assert_eq!(at("Downloads"), Some(SpecialFolder::Downloads));
        assert_eq!(at("Pictures"), Some(SpecialFolder::Pictures));
        assert_eq!(at("Music"), Some(SpecialFolder::Music));
        assert_eq!(at("Videos"), Some(SpecialFolder::Videos));
        assert_eq!(at("Templates"), Some(SpecialFolder::Templates));
        assert_eq!(at("Public"), Some(SpecialFolder::Public));
        assert_eq!(at("Projects"), Some(SpecialFolder::Projects));
        assert_eq!(at("projects"), Some(SpecialFolder::Projects));
        assert_eq!(at("src"), None);
    }

    #[cfg(not(windows))]
    #[test]
    fn follows_user_dirs_that_were_moved_or_renamed() {
        let (_root, env) =
            env("XDG_DOWNLOAD_DIR=\"$HOME/Telechargements\"\nXDG_DOCUMENTS_DIR=\"/data/docs\"\n");
        let dirs = SpecialDirs::from_env(&env);
        assert_eq!(
            dirs.lookup(&env.home.join("Telechargements")),
            Some(SpecialFolder::Downloads)
        );
        assert_eq!(dirs.lookup(&env.home.join("Downloads")), None);
        assert_eq!(
            dirs.lookup(Path::new("/data/docs")),
            Some(SpecialFolder::Documents)
        );
    }

    #[test]
    fn a_folder_inside_a_standard_folder_is_not_one() {
        let (_root, env) = env("");
        let dirs = SpecialDirs::from_env(&env);
        assert_eq!(
            dirs.lookup(&env.home.join("Documents").join("Documents")),
            None
        );
        assert_eq!(dirs.lookup(env.home.parent().unwrap()), None);
    }

    #[test]
    fn lists_every_folder_once() {
        let mut all = SpecialFolder::ALL.to_vec();
        all.dedup();
        assert_eq!(all.len(), 10);
    }
}
