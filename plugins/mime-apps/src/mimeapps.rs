// Reads the `mimeapps.list` files in the order the freedesktop.org specification gives them, and resolves the default and the associated applications of a type from them
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A pure, read-only module. gio answers the plugin's questions on Linux; this module is what the plugin falls back on when gio cannot answer, and what it uses to tell a person which file a default comes from. It never writes.
//!
//! The specification (Association between MIME types and applications): the files are, from the highest priority, `$XDG_CONFIG_HOME`, each of `$XDG_CONFIG_DIRS`, `$XDG_DATA_HOME/applications` and each of `$XDG_DATA_DIRS/applications`; in each directory `$desktop-mimeapps.list` (one per entry of `$XDG_CURRENT_DESKTOP`) comes before `mimeapps.list`. A file may have `[Added Associations]`, `[Removed Associations]` and `[Default Applications]`, each mapping a type to a `;`-separated list of desktop-file ids. An id may be written with or without `.desktop`.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Where the files are looked for. Built from the environment, or made up by a test.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XdgEnv {
    pub config_home: PathBuf,
    pub config_dirs: Vec<PathBuf>,
    pub data_home: PathBuf,
    pub data_dirs: Vec<PathBuf>,
    /// The entries of `$XDG_CURRENT_DESKTOP`, in lower case.
    pub desktops: Vec<String>,
}

impl XdgEnv {
    /// Reads the real environment, applying the specification's defaults.
    pub fn from_env() -> Self {
        XdgEnv::from_vars(
            |name| std::env::var(name).ok(),
            std::env::var_os("HOME").map(PathBuf::from),
        )
    }

    /// Builds the environment from variable lookups, so a test can supply its own.
    pub fn from_vars(get: impl Fn(&str) -> Option<String>, home: Option<PathBuf>) -> Self {
        let set = |name: &str| get(name).filter(|value| !value.is_empty());
        let home = home.unwrap_or_default();
        let list = |value: String| -> Vec<PathBuf> {
            value
                .split(':')
                .filter(|part| !part.is_empty())
                .map(PathBuf::from)
                .collect()
        };
        XdgEnv {
            config_home: set("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".config")),
            config_dirs: list(set("XDG_CONFIG_DIRS").unwrap_or_else(|| "/etc/xdg".into())),
            data_home: set("XDG_DATA_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".local/share")),
            data_dirs: list(
                set("XDG_DATA_DIRS").unwrap_or_else(|| "/usr/local/share:/usr/share".into()),
            ),
            desktops: set("XDG_CURRENT_DESKTOP")
                .map(|value| {
                    value
                        .split(':')
                        .filter(|part| !part.is_empty())
                        .map(str::to_ascii_lowercase)
                        .collect()
                })
                .unwrap_or_default(),
        }
    }

    /// Every file that may hold associations, from the highest priority to the lowest, whether or not it exists.
    pub fn candidate_files(&self) -> Vec<PathBuf> {
        let mut directories: Vec<PathBuf> = vec![self.config_home.clone()];
        directories.extend(self.config_dirs.iter().cloned());
        directories.push(self.data_home.join("applications"));
        directories.extend(self.data_dirs.iter().map(|dir| dir.join("applications")));
        let mut files = Vec::new();
        for directory in directories {
            for desktop in &self.desktops {
                files.push(directory.join(format!("{desktop}-mimeapps.list")));
            }
            files.push(directory.join("mimeapps.list"));
        }
        files
    }
}

/// The three sections of one file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Associations {
    pub added: HashMap<String, Vec<String>>,
    pub removed: HashMap<String, Vec<String>>,
    pub defaults: HashMap<String, Vec<String>>,
}

/// The desktop-file id with its `.desktop` suffix, which a file may leave off.
pub fn desktop_id(id: &str) -> String {
    let id = id.trim();
    if id.ends_with(".desktop") {
        id.to_string()
    } else {
        format!("{id}.desktop")
    }
}

/// Parses one file. Comments, unknown sections, lines without `=` and empty lists are ignored; ids are normalised and a repeated id keeps its first place.
pub fn parse(text: &str) -> Associations {
    #[derive(Clone, Copy)]
    enum Section {
        Added,
        Removed,
        Default,
        Other,
    }
    let mut section = Section::Other;
    let mut result = Associations::default();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(name) = line
            .strip_prefix('[')
            .and_then(|rest| rest.strip_suffix(']'))
        {
            section = match name {
                "Added Associations" => Section::Added,
                "Removed Associations" => Section::Removed,
                "Default Applications" => Section::Default,
                _ => Section::Other,
            };
            continue;
        }
        let Some((mime, value)) = line.split_once('=') else {
            continue;
        };
        let target = match section {
            Section::Added => &mut result.added,
            Section::Removed => &mut result.removed,
            Section::Default => &mut result.defaults,
            Section::Other => continue,
        };
        let entry = target.entry(mime.trim().to_ascii_lowercase()).or_default();
        for id in value.split(';').map(str::trim).filter(|id| !id.is_empty()) {
            let id = desktop_id(id);
            if !entry.contains(&id) {
                entry.push(id);
            }
        }
    }
    result
}

