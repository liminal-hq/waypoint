// Serves the icon stored in a file (a program's, a shortcut's) on `fileicon://`, naming the file by a token the page holds and never by a path
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The System icon set draws a file by its type, which is the same icon for every program. A program, a shortcut, an icon and a cursor carry their own, so the page asks for those by file, and never sends a path to do it. A file in a listing is named the way `wpfile://` names it, by the `ListingHandle` and `EntryId` the page already holds: `fileicon://localhost/{handle}-{entry}?size=16&scale=2` (`http://fileicon.localhost/…` on Windows). That token is resolved through the listing of the webview that asked, so a window can only reach entries of its own listings.
//!
//! A place the page holds as a `Location` and not as an entry of a listing (a Shelf item, a file in the conflict dialog, a file being dragged) has no such token. The page hands those `Location`s to `register_icon_locations`, whose answer is an opaque number per place, and asks for `/l{token}`. Rust validates each place before it hands a number out (a local drive path, a kind of file that carries an icon) and keeps the number's path in the asking window's own table, so a token of another window is a 404 and the address never carries a path.
//!
//! Either way the local path found is handed to `tauri-plugin-mime-apps`, which draws only the kinds of file that carry an icon and only from a local drive. Nothing the page sends is ever joined onto a path, and an entry that is gone, a token that is malformed or unknown, a file of another kind and a place that is not a local file all end the same way: 404, so the page keeps the type's icon.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use tauri::http::{header, Method, Response, StatusCode};
use tauri::{Manager, Runtime, State, Window};
use tauri_plugin_mime_apps::typeicons::{has_own_icon, is_drive_path};
use tauri_plugin_mime_apps::MimeAppsExt;
use tauri_plugin_waypoint_vfs::Vfs;
use waypoint_protocol::{EntryId, Location, WindowKind};
use waypoint_vfs::ListingHandle;

use crate::thumbnails::path_of_location;

/// The scheme's name.
pub const SCHEME: &str = "fileicon";

/// The size and scale used when a request names none.
const DEFAULT_SIZE: u32 = 16;
const DEFAULT_SCALE: u32 = 1;

/// What an address names: an entry of one of the asking window's listings, or a place the window registered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconToken {
    Entry(ListingHandle, EntryId),
    Location(u64),
}

/// Reads `/{handle}-{entry}` or `/l{token}` (decimal digits only) into the token it names. Anything else — a missing or extra part, a sign, a percent escape, a dot, a slash — is `None`.
fn parse_token(path: &str) -> Option<IconToken> {
    let rest = path.strip_prefix('/')?;
    let digits = |text: &str| !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit());
    if let Some(token) = rest.strip_prefix('l') {
        return digits(token)
            .then(|| token.parse().ok())
            .flatten()
            .map(IconToken::Location);
    }
    let (handle, entry) = rest.split_once('-')?;
    if !digits(handle) || !digits(entry) {
        return None;
    }
    Some(IconToken::Entry(
        ListingHandle(handle.parse().ok()?),
        EntryId(entry.parse().ok()?),
    ))
}

fn number(query: Option<&str>, name: &str) -> Option<u32> {
    query
        .into_iter()
        .flat_map(|query| query.split('&'))
        .find_map(|pair| pair.strip_prefix(name)?.strip_prefix('='))
        .and_then(|value| value.parse().ok())
}

fn status(code: StatusCode) -> Response<Vec<u8>> {
    Response::builder()
        .status(code)
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .body(Vec::new())
        .expect("a status-only response is valid")
}

