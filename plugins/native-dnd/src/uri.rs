// Converts between file paths and `file://` URIs without losing a byte, and reads and writes the clipboard and drag formats that carry them
//
// A name on Linux is a sequence of bytes that need not be valid UTF-8, so a URI built from a lossy string cannot reopen the file. These functions work on bytes: the encoder percent-encodes every byte outside `A-Za-z0-9-._~/`, and the decoder returns the bytes it was given. The Windows helpers work on `&str` and are compiled everywhere so they are tested everywhere.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::{Path, PathBuf};

use crate::models::ClipboardFiles;

/// The verb the `x-special/gnome-copied-files` format starts with for a copy, and for a cut.
const GNOME_COPY: &str = "copy";
const GNOME_CUT: &str = "cut";

/// Percent-encodes every byte outside `A-Za-z0-9-._~/`.
pub fn percent_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut out = String::with_capacity(bytes.len());
    for &byte in bytes {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~' | b'/') {
            out.push(byte as char);
        } else {
            out.push('%');
            out.push(HEX[usize::from(byte >> 4)] as char);
            out.push(HEX[usize::from(byte & 0xf)] as char);
        }
    }
    out
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

/// Decodes `%XX` sequences. A `%` that is not followed by two hex digits stays as it is, and every other byte passes through, so raw bytes outside ASCII survive.
pub fn percent_decode(text: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len());
    let mut i = 0;
    while i < text.len() {
        if text[i] == b'%' && i + 2 < text.len() {
            if let (Some(high), Some(low)) = (hex_value(text[i + 1]), hex_value(text[i + 2])) {
                out.push(high << 4 | low);
                i += 3;
                continue;
            }
        }
        out.push(text[i]);
        i += 1;
    }
    out
}

/// A `file:` URI split into its host (lower-cased, empty when there is none) and its decoded path bytes.
struct FileUri {
    host: Vec<u8>,
    path: Vec<u8>,
}

fn split_file_uri(uri: &[u8]) -> Option<FileUri> {
    let uri = uri.trim_ascii();
    if uri.len() < 5 || !uri[..5].eq_ignore_ascii_case(b"file:") {
        return None;
    }
    let rest = &uri[5..];
    let (host, path) = match rest.strip_prefix(b"//") {
        Some(after) => match after.iter().position(|&b| b == b'/') {
            Some(slash) => (&after[..slash], &after[slash..]),
            None => (after, &after[after.len()..]),
        },
        None => (&rest[..0], rest),
    };
    if path.is_empty() {
        return None;
    }
    let host = host.to_ascii_lowercase();
    Some(FileUri {
        host: if host == b"localhost" {
            Vec::new()
        } else {
            host
        },
        path: percent_decode(path),
    })
}

/// The path bytes of a `file:` URI for this machine: `file:///p`, `file:/p` and `file://localhost/p` all give `/p`. A URI with another host, and anything that is not a `file:` URI, gives `None`.
pub fn local_path_bytes(uri: &[u8]) -> Option<Vec<u8>> {
    let parsed = split_file_uri(uri)?;
    parsed.host.is_empty().then_some(parsed.path)
}

/// The canonical spelling of a `file:` URI: `file://` (and the host, when it is not this machine's) followed by the path with every byte outside `A-Za-z0-9-._~/` percent-encoded. `None` for anything that is not a `file:` URI. Two spellings of one file give one result.
pub fn normalise(uri: &[u8]) -> Option<String> {
    let parsed = split_file_uri(uri)?;
    let host = String::from_utf8_lossy(&parsed.host);
    Some(format!("file://{host}{}", percent_encode(&parsed.path)))
}

/// The URI of an absolute Unix-style path, lossless for any bytes. `None` for a relative path.
pub fn unix_path_to_uri(bytes: &[u8]) -> Option<String> {
    (bytes.first() == Some(&b'/')).then(|| format!("file://{}", percent_encode(bytes)))
}

/// The URI of an absolute path, lossless where the platform's names are bytes. `None` for a relative path.
pub fn path_to_uri(path: &Path) -> Option<String> {
    #[cfg(windows)]
    {
        windows_path_to_uri(&path.to_string_lossy())
    }
    #[cfg(not(windows))]
    {
        unix_path_to_uri(path.as_os_str().as_encoded_bytes())
    }
}

/// The path a `file:` URI names, or `None` for a URI that is not a file on this machine.
pub fn uri_to_path(uri: &str) -> Option<PathBuf> {
    #[cfg(windows)]
    {
        uri_to_windows_path(uri).map(PathBuf::from)
    }
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        local_path_bytes(uri.as_bytes())
            .map(|bytes| PathBuf::from(std::ffi::OsString::from_vec(bytes)))
    }
    #[cfg(not(any(windows, unix)))]
    {
        local_path_bytes(uri.as_bytes())
            .map(|bytes| PathBuf::from(String::from_utf8_lossy(&bytes).into_owned()))
    }
}