/// What the files say about one type.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Resolved {
    /// The first installed application of the highest-priority `[Default Applications]` entry.
    pub default: Option<String>,
    /// Every associated application, the default first, then the added ones in priority order, then the ones the system's own cache lists; removed ones are left out.
    pub apps: Vec<String>,
}

/// The files of one system, in priority order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Precedence {
    layers: Vec<(PathBuf, Associations)>,
}

impl Precedence {
    /// Reads the files that exist under `env`. An unreadable file is skipped, as the specification says.
    pub fn load(env: &XdgEnv) -> Self {
        let layers = env
            .candidate_files()
            .into_iter()
            .filter_map(|path| {
                let text = std::fs::read_to_string(&path).ok()?;
                Some((path, parse(&text)))
            })
            .collect();
        Precedence { layers }
    }

    /// A precedence built from text, highest priority first, for a test.
    pub fn from_texts<'a>(files: impl IntoIterator<Item = (&'a str, &'a str)>) -> Self {
        Precedence {
            layers: files
                .into_iter()
                .map(|(name, text)| (PathBuf::from(name), parse(text)))
                .collect(),
        }
    }

    /// The files that were read, from the one that wins to the one that loses.
    pub fn files(&self) -> Vec<&Path> {
        self.layers.iter().map(|(path, _)| path.as_path()).collect()
    }

    /// Resolves a type. `cached` is the system's own list of applications that declare the type (`mimeinfo.cache`), already in desktop-file ids, and `installed` says whether an id has a desktop file; an uninstalled default is skipped and the next one tried.
    pub fn resolve(
        &self,
        mime: &str,
        cached: &[String],
        installed: &dyn Fn(&str) -> bool,
    ) -> Resolved {
        let mime = mime.to_ascii_lowercase();
        let mut default = None;
        let mut apps: Vec<String> = Vec::new();
        let mut removed_above: HashSet<&str> = HashSet::new();
        for (_, layer) in &self.layers {
            if default.is_none() {
                default = layer
                    .defaults
                    .get(&mime)
                    .and_then(|ids| ids.iter().find(|id| installed(id)))
                    .cloned();
            }
            for id in layer.added.get(&mime).into_iter().flatten() {
                if !removed_above.contains(id.as_str()) && !apps.contains(id) {
                    apps.push(id.clone());
                }
            }
            for id in layer.removed.get(&mime).into_iter().flatten() {
                removed_above.insert(id.as_str());
            }
        }
        for id in cached {
            let id = desktop_id(id);
            if !removed_above.contains(id.as_str()) && !apps.contains(&id) {
                apps.push(id);
            }
        }
        apps.retain(|id| installed(id));
        if let Some(default) = &default {
            apps.retain(|id| id != default);
            apps.insert(0, default.clone());
        }
        Resolved { default, apps }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all_installed(_: &str) -> bool {
        true
    }

    fn env(vars: &[(&str, &str)]) -> XdgEnv {
        let vars: HashMap<String, String> = vars
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect();
        XdgEnv::from_vars(
            |name| vars.get(name).cloned(),
            Some(PathBuf::from("/home/u")),
        )
    }

    #[test]
    fn the_specifications_defaults_apply_when_nothing_is_set() {
        let env = env(&[]);
        assert_eq!(env.config_home, PathBuf::from("/home/u/.config"));
        assert_eq!(env.config_dirs, [PathBuf::from("/etc/xdg")]);
        assert_eq!(env.data_home, PathBuf::from("/home/u/.local/share"));
        assert_eq!(
            env.data_dirs,
            [
                PathBuf::from("/usr/local/share"),
                PathBuf::from("/usr/share")
            ]
        );
        assert!(env.desktops.is_empty());
    }

    #[test]
    fn candidate_files_run_from_the_highest_priority_to_the_lowest() {
        let env = env(&[
            ("XDG_CONFIG_HOME", "/c"),
            ("XDG_CONFIG_DIRS", "/e1:/e2"),
            ("XDG_DATA_HOME", "/d"),
            ("XDG_DATA_DIRS", "/s1"),
            ("XDG_CURRENT_DESKTOP", "X-Cinnamon:GNOME"),
        ]);
        let files: Vec<String> = env
            .candidate_files()
            .iter()
            // Windows joins with a backslash; the expected list is written with slashes.
            .map(|path| path.display().to_string().replace('\\', "/"))
            .collect();
        assert_eq!(
            files,
            [
                "/c/x-cinnamon-mimeapps.list",
                "/c/gnome-mimeapps.list",
                "/c/mimeapps.list",
                "/e1/x-cinnamon-mimeapps.list",
                "/e1/gnome-mimeapps.list",
                "/e1/mimeapps.list",
                "/e2/x-cinnamon-mimeapps.list",
                "/e2/gnome-mimeapps.list",
                "/e2/mimeapps.list",
                "/d/applications/x-cinnamon-mimeapps.list",
                "/d/applications/gnome-mimeapps.list",
                "/d/applications/mimeapps.list",
                "/s1/applications/x-cinnamon-mimeapps.list",
                "/s1/applications/gnome-mimeapps.list",
                "/s1/applications/mimeapps.list",
            ]
        );
    }

    #[test]
    fn a_file_is_parsed_into_its_three_sections() {
        let parsed = parse(
            "# comment\n[Default Applications]\nimage/png=eog.desktop;gimp\n\n[Added Associations]\nimage/png=gimp.desktop;\ntext/plain=gedit.desktop;gedit.desktop\n\n[Removed Associations]\nimage/png=old.desktop\n[Other]\nimage/png=ignored.desktop\nnot a pair\n",
        );
        assert_eq!(
            parsed.defaults["image/png"],
            ["eog.desktop", "gimp.desktop"]
        );
        assert_eq!(parsed.added["image/png"], ["gimp.desktop"]);
        assert_eq!(parsed.added["text/plain"], ["gedit.desktop"]);
        assert_eq!(parsed.removed["image/png"], ["old.desktop"]);
        assert!(!parsed.added["image/png"].contains(&"ignored.desktop".to_string()));
    }

    #[test]
    fn ids_with_and_without_the_suffix_are_the_same_application() {
        assert_eq!(desktop_id("eog"), "eog.desktop");
        assert_eq!(desktop_id("eog.desktop"), "eog.desktop");
        let precedence = Precedence::from_texts([(
            "a",
            "[Default Applications]\nimage/png=eog\n[Added Associations]\nimage/png=eog.desktop;gimp\n",
        )]);
        let resolved = precedence.resolve("image/png", &[], &all_installed);
        assert_eq!(resolved.default.as_deref(), Some("eog.desktop"));
        assert_eq!(resolved.apps, ["eog.desktop", "gimp.desktop"]);
    }

    #[test]
    fn the_highest_priority_default_wins() {
        let precedence = Precedence::from_texts([
            ("user", "[Default Applications]\ntext/plain=kate.desktop\n"),
            (
                "system",
                "[Default Applications]\ntext/plain=gedit.desktop\n",
            ),
        ]);
        let resolved = precedence.resolve("text/plain", &[], &all_installed);
        assert_eq!(resolved.default.as_deref(), Some("kate.desktop"));
    }

    #[test]
    fn an_uninstalled_default_is_skipped_for_the_next_one() {
        let precedence = Precedence::from_texts([
            (
                "user",
                "[Default Applications]\ntext/plain=gone.desktop;kate.desktop\n",
            ),
            (
                "system",
                "[Default Applications]\ntext/plain=gedit.desktop\n",
            ),
        ]);
        let installed = |id: &str| id != "gone.desktop";
        let resolved = precedence.resolve("text/plain", &[], &installed);
        assert_eq!(resolved.default.as_deref(), Some("kate.desktop"));
        assert!(!resolved.apps.contains(&"gone.desktop".to_string()));
    }

    #[test]
    fn a_higher_file_removes_what_a_lower_file_adds_and_the_cache_lists() {
        let precedence = Precedence::from_texts([
            (
                "user",
                "[Added Associations]\nimage/png=mine.desktop\n[Removed Associations]\nimage/png=lower.desktop;cached.desktop\n",
            ),
            ("system", "[Added Associations]\nimage/png=lower.desktop;kept.desktop\n"),
        ]);
        let cached = vec!["cached.desktop".to_string(), "other.desktop".to_string()];
        let resolved = precedence.resolve("image/png", &cached, &all_installed);
        assert_eq!(
            resolved.apps,
            ["mine.desktop", "kept.desktop", "other.desktop"]
        );
        assert_eq!(resolved.default, None);
    }

    #[test]
    fn a_removal_does_not_remove_the_same_files_own_additions() {
        let precedence = Precedence::from_texts([(
            "user",
            "[Added Associations]\nimage/png=a.desktop\n[Removed Associations]\nimage/png=a.desktop\n",
        )]);
        let resolved = precedence.resolve("image/png", &[], &all_installed);
        assert_eq!(resolved.apps, ["a.desktop"]);
    }

    #[test]
    fn the_default_leads_the_list_once() {
        let precedence = Precedence::from_texts([(
            "user",
            "[Default Applications]\nimage/png=b.desktop\n[Added Associations]\nimage/png=a.desktop;b.desktop\n",
        )]);
        let resolved = precedence.resolve("image/png", &[], &all_installed);
        assert_eq!(resolved.apps, ["b.desktop", "a.desktop"]);
    }

    #[test]
    fn types_compare_without_regard_to_case() {
        let precedence =
            Precedence::from_texts([("a", "[Default Applications]\nIMAGE/PNG=eog.desktop\n")]);
        let resolved = precedence.resolve("Image/Png", &[], &all_installed);
        assert_eq!(resolved.default.as_deref(), Some("eog.desktop"));
    }

    #[test]
    fn load_reads_only_the_files_that_exist_in_priority_order() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join("config")).unwrap();
        std::fs::create_dir_all(root.join("share/applications")).unwrap();
        std::fs::write(
            root.join("config/mimeapps.list"),
            "[Default Applications]\nimage/png=user.desktop\n",
        )
        .unwrap();
        std::fs::write(
            root.join("share/applications/mimeapps.list"),
            "[Default Applications]\nimage/png=old.desktop\n",
        )
        .unwrap();
        let env = XdgEnv {
            config_home: root.join("config"),
            config_dirs: vec![root.join("missing")],
            data_home: root.join("share"),
            data_dirs: Vec::new(),
            desktops: vec!["gnome".into()],
        };
        let precedence = Precedence::load(&env);
        assert_eq!(
            precedence.files(),
            [
                root.join("config/mimeapps.list"),
                root.join("share/applications/mimeapps.list")
            ]
        );
        let resolved = precedence.resolve("image/png", &[], &all_installed);
        assert_eq!(resolved.default.as_deref(), Some("user.desktop"));
    }
}
