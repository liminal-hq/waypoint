// Serving one file's bytes to a webview, with HTTP `Range` support, for the `wpfile` protocol.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::io::Read;

use waypoint_path::VfsPath;
use waypoint_protocol::VfsError;

use crate::text::require_file;
use crate::{guess_mime, Provider, SNIFF_LEN};

/// The most one response carries when the request has a `Range`: a longer range is answered with
/// its first 8 MiB (a server may send less than was asked, and a media element asks again for the
/// rest). A response is built in memory, so this bounds what one request holds.
pub const MAX_CHUNK: u64 = 8 * 1024 * 1024;

/// The largest file served whole, for a request with no `Range` (an image, a PDF). A bigger file
/// is refused with 413 rather than read into memory; media elements always send `Range`.
pub const MAX_WHOLE: u64 = 64 * 1024 * 1024;

/// What a request's `Range` header asks for, against a file of a known size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ByteRange {
    /// No usable range: send the whole file with 200. A header that is malformed, names another
    /// unit, or asks for several ranges is ignored, as the standard allows.
    Whole,
    /// Bytes `start..=end` (both in bounds), answered with 206.
    Partial { start: u64, end: u64 },
    /// The range lies outside the file: answered with 416.
    Unsatisfiable,
}

/// Reads a `Range` header value (`bytes=0-99`, `bytes=100-`, `bytes=-500`) against `size`.
pub fn parse_range(header: Option<&str>, size: u64) -> ByteRange {
    let Some(header) = header else {
        return ByteRange::Whole;
    };
    let Some((unit, spec)) = header.trim().split_once('=') else {
        return ByteRange::Whole;
    };
    if !unit.trim().eq_ignore_ascii_case("bytes") || spec.contains(',') {
        return ByteRange::Whole;
    }
    let Some((first, last)) = spec.trim().split_once('-') else {
        return ByteRange::Whole;
    };
    let number = |text: &str| -> Option<u64> {
        (!text.is_empty() && text.bytes().all(|b| b.is_ascii_digit()))
            .then(|| text.parse::<u64>().ok())
            .flatten()
    };
    let (first, last) = (first.trim(), last.trim());
    match (first.is_empty(), last.is_empty()) {
        // `-n`: the last n bytes.
        (true, false) => match number(last) {
            None => ByteRange::Whole,
            Some(0) => ByteRange::Unsatisfiable,
            Some(_) if size == 0 => ByteRange::Unsatisfiable,
            Some(n) => ByteRange::Partial {
                start: size.saturating_sub(n),
                end: size - 1,
            },
        },
        // `a-` and `a-b`.
        (false, _) => {
            let Some(start) = number(first) else {
                return ByteRange::Whole;
            };
            let end = if last.is_empty() {
                None
            } else {
                match number(last) {
                    Some(end) if end >= start => Some(end),
                    // `a-b` with b before a is malformed, which means "ignore the header".
                    _ => return ByteRange::Whole,
                }
            };
            if start >= size {
                return ByteRange::Unsatisfiable;
            }
            ByteRange::Partial {
                start,
                end: end.map_or(size - 1, |end| end.min(size - 1)),
            }
        }
        (true, true) => ByteRange::Whole,
    }
}

/// A response ready for the webview.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServedFile {
    pub status: u16,
    pub headers: Vec<(&'static str, String)>,
    pub body: Vec<u8>,
}

