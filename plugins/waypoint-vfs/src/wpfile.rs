// The `wpfile` custom URI scheme: serves a file of the requesting window's own listings
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A URL names a file by a token, never by a path: `wpfile://localhost/{handle}-{entry}` (and
//! `http://wpfile.localhost/{handle}-{entry}` where the webview cannot register a custom scheme,
//! as on Windows), where both numbers are the `ListingHandle` and `EntryId` the page already
//! holds. The handler resolves the token through the registry for the webview that asked, so a
//! window can only read entries of listings it opened, and an entry the listing no longer holds,
//! another window's handle, a malformed token and a path-like string all end the same way: 404.
//! Nothing the page sends is ever joined onto a path.

use waypoint_protocol::EntryId;
use waypoint_vfs::{serve_file, status_response, ListingHandle, ServedFile};

use crate::registry::Registry;

/// The scheme's name.
pub const SCHEME: &str = "wpfile";

/// Reads `/{handle}-{entry}` (decimal digits only) into the token it names. Anything else — a
/// missing or extra part, a sign, a percent escape, a dot, a slash — is `None`.
pub(crate) fn parse_token(path: &str) -> Option<(ListingHandle, EntryId)> {
    let token = path.strip_prefix('/')?;
    let (handle, entry) = token.split_once('-')?;
    let digits = |text: &str| !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit());
    if !digits(handle) || !digits(entry) {
        return None;
    }
    Some((
        ListingHandle(handle.parse().ok()?),
        EntryId(entry.parse().ok()?),
    ))
}

/// Answers one request from webview `window`. `method` is the HTTP method, `path` the URL's path
/// (without its query) and `range` the `Range` header.
pub(crate) fn respond(
    registry: &Registry,
    window: &str,
    method: &str,
    path: &str,
    range: Option<&str>,
) -> ServedFile {
    let mut served = match method {
        "OPTIONS" => status_response(204, ""),
        "GET" | "HEAD" => answer(registry, window, path, range),
        _ => {
            let mut response = status_response(405, "method not allowed");
            response
                .headers
                .push(("Allow", "GET, HEAD, OPTIONS".to_owned()));
            response
        }
    };
    // The scheme is only reachable from this app's own webviews, and the token already limits what
    // a window can read, so a page may fetch what it was handed a URL for.
    served
        .headers
        .push(("Access-Control-Allow-Origin", "*".to_owned()));
    served.headers.push((
        "Access-Control-Expose-Headers",
        "Content-Range, Content-Length, Accept-Ranges".to_owned(),
    ));
    if method == "OPTIONS" {
        served
            .headers
            .push(("Access-Control-Allow-Headers", "Range".to_owned()));
        served.headers.push((
            "Access-Control-Allow-Methods",
            "GET, HEAD, OPTIONS".to_owned(),
        ));
    }
    if method == "HEAD" {
        served.body.clear();
    }
    served
}

/// Turns a served file into the webview's response type.
pub(crate) fn into_response(
    served: ServedFile,
) -> tauri::http::Response<std::borrow::Cow<'static, [u8]>> {
    let mut builder = tauri::http::Response::builder().status(served.status);
    for (name, value) in &served.headers {
        builder = builder.header(*name, value);
    }
    builder
        .body(std::borrow::Cow::Owned(served.body))
        .unwrap_or_else(|_| {
            let mut fallback = tauri::http::Response::new(std::borrow::Cow::Borrowed(&[][..]));
            *fallback.status_mut() = tauri::http::StatusCode::INTERNAL_SERVER_ERROR;
            fallback
        })
}

fn answer(registry: &Registry, window: &str, path: &str, range: Option<&str>) -> ServedFile {
    let not_found = || status_response(404, "not found");
    let Some((handle, id)) = parse_token(path) else {
        return not_found();
    };
    let Ok(listing) = registry.get(window, handle) else {
        return not_found();
    };
    let Ok(file) = listing.path_of(id) else {
        return not_found();
    };
    serve_file(listing.provider().as_ref(), &file, range)
}

#[cfg(all(test, unix))]
mod tests {
    use std::fs;
    use std::sync::Arc;

    use waypoint_path::{FilePath, VfsPath};
    use waypoint_vfs::{Filter, Listing, ListingOptions, LocalProvider, SortSpec};

    use super::*;

