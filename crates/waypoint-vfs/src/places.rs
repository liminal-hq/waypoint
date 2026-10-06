// Places and favourites: the sidebar's fixed shortcuts and the freedesktop bookmarks file behind them.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The pure logic lives in free functions that take a `PlacesEnv`, so tests run on temporary
//! directories and never touch the real home folder. `PlacesEnv::detect` reads the real one.
//!
//! Favourites are stored in `~/.config/gtk-3.0/bookmarks` on Linux, the file GTK file choosers and
//! other file managers share: one `file:///uri Optional Label` per line. Waypoint keeps every line
//! it does not understand (and every other app's bookmark) exactly as found, and writes the file
//! by renaming a finished temporary file over it, so a crash never leaves half a file. Windows has
//! no such shared file, so favourites live in an app-owned file of the same format.
//!
//! A favourite is a local folder or a folder on a server (`sftp://user@host/path`, `smb://`,
//! `dav://`, `davs://`, `s3://`), written in the canonical address, which never holds a password
//! (D149): a line whose address carries one is not a favourite, so it is kept and not shown, and a
//! password cannot be written. A bookmark in a form Waypoint cannot read (`ftp://`, `ssh://`,
//! `network://`, `x-nautilus-desktop://`, a malformed address) is kept exactly as found and not
//! listed. Reading never opens a connection: whether a server is reachable is the sidebar's
//! question, answered from the connection state it already holds.

use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use waypoint_path::{FilePath, VfsPath};
use waypoint_protocol::{Location, VfsError};

use crate::error::from_io;
use crate::special::{SpecialDirs, SpecialFolder};

/// Which fixed place a sidebar row stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum PlaceKind {
    /// Overview (`overview:/`), the built-in view of the volumes and the Trash. It is not a folder:
    /// no listing is opened for it, and the sidebar lists it first, above Home.
    Overview,
    Home,
    Desktop,
    Documents,
    Downloads,
    Pictures,
    Music,
    Videos,
    /// The Trash (`trash:/`), always last and always present: whether it can be browsed here is the
    /// Trash's own status, which the sidebar explains rather than hides.
    Trash,
}

/// The URI scheme of Overview, a built-in view that is not backed by a provider.
pub const OVERVIEW_SCHEME: &str = "overview";

/// The location of Overview. It names a view, not a listing, so the front end shows the page itself
/// and never asks a provider to open it.
pub fn overview_location() -> Location {
    Location {
        display: "Overview".to_owned(),
        uri: format!("{OVERVIEW_SCHEME}:/"),
    }
}

/// Whether `text` is written in Overview's scheme (`overview:`, `overview:/`, `overview:///`), whatever case.
pub fn is_overview_uri(text: &str) -> bool {
    let text = text.trim();
    let Some(rest) = text.get(OVERVIEW_SCHEME.len()..) else {
        return false;
    };
    text[..OVERVIEW_SCHEME.len()].eq_ignore_ascii_case(OVERVIEW_SCHEME)
        && rest
            .strip_prefix(':')
            .is_some_and(|after| after.is_empty() || after.chars().all(|c| c == '/'))
}

/// A fixed place in the sidebar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct Place {
    pub kind: PlaceKind,
    pub label: String,
    pub location: Location,
}

/// A folder the person pinned to the sidebar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct Favourite {
    pub label: String,
    pub location: Location,
    /// Which of the user's standard folders this is, when it is one, so its icon can say so.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub special: Option<SpecialFolder>,
    /// The login a server folder belongs to (`sftp://me@nas.lan`), the key the sidebar looks the
    /// connection's state up by. Absent for a local folder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub connection: Option<String>,
}

/// Everything the sidebar's Places and Favourites sections show.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct Places {
    /// Home and the user folders that exist, in a fixed order.
    pub places: Vec<Place>,
    /// Favourites in file order.
    pub favourites: Vec<Favourite>,
}

