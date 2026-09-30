// The Windows path rules, as pure functions over strings so they are unit-tested on Linux too.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

// Covers drive letters (`C:\a`), UNC (`\\server\share\a`), verbatim long paths (`\\?\C:\a`,
// `\\?\UNC\server\share\a`) and device paths (`\\.\COM1`). Ordinary paths accept both separators and
// are normalised lexically; verbatim paths are taken literally, as Windows does (only `\` separates,
// and `.` and `..` are names). Names compare case-insensitively.

use crate::{encoding, PathError};

/// What comes before the first separator of a path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Prefix {
    /// `C:`, with the letter in upper case.
    Drive(char),
    /// `\\server\share`
    Unc { server: String, share: String },
    /// `\\?\C:`
    VerbatimDrive(char),
    /// `\\?\UNC\server\share`
    VerbatimUnc { server: String, share: String },
    /// `\\?\Volume{guid}` and other verbatim roots.
    Verbatim(String),
    /// `\\.\COM1` and other device namespace roots.
    Device(String),
}

/// A parsed Windows path: an optional prefix, whether it is rooted, and its components.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WinPath {
    pub prefix: Option<Prefix>,
    pub rooted: bool,
    pub components: Vec<String>,
}

fn is_sep(c: char) -> bool {
    c == '\\' || c == '/'
}

fn drive_letter(text: &str) -> Option<char> {
    let mut chars = text.chars();
    let letter = chars.next()?;
    (letter.is_ascii_alphabetic() && chars.next() == Some(':')).then(|| letter.to_ascii_uppercase())
}

/// Splits off the first `\`-separated piece of `text`.
fn take_verbatim(text: &str) -> (&str, &str) {
    match text.find('\\') {
        Some(at) => (&text[..at], &text[at + 1..]),
        None => (text, ""),
    }
}

fn take_any(text: &str) -> (&str, &str) {
    match text.find(is_sep) {
        Some(at) => (&text[..at], &text[at + 1..]),
        None => (text, ""),
    }
}

fn unc_parts(
    text: &str,
    split: fn(&str) -> (&str, &str),
) -> Result<(String, String, &str), PathError> {
    let (server, rest) = split(text);
    let (share, rest) = split(rest);
    if server.is_empty() || share.is_empty() {
        return Err(PathError::Invalid("a UNC path needs a server and a share"));
    }
    Ok((server.to_owned(), share.to_owned(), rest))
}

/// Splits `rest` into components on top of `stack`. Outside verbatim paths `.` is dropped and `..`
/// pops a component; at the start of a rooted path it is ignored, at the start of a relative one kept.
fn components(rest: &str, verbatim: bool, rooted: bool, mut stack: Vec<String>) -> Vec<String> {
    let parts: Vec<&str> = if verbatim {
        rest.split('\\').collect()
    } else {
        rest.split(is_sep).collect()
    };
    for part in parts {
        match part {
            "" => {}
            "." if !verbatim => {}
            ".." if !verbatim => {
                if matches!(stack.last(), Some(last) if last != "..") {
                    stack.pop();
                } else if !rooted {
                    stack.push("..".to_owned());
                }
            }
            name => stack.push(name.to_owned()),
        }
    }
    stack
}