/// A Windows path as a `file:` URI: `C:\a b\c` is `file:///C:/a%20b/c` and `\\server\share\x` is `file://server/share/x`. A `\\?\` prefix is dropped. `None` for a path that is neither a drive path nor a UNC path.
pub fn windows_path_to_uri(path: &str) -> Option<String> {
    let path = path.strip_prefix(r"\\?\UNC\").map_or_else(
        || path.strip_prefix(r"\\?\").unwrap_or(path).to_string(),
        |unc| format!(r"\\{unc}"),
    );
    let slashed = path.replace('\\', "/");
    if let Some(unc) = slashed.strip_prefix("//") {
        let (server, rest) = unc.split_once('/').unwrap_or((unc, ""));
        if server.is_empty() {
            return None;
        }
        return Some(format!(
            "file://{}/{}",
            percent_encode(server.as_bytes()),
            percent_encode(rest.as_bytes())
        ));
    }
    let bytes = slashed.as_bytes();
    if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        let drive = &slashed[..2];
        let rest = &slashed[2..];
        if rest.is_empty() || rest.starts_with('/') {
            return Some(format!(
                "file:///{drive}{}",
                percent_encode(rest.as_bytes())
            ));
        }
    }
    None
}

/// A `file:` URI as a Windows path: the inverse of `windows_path_to_uri`. A host other than `localhost` is a UNC server. Bytes that are not UTF-8 become replacement characters, because the path is then not representable as UTF-8; the encoder never produces them from a valid path.
pub fn uri_to_windows_path(uri: &str) -> Option<String> {
    let parsed = split_file_uri(uri.as_bytes())?;
    let path = String::from_utf8_lossy(&parsed.path).into_owned();
    if !parsed.host.is_empty() {
        let host = String::from_utf8_lossy(&parsed.host).into_owned();
        return Some(format!(r"\\{host}{}", path.replace('/', "\\")));
    }
    let bytes = path.as_bytes();
    let drive =
        bytes.len() >= 3 && bytes[0] == b'/' && bytes[1].is_ascii_alphabetic() && bytes[2] == b':';
    let rooted = if drive { &path[1..] } else { path.as_str() };
    Some(rooted.replace('/', "\\"))
}

/// The `file:` URIs of a `text/uri-list` body, normalised and in order. Lines end in CRLF or LF; blank lines, `#` comments and URIs that are not `file:` are skipped, and a trailing NUL some sources add is ignored.
pub fn parse_uri_list(body: &[u8]) -> Vec<String> {
    body.split(|&b| b == b'\n')
        .map(|line| line.trim_ascii().trim_ascii_end_nul())
        .filter(|line| !line.is_empty() && line[0] != b'#')
        .filter_map(normalise)
        .collect()
}

trait TrimNul {
    fn trim_ascii_end_nul(&self) -> &[u8];
}

impl TrimNul for [u8] {
    fn trim_ascii_end_nul(&self) -> &[u8] {
        let mut end = self.len();
        while end > 0 && self[end - 1] == 0 {
            end -= 1;
        }
        self[..end].trim_ascii()
    }
}

/// A `text/uri-list` body: each URI followed by CRLF.
pub fn format_uri_list(uris: &[String]) -> String {
    uris.iter().map(|uri| format!("{uri}\r\n")).collect()
}