/// Where places are read from and favourites are stored.
#[derive(Debug, Clone)]
pub struct PlacesEnv {
    pub home: PathBuf,
    /// The file Linux reads the user directories from (`user-dirs.dirs`). Unused on Windows.
    pub user_dirs_file: PathBuf,
    /// The bookmarks file favourites are read from and written to.
    pub bookmarks_file: PathBuf,
}

impl PlacesEnv {
    /// The real environment of the current user.
    pub fn detect() -> Result<Self, VfsError> {
        let home = dirs::home_dir().ok_or_else(|| VfsError::Io {
            message: "the home folder could not be found".to_owned(),
            location: None,
        })?;
        let config = dirs::config_dir().unwrap_or_else(|| home.join(".config"));
        #[cfg(windows)]
        let bookmarks_file = config.join("Waypoint").join("favourites");
        #[cfg(not(windows))]
        let bookmarks_file = config.join("gtk-3.0").join("bookmarks");
        Ok(Self {
            user_dirs_file: config.join("user-dirs.dirs"),
            bookmarks_file,
            home,
        })
    }
}

fn to_location(path: &Path) -> Result<Location, VfsError> {
    FilePath::from_path(path)
        .map(|path| path.to_location())
        .map_err(|_| VfsError::InvalidLocation {
            input: path.display().to_string(),
        })
}

fn parse_location(location: &Location) -> Result<VfsPath, VfsError> {
    VfsPath::from_location(location).map_err(|_| VfsError::InvalidLocation {
        input: location.uri.clone(),
    })
}

/// The home folder as a `Location`, for a window's first folder.
pub fn home_location(env: &PlacesEnv) -> Result<Location, VfsError> {
    to_location(&env.home)
}

/// Reads `XDG_*_DIR="…"` lines from `user-dirs.dirs`. `$HOME` expands to `home`. A directory that
/// points at the home folder itself means the person disabled it (the XDG convention), so it is
/// left out.
pub fn parse_user_dirs(text: &str, home: &Path) -> HashMap<String, PathBuf> {
    let mut dirs = HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if !(key.starts_with("XDG_") && key.ends_with("_DIR")) {
            continue;
        }
        let value = value.trim();
        let value = value
            .strip_prefix('"')
            .and_then(|v| v.strip_suffix('"'))
            .unwrap_or(value);
        let path = if let Some(rest) = value.strip_prefix("$HOME") {
            home.join(rest.trim_start_matches('/'))
        } else {
            PathBuf::from(value)
        };
        if !path.is_absolute() || path == home {
            continue;
        }
        dirs.insert(key.to_owned(), path);
    }
    dirs
}

pub(crate) const KINDS: [(PlaceKind, &str, &str, &str); 6] = [
    (PlaceKind::Desktop, "Desktop", "XDG_DESKTOP_DIR", "Desktop"),
    (
        PlaceKind::Documents,
        "Documents",
        "XDG_DOCUMENTS_DIR",
        "Documents",
    ),
    (
        PlaceKind::Downloads,
        "Downloads",
        "XDG_DOWNLOAD_DIR",
        "Downloads",
    ),
    (
        PlaceKind::Pictures,
        "Pictures",
        "XDG_PICTURES_DIR",
        "Pictures",
    ),
    (PlaceKind::Music, "Music", "XDG_MUSIC_DIR", "Music"),
    (PlaceKind::Videos, "Videos", "XDG_VIDEOS_DIR", "Videos"),
];

#[cfg(not(windows))]
pub(crate) fn user_folder(
    env: &PlacesEnv,
    xdg: &HashMap<String, PathBuf>,
    index: usize,
) -> PathBuf {
    let (_, _, key, fallback) = KINDS[index];
    xdg.get(key)
        .cloned()
        .unwrap_or_else(|| env.home.join(fallback))
}

