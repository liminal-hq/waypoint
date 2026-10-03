// The icons the system draws for a type of file and for folders: what a request names, which icon names a theme is asked for, and the bounded cache of the pictures made
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! An icon is asked for by a *type* (a content type, an extension, or a kind of folder), never by a file, so a listing of ten thousand files needs as many requests as it has distinct types. Everything here is pure: the platform backends turn a request into a picture, and the `typeicon://` scheme in `scheme.rs` serves and caches it.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use crate::target::decode;

/// The name of the scheme.
pub const SCHEME: &str = "typeicon";

/// Where the webview finds the scheme: Windows (and Android) webviews serve custom schemes from `http://{scheme}.localhost`.
#[cfg(windows)]
pub const URL_BASE: &str = "http://typeicon.localhost";
#[cfg(not(windows))]
pub const URL_BASE: &str = "typeicon://localhost";

/// The logical edge of an icon in pixels; a request outside the range is clamped into it.
pub const MIN_SIZE: u32 = 16;
pub const MAX_SIZE: u32 = 256;
pub const DEFAULT_SIZE: u32 = 32;
/// The device pixel ratios served; a request outside the range is clamped into it.
pub const MAX_SCALE: u32 = 3;
/// The most pixels along one edge of a picture, whatever the size and the scale were.
pub const MAX_PIXELS: u32 = 512;

/// How many pictures are kept, and how many bytes of them, before the oldest are dropped.
pub const CACHE_ENTRIES: usize = 1024;
pub const CACHE_BYTES: usize = 32 * 1024 * 1024;

/// One of the user's standard folders, or an ordinary one. The names are the ones the front end sends, which are the lower-case names of the freedesktop.org user directories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FolderKind {
    Plain,
    Home,
    Desktop,
    Documents,
    Downloads,
    Pictures,
    Music,
    Videos,
    Templates,
    Public,
}

impl FolderKind {
    pub const ALL: [FolderKind; 10] = [
        FolderKind::Plain,
        FolderKind::Home,
        FolderKind::Desktop,
        FolderKind::Documents,
        FolderKind::Downloads,
        FolderKind::Pictures,
        FolderKind::Music,
        FolderKind::Videos,
        FolderKind::Templates,
        FolderKind::Public,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            FolderKind::Plain => "plain",
            FolderKind::Home => "home",
            FolderKind::Desktop => "desktop",
            FolderKind::Documents => "documents",
            FolderKind::Downloads => "downloads",
            FolderKind::Pictures => "pictures",
            FolderKind::Music => "music",
            FolderKind::Videos => "videos",
            FolderKind::Templates => "templates",
            FolderKind::Public => "public",
        }
    }

    pub fn parse(name: &str) -> Option<FolderKind> {
        FolderKind::ALL
            .into_iter()
            .find(|kind| kind.as_str() == name)
    }

    /// The icon names themes use for this folder, most specific first, then the plain folder's. A theme that has none of the specific ones draws the plain folder.
    pub fn icon_names(self) -> Vec<&'static str> {
        let special: &[&str] = match self {
            FolderKind::Plain => &[],
            FolderKind::Home => &["user-home", "folder-home"],
            FolderKind::Desktop => &["user-desktop", "folder-desktop"],
            FolderKind::Documents => &["folder-documents", "folder-document"],
            FolderKind::Downloads => &["folder-download", "folder-downloads"],
            FolderKind::Pictures => &["folder-pictures", "folder-image"],
            FolderKind::Music => &["folder-music", "folder-sound"],
            FolderKind::Videos => &["folder-videos", "folder-video"],
            FolderKind::Templates => &["folder-templates", "folder-template"],
            FolderKind::Public => &["folder-publicshare", "folder-public"],
        };
        special.iter().chain(PLAIN_FOLDER_NAMES).copied().collect()
    }
}

/// What gio gives the `inode/directory` type: the plain folder.
const PLAIN_FOLDER_NAMES: &[&str] = &["inode-directory", "folder"];

/// What an icon is for.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum IconKind {
    /// A content type (`application/pdf`).
    Mime(String),
    /// A file name extension without its dot, in lower case (`pdf`): the type is what the system guesses from `x.pdf`.
    Extension(String),
    Folder(FolderKind),
}