/// Parses `text`. Relative and drive-relative paths are accepted (see [`WinPath::is_absolute`]).
pub fn parse(text: &str) -> Result<WinPath, PathError> {
    if text.is_empty() {
        return Err(PathError::Empty);
    }
    if text.contains('\0') {
        return Err(PathError::InteriorNul);
    }
    if let Some(rest) = text.strip_prefix(r"\\?\") {
        let (first, tail) = take_verbatim(rest);
        if first.eq_ignore_ascii_case("UNC") {
            let (server, share, tail) = unc_parts(tail, take_verbatim)?;
            return Ok(WinPath {
                prefix: Some(Prefix::VerbatimUnc { server, share }),
                rooted: true,
                components: components(tail, true, true, Vec::new()),
            });
        }
        let prefix = match drive_letter(first) {
            Some(letter) => Prefix::VerbatimDrive(letter),
            None if first.is_empty() => {
                return Err(PathError::Invalid("a verbatim path needs a root"))
            }
            None => Prefix::Verbatim(first.to_owned()),
        };
        return Ok(WinPath {
            prefix: Some(prefix),
            rooted: true,
            components: components(tail, true, true, Vec::new()),
        });
    }
    let mut chars = text.chars();
    let (a, b) = (chars.next(), chars.next());
    if matches!((a, b), (Some(a), Some(b)) if is_sep(a) && is_sep(b)) {
        let rest = &text[2..];
        if let Some(device) = rest.strip_prefix(".\\").or_else(|| rest.strip_prefix("./")) {
            let (first, tail) = take_any(device);
            if first.is_empty() {
                return Err(PathError::Invalid("a device path needs a name"));
            }
            return Ok(WinPath {
                prefix: Some(Prefix::Device(first.to_owned())),
                rooted: true,
                components: components(tail, false, true, Vec::new()),
            });
        }
        let (server, share, tail) = unc_parts(rest, take_any)?;
        return Ok(WinPath {
            prefix: Some(Prefix::Unc { server, share }),
            rooted: true,
            components: components(tail, false, true, Vec::new()),
        });
    }
    if let Some(letter) = drive_letter(text) {
        let rest = &text[2..];
        return Ok(WinPath {
            prefix: Some(Prefix::Drive(letter)),
            rooted: rest.starts_with(is_sep),
            components: components(rest, false, rest.starts_with(is_sep), Vec::new()),
        });
    }
    let rooted = text.starts_with(is_sep);
    Ok(WinPath {
        prefix: None,
        rooted,
        components: components(text, false, rooted, Vec::new()),
    })
}

impl Prefix {
    fn is_verbatim(&self) -> bool {
        matches!(
            self,
            Prefix::VerbatimDrive(_) | Prefix::VerbatimUnc { .. } | Prefix::Verbatim(_)
        )
    }
}

impl WinPath {
    /// A path is absolute when a prefix names where it starts and it is rooted there.
    pub fn is_absolute(&self) -> bool {
        self.prefix.is_some() && self.rooted
    }

    pub fn is_verbatim(&self) -> bool {
        self.prefix.as_ref().is_some_and(Prefix::is_verbatim)
    }

