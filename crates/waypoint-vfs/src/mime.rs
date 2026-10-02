// A small content-type guess from a file's name and its first bytes.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! This is the dependency-free fallback every provider shares so the preview protocol can set a
//! `Content-Type` and the Inspector can show a type. It knows the formats a preview handles; the
//! `mime-apps` plugin (A63) owns the system's full database for Open With.

/// How many leading bytes `guess` looks at. Callers read this many and no more.
pub const SNIFF_LEN: usize = 512;

const BY_EXTENSION: &[(&str, &str)] = &[
    ("txt", "text/plain"),
    ("log", "text/plain"),
    ("md", "text/markdown"),
    ("markdown", "text/markdown"),
    ("csv", "text/csv"),
    ("tsv", "text/tab-separated-values"),
    ("html", "text/html"),
    ("htm", "text/html"),
    ("css", "text/css"),
    ("js", "text/javascript"),
    ("mjs", "text/javascript"),
    ("json", "application/json"),
    ("xml", "application/xml"),
    ("yaml", "application/yaml"),
    ("yml", "application/yaml"),
    ("toml", "application/toml"),
    ("rs", "text/x-rust"),
    ("ts", "text/x-typescript"),
    ("tsx", "text/x-typescript"),
    ("py", "text/x-python"),
    ("sh", "application/x-shellscript"),
    ("c", "text/x-csrc"),
    ("h", "text/x-chdr"),
    ("cpp", "text/x-c++src"),
    ("java", "text/x-java"),
    ("go", "text/x-go"),
    ("png", "image/png"),
    ("jpg", "image/jpeg"),
    ("jpeg", "image/jpeg"),
    ("gif", "image/gif"),
    ("webp", "image/webp"),
    ("avif", "image/avif"),
    ("bmp", "image/bmp"),
    ("ico", "image/vnd.microsoft.icon"),
    ("svg", "image/svg+xml"),
    ("tif", "image/tiff"),
    ("tiff", "image/tiff"),
    ("heic", "image/heic"),
    ("pdf", "application/pdf"),
    ("mp3", "audio/mpeg"),
    ("wav", "audio/x-wav"),
    ("flac", "audio/flac"),
    ("ogg", "audio/ogg"),
    ("oga", "audio/ogg"),
    ("opus", "audio/ogg"),
    ("m4a", "audio/mp4"),
    ("aac", "audio/aac"),
    ("mp4", "video/mp4"),
    ("m4v", "video/mp4"),
    ("webm", "video/webm"),
    ("mkv", "video/x-matroska"),
    ("mov", "video/quicktime"),
    ("avi", "video/x-msvideo"),
    ("ogv", "video/ogg"),
    ("zip", "application/zip"),
    ("jar", "application/java-archive"),
    (
        "docx",
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    ),
    (
        "xlsx",
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    ),
    (
        "pptx",
        "application/vnd.openxmlformats-officedocument.presentationml.presentation",
    ),
    ("odt", "application/vnd.oasis.opendocument.text"),
    ("ods", "application/vnd.oasis.opendocument.spreadsheet"),
    ("odp", "application/vnd.oasis.opendocument.presentation"),
    ("epub", "application/epub+zip"),
    ("gz", "application/gzip"),
    ("tgz", "application/gzip"),
    ("bz2", "application/x-bzip2"),
    ("xz", "application/x-xz"),
    ("zst", "application/zstd"),
    ("7z", "application/x-7z-compressed"),
    ("rar", "application/vnd.rar"),
    ("tar", "application/x-tar"),
    ("woff", "font/woff"),
    ("woff2", "font/woff2"),
    ("ttf", "font/ttf"),
    ("otf", "font/otf"),
    ("wasm", "application/wasm"),
    ("sqlite", "application/vnd.sqlite3"),
    ("iso", "application/x-iso9660-image"),
];

/// The type a file name's extension implies, ignoring case. A name with no extension, or a
/// leading-dot name with nothing after it, implies nothing.
pub fn from_name(name: &str) -> Option<&'static str> {
    let (stem, extension) = name.rsplit_once('.')?;
    if stem.is_empty() {
        return None;
    }
    let extension = extension.to_ascii_lowercase();
    BY_EXTENSION
        .iter()
        .find(|(known, _)| *known == extension)
        .map(|(_, mime)| *mime)
}