impl IconKind {
    /// Reads the path of a request: `/mime/{type}`, `/ext/{extension}` or `/folder/{kind}`, percent-decoded, with nothing that could name a file. `None` for anything else.
    pub fn parse_path(path: &str) -> Option<IconKind> {
        let rest = path.strip_prefix('/')?;
        let (kind, value) = rest.split_once('/')?;
        let value = decode(value)?;
        match kind {
            "mime" => valid_mime(&value).then_some(IconKind::Mime(value.to_ascii_lowercase())),
            "ext" => {
                valid_extension(&value).then_some(IconKind::Extension(value.to_ascii_lowercase()))
            }
            "folder" => FolderKind::parse(&value).map(IconKind::Folder),
            _ => None,
        }
    }

    /// The path a request for this kind uses, escaped so the value stays one segment.
    pub fn path(&self) -> String {
        let (kind, value) = match self {
            IconKind::Mime(mime) => ("mime", mime.as_str()),
            IconKind::Extension(extension) => ("ext", extension.as_str()),
            IconKind::Folder(folder) => ("folder", folder.as_str()),
        };
        format!("/{kind}/{}", escape(value))
    }
}

/// A content type: `type/subtype` of the characters the registry allows, and nothing longer than it needs to be.
fn valid_mime(text: &str) -> bool {
    let allowed = |c: char| c.is_ascii_alphanumeric() || "!#$&^_.+-".contains(c);
    text.len() <= 127
        && text.split_once('/').is_some_and(|(top, sub)| {
            !top.is_empty()
                && !sub.is_empty()
                && top.chars().all(allowed)
                && sub.chars().all(allowed)
                && !sub.contains('/')
        })
}

/// An extension: letters, digits and a few marks, short enough to be one.
fn valid_extension(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= 24
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '+' | '~'))
}

/// A theme name as `gtk-icon-theme-name` and the folders under `icons/` write it.
pub fn valid_theme(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= 64
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '+' | '.' | ' '))
        && !text.starts_with('.')
}

fn escape(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// One picture asked for: what, how big, at what device pixel ratio and in which icon theme.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IconRequest {
    pub kind: IconKind,
    /// The logical edge, 16 to 256.
    pub size: u32,
    /// The device pixel ratio, 1 to 3; the picture is `size * scale` pixels along an edge.
    pub scale: u32,
    /// The icon theme to draw from. `None` is the one the system is set to now.
    pub theme: Option<String>,
}

impl IconRequest {
    /// Reads a request's path and query. `None` when the path is not one the scheme serves.
    pub fn from_uri(path: &str, query: Option<&str>) -> Option<IconRequest> {
        let kind = IconKind::parse_path(path)?;
        let param = |name: &str| {
            query
                .into_iter()
                .flat_map(|query| query.split('&'))
                .find_map(|pair| pair.strip_prefix(name)?.strip_prefix('='))
        };
        let number = |name: &str| param(name).and_then(|value| value.parse::<u32>().ok());
        let theme = param("theme")
            .and_then(decode)
            .filter(|theme| valid_theme(theme));
        Some(IconRequest::new(
            kind,
            number("size").unwrap_or(DEFAULT_SIZE),
            number("scale").unwrap_or(1),
            theme,
        ))
    }

    /// A request with the size and the scale brought into range.
    pub fn new(kind: IconKind, size: u32, scale: u32, theme: Option<String>) -> IconRequest {
        let scale = scale.clamp(1, MAX_SCALE);
        let size = size.clamp(MIN_SIZE, MAX_SIZE).min(MAX_PIXELS / scale);
        IconRequest {
            kind,
            size,
            scale,
            theme,
        }
    }

    /// The edge of the picture in device pixels.
    pub fn pixels(&self) -> u32 {
        self.size * self.scale
    }
}

/// What the cache is keyed by: the request with the theme settled (the system's own name when the request had none).
pub type CacheKey = IconRequest;

/// What a platform can say about names, so the choice of what to ask a theme for is the same everywhere and tested without one.
pub trait NameSource {
    /// The icon names for a content type, most specific first.
    fn mime_names(&self, mime: &str) -> Vec<String>;
    /// The content type the system guesses for a file called `x.{extension}`.
    fn mime_for_extension(&self, extension: &str) -> String;
}

