// Reads and writes the text formats of the freedesktop.org Trash: percent-encoded paths, `.trashinfo` files and `directorysizes`
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::ffi::{OsStr, OsString};
use std::os::unix::ffi::{OsStrExt, OsStringExt};

use chrono::NaiveDateTime;

/// The format of `DeletionDate`: local time, no zone.
pub const DATE_FORMAT: &str = "%Y-%m-%dT%H:%M:%S";

/// The suffix of every file in `info/`.
pub const INFO_SUFFIX: &str = ".trashinfo";

fn is_unreserved(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~' | b'/')
}

/// Percent-encodes raw bytes: everything but letters, digits, `-._~` and `/` becomes `%XX`, so spaces, `%`, `#`, newlines and bytes that are not UTF-8 all survive.
pub fn encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len());
    for &byte in bytes {
        if is_unreserved(byte) {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// Percent-decodes to raw bytes. A `%` that does not start two hex digits is kept as it is, because other implementations write sloppy files.
pub fn decode(text: &str) -> Vec<u8> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let (Some(high), Some(low)) = (hex(bytes[index + 1]), hex(bytes[index + 2])) {
                out.push(high << 4 | low);
                index += 3;
                continue;
            }
        }
        out.push(bytes[index]);
        index += 1;
    }
    out
}

fn hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

/// Percent-encodes a path or a name.
pub fn encode_os(value: &OsStr) -> String {
    encode(value.as_bytes())
}

/// Decodes a percent-encoded path or name.
pub fn decode_os(text: &str) -> OsString {
    OsString::from_vec(decode(text))
}

/// What a `.trashinfo` file says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrashInfo {
    /// The decoded `Path` value: absolute, or relative to the top directory of the volume.
    pub path: OsString,
    /// `DeletionDate`, if it was present and well formed.
    pub deleted_at: Option<NaiveDateTime>,
}

/// The text of a `.trashinfo` file. `encoded_path` is already percent-encoded.
pub fn format_info(encoded_path: &str, deleted_at: &NaiveDateTime) -> String {
    format!(
        "[Trash Info]\nPath={encoded_path}\nDeletionDate={}\n",
        deleted_at.format(DATE_FORMAT)
    )
}

/// Parses a `.trashinfo` file. `None` when there is no `[Trash Info]` group with a `Path`. Only the first `[Trash Info]` group is read, and only the first `Path` and `DeletionDate` in it.
pub fn parse_info(text: &str) -> Option<TrashInfo> {
    let mut in_group = false;
    let mut seen_group = false;
    let mut path = None;
    let mut date = None;
    for line in text.lines() {
        let line = line.trim_end_matches('\r');
        if line.starts_with('[') {
            if seen_group {
                break;
            }
            in_group = line.trim() == "[Trash Info]";
            seen_group |= in_group;
            continue;
        }
        if !in_group {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            match key.trim() {
                "Path" if path.is_none() => path = Some(decode_os(value.trim_start())),
                "DeletionDate" if date.is_none() => date = Some(value.trim()),
                _ => {}
            }
        }
    }
    let path = path.filter(|path| !path.is_empty())?;
    let deleted_at = date.and_then(parse_date);
    Some(TrashInfo { path, deleted_at })
}

/// Parses `YYYY-MM-DDThh:mm:ss`, tolerating a fraction of a second or a trailing `Z`, which some writers add.
pub fn parse_date(text: &str) -> Option<NaiveDateTime> {
    let text = text.trim().trim_end_matches('Z');
    let text = text.split('.').next()?;
    NaiveDateTime::parse_from_str(text, DATE_FORMAT).ok()
}

/// One line of `directorysizes`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SizeEntry {
    pub size: u64,
    /// The modification time of the matching `.trashinfo` file, in seconds since the Unix epoch.
    pub mtime: i64,
    pub name: OsString,
}

/// Parses the lines of a `directorysizes` file, dropping any that are malformed.
pub fn parse_sizes(text: &str) -> Vec<SizeEntry> {
    text.lines()
        .filter_map(|line| {
            let mut parts = line.splitn(3, ' ');
            let size = parts.next()?.parse().ok()?;
            let mtime = parts.next()?.parse().ok()?;
            let name = decode_os(parts.next()?);
            (!name.is_empty()).then_some(SizeEntry { size, mtime, name })
        })
        .collect()
}

/// Formats one `directorysizes` line, without the newline.
pub fn format_size_entry(entry: &SizeEntry) -> String {
    format!("{} {} {}", entry.size, entry.mtime, encode_os(&entry.name))
}