/// Answers one request: `GET` and `HEAD` for `/{handle}-{entry}` and `/l{token}`. `resolve` turns the token into the local path it names for the asking window, and `render` draws the file's icon (`size`, `scale`) or says it has none.
pub fn respond(
    resolve: impl Fn(IconToken) -> Option<PathBuf>,
    render: impl Fn(&Path, u32, u32) -> Option<Arc<Vec<u8>>>,
    method: &Method,
    path: &str,
    query: Option<&str>,
) -> Response<Vec<u8>> {
    if method != Method::GET && method != Method::HEAD {
        return status(StatusCode::METHOD_NOT_ALLOWED);
    }
    let Some(token) = parse_token(path) else {
        return status(StatusCode::NOT_FOUND);
    };
    let Some(file) = resolve(token) else {
        return status(StatusCode::NOT_FOUND);
    };
    let size = number(query, "size").unwrap_or(DEFAULT_SIZE);
    let scale = number(query, "scale").unwrap_or(DEFAULT_SCALE);
    let Some(bytes) = render(&file, size, scale) else {
        return status(StatusCode::NOT_FOUND);
    };
    let body = if method == Method::HEAD {
        Vec::new()
    } else {
        bytes.as_ref().clone()
    };
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "image/png")
        .header(header::CONTENT_LENGTH, bytes.len())
        // The page puts the file's modified time in the address, so a changed file is a new address.
        .header(header::CACHE_CONTROL, "private, max-age=3600")
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header("X-Content-Type-Options", "nosniff")
        .body(body)
        .unwrap_or_else(|_| status(StatusCode::INTERNAL_SERVER_ERROR))
}

/// How many places one window may hold tokens for. The oldest are dropped past it, and a page that still holds one gets a 404 and the type's icon.
pub const MAX_LOCATIONS_PER_WINDOW: usize = 4096;

/// One window's places, numbered from 1 in the order they were first registered.
#[derive(Default)]
struct WindowLocations {
    next: u64,
    by_path: HashMap<PathBuf, u64>,
    by_token: BTreeMap<u64, PathBuf>,
}

impl WindowLocations {
    /// The token of `path`, the same one every time while it is held.
    fn token_of(&mut self, path: PathBuf, limit: usize) -> u64 {
        if let Some(token) = self.by_path.get(&path) {
            return *token;
        }
        while self.by_token.len() >= limit.max(1) {
            let Some((_, oldest)) = self.by_token.pop_first() else {
                break;
            };
            self.by_path.remove(&oldest);
        }
        self.next += 1;
        self.by_path.insert(path.clone(), self.next);
        self.by_token.insert(self.next, path);
        self.next
    }
}

/// The places each window has registered for their file icons, by window label. A token only means something to the window it was given to.
#[derive(Default)]
pub struct IconLocations {
    windows: Mutex<HashMap<String, WindowLocations>>,
}