/// The names a theme is asked for, in order: for a folder the standard folder's names and then the plain folder's; for a type the names the system gives it. Symbolic names are dropped (an icon in the file manager is the coloured one), as are empty names and repeats.
pub fn candidate_names(kind: &IconKind, source: &dyn NameSource) -> Vec<String> {
    let raw: Vec<String> = match kind {
        IconKind::Folder(folder) => folder.icon_names().into_iter().map(String::from).collect(),
        IconKind::Mime(mime) => source.mime_names(mime),
        IconKind::Extension(extension) => {
            let mime = source.mime_for_extension(extension);
            source.mime_names(&mime)
        }
    };
    let mut seen = Vec::<String>::new();
    for name in raw {
        if !name.is_empty() && !name.ends_with("-symbolic") && !seen.contains(&name) {
            seen.push(name);
        }
    }
    seen
}

/// The first of `names` the theme has. Themes are searched name by name, so a name earlier in the list wins over any later one whatever the theme inherits.
pub fn first_available(names: &[String], has: impl Fn(&str) -> bool) -> Option<&str> {
    names.iter().map(String::as_str).find(|name| has(name))
}

/// The address of an icon, for tests and Rust callers; the guest-side `typeIconUrl` makes the same one. `revision` only changes the address, so a page that has been told the theme changed does not get a picture the webview kept.
pub fn url_for(request: &IconRequest, revision: u32) -> String {
    let mut url = format!(
        "{URL_BASE}{}?size={}&scale={}",
        request.kind.path(),
        request.size,
        request.scale
    );
    if let Some(theme) = &request.theme {
        url.push_str("&theme=");
        url.push_str(&escape(theme));
    }
    if revision > 0 {
        url.push_str(&format!("&v={revision}"));
    }
    url
}

struct Slots {
    /// `None` is a request the system could not answer, remembered so a missing icon is not looked for again.
    pictures: HashMap<CacheKey, Option<Arc<Vec<u8>>>>,
    order: VecDeque<CacheKey>,
    bytes: usize,
}

/// Pictures already made, by request. Bounded in entries and in bytes: the oldest go first.
pub struct TypeIconCache {
    slots: Mutex<Slots>,
    max_entries: usize,
    max_bytes: usize,
}

impl Default for TypeIconCache {
    fn default() -> Self {
        TypeIconCache::with_limits(CACHE_ENTRIES, CACHE_BYTES)
    }
}

impl TypeIconCache {
    pub fn with_limits(max_entries: usize, max_bytes: usize) -> Self {
        TypeIconCache {
            slots: Mutex::new(Slots {
                pictures: HashMap::new(),
                order: VecDeque::new(),
                bytes: 0,
            }),
            max_entries: max_entries.max(1),
            max_bytes,
        }
    }

    /// The picture for the key, made once by `make` (a miss that `make` could not fill is remembered too). `None` when there is none.
    pub fn get_or_make(
        &self,
        key: &CacheKey,
        make: impl FnOnce() -> Option<Vec<u8>>,
    ) -> Option<Arc<Vec<u8>>> {
        if let Some(found) = self.slots.lock().ok()?.pictures.get(key) {
            return found.clone();
        }
        let made = make().map(Arc::new);
        let mut slots = self.slots.lock().ok()?;
        let size = made.as_ref().map_or(0, |bytes| bytes.len());
        // Too large for the cache on its own: serve it, keep nothing.
        if size > self.max_bytes {
            return made;
        }
        while slots.pictures.len() >= self.max_entries
            || (slots.bytes + size > self.max_bytes && !slots.order.is_empty())
        {
            let Some(oldest) = slots.order.pop_front() else {
                break;
            };
            if let Some(Some(dropped)) = slots.pictures.remove(&oldest) {
                slots.bytes -= dropped.len();
            }
        }
        if slots.pictures.insert(key.clone(), made.clone()).is_none() {
            slots.order.push_back(key.clone());
        }
        slots.bytes += size;
        made
    }

    /// Forgets every picture: the icon theme changed, or its files did.
    pub fn clear(&self) {
        if let Ok(mut slots) = self.slots.lock() {
            slots.pictures.clear();
            slots.order.clear();
            slots.bytes = 0;
        }
    }