#[cfg(windows)]
pub(crate) fn user_folder(
    env: &PlacesEnv,
    _xdg: &HashMap<String, PathBuf>,
    index: usize,
) -> PathBuf {
    // Windows Known Folders, which the person may have moved to another drive.
    let known = match KINDS[index].0 {
        PlaceKind::Desktop => dirs::desktop_dir(),
        PlaceKind::Documents => dirs::document_dir(),
        PlaceKind::Downloads => dirs::download_dir(),
        PlaceKind::Pictures => dirs::picture_dir(),
        PlaceKind::Music => dirs::audio_dir(),
        PlaceKind::Videos => dirs::video_dir(),
        PlaceKind::Overview | PlaceKind::Home | PlaceKind::Trash => None,
    };
    known.unwrap_or_else(|| env.home.join(KINDS[index].3))
}

/// Overview, Home, whichever of the user folders exist, and the Trash.
pub fn standard_places(env: &PlacesEnv) -> Vec<Place> {
    let xdg = if cfg!(windows) {
        HashMap::new()
    } else {
        fs::read_to_string(&env.user_dirs_file)
            .map(|text| parse_user_dirs(&text, &env.home))
            .unwrap_or_default()
    };
    let mut places = vec![Place {
        kind: PlaceKind::Overview,
        label: "Overview".to_owned(),
        location: overview_location(),
    }];
    if let Ok(location) = to_location(&env.home) {
        places.push(Place {
            kind: PlaceKind::Home,
            label: "Home".to_owned(),
            location,
        });
    }
    for (index, (kind, label, _, _)) in KINDS.iter().enumerate() {
        let path = user_folder(env, &xdg, index);
        if !path.is_dir() {
            continue;
        }
        if let Ok(location) = to_location(&path) {
            places.push(Place {
                kind: *kind,
                label: (*label).to_owned(),
                location,
            });
        }
    }
    places.push(Place {
        kind: PlaceKind::Trash,
        label: "Trash".to_owned(),
        location: VfsPath::Trash(waypoint_path::TrashPath::Root).to_location(),
    });
    places
}

/// One line of a bookmarks file. Lines that are not a favourite Waypoint can read are kept verbatim.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Line {
    Bookmark {
        raw: String,
        path: VfsPath,
        label: Option<String>,
    },
    Other(String),
}

impl Line {
    fn parse(raw: &str) -> Self {
        let trimmed = raw.trim();
        let (uri, label) = match trimmed.split_once(char::is_whitespace) {
            Some((uri, label)) => (uri, Some(label.trim()).filter(|l| !l.is_empty())),
            None => (trimmed, None),
        };
        // Local folders and server folders are favourites. `from_uri` refuses a server address
        // that holds a password, so such a line (and any other file manager's bookmark in a form
        // Waypoint does not read) is kept as it is, untouched.
        match VfsPath::from_uri(uri) {
            Ok(path @ (VfsPath::File(_) | VfsPath::Remote(_))) if uri.contains("://") => {
                Line::Bookmark {
                    raw: raw.to_owned(),
                    path,
                    label: label.map(str::to_owned),
                }
            }
            _ => Line::Other(raw.to_owned()),
        }
    }

    fn text(&self) -> &str {
        match self {
            Line::Bookmark { raw, .. } | Line::Other(raw) => raw,
        }
    }

    fn bookmark(path: &VfsPath, label: Option<&str>) -> Self {
        let label = label
            .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
            .filter(|l| !l.is_empty());
        let raw = match &label {
            Some(label) => format!("{} {label}", path.to_uri()),
            None => path.to_uri(),
        };
        Line::Bookmark {
            raw,
            path: path.clone(),
            label,
        }
    }
}

/// The contents of a bookmarks file, editable line by line.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Bookmarks {
    lines: Vec<Line>,
}

impl Bookmarks {
    pub fn parse(text: &str) -> Self {
        Self {
            lines: text.lines().map(Line::parse).collect(),
        }
    }

    /// The file text: every line, each ending in a newline.
    pub fn to_text(&self) -> String {
        let mut text = String::new();
        for line in &self.lines {
            text.push_str(line.text());
            text.push('\n');
        }
        text
    }