impl ServedFile {
    fn new(status: u16, headers: Vec<(&'static str, String)>, body: Vec<u8>) -> Self {
        Self {
            status,
            headers,
            body,
        }
    }

    /// The value of a header, for tests and callers that need one.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

/// An empty response with `status`, carrying a short plain-text reason that never includes a path.
pub fn status_response(status: u16, reason: &str) -> ServedFile {
    ServedFile::new(
        status,
        vec![
            ("Content-Type", "text/plain; charset=utf-8".to_owned()),
            ("X-Content-Type-Options", "nosniff".to_owned()),
        ],
        reason.as_bytes().to_vec(),
    )
}

/// The response a failed read becomes.
pub fn error_response(error: &VfsError) -> ServedFile {
    match error {
        VfsError::NotFound { .. }
        | VfsError::StaleHandle
        | VfsError::NotADirectory { .. }
        | VfsError::IsADirectory { .. } => status_response(404, "not found"),
        VfsError::PermissionDenied { .. } => status_response(403, "not permitted"),
        VfsError::Unsupported { .. } | VfsError::NotText { .. } => {
            status_response(415, "cannot be served")
        }
        _ => status_response(500, "could not read the file"),
    }
}

/// Serves the file at `path` through `provider`, honouring a `Range` header (see `ByteRange`).
///
/// Only a regular file is read (a link to one is followed). The `Content-Type` comes from the name
/// and a sniff of the first bytes; `X-Content-Type-Options: nosniff` stops the webview second-guessing
/// it, and a document that could run script (HTML, SVG) is sandboxed by `Content-Security-Policy`.
pub fn serve_file<P: Provider + ?Sized>(
    provider: &P,
    path: &VfsPath,
    range: Option<&str>,
) -> ServedFile {
    match serve(provider, path, range) {
        Ok(served) => served,
        Err(error) => error_response(&error),
    }
}

fn serve<P: Provider + ?Sized>(
    provider: &P,
    path: &VfsPath,
    range: Option<&str>,
) -> Result<ServedFile, VfsError> {
    let entry = provider.stat(path)?;
    require_file(path, entry.kind, entry.link_target)?;
    let size = entry.size.ok_or_else(|| VfsError::Unsupported {
        what: "serving a file of unknown size".to_owned(),
    })?;
    let location = path.to_location();

    let (status, start, length) = match parse_range(range, size) {
        ByteRange::Unsatisfiable => {
            let mut response = status_response(416, "range not satisfiable");
            response
                .headers
                .push(("Content-Range", format!("bytes */{size}")));
            return Ok(response);
        }
        ByteRange::Partial { start, end } => (206, start, (end - start + 1).min(MAX_CHUNK)),
        ByteRange::Whole => {
            if size > MAX_WHOLE {
                return Ok(status_response(413, "too large to serve without a range"));
            }
            (200, 0, size)
        }
    };

    let name = entry.name.to_string_lossy();
    let mut head = Vec::with_capacity(SNIFF_LEN);
    provider
        .open_read(path)?
        .take(SNIFF_LEN as u64)
        .read_to_end(&mut head)
        .map_err(|error| crate::from_io(&error, &location))?;
    let mime = guess_mime(&name, Some(&head)).unwrap_or_else(|| "application/octet-stream".into());

    let mut body = Vec::with_capacity(length.min(MAX_CHUNK) as usize);
    if length > 0 {
        provider
            .open_read_at(path, start)?
            .take(length)
            .read_to_end(&mut body)
            .map_err(|error| crate::from_io(&error, &location))?;
    }

    let mut headers = vec![
        ("Content-Type", mime.clone()),
        ("Content-Length", body.len().to_string()),
        ("Accept-Ranges", "bytes".to_owned()),
        ("X-Content-Type-Options", "nosniff".to_owned()),
        ("Cache-Control", "no-cache".to_owned()),
    ];
    if status == 206 {
        let last = start + (body.len() as u64).max(1) - 1;
        headers.push(("Content-Range", format!("bytes {start}-{last}/{size}")));
    }
    if mime == "text/html" || mime == "image/svg+xml" {
        headers.push(("Content-Security-Policy", "sandbox".to_owned()));
    }
    Ok(ServedFile::new(status, headers, body))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn range(header: &str, size: u64) -> ByteRange {
        parse_range(Some(header), size)
    }

    #[test]
    fn no_header_or_an_unusable_one_means_the_whole_file() {
        assert_eq!(parse_range(None, 10), ByteRange::Whole);
        assert_eq!(range("items=0-1", 10), ByteRange::Whole);
        assert_eq!(range("bytes=0-1,4-5", 10), ByteRange::Whole);
        assert_eq!(range("bytes=abc", 10), ByteRange::Whole);
        assert_eq!(range("bytes=a-b", 10), ByteRange::Whole);
        assert_eq!(range("bytes=5-2", 10), ByteRange::Whole);
        assert_eq!(range("bytes=-", 10), ByteRange::Whole);
        assert_eq!(range("bytes=0x1-2", 10), ByteRange::Whole);
        assert_eq!(range("bytes=+1-2", 10), ByteRange::Whole);
    }

    #[test]
    fn closed_open_ended_and_suffix_ranges() {
        assert_eq!(
            range("bytes=2-5", 10),
            ByteRange::Partial { start: 2, end: 5 }
        );
        assert_eq!(
            range("bytes=4-", 10),
            ByteRange::Partial { start: 4, end: 9 }
        );
        assert_eq!(
            range("bytes=-3", 10),
            ByteRange::Partial { start: 7, end: 9 }
        );
        assert_eq!(
            range("BYTES= 0 - 0 ", 10),
            ByteRange::Partial { start: 0, end: 0 }
        );
    }

    #[test]
    fn an_end_past_the_file_is_clamped_and_a_long_suffix_is_the_whole_file() {
        assert_eq!(
            range("bytes=5-999", 10),
            ByteRange::Partial { start: 5, end: 9 }
        );
        assert_eq!(
            range("bytes=-999", 10),
            ByteRange::Partial { start: 0, end: 9 }
        );
    }

    #[test]
    fn a_range_outside_the_file_is_unsatisfiable() {
        assert_eq!(range("bytes=10-12", 10), ByteRange::Unsatisfiable);
        assert_eq!(range("bytes=10-", 10), ByteRange::Unsatisfiable);
        assert_eq!(range("bytes=-0", 10), ByteRange::Unsatisfiable);
        assert_eq!(range("bytes=-5", 0), ByteRange::Unsatisfiable);
        assert_eq!(range("bytes=0-", 0), ByteRange::Unsatisfiable);
    }

    #[test]
    fn a_number_too_big_for_a_u64_is_ignored() {
        assert_eq!(range("bytes=99999999999999999999-", 10), ByteRange::Whole);
    }
}