    /// A registry holding a listing of `dir` opened by window "main", scanned.
    fn registry_with(dir: &std::path::Path) -> (Registry, ListingHandle, Vec<waypoint_vfs::Entry>) {
        let registry = Registry::default();
        let handle = registry.allocate();
        let listing = Listing::new(
            handle,
            VfsPath::File(FilePath::from_path(dir).unwrap()),
            Arc::new(LocalProvider::new()),
            SortSpec::default(),
            Filter::default(),
            ListingOptions {
                watch: false,
                ..ListingOptions::default()
            },
            Arc::new(|_| {}),
        );
        listing.scan().unwrap();
        let entries = listing.get_range(0, 100);
        registry.insert("main", listing);
        (registry, handle, entries)
    }

    fn url(handle: ListingHandle, id: EntryId) -> String {
        format!("/{}-{}", handle.0, id.0)
    }

    #[test]
    fn a_token_is_two_plain_numbers() {
        assert_eq!(parse_token("/3-17"), Some((ListingHandle(3), EntryId(17))));
        for bad in [
            "",
            "/",
            "3-17",
            "/3",
            "/3-",
            "/-3",
            "/3-17/",
            "/3-17/x",
            "/3-17-1",
            "/ 3-17",
            "/+3-17",
            "/3_0-17",
            "/3%2D17",
            "/../etc/passwd",
            "/3-..",
            "/3-17.png",
            "//3-17",
            "/99999999999-1",
            "/3-99999999999",
            "/٣-1",
        ] {
            assert_eq!(parse_token(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn a_window_reads_an_entry_of_its_own_listing() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.txt"), "hello world").unwrap();
        let (registry, handle, entries) = registry_with(dir.path());
        let id = entries[0].id;
        let served = respond(&registry, "main", "GET", &url(handle, id), None);
        assert_eq!(served.status, 200);
        assert_eq!(served.body, b"hello world");
        let ranged = respond(&registry, "main", "GET", &url(handle, id), Some("bytes=6-"));
        assert_eq!(ranged.status, 206);
        assert_eq!(ranged.body, b"world");
        assert_eq!(ranged.header("Content-Range"), Some("bytes 6-10/11"));
        let beyond = respond(
            &registry,
            "main",
            "GET",
            &url(handle, id),
            Some("bytes=50-"),
        );
        assert_eq!(beyond.status, 416);
    }

    #[test]
    fn another_windows_handle_an_unknown_token_and_traversal_are_all_404() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.txt"), "secret").unwrap();
        let (registry, handle, entries) = registry_with(dir.path());
        let id = entries[0].id;
        let refused = |window: &str, path: &str| respond(&registry, window, "GET", path, None);
        assert_eq!(refused("other", &url(handle, id)).status, 404);
        assert_eq!(refused("main", &url(ListingHandle(999), id)).status, 404);
        assert_eq!(refused("main", &url(handle, EntryId(999))).status, 404);
        assert_eq!(refused("main", "/../../etc/passwd").status, 404);
        assert_eq!(
            refused("main", &format!("/{}-{}/../x", handle.0, id.0)).status,
            404
        );
        assert_eq!(
            refused("main", &dir.path().join("a.txt").to_string_lossy()).status,
            404
        );
        assert!(refused("other", &url(handle, id)).body != b"secret");
    }

    #[test]
    fn a_closed_listing_stops_serving() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.txt"), "x").unwrap();
        let (registry, handle, entries) = registry_with(dir.path());
        let path = url(handle, entries[0].id);
        assert_eq!(respond(&registry, "main", "GET", &path, None).status, 200);
        registry.close("main", handle);
        assert_eq!(respond(&registry, "main", "GET", &path, None).status, 404);
    }

    #[test]
    fn a_folder_entry_is_not_a_file() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("sub")).unwrap();
        let (registry, handle, entries) = registry_with(dir.path());
        let served = respond(&registry, "main", "GET", &url(handle, entries[0].id), None);
        assert_eq!(served.status, 404);
    }

    #[test]
    fn head_has_the_headers_and_no_body_and_other_methods_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.txt"), "hello").unwrap();
        let (registry, handle, entries) = registry_with(dir.path());
        let path = url(handle, entries[0].id);
        let head = respond(&registry, "main", "HEAD", &path, None);
        assert_eq!(head.status, 200);
        assert!(head.body.is_empty());
        assert_eq!(head.header("Content-Length"), Some("5"));
        assert_eq!(respond(&registry, "main", "POST", &path, None).status, 405);
        assert_eq!(
            respond(&registry, "main", "DELETE", &path, None).status,
            405
        );
        let preflight = respond(&registry, "main", "OPTIONS", &path, None);
        assert_eq!(preflight.status, 204);
        assert_eq!(
            preflight.header("Access-Control-Allow-Headers"),
            Some("Range")
        );
    }
}