    fn position(&self, target: &VfsPath) -> Option<usize> {
        self.lines.iter().position(|line| match line {
            Line::Bookmark { path, .. } => path == target,
            Line::Other(_) => false,
        })
    }

    fn favourite_lines(&self) -> Vec<usize> {
        (0..self.lines.len())
            .filter(|&i| matches!(self.lines[i], Line::Bookmark { .. }))
            .collect()
    }

    pub fn favourites(&self) -> Vec<Favourite> {
        self.lines
            .iter()
            .filter_map(|line| match line {
                Line::Bookmark { path, label, .. } => Some(Favourite {
                    label: label.clone().unwrap_or_else(|| default_label(path)),
                    location: path.to_location(),
                    special: None,
                    connection: path.connection_key().map(|key| key.as_str().to_owned()),
                }),
                Line::Other(_) => None,
            })
            .collect()
    }

    /// Adds a bookmark at the end. Adding one that already exists changes nothing.
    pub fn add(&mut self, path: &VfsPath, label: Option<&str>) {
        if self.position(path).is_none() {
            self.lines.push(Line::bookmark(path, label));
        }
    }

    /// Removes a bookmark; false when there was none.
    pub fn remove(&mut self, path: &VfsPath) -> bool {
        match self.position(path) {
            Some(at) => {
                self.lines.remove(at);
                true
            }
            None => false,
        }
    }

    /// Sets or clears (`None` or blank) a bookmark's label; false when there was no such bookmark.
    pub fn rename(&mut self, path: &VfsPath, label: Option<&str>) -> bool {
        match self.position(path) {
            Some(at) => {
                self.lines[at] = Line::bookmark(path, label);
                true
            }
            None => false,
        }
    }

    /// Moves a bookmark so it becomes the favourite at `to` (clamped to the end); false when there
    /// was no such bookmark. Other lines stay where they are.
    pub fn move_to(&mut self, path: &VfsPath, to: usize) -> bool {
        let Some(at) = self.position(path) else {
            return false;
        };
        let line = self.lines.remove(at);
        let favourites = self.favourite_lines();
        let insert_at = match favourites.get(to) {
            Some(&index) => index,
            None => favourites.last().map_or(self.lines.len(), |&last| last + 1),
        };
        self.lines.insert(insert_at, line);
        true
    }
}

fn default_label(path: &VfsPath) -> String {
    match path {
        VfsPath::File(file) => file
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| file.display()),
        other => other.label(),
    }
}

/// Whether a location can be a favourite: a local folder or a folder on a server. The Trash, an
/// archive and a Git revision are views Waypoint opens, not places another file manager could
/// share, and a line for one would not read back.
fn can_pin(path: &VfsPath) -> bool {
    matches!(path, VfsPath::File(_) | VfsPath::Remote(_))
}