impl IconLocations {
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, WindowLocations>> {
        self.windows.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The token for `path` in `window`'s table.
    pub fn register(&self, window: &str, path: PathBuf) -> u64 {
        self.lock()
            .entry(window.to_owned())
            .or_default()
            .token_of(path, MAX_LOCATIONS_PER_WINDOW)
    }

    /// The path `token` names for `window`, if the window holds it.
    pub fn resolve(&self, window: &str, token: u64) -> Option<PathBuf> {
        self.lock().get(window)?.by_token.get(&token).cloned()
    }

    /// Drops everything `window` registered, when the window is gone.
    pub fn forget_window(&self, window: &str) {
        self.lock().remove(window);
    }
}

/// Whether a window of this label shows files it names by `Location`: the browsing windows, the Shelf and the Operations window.
fn may_register(label: &str) -> bool {
    matches!(
        WindowKind::from_label(label),
        Some(WindowKind::Main | WindowKind::Shelf | WindowKind::Ops)
    )
}

/// Whether the file at `path` is one whose own icon may be drawn: a kind that carries one, on a local drive. The same rules the drawing applies again.
fn eligible(path: &Path) -> bool {
    path.to_str()
        .is_some_and(|text| has_own_icon(text) && is_drive_path(text))
}

/// The token for each place that is a local file of a kind that carries its own icon, and `None` for every other, in the order given. One place that cannot have an icon is never an error.
fn tokens_for(
    locations: &[Location],
    to_path: impl Fn(&Location) -> Result<PathBuf, String>,
    mut register: impl FnMut(PathBuf) -> u64,
) -> Vec<Option<u64>> {
    locations
        .iter()
        .enumerate()
        .map(|(index, location)| {
            // A batch past the table's size would drop its own first tokens.
            if index >= MAX_LOCATIONS_PER_WINDOW {
                return None;
            }
            to_path(location)
                .ok()
                .filter(|path| eligible(path))
                .map(&mut register)
        })
        .collect()
}

/// Gives the page a token for each place it holds (a Shelf item, a file in a dialog) whose own icon can be drawn, to ask for as `fileicon://…/l{token}`. The answer is a number, never a path, and the page can only ever draw what Rust has accepted here.
#[tauri::command]
pub async fn register_icon_locations<R: Runtime>(
    window: Window<R>,
    icons: State<'_, IconLocations>,
    locations: Vec<Location>,
) -> Result<Vec<Option<u64>>, String> {
    let label = window.label();
    if !may_register(label) {
        return Err(format!("`{label}` does not draw files by place"));
    }
    Ok(tokens_for(&locations, path_of_location, |path| {
        icons.register(label, path)
    }))
}

/// Forgets a window's tokens when it is destroyed.
pub fn on_window_event<R: Runtime>(window: &Window<R>, event: &tauri::WindowEvent) {
    if matches!(event, tauri::WindowEvent::Destroyed) {
        if let Some(icons) = window.app_handle().try_state::<IconLocations>() {
            icons.forget_window(window.label());
        }
    }
}

/// Registers the scheme. The drawing waits on the shell, so it runs off the main thread.
pub fn register<R: Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    builder.register_asynchronous_uri_scheme_protocol(SCHEME, |ctx, request, responder| {
        let app = ctx.app_handle().clone();
        let window = ctx.webview_label().to_owned();
        tauri::async_runtime::spawn_blocking(move || {
            let response = respond(
                |token| match token {
                    IconToken::Entry(handle, entry) => {
                        let located = app
                            .try_state::<Vfs>()?
                            .locate_entries(&window, handle, &[entry])
                            .ok()?;
                        path_of_location(&located.into_iter().next()??).ok()
                    }
                    IconToken::Location(token) => {
                        app.try_state::<IconLocations>()?.resolve(&window, token)
                    }
                },
                |path, size, scale| app.mime_apps().file_icon(path, size, scale),
                request.method(),
                request.uri().path(),
                request.uri().query(),
            );
            responder.respond(response);
        });
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    fn png() -> Option<Arc<Vec<u8>>> {
        Some(Arc::new(b"png".to_vec()))
    }

    fn get(path: &str, query: Option<&str>) -> Response<Vec<u8>> {
        respond(
            |token| match token {
                IconToken::Entry(handle, entry) => {
                    (handle.0 == 3 && entry.0 == 9).then(|| PathBuf::from("C:\\a\\x.exe"))
                }
                IconToken::Location(7) => Some(PathBuf::from("C:\\a\\y.lnk")),
                IconToken::Location(_) => None,
            },
            |_, _, _| png(),
            &Method::GET,
            path,
            query,
        )
    }

    #[test]
    fn a_token_of_two_numbers_is_the_only_address() {
        assert_eq!(get("/3-9", None).status(), StatusCode::OK);
        for path in [
            "",
            "/",
            "/3",
            "/3-",
            "/-9",
            "/3-9-1",
            "/+3-9",
            "/3-9/",
            "/3-9.exe",
            "/%33-9",
            "/3%2D9",
            "/C%3A%5Ca%5Cx.exe",
            "/../3-9",
            "/3-99999999999",
            "/3-9\0",
        ] {
            assert_eq!(get(path, None).status(), StatusCode::NOT_FOUND, "{path:?}");
        }
    }

    #[test]
    fn a_place_token_is_l_and_digits_and_nothing_else() {
        assert_eq!(get("/l7", None).status(), StatusCode::OK);
        assert_eq!(parse_token("/l7"), Some(IconToken::Location(7)));
        assert_eq!(
            parse_token("/3-9"),
            Some(IconToken::Entry(ListingHandle(3), EntryId(9)))
        );
        for path in [
            "/l",
            "/l-7",
            "/l+7",
            "/L7",
            "/l7/",
            "/l7.exe",
            "/l7-1",
            "/l 7",
            "/l%37",
            "/%6C7",
            "/l../7",
            "/l99999999999999999999999",
            "/l7\0",
            "l7",
        ] {
            assert_eq!(get(path, None).status(), StatusCode::NOT_FOUND, "{path:?}");
        }
    }

    #[test]
    fn a_place_nobody_registered_is_a_404() {
        assert_eq!(get("/l8", None).status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn an_entry_the_window_cannot_resolve_is_a_404() {
        assert_eq!(get("/4-9", None).status(), StatusCode::NOT_FOUND);
        assert_eq!(get("/3-10", None).status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn a_file_without_an_icon_is_a_404_so_the_page_keeps_the_types() {
        let response = respond(
            |_| Some(PathBuf::from("C:\\a\\x.exe")),
            |_, _, _| None,
            &Method::GET,
            "/1-1",
            None,
        );
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn the_picture_is_a_png_and_the_size_and_scale_come_from_the_query() {
        let asked = RefCell::new(None);
        let response = respond(
            |_| Some(PathBuf::from("C:\\a\\x.exe")),
            |path, size, scale| {
                *asked.borrow_mut() = Some((path.to_path_buf(), size, scale));
                png()
            },
            &Method::GET,
            "/1-1",
            Some("size=32&scale=2&m=123"),
        );
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CONTENT_TYPE], "image/png");
        assert_eq!(response.body(), b"png");
        assert_eq!(
            asked.into_inner(),
            Some((PathBuf::from("C:\\a\\x.exe"), 32, 2))
        );
        let head = respond(
            |_| Some(PathBuf::from("C:\\a\\x.exe")),
            |_, _, _| png(),
            &Method::HEAD,
            "/1-1",
            None,
        );
        assert!(head.body().is_empty());
        assert_eq!(head.headers()[header::CONTENT_LENGTH], "3");
    }

    #[test]
    fn only_get_and_head_are_answered() {
        let response = respond(
            |_| Some(PathBuf::from("C:\\a\\x.exe")),
            |_, _, _| png(),
            &Method::POST,
            "/1-1",
            None,
        );
        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    }

    fn place(uri: &str) -> Location {
        Location::new(uri, uri)
    }

    /// Stands for the file system plugin: a place's `uri` is its path, and `sftp://` is not a local file.
    fn local(location: &Location) -> Result<PathBuf, String> {
        if location.uri.contains("://") {
            Err("not a local file".to_owned())
        } else {
            Ok(PathBuf::from(&location.uri))
        }
    }

    #[test]
    fn only_a_kind_that_carries_an_icon_on_a_local_drive_is_eligible() {
        for path in [
            "C:\\Apps\\Setup.exe",
            "c:/apps/setup.EXE",
            "D:\\a\\b.lnk",
            "D:\\a\\b.ico",
            "D:\\a\\b.cur",
            "D:\\a\\b.ani",
            "D:\\a\\b.scr",
        ] {
            assert!(eligible(Path::new(path)), "{path}");
        }
        for path in [
            "C:\\a\\notes.txt",
            "C:\\a\\exe",
            "C:\\a\\.exe",
            "C:\\a\\app.exe.txt",
            "\\\\server\\share\\a.exe",
            "\\\\?\\C:\\a.exe",
            "/home/me/a.exe",
            "a.exe",
            "",
        ] {
            assert!(!eligible(Path::new(path)), "{path}");
        }
    }

    #[test]
    fn tokens_go_to_eligible_local_places_and_none_to_the_rest_in_order() {
        let places = [
            place("C:\\Apps\\a.exe"),
            place("C:\\Apps\\notes.txt"),
            place("sftp://host/a.exe"),
            place("C:\\Apps\\b.lnk"),
            place("not a path"),
            place("C:\\Apps\\a.exe"),
        ];
        let mut registered = Vec::new();
        let tokens = tokens_for(&places, local, |path| {
            registered.push(path);
            registered.len() as u64
        });
        assert_eq!(tokens, [Some(1), None, None, Some(2), None, Some(3)]);
        assert_eq!(registered.len(), 3);
    }

    #[test]
    fn a_batch_past_the_bound_gets_none_for_the_rest() {
        let places = vec![place("C:\\a.exe"); MAX_LOCATIONS_PER_WINDOW + 3];
        let tokens = tokens_for(&places, local, |_| 1);
        assert_eq!(tokens.len(), places.len());
        assert!(tokens[..MAX_LOCATIONS_PER_WINDOW]
            .iter()
            .all(Option::is_some));
        assert!(tokens[MAX_LOCATIONS_PER_WINDOW..]
            .iter()
            .all(Option::is_none));
    }

    #[test]
    fn only_the_browsing_windows_the_shelf_and_the_operations_window_may_register() {
        for label in ["main-1", "main-12", "shelf", "ops"] {
            assert!(may_register(label), "{label}");
        }
        for label in [
            "settings",
            "properties-3",
            "tear-ghost",
            "main",
            "other",
            "",
        ] {
            assert!(!may_register(label), "{label}");
        }
    }

    #[test]
    fn the_same_file_always_gets_the_same_token_in_a_window() {
        let icons = IconLocations::default();
        let first = icons.register("main-1", PathBuf::from("C:\\a\\x.exe"));
        let other = icons.register("main-1", PathBuf::from("C:\\a\\y.exe"));
        assert_ne!(first, other);
        assert_eq!(
            icons.register("main-1", PathBuf::from("C:\\a\\x.exe")),
            first
        );
        assert_eq!(
            icons.resolve("main-1", first),
            Some(PathBuf::from("C:\\a\\x.exe"))
        );
    }

    #[test]
    fn a_token_of_another_window_does_not_resolve() {
        let icons = IconLocations::default();
        let mine = icons.register("main-1", PathBuf::from("C:\\a\\x.exe"));
        assert_eq!(icons.resolve("main-2", mine), None);
        assert_eq!(icons.resolve("shelf", mine), None);
        let theirs = icons.register("main-2", PathBuf::from("C:\\b\\z.exe"));
        assert_eq!(
            icons.resolve("main-2", theirs),
            Some(PathBuf::from("C:\\b\\z.exe"))
        );
        assert_ne!(
            icons.resolve("main-1", theirs),
            Some(PathBuf::from("C:\\b\\z.exe"))
        );
    }

    #[test]
    fn a_full_table_drops_its_oldest_places_and_never_reuses_a_number() {
        let mut table = WindowLocations::default();
        let tokens: Vec<u64> = (0..5)
            .map(|n| table.token_of(PathBuf::from(format!("C:\\a\\{n}.exe")), 3))
            .collect();
        assert_eq!(tokens, [1, 2, 3, 4, 5]);
        assert_eq!(table.by_token.len(), 3);
        assert_eq!(table.by_path.len(), 3);
        assert!(!table.by_token.contains_key(&1));
        assert!(!table.by_token.contains_key(&2));
        // A dropped place that comes back is a new number.
        assert_eq!(table.token_of(PathBuf::from("C:\\a\\0.exe"), 3), 6);
        assert_eq!(table.by_token.len(), 3);
    }

    #[test]
    fn a_destroyed_window_forgets_its_places_and_only_its_own() {
        let icons = IconLocations::default();
        let gone = icons.register("main-1", PathBuf::from("C:\\a\\x.exe"));
        let kept = icons.register("main-2", PathBuf::from("C:\\a\\x.exe"));
        icons.forget_window("main-1");
        assert_eq!(icons.resolve("main-1", gone), None);
        assert!(icons.resolve("main-2", kept).is_some());
    }
}
