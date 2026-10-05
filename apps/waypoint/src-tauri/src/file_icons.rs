// Serves the icon stored in a file (a program's, a shortcut's) on `fileicon://`, naming the file by the listing token the page already holds
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The System icon set draws a file by its type, which is the same icon for every program. A program, a shortcut, an icon and a cursor carry their own, so the page asks for those by file. It names one the way `wpfile://` does, by the `ListingHandle` and `EntryId` it already holds: `fileicon://localhost/{handle}-{entry}?size=16&scale=2` (`http://fileicon.localhost/…` on Windows). This module resolves the token through the listing of the webview that asked, so a window can only reach entries of its own listings, and hands the local path it found to `tauri-plugin-mime-apps`, which draws only the kinds of file that carry an icon and only from a local drive. Nothing the page sends is ever joined onto a path, and an entry that is gone, a token that is malformed, a file of another kind and a place that is not a local file all end the same way: 404, so the page keeps the type's icon.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use tauri::http::{header, Method, Response, StatusCode};
use tauri::{Manager, Runtime};
use tauri_plugin_mime_apps::MimeAppsExt;
use tauri_plugin_waypoint_vfs::Vfs;
use waypoint_protocol::EntryId;
use waypoint_vfs::ListingHandle;

use crate::thumbnails::path_of_location;

/// The scheme's name.
pub const SCHEME: &str = "fileicon";

/// The size and scale used when a request names none.
const DEFAULT_SIZE: u32 = 16;
const DEFAULT_SCALE: u32 = 1;

/// Reads `/{handle}-{entry}` (decimal digits only) into the token it names. Anything else — a missing or extra part, a sign, a percent escape, a dot, a slash — is `None`.
fn parse_token(path: &str) -> Option<(ListingHandle, EntryId)> {
    let (handle, entry) = path.strip_prefix('/')?.split_once('-')?;
    let digits = |text: &str| !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit());
    if !digits(handle) || !digits(entry) {
        return None;
    }
    Some((
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

/// Answers one request: `GET` and `HEAD` for `/{handle}-{entry}`. `resolve` turns the token into the local path of the entry in the asking window's listing, and `render` draws the file's icon (`size`, `scale`) or says it has none.
pub fn respond(
    resolve: impl Fn(ListingHandle, EntryId) -> Option<PathBuf>,
    render: impl Fn(&Path, u32, u32) -> Option<Arc<Vec<u8>>>,
    method: &Method,
    path: &str,
    query: Option<&str>,
) -> Response<Vec<u8>> {
    if method != Method::GET && method != Method::HEAD {
        return status(StatusCode::METHOD_NOT_ALLOWED);
    }
    let Some((handle, entry)) = parse_token(path) else {
        return status(StatusCode::NOT_FOUND);
    };
    let Some(file) = resolve(handle, entry) else {
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

/// Registers the scheme. The drawing waits on the shell, so it runs off the main thread.
pub fn register<R: Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    builder.register_asynchronous_uri_scheme_protocol(SCHEME, |ctx, request, responder| {
        let app = ctx.app_handle().clone();
        let window = ctx.webview_label().to_owned();
        tauri::async_runtime::spawn_blocking(move || {
            let response = respond(
                |handle, entry| {
                    let located = app
                        .try_state::<Vfs>()?
                        .locate_entries(&window, handle, &[entry])
                        .ok()?;
                    path_of_location(&located.into_iter().next()??).ok()
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
            |handle, entry| (handle.0 == 3 && entry.0 == 9).then(|| PathBuf::from("C:\\a\\x.exe")),
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
    fn an_entry_the_window_cannot_resolve_is_a_404() {
        assert_eq!(get("/4-9", None).status(), StatusCode::NOT_FOUND);
        assert_eq!(get("/3-10", None).status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn a_file_without_an_icon_is_a_404_so_the_page_keeps_the_types() {
        let response = respond(
            |_, _| Some(PathBuf::from("C:\\a\\x.exe")),
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
            |_, _| Some(PathBuf::from("C:\\a\\x.exe")),
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
            |_, _| Some(PathBuf::from("C:\\a\\x.exe")),
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
            |_, _| Some(PathBuf::from("C:\\a\\x.exe")),
            |_, _, _| png(),
            &Method::POST,
            "/1-1",
            None,
        );
        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    }
}