/// Writes `contents` to `path` by renaming a finished temporary file over it, creating parent
/// folders as needed. A symlinked file is written through, so the link survives.
pub fn write_atomic(path: &Path, contents: &str) -> io::Result<()> {
    let target = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let parent = target.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let name = target
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let temp = parent.join(format!(".{name}.waypoint-{}.tmp", std::process::id()));
    let result = (|| {
        let mut file = fs::File::create(&temp)?;
        file.write_all(contents.as_bytes())?;
        file.sync_all()?;
        if let Ok(meta) = fs::metadata(&target) {
            let _ = fs::set_permissions(&temp, meta.permissions());
        }
        fs::rename(&temp, &target)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

fn read_bookmarks(env: &PlacesEnv) -> Result<Bookmarks, VfsError> {
    match fs::read_to_string(&env.bookmarks_file) {
        Ok(text) => Ok(Bookmarks::parse(&text)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Bookmarks::default()),
        Err(error) => Err(from_io(&error, &to_location(&env.bookmarks_file)?)),
    }
}

fn save(env: &PlacesEnv, bookmarks: &Bookmarks) -> Result<(), VfsError> {
    write_atomic(&env.bookmarks_file, &bookmarks.to_text()).map_err(|error| {
        from_io(
            &error,
            &to_location(&env.bookmarks_file).unwrap_or_else(|_| Location::new("", "")),
        )
    })
}

/// Home, the user folders that exist and the favourites.
pub fn list_places(env: &PlacesEnv) -> Result<Places, VfsError> {
    let special = SpecialDirs::from_env(env);
    let mut favourites = read_bookmarks(env)?.favourites();
    for favourite in &mut favourites {
        if let Ok(VfsPath::File(path)) = VfsPath::from_location(&favourite.location) {
            favourite.special = special.lookup(path.as_path());
        }
    }
    Ok(Places {
        places: standard_places(env),
        favourites,
    })
}

fn edit(
    env: &PlacesEnv,
    location: &Location,
    change: impl FnOnce(&mut Bookmarks, &VfsPath) -> bool,
) -> Result<Places, VfsError> {
    let path = parse_location(location)?;
    let mut bookmarks = read_bookmarks(env)?;
    if change(&mut bookmarks, &path) {
        save(env, &bookmarks)?;
    }
    list_places(env)
}

/// Pins a folder to the favourites; pinning one that is already there changes nothing.
pub fn add_favourite(
    env: &PlacesEnv,
    location: &Location,
    label: Option<&str>,
) -> Result<Places, VfsError> {
    if !can_pin(&parse_location(location)?) {
        return Err(VfsError::InvalidLocation {
            input: location.uri.clone(),
        });
    }
    edit(env, location, |bookmarks, path| {
        let before = bookmarks.lines.len();
        bookmarks.add(path, label);
        bookmarks.lines.len() != before
    })
}

/// Unpins a folder; unpinning one that is not there is not an error.
pub fn remove_favourite(env: &PlacesEnv, location: &Location) -> Result<Places, VfsError> {
    edit(env, location, |bookmarks, path| bookmarks.remove(path))
}

/// Gives a favourite a label, or clears it with `None` or blank text.
pub fn rename_favourite(
    env: &PlacesEnv,
    location: &Location,
    label: Option<&str>,
) -> Result<Places, VfsError> {
    edit(env, location, |bookmarks, path| {
        bookmarks.rename(path, label)
    })
}

/// Moves a favourite to position `to` in the list.
pub fn move_favourite(env: &PlacesEnv, location: &Location, to: usize) -> Result<Places, VfsError> {
    edit(env, location, |bookmarks, path| bookmarks.move_to(path, to))
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn env(dir: &Path) -> PlacesEnv {
        PlacesEnv {
            home: dir.join("home"),
            user_dirs_file: dir.join("config/user-dirs.dirs"),
            bookmarks_file: dir.join("config/gtk-3.0/bookmarks"),
        }
    }

    fn loc(path: &str) -> Location {
        FilePath::parse(path).unwrap().to_location()
    }

    #[test]
    fn user_dirs_expand_home_and_skip_disabled_entries() {
        let home = Path::new("/home/a");
        let dirs = parse_user_dirs(
            "# comment\nXDG_DESKTOP_DIR=\"$HOME/Bureau\"\nXDG_MUSIC_DIR=\"/mnt/music\"\nXDG_VIDEOS_DIR=\"$HOME/\"\nXDG_PICTURES_DIR=\"relative\"\nnoise\n",
            home,
        );
        assert_eq!(dirs["XDG_DESKTOP_DIR"], Path::new("/home/a/Bureau"));
        assert_eq!(dirs["XDG_MUSIC_DIR"], Path::new("/mnt/music"));
        assert!(!dirs.contains_key("XDG_VIDEOS_DIR"));
        assert!(!dirs.contains_key("XDG_PICTURES_DIR"));
    }

    #[test]
    fn places_use_user_dirs_then_home_fallbacks_and_only_existing_folders() {
        let tmp = tempfile::tempdir().unwrap();
        let env = env(tmp.path());
        fs::create_dir_all(env.home.join("Bureau")).unwrap();
        fs::create_dir_all(env.home.join("Downloads")).unwrap();
        fs::create_dir_all(env.home.join("Documents")).unwrap();
        fs::create_dir_all(env.user_dirs_file.parent().unwrap()).unwrap();
        fs::write(
            &env.user_dirs_file,
            "XDG_DESKTOP_DIR=\"$HOME/Bureau\"\nXDG_DOCUMENTS_DIR=\"$HOME/Nowhere\"\n",
        )
        .unwrap();
        let places = standard_places(&env);
        let kinds: Vec<_> = places.iter().map(|p| p.kind).collect();
        // Documents points at a folder that does not exist, so it is hidden; the fallback is
        // used only when the file says nothing.
        assert_eq!(
            kinds,
            [
                PlaceKind::Overview,
                PlaceKind::Home,
                PlaceKind::Desktop,
                PlaceKind::Downloads,
                PlaceKind::Trash
            ]
        );
        assert!(places[2].location.uri.ends_with("/home/Bureau"));
        assert_eq!(places[2].label, "Desktop");
    }

    #[test]
    fn overview_is_the_first_place_and_a_view_rather_than_a_folder() {
        let tmp = tempfile::tempdir().unwrap();
        let places = standard_places(&env(tmp.path()));
        assert_eq!(places[0].kind, PlaceKind::Overview);
        assert_eq!(places[0].location, overview_location());
        assert_eq!(places[0].location.uri, "overview:/");
        // Overview is not a path of any provider, so nothing can open a listing for it.
        assert!(VfsPath::from_location(&places[0].location).is_err());
    }

    #[test]
    fn overview_uris_are_recognised_in_any_case_and_nothing_else_is() {
        for text in ["overview:", "overview:/", "overview:///", " Overview:/ "] {
            assert!(is_overview_uri(text), "{text}");
        }
        for text in [
            "overview:/x",
            "overviews:/",
            "overview",
            "file:///overview:",
            "",
        ] {
            assert!(!is_overview_uri(text), "{text}");
        }
    }

    #[test]
    fn bookmarks_parse_labels_and_keep_unknown_lines() {
        let text = "file:///a/b Bee\nfile:///c\n# odd\nftp://host/share Remote\n";
        let bookmarks = Bookmarks::parse(text);
        let favourites = bookmarks.favourites();
        assert_eq!(favourites.len(), 2);
        assert_eq!(favourites[0].label, "Bee");
        assert_eq!(favourites[1].label, "c");
        assert_eq!(bookmarks.to_text(), text);
    }

    #[test]
    fn edits_leave_other_apps_lines_intact() {
        let mut bookmarks = Bookmarks::parse("file:///keep Keep\nftp://host/share Remote\n");
        let added = FilePath::parse("/new/folder").unwrap().into();
        bookmarks.add(&added, Some("  My   folder "));
        bookmarks.add(&added, Some("duplicate"));
        assert_eq!(
            bookmarks.to_text(),
            "file:///keep Keep\nftp://host/share Remote\nfile:///new/folder My folder\n"
        );
        assert!(bookmarks.rename(&added, None));
        assert!(bookmarks.remove(&VfsPath::parse_input("/keep").unwrap()));
        assert!(!bookmarks.remove(&VfsPath::parse_input("/keep").unwrap()));
        assert_eq!(
            bookmarks.to_text(),
            "ftp://host/share Remote\nfile:///new/folder\n"
        );
    }

    #[test]
    fn moving_reorders_favourites_only() {
        let mut bookmarks = Bookmarks::parse("file:///a\nfile:///b\nweird\nfile:///c\n");
        let c = VfsPath::parse_input("/c").unwrap();
        assert!(bookmarks.move_to(&c, 0));
        assert_eq!(
            bookmarks.to_text(),
            "file:///c\nfile:///a\nfile:///b\nweird\n"
        );
        let c_again = VfsPath::parse_input("/c").unwrap();
        assert!(bookmarks.move_to(&c_again, 99));
        assert_eq!(
            bookmarks.to_text(),
            "file:///a\nfile:///b\nfile:///c\nweird\n"
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn a_favourite_that_is_a_standard_folder_says_which() {
        let tmp = tempfile::tempdir().unwrap();
        let env = env(tmp.path());
        let downloads = env.home.join("Downloads").display().to_string();
        add_favourite(&env, &loc(&downloads), None).unwrap();
        let places = add_favourite(&env, &loc("/srv/data"), None).unwrap();
        let specials: Vec<_> = places.favourites.iter().map(|f| f.special).collect();
        assert_eq!(specials, [Some(SpecialFolder::Downloads), None]);
    }

    #[test]
    fn favourites_round_trip_through_the_file_atomically() {
        let tmp = tempfile::tempdir().unwrap();
        let env = env(tmp.path());
        assert!(list_places(&env).unwrap().favourites.is_empty());
        add_favourite(&env, &loc("/srv/data"), Some("Data")).unwrap();
        let places = add_favourite(&env, &loc("/srv/other"), None).unwrap();
        assert_eq!(places.favourites.len(), 2);
        assert_eq!(
            fs::read_to_string(&env.bookmarks_file).unwrap(),
            "file:///srv/data Data\nfile:///srv/other\n"
        );
        rename_favourite(&env, &loc("/srv/other"), Some("Other")).unwrap();
        move_favourite(&env, &loc("/srv/other"), 0).unwrap();
        let places = remove_favourite(&env, &loc("/srv/data")).unwrap();
        assert_eq!(places.favourites[0].label, "Other");
        assert_eq!(places.favourites.len(), 1);
        // No temporary file is left behind.
        let leftovers: Vec<_> = fs::read_dir(env.bookmarks_file.parent().unwrap())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(leftovers, [std::ffi::OsString::from("bookmarks")]);
    }

    #[test]
    fn a_failed_write_keeps_the_old_file_and_cleans_up() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("bookmarks");
        fs::write(&target, "old\n").unwrap();
        // Renaming a file over a directory fails, after the temporary file was written.
        let dir_target = tmp.path().join("dir");
        fs::create_dir(&dir_target).unwrap();
        assert!(write_atomic(&dir_target, "new\n").is_err());
        assert_eq!(fs::read_to_string(&target).unwrap(), "old\n");
        let names = fs::read_dir(tmp.path()).unwrap().count();
        assert_eq!(names, 2);
    }

    const SERVER_FILE: &str = "# my bookmarks\n\
        file:///srv/data Data\n\
        sftp://scott@nas.lan/home/scott/My%20Pictures Pictures on the NAS\n\
        smb://host/share\n\
        ssh://old@host/path SSH form\n\
        ftp://ftp.example.org/pub\n\
        network:///\n\
        x-nautilus-desktop:///computer Computer\n\
        sftp://me:hunter2@host/secret Has password\n\
        davs://user@dav.example.org/files/\n\
        \n\
        sftp://[bad/x\n";

    #[test]
    fn server_bookmarks_are_favourites_and_what_waypoint_cannot_read_stays_as_found() {
        let bookmarks = Bookmarks::parse(SERVER_FILE);
        let favourites = bookmarks.favourites();
        let seen: Vec<_> = favourites
            .iter()
            .map(|f| {
                (
                    f.label.as_str(),
                    f.location.uri.as_str(),
                    f.connection.as_deref(),
                )
            })
            .collect();
        assert_eq!(
            seen,
            [
                ("Data", "file:///srv/data", None),
                (
                    "Pictures on the NAS",
                    "sftp://scott@nas.lan/home/scott/My%20Pictures",
                    Some("sftp://scott@nas.lan")
                ),
                ("share", "smb://host/share", Some("smb://host")),
                (
                    "files",
                    "davs://user@dav.example.org/files",
                    Some("davs://user@dav.example.org")
                ),
            ]
        );
        // Nothing is rewritten by reading, and a password never becomes a favourite.
        assert_eq!(bookmarks.to_text(), SERVER_FILE);
        assert!(!favourites
            .iter()
            .any(|f| f.location.uri.contains("hunter2")));
    }

    #[test]
    fn editing_server_favourites_keeps_every_other_line_byte_for_byte() {
        let mut bookmarks = Bookmarks::parse(SERVER_FILE);
        let nas = VfsPath::from_uri("sftp://scott@nas.lan/home/scott/My%20Pictures").unwrap();
        assert!(bookmarks.rename(&nas, Some("  NAS   pictures ")));
        let moved = VfsPath::from_uri("davs://user@dav.example.org/files").unwrap();
        assert!(bookmarks.move_to(&moved, 0));
        let new = VfsPath::parse_input("sftp://scott@nas.lan/home/scott/Odd #1 & 100%").unwrap();
        bookmarks.add(&new, Some("Odd"));
        let text = bookmarks.to_text();
        assert!(text.contains("sftp://scott@nas.lan/home/scott/My%20Pictures NAS pictures\n"));
        assert!(text.contains("sftp://me:hunter2@host/secret Has password\n"));
        assert!(text.contains("ssh://old@host/path SSH form\n"));
        assert!(text.contains("network:///\n"));
        assert!(text.contains("sftp://[bad/x\n"));
        assert!(text.starts_with("# my bookmarks\ndavs://user@dav.example.org/files/\n"));
        // The odd characters are percent-encoded, so the line reads back as the same folder.
        let line = text.lines().last().unwrap();
        assert!(line.ends_with(" Odd"), "{line}");
        assert!(!line.split(' ').next().unwrap().contains(['#', '&', ' ']));
        let again = Bookmarks::parse(&text);
        assert!(again
            .favourites()
            .iter()
            .any(|f| f.location.uri == new.to_uri()));
        assert!(bookmarks.remove(&nas));
        assert!(!bookmarks.to_text().contains("My%20Pictures"));
    }

    #[test]
    fn a_server_folder_is_pinned_without_a_password_and_round_trips_through_the_file() {
        let tmp = tempfile::tempdir().unwrap();
        let env = env(tmp.path());
        let places = add_favourite(
            &env,
            &Location::new("", "sftp://scott@192.168.1.100/home/scott/Pictures"),
            None,
        )
        .unwrap();
        assert_eq!(places.favourites[0].label, "Pictures");
        assert_eq!(
            places.favourites[0].connection.as_deref(),
            Some("sftp://scott@192.168.1.100")
        );
        assert_eq!(
            fs::read_to_string(&env.bookmarks_file).unwrap(),
            "sftp://scott@192.168.1.100/home/scott/Pictures\n"
        );
        // A stored address never holds a password: one that does is refused, not stripped.
        let error = add_favourite(
            &env,
            &Location::new("", "sftp://scott:pw@192.168.1.100/home"),
            None,
        )
        .unwrap_err();
        assert!(matches!(error, VfsError::InvalidLocation { .. }));
        // The Trash, an archive and a Git revision are not favourites.
        for uri in ["trash:///", "archive+file:///a.zip!/"] {
            assert!(
                add_favourite(&env, &Location::new("", uri), None).is_err(),
                "{uri}"
            );
        }
        rename_favourite(
            &env,
            &Location::new("", "sftp://scott@192.168.1.100/home/scott/Pictures"),
            Some("NAS"),
        )
        .unwrap();
        let places = remove_favourite(
            &env,
            &Location::new("", "sftp://scott@192.168.1.100/home/scott/Pictures"),
        )
        .unwrap();
        assert!(places.favourites.is_empty());
    }

    #[test]
    fn a_non_file_location_is_invalid() {
        let tmp = tempfile::tempdir().unwrap();
        let env = env(tmp.path());
        let error = add_favourite(&env, &Location::new("x", "nonsense"), None).unwrap_err();
        assert!(matches!(error, VfsError::InvalidLocation { .. }));
    }

    #[test]
    fn home_is_a_location() {
        let tmp = tempfile::tempdir().unwrap();
        let env = env(tmp.path());
        assert!(home_location(&env).unwrap().uri.ends_with("/home"));
    }
}