/// An `x-special/gnome-copied-files` body: the verb `copy` or `cut`, then the URIs, separated by LF with none after the last.
pub fn format_gnome_copied_files(files: &ClipboardFiles) -> String {
    let verb = if files.cut { GNOME_CUT } else { GNOME_COPY };
    std::iter::once(verb)
        .chain(files.uris.iter().map(String::as_str))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Reads an `x-special/gnome-copied-files` body. `None` if it holds no `file:` URI. A verb other than `cut` counts as a copy.
pub fn parse_gnome_copied_files(body: &[u8]) -> Option<ClipboardFiles> {
    let mut lines = body
        .split(|&b| b == b'\n')
        .map(|line| line.trim_ascii().trim_ascii_end_nul());
    let verb = lines.next()?;
    let uris: Vec<String> = lines.filter_map(normalise).collect();
    (!uris.is_empty()).then(|| ClipboardFiles {
        uris,
        cut: verb.eq_ignore_ascii_case(GNOME_CUT.as_bytes()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(bytes: &[u8]) {
        let uri = unix_path_to_uri(bytes).expect("an absolute path");
        assert!(uri.starts_with("file:///"), "{uri}");
        assert!(uri.is_ascii(), "{uri}");
        assert_eq!(
            local_path_bytes(uri.as_bytes()).as_deref(),
            Some(bytes),
            "{uri}"
        );
        assert_eq!(
            normalise(uri.as_bytes()).as_deref(),
            Some(uri.as_str()),
            "{uri}"
        );
    }

    #[test]
    fn plain_names_are_left_alone() {
        assert_eq!(
            unix_path_to_uri(b"/home/a/b-c_d.e~f/g").unwrap(),
            "file:///home/a/b-c_d.e~f/g"
        );
    }

    #[test]
    fn spaces_hashes_and_percents_are_encoded_and_round_trip() {
        assert_eq!(
            unix_path_to_uri(b"/t/with space.txt").unwrap(),
            "file:///t/with%20space.txt"
        );
        assert_eq!(
            unix_path_to_uri(b"/t/hash#tag.txt").unwrap(),
            "file:///t/hash%23tag.txt"
        );
        assert_eq!(
            unix_path_to_uri(b"/t/per%cent.txt").unwrap(),
            "file:///t/per%25cent.txt"
        );
        assert_eq!(
            unix_path_to_uri(b"/t/q?x=1&y.txt").unwrap(),
            "file:///t/q%3Fx%3D1%26y.txt"
        );
        for name in [
            &b"/t/with space.txt"[..],
            b"/t/hash#tag.txt",
            b"/t/per%cent.txt",
            b"/t/100%25",
            b"/t/a+b (1).txt",
        ] {
            round_trip(name);
        }
    }

    #[test]
    fn names_that_are_not_utf8_survive() {
        let name = b"/t/bad\xff\xfe.txt";
        assert_eq!(unix_path_to_uri(name).unwrap(), "file:///t/bad%FF%FE.txt");
        round_trip(name);
        round_trip("/t/café ✓.txt".as_bytes());
        assert_eq!(
            unix_path_to_uri("/t/café ✓.txt".as_bytes()).unwrap(),
            "file:///t/caf%C3%A9%20%E2%9C%93.txt"
        );
    }

    #[test]
    fn every_byte_value_round_trips() {
        let mut name = b"/t/".to_vec();
        name.extend((1u8..=255).filter(|&b| b != b'/'));
        round_trip(&name);
    }

    #[test]
    fn a_relative_path_has_no_uri() {
        assert_eq!(unix_path_to_uri(b"rel/path"), None);
        assert_eq!(unix_path_to_uri(b""), None);
    }

    #[test]
    fn the_decoder_keeps_a_stray_percent_and_raw_bytes() {
        assert_eq!(percent_decode(b"100%"), b"100%");
        assert_eq!(percent_decode(b"a%zzb"), b"a%zzb");
        assert_eq!(percent_decode(b"%4"), b"%4");
        assert_eq!(percent_decode(b"%41%7e"), b"A~");
        assert_eq!(percent_decode(b"caf\xc3\xa9"), b"caf\xc3\xa9");
        assert_eq!(percent_decode(b"ends%2"), b"ends%2");
    }

    #[test]
    fn localhost_and_empty_hosts_are_this_machine() {
        for uri in [
            "file:///p/a",
            "file:/p/a",
            "file://localhost/p/a",
            "FILE://LocalHost/p/a",
        ] {
            assert_eq!(
                local_path_bytes(uri.as_bytes()).as_deref(),
                Some(&b"/p/a"[..]),
                "{uri}"
            );
            assert_eq!(
                normalise(uri.as_bytes()).as_deref(),
                Some("file:///p/a"),
                "{uri}"
            );
        }
    }

    #[test]
    fn another_host_is_not_a_local_path_but_keeps_its_name() {
        assert_eq!(local_path_bytes(b"file://server/share/a"), None);
        assert_eq!(
            normalise(b"file://Server/share/a").as_deref(),
            Some("file://server/share/a")
        );
    }

    #[test]
    fn other_schemes_and_empty_paths_are_not_file_uris() {
        for uri in [
            "https://example.com/a",
            "smb://host/share",
            "ftp://h/a",
            "file:",
            "file://",
            "",
            "/plain/path",
        ] {
            assert_eq!(normalise(uri.as_bytes()), None, "{uri:?}");
        }
    }

    #[test]
    fn a_uri_list_skips_comments_blanks_and_other_schemes_and_takes_crlf_or_lf() {
        let body = b"# a comment\r\nfile:///t/a.txt\r\n\r\nhttps://example.com/x\r\nfile://localhost/t/b%20c.txt\n  \nfile:///t/bad%FF.txt\r\n";
        assert_eq!(
            parse_uri_list(body),
            vec![
                "file:///t/a.txt".to_string(),
                "file:///t/b%20c.txt".to_string(),
                "file:///t/bad%FF.txt".to_string(),
            ]
        );
        assert_eq!(
            parse_uri_list(b"file:///t/a\0"),
            vec!["file:///t/a".to_string()]
        );
        assert!(parse_uri_list(b"").is_empty());
        assert!(parse_uri_list(b"\r\n\r\n").is_empty());
    }

    #[test]
    fn a_uri_list_respells_raw_bytes_as_percent_escapes() {
        assert_eq!(
            parse_uri_list(b"file:///t/caf\xc3\xa9 x\xff\r\n"),
            vec!["file:///t/caf%C3%A9%20x%FF".to_string()]
        );
    }

    #[test]
    fn a_uri_list_round_trips_through_the_formatter() {
        let uris = vec!["file:///t/a%20b".to_string(), "file:///t/c".to_string()];
        let body = format_uri_list(&uris);
        assert_eq!(body, "file:///t/a%20b\r\nfile:///t/c\r\n");
        assert_eq!(parse_uri_list(body.as_bytes()), uris);
    }

    #[test]
    fn the_gnome_format_has_a_verb_then_lf_separated_uris() {
        let copy = ClipboardFiles {
            uris: vec!["file:///a".into(), "file:///b%20c".into()],
            cut: false,
        };
        assert_eq!(
            format_gnome_copied_files(&copy),
            "copy\nfile:///a\nfile:///b%20c"
        );
        let cut = ClipboardFiles {
            cut: true,
            ..copy.clone()
        };
        assert_eq!(
            format_gnome_copied_files(&cut),
            "cut\nfile:///a\nfile:///b%20c"
        );
        assert_eq!(
            parse_gnome_copied_files(format_gnome_copied_files(&copy).as_bytes()),
            Some(copy)
        );
        assert_eq!(
            parse_gnome_copied_files(format_gnome_copied_files(&cut).as_bytes()),
            Some(cut)
        );
    }

    #[test]
    fn the_gnome_format_tolerates_crlf_a_trailing_newline_and_an_odd_verb() {
        let parsed = parse_gnome_copied_files(b"CUT\r\nfile:///a\r\nfile:///b\r\n\r\n").unwrap();
        assert!(parsed.cut);
        assert_eq!(
            parsed.uris,
            vec!["file:///a".to_string(), "file:///b".to_string()]
        );
        assert!(!parse_gnome_copied_files(b"link\nfile:///a\n").unwrap().cut);
        assert_eq!(parse_gnome_copied_files(b"copy\n"), None);
        assert_eq!(parse_gnome_copied_files(b"copy\nhttps://x/y\n"), None);
        assert_eq!(parse_gnome_copied_files(b""), None);
    }

    #[test]
    fn windows_paths_become_uris_and_back() {
        for (path, uri) in [
            (r"C:\Users\me\a b.txt", "file:///C:/Users/me/a%20b.txt"),
            (r"D:\", "file:///D:/"),
            (r"C:\x\café ✓.txt", "file:///C:/x/caf%C3%A9%20%E2%9C%93.txt"),
            (r"C:\x\hash#tag.txt", "file:///C:/x/hash%23tag.txt"),
            (r"\\server\share\dir\f.txt", "file://server/share/dir/f.txt"),
        ] {
            assert_eq!(windows_path_to_uri(path).as_deref(), Some(uri), "{path}");
            assert_eq!(uri_to_windows_path(uri).as_deref(), Some(path), "{uri}");
        }
        assert_eq!(
            windows_path_to_uri(r"\\?\C:\x\y").as_deref(),
            Some("file:///C:/x/y")
        );
        assert_eq!(
            windows_path_to_uri(r"\\?\UNC\srv\sh\y").as_deref(),
            Some("file://srv/sh/y")
        );
        assert_eq!(windows_path_to_uri(r"relative\path"), None);
        assert_eq!(windows_path_to_uri(r"\\"), None);
        assert_eq!(
            uri_to_windows_path("file:///C%3A/a/b").as_deref(),
            Some(r"C:\a\b")
        );
        assert_eq!(
            uri_to_windows_path("file://localhost/C:/a").as_deref(),
            Some(r"C:\a")
        );
        assert_eq!(uri_to_windows_path("https://x/y"), None);
    }

    #[cfg(unix)]
    #[test]
    fn paths_round_trip_through_uris_on_unix() {
        use std::os::unix::ffi::OsStrExt;
        let path = PathBuf::from(std::ffi::OsStr::from_bytes(b"/t/bad\xff name#1.txt"));
        let uri = path_to_uri(&path).unwrap();
        assert_eq!(uri, "file:///t/bad%FF%20name%231.txt");
        assert_eq!(uri_to_path(&uri), Some(path));
        assert_eq!(uri_to_path("https://x/y"), None);
        assert_eq!(path_to_uri(Path::new("relative")), None);
    }
}