    fn render(&self, simplify: bool) -> String {
        let mut out = String::new();
        match &self.prefix {
            None => {}
            Some(Prefix::Drive(letter)) => {
                out.push(*letter);
                out.push(':');
            }
            Some(Prefix::VerbatimDrive(letter)) if simplify => {
                out.push(*letter);
                out.push(':');
            }
            Some(Prefix::VerbatimDrive(letter)) => {
                out.push_str(r"\\?\");
                out.push(*letter);
                out.push(':');
            }
            Some(Prefix::Unc { server, share }) => {
                out.push_str(&format!(r"\\{server}\{share}"));
            }
            Some(Prefix::VerbatimUnc { server, share }) if simplify => {
                out.push_str(&format!(r"\\{server}\{share}"));
            }
            Some(Prefix::VerbatimUnc { server, share }) => {
                out.push_str(&format!(r"\\?\UNC\{server}\{share}"));
            }
            Some(Prefix::Verbatim(name)) => out.push_str(&format!(r"\\?\{name}")),
            Some(Prefix::Device(name)) => out.push_str(&format!(r"\\.\{name}")),
        }
        if self.rooted {
            if self.components.is_empty() {
                out.push('\\');
            }
            for part in &self.components {
                out.push('\\');
                out.push_str(part);
            }
        } else {
            out.push_str(&self.components.join("\\"));
        }
        out
    }

    /// The canonical text, backslash-separated and with the verbatim prefix kept.
    pub fn to_native_string(&self) -> String {
        self.render(false)
    }

    /// The text for people: a verbatim drive or UNC path shows as the ordinary path it names.
    pub fn to_display_string(&self) -> String {
        self.render(true)
    }

    /// Joins `child` (another parsed path) onto this one.
    ///
    /// An absolute child replaces this path; a child rooted with no prefix (`\a`) keeps this path's
    /// prefix; a drive-relative child on the same drive is appended; anything else is appended.
    pub fn join(&self, child: &WinPath) -> WinPath {
        if child.is_absolute() {
            return child.clone();
        }
        match (&child.prefix, child.rooted) {
            (None, true) => WinPath {
                prefix: self.prefix.clone(),
                rooted: true,
                components: components(
                    &child.components.join("\\"),
                    self.is_verbatim(),
                    true,
                    Vec::new(),
                ),
            },
            (Some(Prefix::Drive(other)), _) if self.prefix != Some(Prefix::Drive(*other)) => {
                child.clone()
            }
            _ => WinPath {
                prefix: self.prefix.clone(),
                rooted: self.rooted,
                components: components(
                    &child.components.join("\\"),
                    self.is_verbatim(),
                    self.rooted,
                    self.components.clone(),
                ),
            },
        }
    }

    /// The path without its last component, or `None` at a root.
    pub fn parent(&self) -> Option<WinPath> {
        let (_, rest) = self.components.split_last()?;
        Some(WinPath {
            prefix: self.prefix.clone(),
            rooted: self.rooted,
            components: rest.to_vec(),
        })
    }

    pub fn file_name(&self) -> Option<&str> {
        self.components.last().map(String::as_str)
    }

    /// A key that is equal for paths naming the same file under Windows' case rules, and for a
    /// verbatim path and the ordinary path it spells. NTFS folds case with an upper-case table, so
    /// this upper-cases.
    pub fn fold_key(&self) -> String {
        fold(&self.to_display_string())
    }
}

/// Upper-cases text the way the file system compares names.
pub fn fold(text: &str) -> String {
    text.chars().flat_map(char::to_uppercase).collect()
}

/// Whether two names are the same file name to Windows.
pub fn names_equal(a: &str, b: &str) -> bool {
    fold(a) == fold(b)
}

/// Whether `name` is a device name Windows reserves in every directory (`CON`, `NUL`, `COM1`…), even
/// with an extension (`nul.txt`).
pub fn is_reserved_name(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or("").trim_end_matches(' ');
    let upper = fold(stem);
    match upper.as_str() {
        "CON" | "PRN" | "AUX" | "NUL" => true,
        _ => {
            let digit = upper
                .strip_prefix("COM")
                .or_else(|| upper.strip_prefix("LPT"))
                .and_then(|n| n.chars().next().filter(|_| n.len() == 1));
            matches!(digit, Some('1'..='9'))
        }
    }
}

/// Whether a single component can be created as a file name: no reserved characters, no control
/// characters, no trailing dot or space, and not a reserved device name.
pub fn is_valid_name(name: &str) -> bool {
    !name.is_empty()
        && !name
            .chars()
            .any(|c| c.is_control() || "<>:\"/\\|?*".contains(c))
        && !name.ends_with(['.', ' '])
        && !is_reserved_name(name)
}

fn encode_segment(out: &mut String, segment: &str, keep: &[u8]) {
    encoding::encode_into(out, segment.as_bytes(), keep);
}

/// The percent-encoded `file://` URI for an absolute path.
///
/// Drive paths become `file:///C:/a`, UNC paths `file://server/share/a`. Verbatim and device paths
/// use the empty-authority form `file:////%3F/C:/a` (the path is `//?/C:/a`), so the verbatim prefix
/// survives the round trip.
pub fn to_uri(path: &WinPath) -> Result<String, PathError> {
    if !path.is_absolute() {
        return Err(PathError::NotAbsolute);
    }
    let mut out = String::from("file://");
    match &path.prefix {
        Some(Prefix::Drive(letter)) => {
            out.push('/');
            out.push(*letter);
            out.push(':');
            if path.components.is_empty() {
                out.push('/');
            }
        }
        Some(Prefix::Unc { server, share }) => {
            encode_segment(&mut out, server, b"");
            out.push('/');
            encode_segment(&mut out, share, b"");
            if path.components.is_empty() {
                out.push('/');
            }
        }
        Some(prefix) => {
            // The path is `//?/C:/a`: an empty authority, then the verbatim or device prefix.
            out.push_str("//");
            let (lead, names): (&str, Vec<String>) = match prefix {
                Prefix::VerbatimDrive(letter) => ("?", vec![format!("{letter}:")]),
                Prefix::VerbatimUnc { server, share } => {
                    ("?", vec!["UNC".to_owned(), server.clone(), share.clone()])
                }
                Prefix::Verbatim(name) => ("?", vec![name.clone()]),
                Prefix::Device(name) => (".", vec![name.clone()]),
                Prefix::Drive(_) | Prefix::Unc { .. } => return Err(PathError::NotAbsolute),
            };
            out.push_str(if lead == "?" { "%3F" } else { lead });
            for name in names {
                out.push('/');
                let keep: &[u8] = if drive_letter(&name).is_some() {
                    b":"
                } else {
                    b""
                };
                encode_segment(&mut out, &name, keep);
            }
            if path.components.is_empty() {
                out.push('/');
            }
        }
        None => return Err(PathError::NotAbsolute),
    }
    for part in &path.components {
        out.push('/');
        encode_segment(&mut out, part, b"");
    }
    Ok(out)
}

fn decode_str(text: &str) -> Result<String, PathError> {
    String::from_utf8(encoding::decode(text)?)
        .map_err(|_| PathError::Invalid("a Windows path must be valid Unicode"))
}

/// The absolute path a `file://` URI names (the text after `file://`).
pub fn from_uri(rest: &str) -> Result<WinPath, PathError> {
    let (host, path) = match rest.find('/') {
        Some(at) => rest.split_at(at),
        None => (rest, ""),
    };
    if path.contains(['?', '#']) {
        return Err(PathError::InvalidUri(
            "a query or fragment is not part of a path",
        ));
    }
    let native = if host.is_empty() || host.eq_ignore_ascii_case("localhost") {
        if path.starts_with("//") {
            decode_str(path)?.replace('/', "\\")
        } else {
            let decoded = decode_str(path.strip_prefix('/').unwrap_or(path))?;
            decoded.replace('/', "\\")
        }
    } else {
        format!(
            r"\\{}{}",
            decode_str(host)?,
            decode_str(path)?.replace('/', "\\")
        )
    };
    let parsed = parse(&native)?;
    if parsed.is_absolute() {
        Ok(parsed)
    } else {
        Err(PathError::NotAbsolute)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(text: &str) -> WinPath {
        parse(text).unwrap()
    }

    #[test]
    fn parses_drive_paths_and_accepts_both_separators() {
        let path = p("c:/Users//me/./Docs/../x");
        assert_eq!(path.prefix, Some(Prefix::Drive('C')));
        assert_eq!(path.to_native_string(), r"C:\Users\me\x");
        assert_eq!(p(r"C:\").to_native_string(), r"C:\");
        assert!(p(r"C:\").is_absolute());
    }

    #[test]
    fn tells_relative_forms_from_absolute_ones() {
        assert!(!p("C:foo").is_absolute());
        assert_eq!(p("C:foo").to_native_string(), "C:foo");
        assert!(!p(r"\foo").is_absolute());
        assert!(!p(r"foo\bar").is_absolute());
        assert_eq!(p(r"..\..\a").to_native_string(), r"..\..\a");
    }

    #[test]
    fn parses_unc_paths() {
        let path = p(r"\\server\share\dir\file.txt");
        assert_eq!(
            path.prefix,
            Some(Prefix::Unc {
                server: "server".to_owned(),
                share: "share".to_owned()
            })
        );
        assert_eq!(path.file_name(), Some("file.txt"));
        assert_eq!(p("//server/share").to_native_string(), r"\\server\share\");
        assert!(parse(r"\\server").is_err());
        assert!(parse(r"\\\share").is_err());
    }

    #[test]
    fn keeps_verbatim_paths_literal() {
        let path = p(r"\\?\C:\a\..\b/c\");
        assert!(path.is_verbatim());
        assert_eq!(path.components, vec!["a", "..", "b/c"]);
        assert_eq!(path.to_native_string(), r"\\?\C:\a\..\b/c");
    }

    #[test]
    fn parses_verbatim_unc_and_device_paths() {
        let unc = p(r"\\?\UNC\srv\sh\x");
        assert_eq!(unc.to_display_string(), r"\\srv\sh\x");
        assert_eq!(unc.to_native_string(), r"\\?\UNC\srv\sh\x");
        let volume = p(r"\\?\Volume{1234}\x");
        assert_eq!(volume.to_display_string(), r"\\?\Volume{1234}\x");
        let device = p(r"\\.\COM1");
        assert_eq!(device.prefix, Some(Prefix::Device("COM1".to_owned())));
    }

    #[test]
    fn displays_a_long_path_as_the_ordinary_path() {
        assert_eq!(p(r"\\?\c:\Users\me").to_display_string(), r"C:\Users\me");
    }

    #[test]
    fn joins_and_takes_parents() {
        let base = p(r"C:\a\b");
        assert_eq!(base.join(&p(r"c\d")).to_native_string(), r"C:\a\b\c\d");
        assert_eq!(base.join(&p(r"..\x")).to_native_string(), r"C:\a\x");
        assert_eq!(base.join(&p(r"\x")).to_native_string(), r"C:\x");
        assert_eq!(base.join(&p(r"D:\x")).to_native_string(), r"D:\x");
        assert_eq!(base.join(&p("c:x")).to_native_string(), r"C:\a\b\x");
        assert!(!base.join(&p("d:x")).is_absolute());
        assert_eq!(base.parent().unwrap().to_native_string(), r"C:\a");
        assert_eq!(p(r"C:\a").parent().unwrap().to_native_string(), r"C:\");
        assert!(p(r"C:\").parent().is_none());
        assert!(p(r"\\s\sh").parent().is_none());
        assert_eq!(p(r"C:\").file_name(), None);
    }

    #[test]
    fn joins_onto_a_verbatim_path_without_resolving_dots() {
        let base = p(r"\\?\C:\a");
        assert_eq!(base.join(&p("b")).to_native_string(), r"\\?\C:\a\b");
    }

    #[test]
    fn compares_case_insensitively_and_across_verbatim() {
        assert_eq!(p(r"c:\Users\Me").fold_key(), p(r"C:\USERS\me").fold_key());
        assert_eq!(p(r"\\?\C:\x").fold_key(), p(r"C:\X").fold_key());
        assert_ne!(p(r"C:\x").fold_key(), p(r"C:\y").fold_key());
        assert!(names_equal("Straße.TXT", "STRASSE.txt"));
    }

    #[test]
    fn spots_reserved_and_invalid_names() {
        for name in ["CON", "nul", "Aux.txt", "com1", "LPT9.log", "prn "] {
            assert!(is_reserved_name(name), "{name}");
        }
        for name in ["COM0", "COM10", "console", "communal"] {
            assert!(!is_reserved_name(name), "{name}");
        }
        assert!(is_valid_name("report.txt"));
        assert!(!is_valid_name("a:b"));
        assert!(!is_valid_name("trailing."));
        assert!(!is_valid_name("nul.txt"));
        assert!(!is_valid_name(""));
    }

    #[test]
    fn writes_uris_for_drive_unc_and_verbatim_paths() {
        assert_eq!(
            to_uri(&p(r"C:\Users\me\a b")).unwrap(),
            "file:///C:/Users/me/a%20b"
        );
        assert_eq!(to_uri(&p(r"C:\")).unwrap(), "file:///C:/");
        assert_eq!(to_uri(&p(r"\\srv\sh\x")).unwrap(), "file://srv/sh/x");
        assert_eq!(to_uri(&p(r"\\?\C:\x")).unwrap(), "file:////%3F/C:/x");
        assert_eq!(
            to_uri(&p(r"\\?\UNC\srv\sh\x")).unwrap(),
            "file:////%3F/UNC/srv/sh/x"
        );
        assert_eq!(to_uri(&p(r"\\?\C:\")).unwrap(), "file:////%3F/C:/");
        assert_eq!(to_uri(&p("C:foo")), Err(PathError::NotAbsolute));
    }

    #[test]
    fn reads_the_uris_it_writes() {
        for text in [
            r"C:\",
            r"C:\Users\me\a b\é#%.txt",
            r"\\srv\sh\x",
            r"\\srv\sh\",
            r"\\?\C:\x\y",
            r"\\?\C:\",
            r"\\?\UNC\srv\sh",
            r"\\?\UNC\srv\sh\x",
            r"\\?\Volume{1234}\x",
            r"\\.\COM1",
        ] {
            let path = p(text);
            let uri = to_uri(&path).unwrap();
            let back = from_uri(uri.strip_prefix("file://").unwrap()).unwrap();
            assert_eq!(back, path, "{text} via {uri}");
        }
    }

    #[test]
    fn reads_common_uri_spellings() {
        assert_eq!(from_uri("/c:/a").unwrap().to_native_string(), r"C:\a");
        assert_eq!(
            from_uri("localhost/C:/a").unwrap().to_native_string(),
            r"C:\a"
        );
        assert_eq!(
            from_uri("srv/sh/a").unwrap().to_native_string(),
            r"\\srv\sh\a"
        );
        assert!(from_uri("/a/b").is_err());
        assert!(from_uri("/C:/a?x").is_err());
        assert!(from_uri("/C:/%FF").is_err());
    }
}