    pub fn len(&self) -> usize {
        self.slots.lock().map_or(0, |slots| slots.pictures.len())
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn bytes(&self) -> usize {
        self.slots.lock().map_or(0, |slots| slots.bytes)
    }
}

/// The extension Windows is asked about for a content type the front end sends when a file has no extension to go by. Windows has no content types of its own, so a few well-known ones stand for the shell's file types; anything else is the shell's plain file.
pub fn extension_for_mime(mime: &str) -> &'static str {
    match mime {
        "text/plain" => ".txt",
        "text/markdown" => ".md",
        "text/html" => ".html",
        "application/pdf" => ".pdf",
        "application/zip" => ".zip",
        "application/x-executable" => ".exe",
        "application/x-shellscript" => ".bat",
        "inode/symlink" => ".lnk",
        "application/x-iso9660-image" => ".iso",
        m if m.starts_with("image/") => ".png",
        m if m.starts_with("audio/") => ".mp3",
        m if m.starts_with("video/") => ".mp4",
        m if m.starts_with("text/") => ".txt",
        _ => ".unknown-type",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Names;

    impl NameSource for Names {
        fn mime_names(&self, mime: &str) -> Vec<String> {
            match mime {
                "application/pdf" => vec![
                    "application-pdf".into(),
                    "gnome-mime-application-pdf".into(),
                    "x-office-document".into(),
                    "application-pdf-symbolic".into(),
                ],
                _ => vec!["application-x-generic".into(), "".into()],
            }
        }
        fn mime_for_extension(&self, extension: &str) -> String {
            match extension {
                "pdf" => "application/pdf".into(),
                _ => "application/octet-stream".into(),
            }
        }
    }

    fn request(kind: IconKind, size: u32, scale: u32) -> IconRequest {
        IconRequest::new(kind, size, scale, None)
    }

    #[test]
    fn paths_name_a_mime_an_extension_or_a_folder_kind_and_nothing_else() {
        assert_eq!(
            IconKind::parse_path("/mime/application%2Fpdf"),
            Some(IconKind::Mime("application/pdf".into()))
        );
        assert_eq!(
            IconKind::parse_path("/ext/PDF"),
            Some(IconKind::Extension("pdf".into()))
        );
        assert_eq!(
            IconKind::parse_path("/folder/downloads"),
            Some(IconKind::Folder(FolderKind::Downloads))
        );
        for path in [
            "",
            "/",
            "/mime",
            "/mime/",
            "/mime/pdf",
            "/mime/a%2Fb%2Fc",
            "/mime/..%2F..%2Fetc",
            "/mime/%2Fetc%2Fpasswd",
            "/mime/image%2F",
            "/ext/",
            "/ext/a.b",
            "/ext/a%2Fb",
            "/ext/..",
            "/ext/%00",
            "/ext/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "/folder/etc",
            "/folder/../home",
            "/folder/home/extra",
            "/other/x",
            "/%zz",
        ] {
            assert_eq!(IconKind::parse_path(path), None, "{path}");
        }
    }

    #[test]
    fn a_kind_round_trips_through_its_path() {
        for kind in [
            IconKind::Mime("application/vnd.oasis.opendocument.text".into()),
            IconKind::Mime("image/svg+xml".into()),
            IconKind::Extension("tar-gz".into()),
            IconKind::Folder(FolderKind::Public),
        ] {
            assert_eq!(IconKind::parse_path(&kind.path()), Some(kind));
        }
    }

    #[test]
    fn size_and_scale_are_clamped_and_never_make_a_huge_picture() {
        let r = request(IconKind::Folder(FolderKind::Plain), 1, 0);
        assert_eq!((r.size, r.scale), (MIN_SIZE, 1));
        let r = request(IconKind::Folder(FolderKind::Plain), 9999, 9);
        assert_eq!(r.scale, MAX_SCALE);
        assert!(r.pixels() <= MAX_PIXELS);
        let r = request(IconKind::Folder(FolderKind::Plain), 256, 2);
        assert_eq!(r.pixels(), 512);
    }

    #[test]
    fn a_request_is_read_from_its_query_with_defaults() {
        let r = IconRequest::from_uri("/ext/png", None).unwrap();
        assert_eq!((r.size, r.scale, r.theme), (DEFAULT_SIZE, 1, None));
        let r = IconRequest::from_uri("/ext/png", Some("v=3&size=48&scale=2&theme=Papirus%20Dark"))
            .unwrap();
        assert_eq!((r.size, r.scale), (48, 2));
        assert_eq!(r.theme.as_deref(), Some("Papirus Dark"));
        // A theme that is not a name is ignored, so the system's own is used.
        for bad in ["theme=..%2F..%2Fx", "theme=", "theme=%zz", "theme=.hidden"] {
            let r = IconRequest::from_uri("/ext/png", Some(bad)).unwrap();
            assert_eq!(r.theme, None, "{bad}");
        }
        assert_eq!(IconRequest::from_uri("/nope", None), None);
    }

    #[test]
    fn urls_carry_the_kind_size_scale_theme_and_revision() {
        let r = IconRequest::new(
            IconKind::Mime("image/png".into()),
            24,
            2,
            Some("Breeze Dark".into()),
        );
        assert_eq!(
            url_for(&r, 0),
            format!("{URL_BASE}/mime/image%2Fpng?size=24&scale=2&theme=Breeze%20Dark")
        );
        assert!(url_for(&r, 4).ends_with("&v=4"));
        let again = {
            let url = url_for(&r, 4);
            let rest = url.strip_prefix(URL_BASE).unwrap();
            let (path, query) = rest.split_once('?').unwrap();
            IconRequest::from_uri(path, Some(query)).unwrap()
        };
        assert_eq!(again, r);
    }

    #[test]
    fn standard_folders_ask_for_their_own_names_then_the_plain_folders() {
        let expected: [(FolderKind, &str); 9] = [
            (FolderKind::Home, "user-home"),
            (FolderKind::Desktop, "user-desktop"),
            (FolderKind::Documents, "folder-documents"),
            (FolderKind::Downloads, "folder-download"),
            (FolderKind::Pictures, "folder-pictures"),
            (FolderKind::Music, "folder-music"),
            (FolderKind::Videos, "folder-videos"),
            (FolderKind::Templates, "folder-templates"),
            (FolderKind::Public, "folder-publicshare"),
        ];
        for (kind, first) in expected {
            let names = kind.icon_names();
            assert_eq!(names[0], first, "{kind:?}");
            assert_eq!(&names[names.len() - 2..], PLAIN_FOLDER_NAMES, "{kind:?}");
        }
        assert_eq!(FolderKind::Plain.icon_names(), PLAIN_FOLDER_NAMES);
        for kind in FolderKind::ALL {
            assert_eq!(FolderKind::parse(kind.as_str()), Some(kind));
        }
    }

    #[test]
    fn a_type_asks_for_the_names_the_system_gives_it_without_symbolic_or_empty_ones() {
        let names = candidate_names(&IconKind::Mime("application/pdf".into()), &Names);
        assert_eq!(
            names,
            [
                "application-pdf",
                "gnome-mime-application-pdf",
                "x-office-document"
            ]
        );
        // An extension goes through the type the system guesses for it.
        assert_eq!(
            candidate_names(&IconKind::Extension("pdf".into()), &Names),
            names
        );
        assert_eq!(
            candidate_names(&IconKind::Extension("zzz".into()), &Names),
            ["application-x-generic"]
        );
    }

    #[test]
    fn names_repeat_nowhere_and_keep_their_order() {
        struct Repeats;
        impl NameSource for Repeats {
            fn mime_names(&self, _: &str) -> Vec<String> {
                ["b", "a", "b", "a-symbolic", "c", "a"]
                    .map(String::from)
                    .to_vec()
            }
            fn mime_for_extension(&self, _: &str) -> String {
                String::new()
            }
        }
        assert_eq!(
            candidate_names(&IconKind::Mime("a/b".into()), &Repeats),
            ["b", "a", "c"]
        );
    }

    #[test]
    fn the_first_name_the_theme_has_wins_in_the_order_asked() {
        let names: Vec<String> = ["a", "b", "c"].map(String::from).to_vec();
        assert_eq!(first_available(&names, |n| n == "c" || n == "b"), Some("b"));
        assert_eq!(first_available(&names, |_| false), None);
        assert_eq!(first_available(&[], |_| true), None);
    }

    #[test]
    fn a_picture_is_made_once_and_a_miss_is_remembered() {
        let cache = TypeIconCache::default();
        let key = request(IconKind::Extension("pdf".into()), 32, 1);
        let mut made = 0;
        for _ in 0..3 {
            cache.get_or_make(&key, || {
                made += 1;
                Some(vec![1, 2, 3])
            });
        }
        assert_eq!(made, 1);
        let missing = request(IconKind::Extension("nope".into()), 32, 1);
        for _ in 0..3 {
            assert!(cache
                .get_or_make(&missing, || {
                    made += 1;
                    None
                })
                .is_none());
        }
        assert_eq!(made, 2);
    }

    #[test]
    fn the_cache_is_keyed_by_size_scale_and_theme() {
        let cache = TypeIconCache::default();
        let kind = IconKind::Folder(FolderKind::Home);
        let mut made = 0;
        let mut ask = |size, scale, theme: Option<&str>| {
            let key = IconRequest::new(kind.clone(), size, scale, theme.map(String::from));
            cache.get_or_make(&key, || {
                made += 1;
                Some(vec![0; 4])
            });
        };
        ask(32, 1, None);
        ask(32, 1, None);
        ask(32, 2, None);
        ask(48, 1, None);
        ask(32, 1, Some("Adwaita"));
        ask(32, 1, Some("Adwaita"));
        ask(32, 1, Some("Breeze"));
        assert_eq!(made, 5);
        assert_eq!(cache.len(), 5);
    }

    #[test]
    fn the_cache_drops_the_oldest_when_it_is_full_of_entries() {
        let cache = TypeIconCache::with_limits(3, 1 << 20);
        for n in 0..10 {
            let key = request(IconKind::Extension(format!("e{n}")), 32, 1);
            cache.get_or_make(&key, || Some(vec![n as u8]));
            assert!(cache.len() <= 3);
        }
        assert_eq!(cache.len(), 3);
        let mut made = 0;
        let newest = request(IconKind::Extension("e9".into()), 32, 1);
        cache.get_or_make(&newest, || {
            made += 1;
            Some(vec![])
        });
        let oldest = request(IconKind::Extension("e0".into()), 32, 1);
        cache.get_or_make(&oldest, || {
            made += 1;
            Some(vec![])
        });
        assert_eq!(made, 1, "the newest stayed and the oldest went");
    }

    #[test]
    fn the_cache_is_bounded_in_bytes_and_keeps_nothing_larger_than_itself() {
        let cache = TypeIconCache::with_limits(100, 10);
        for n in 0..20 {
            let key = request(IconKind::Extension(format!("e{n}")), 32, 1);
            cache.get_or_make(&key, || Some(vec![0; 4]));
            assert!(cache.bytes() <= 10);
        }
        assert_eq!(cache.len(), 2);
        let big = request(IconKind::Extension("big".into()), 32, 1);
        let served = cache.get_or_make(&big, || Some(vec![0; 50])).unwrap();
        assert_eq!(served.len(), 50);
        assert!(cache.bytes() <= 10);
    }

    #[test]
    fn clearing_forgets_everything() {
        let cache = TypeIconCache::default();
        let key = request(IconKind::Extension("pdf".into()), 32, 1);
        cache.get_or_make(&key, || Some(vec![1]));
        cache.clear();
        assert!(cache.is_empty());
        assert_eq!(cache.bytes(), 0);
        let mut made = 0;
        cache.get_or_make(&key, || {
            made += 1;
            Some(vec![1])
        });
        assert_eq!(made, 1);
    }

    #[test]
    fn windows_stands_a_known_type_for_an_extension_and_falls_back_to_a_plain_file() {
        assert_eq!(extension_for_mime("application/pdf"), ".pdf");
        assert_eq!(extension_for_mime("image/webp"), ".png");
        assert_eq!(extension_for_mime("text/x-rust"), ".txt");
        assert_eq!(extension_for_mime("application/x-foo"), ".unknown-type");
    }
}