/// The type the first bytes show, for formats with a signature. `application/zip` is reported for
/// every zip container, so a caller prefers the name's more specific type (`.docx`).
pub fn from_content(head: &[u8]) -> Option<&'static str> {
    let starts = |magic: &[u8]| head.starts_with(magic);
    let at = |offset: usize, magic: &[u8]| head.get(offset..offset + magic.len()) == Some(magic);
    Some(if starts(b"\x89PNG\r\n\x1a\n") {
        "image/png"
    } else if starts(b"\xff\xd8\xff") {
        "image/jpeg"
    } else if starts(b"GIF87a") || starts(b"GIF89a") {
        "image/gif"
    } else if starts(b"RIFF") && at(8, b"WEBP") {
        "image/webp"
    } else if starts(b"RIFF") && at(8, b"WAVE") {
        "audio/x-wav"
    } else if starts(b"RIFF") && at(8, b"AVI ") {
        "video/x-msvideo"
    } else if starts(b"BM") && head.len() > 14 && at(6, b"\0\0\0\0") {
        "image/bmp"
    } else if starts(b"%PDF-") {
        "application/pdf"
    } else if starts(b"PK\x03\x04") {
        "application/zip"
    } else if starts(b"\x1f\x8b") {
        "application/gzip"
    } else if starts(b"BZh") {
        "application/x-bzip2"
    } else if starts(b"\xfd7zXZ\0") {
        "application/x-xz"
    } else if starts(b"7z\xbc\xaf\x27\x1c") {
        "application/x-7z-compressed"
    } else if starts(b"Rar!\x1a\x07") {
        "application/vnd.rar"
    } else if starts(b"\x28\xb5\x2f\xfd") {
        "application/zstd"
    } else if starts(b"\x7fELF") {
        "application/x-executable"
    } else if starts(b"SQLite format 3\0") {
        "application/vnd.sqlite3"
    } else if starts(b"\0asm") {
        "application/wasm"
    } else if starts(b"ID3") || (head.len() > 1 && head[0] == 0xff && head[1] & 0xe0 == 0xe0) {
        "audio/mpeg"
    } else if starts(b"fLaC") {
        "audio/flac"
    } else if starts(b"OggS") {
        "audio/ogg"
    } else if at(4, b"ftyp") {
        "video/mp4"
    } else if starts(b"\x1a\x45\xdf\xa3") {
        "video/webm"
    } else if starts(b"wOFF") {
        "font/woff"
    } else if starts(b"wOF2") {
        "font/woff2"
    } else {
        return None;
    })
}

/// Whether the bytes look like text: no NUL byte, and valid UTF-8 apart from a character cut off
/// at the end of the sample.
pub fn looks_like_text(head: &[u8]) -> bool {
    if head.is_empty() || head.contains(&0) {
        return false;
    }
    match std::str::from_utf8(head) {
        Ok(_) => true,
        Err(error) => error.error_len().is_none(),
    }
}

/// The best guess from a name and the file's first bytes (`None` when the caller did not read any).
///
/// A signature wins over the name, so a PNG saved as `.txt` is served as an image; but a zip
/// signature defers to a more specific name (`.docx`, `.jar`, `.epub`). Without a signature the
/// name decides, and with neither a sample that looks like text is `text/plain`.
pub fn guess(name: &str, head: Option<&[u8]>) -> Option<String> {
    let by_name = from_name(name);
    let by_content = head.and_then(from_content);
    let best = match (by_content, by_name) {
        (Some("application/zip"), Some(named)) => Some(named),
        (Some(signature), _) => Some(signature),
        (None, named) => named,
    };
    if let Some(found) = best {
        return Some(found.to_owned());
    }
    match head {
        Some(bytes) if looks_like_text(bytes) => Some("text/plain".to_owned()),
        Some(bytes) if !bytes.is_empty() => Some("application/octet-stream".to_owned()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_implies_its_type_ignoring_case_and_dots() {
        assert_eq!(from_name("Photo.JPG"), Some("image/jpeg"));
        assert_eq!(from_name("archive.tar.gz"), Some("application/gzip"));
        assert_eq!(from_name(".png"), None);
        assert_eq!(from_name("README"), None);
        assert_eq!(from_name("x.unknownext"), None);
    }

    #[test]
    fn a_signature_beats_a_misleading_name() {
        let png = b"\x89PNG\r\n\x1a\nrest";
        assert_eq!(guess("notes.txt", Some(png)).as_deref(), Some("image/png"));
    }

    #[test]
    fn a_zip_signature_defers_to_a_specific_name() {
        let zip = b"PK\x03\x04rest";
        assert_eq!(
            guess("book.epub", Some(zip)).as_deref(),
            Some("application/epub+zip")
        );
        assert_eq!(
            guess("stuff.bin", Some(zip)).as_deref(),
            Some("application/zip")
        );
    }

    #[test]
    fn text_without_a_known_name_is_plain_text_and_binary_is_octet_stream() {
        assert_eq!(
            guess("LICENCE", Some("héllo\n".as_bytes())).as_deref(),
            Some("text/plain")
        );
        assert_eq!(
            guess("blob", Some(&[1, 2, 0, 3])).as_deref(),
            Some("application/octet-stream")
        );
        assert_eq!(guess("blob", None), None);
        assert_eq!(guess("blob", Some(&[])), None);
    }

    #[test]
    fn a_character_cut_off_at_the_end_still_looks_like_text() {
        let mut bytes = "ab".as_bytes().to_vec();
        bytes.extend_from_slice(&"é".as_bytes()[..1]);
        assert!(looks_like_text(&bytes));
        assert!(!looks_like_text(b"ab\xffcd"));
    }

    #[test]
    fn media_signatures_are_recognised() {
        assert_eq!(from_content(b"RIFF\0\0\0\0WEBPVP8 "), Some("image/webp"));
        assert_eq!(from_content(b"\0\0\0\x20ftypisom"), Some("video/mp4"));
        assert_eq!(from_content(b"OggS\0"), Some("audio/ogg"));
        assert_eq!(from_content(b"nothing"), None);
    }
}
