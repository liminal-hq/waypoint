// The `appicon://` URI scheme: serves an application's icon by its id and by nothing else
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tauri::http::{header, Method, Response, StatusCode};

use crate::backend::Backend;
use crate::target::decode;

/// The name of the scheme.
pub const SCHEME: &str = "appicon";

/// Where the webview finds the scheme: Windows (and Android) webviews serve custom schemes from `http://{scheme}.localhost`.
#[cfg(windows)]
pub const URL_BASE: &str = "http://appicon.localhost";
#[cfg(not(windows))]
pub const URL_BASE: &str = "appicon://localhost";

/// The icon sizes the scheme will make, in pixels. A request outside the range is clamped into it.
pub const MIN_SIZE: u32 = 16;
pub const MAX_SIZE: u32 = 256;
pub const DEFAULT_SIZE: u32 = 32;

/// How many pictures are kept before the cache starts over. An icon is a few kilobytes and a system has a few hundred applications.
const CAPACITY: usize = 512;

/// The address of an application's icon at a size.
pub fn url_for(app_id: &str, size: u32) -> String {
    let mut id = String::new();
    for byte in app_id.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            id.push(byte as char);
        } else {
            id.push_str(&format!("%{byte:02X}"));
        }
    }
    format!("{URL_BASE}/{id}?size={size}")
}

/// An application id and a size in pixels.
type IconKey = (String, u32);

/// Pictures already made, by application and size.
#[derive(Default)]
pub struct IconCache {
    entries: Mutex<HashMap<IconKey, Arc<Vec<u8>>>>,
}

impl IconCache {
    fn get_or_make(
        &self,
        id: &str,
        size: u32,
        make: impl FnOnce() -> Option<Vec<u8>>,
    ) -> Option<Arc<Vec<u8>>> {
        let key = (id.to_string(), size);
        if let Some(found) = self.entries.lock().ok()?.get(&key) {
            return Some(Arc::clone(found));
        }
        let made = Arc::new(make()?);
        let mut entries = self.entries.lock().ok()?;
        if entries.len() >= CAPACITY {
            entries.clear();
        }
        entries.insert(key, Arc::clone(&made));
        Some(made)
    }
}

/// The application id a request path names: one segment, decoded, with nothing that could reach a file. `None` for anything else.
fn app_id_of(path: &str) -> Option<String> {
    let id = decode(path.strip_prefix('/')?)?;
    let safe = !id.is_empty()
        && id.len() <= 255
        && id != "."
        && id != ".."
        && !id.contains(['/', '\\', '\0']);
    safe.then_some(id)
}

fn size_of(query: Option<&str>) -> u32 {
    query
        .into_iter()
        .flat_map(|query| query.split('&'))
        .find_map(|pair| pair.strip_prefix("size="))
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(DEFAULT_SIZE)
        .clamp(MIN_SIZE, MAX_SIZE)
}

