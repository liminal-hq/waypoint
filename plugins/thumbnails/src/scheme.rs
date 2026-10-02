// The `thumb://` URI scheme: serves cached thumbnails by cache key and by nothing else
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use tauri::http::{header, Method, Response, StatusCode};

use crate::cache::CacheKey;
use crate::service::Thumbnails;

/// The name of the scheme.
pub const SCHEME: &str = "thumb";

/// Where the webview finds the scheme: Windows (and Android) webviews serve custom schemes from `http://{scheme}.localhost`.
#[cfg(windows)]
pub const URL_BASE: &str = "http://thumb.localhost";
#[cfg(not(windows))]
pub const URL_BASE: &str = "thumb://localhost";

/// The address of a cache entry. The `v` parameter is the file's modified time, so a thumbnail made from a newer version of the file is a different address and the webview's own cache never shows the old one.
pub fn url_for(key: &CacheKey, mtime_secs: i64) -> String {
    format!("{URL_BASE}/{}?v={mtime_secs}", key.relative())
}

/// Answers one request. Only `GET` and `HEAD` for `/{size}/{md5}.png` are served, and only when that entry is in the cache; every other path, however it is written, is a 404, so the scheme cannot be used to read anything outside the cache.
pub fn respond(thumbnails: &Thumbnails, method: &Method, path: &str) -> Response<Vec<u8>> {
    if method != Method::GET && method != Method::HEAD {
        return status(StatusCode::METHOD_NOT_ALLOWED);
    }
    let Some(bytes) = thumbnails.entry_bytes(path) else {
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
        .header(header::CACHE_CONTROL, "private, max-age=31536000")
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