/// Answers one request. Only `GET` and `HEAD` for `/{app id}` are served, and only when the backend knows the application; every other path, however it is written, is a 404, so the scheme cannot be used to read a file.
pub fn respond(
    backend: &dyn Backend,
    cache: &IconCache,
    method: &Method,
    path: &str,
    query: Option<&str>,
) -> Response<Vec<u8>> {
    if method != Method::GET && method != Method::HEAD {
        return status(StatusCode::METHOD_NOT_ALLOWED);
    }
    let Some(id) = app_id_of(path) else {
        return status(StatusCode::NOT_FOUND);
    };
    if !backend.knows_app(&id) {
        return status(StatusCode::NOT_FOUND);
    }
    let size = size_of(query);
    let Some(bytes) = cache.get_or_make(&id, size, || backend.app_icon(&id, size)) else {
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
        .header(header::CACHE_CONTROL, "private, max-age=3600")
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header("X-Content-Type-Options", "nosniff")
        .body(body)
        .unwrap_or_else(|_| status(StatusCode::INTERNAL_SERVER_ERROR))
}

fn status(code: StatusCode) -> Response<Vec<u8>> {
    Response::builder()
        .status(code)
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .body(Vec::new())
        .expect("a status-only response is valid")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::directory::fake::{entry, Fake};
    use crate::directory::DirectoryBackend;
    use crate::models::{Flavour, PluginStatus, Reason};

    fn backend() -> DirectoryBackend<Fake> {
        let fake = Fake::default();
        fake.0.lock().unwrap().apps = vec![entry("eog.desktop", "Image Viewer")];
        DirectoryBackend::new(fake, None, || {
            PluginStatus::all_unavailable(Flavour::Gio, Reason::NoSystemChooser, "test")
        })
    }

    fn get(
        backend: &dyn Backend,
        cache: &IconCache,
        path: &str,
        query: Option<&str>,
    ) -> StatusCode {
        respond(backend, cache, &Method::GET, path, query).status()
    }

    #[test]
    fn a_known_application_is_served_as_a_png_at_the_requested_size() {
        let backend = backend();
        let cache = IconCache::default();
        let response = respond(
            &backend,
            &cache,
            &Method::GET,
            "/eog.desktop",
            Some("size=48"),
        );
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CONTENT_TYPE], "image/png");
        assert_eq!(response.body(), b"png:eog.desktop:48");
        let head = respond(&backend, &cache, &Method::HEAD, "/eog.desktop", None);
        assert_eq!(head.status(), StatusCode::OK);
        assert!(head.body().is_empty());
        assert_eq!(head.headers()[header::CONTENT_LENGTH], "18");
    }

    #[test]
    fn nothing_but_a_known_application_id_is_served() {
        let backend = backend();
        let cache = IconCache::default();
        for path in [
            "/",
            "",
            "/unknown.desktop",
            "/..",
            "/../etc/passwd",
            "/%2e%2e%2fetc%2fpasswd",
            "/..%2F..%2Fetc%2Fpasswd",
            "/eog.desktop/extra",
            "/eog.desktop%2F..%2Fx",
            "/etc/passwd",
            "/%2Fetc%2Fpasswd",
            "/C%3A%5CWindows%5Cnotepad.exe",
            "/eog.desktop%00",
            "/%zz",
        ] {
            assert_eq!(
                get(&backend, &cache, path, None),
                StatusCode::NOT_FOUND,
                "{path}"
            );
        }
    }

    #[test]
    fn only_get_and_head_are_answered() {
        let backend = backend();
        let cache = IconCache::default();
        let response = respond(&backend, &cache, &Method::POST, "/eog.desktop", None);
        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    }

    #[test]
    fn the_size_is_clamped() {
        assert_eq!(size_of(None), DEFAULT_SIZE);
        assert_eq!(size_of(Some("size=1")), MIN_SIZE);
        assert_eq!(size_of(Some("size=99999")), MAX_SIZE);
        assert_eq!(size_of(Some("x=1&size=64")), 64);
        assert_eq!(size_of(Some("size=big")), DEFAULT_SIZE);
    }

    #[test]
    fn a_picture_is_made_once_per_size() {
        let cache = IconCache::default();
        let mut made = 0;
        for _ in 0..3 {
            cache.get_or_make("a", 32, || {
                made += 1;
                Some(vec![1])
            });
        }
        assert_eq!(made, 1);
        cache.get_or_make("a", 64, || {
            made += 1;
            Some(vec![2])
        });
        assert_eq!(made, 2);
    }

    #[test]
    fn urls_escape_the_id_and_round_trip() {
        let url = url_for("a b/c.desktop", 24);
        assert!(url.ends_with("/a%20b%2Fc.desktop?size=24"), "{url}");
        let path = url
            .strip_prefix(URL_BASE)
            .unwrap()
            .split('?')
            .next()
            .unwrap();
        // A decoded id with a slash is refused, so a crafted id can never name a path.
        assert_eq!(app_id_of(path), None);
        assert_eq!(
            app_id_of("/org.gnome.eog.desktop").as_deref(),
            Some("org.gnome.eog.desktop")
        );
    }
}
