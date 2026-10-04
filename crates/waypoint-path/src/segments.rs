// Byte segments of a URI path, shared by the remote, archive and Git paths: parsing, folding,
// joining, encoding and the lossy form people read.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::ffi::{OsStr, OsString};

use crate::encoding::{decode, encode_into};
use crate::PathError;

/// One name in a path, as raw bytes: a name on a server need not be UTF-8.
pub(crate) type Segment = Vec<u8>;

/// Decodes percent-escapes. Strict decoding refuses an incomplete escape; lenient decoding (typed
/// text, where `100%` is a name) keeps a `%` that does not start an escape as itself.
pub(crate) fn decode_text(text: &str, lenient: bool) -> Result<Vec<u8>, PathError> {
    if !lenient {
        return decode(text);
    }
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            if let Some(escape) = text.get(i..i + 3) {
                if let Ok(decoded) = decode(escape) {
                    out.extend_from_slice(&decoded);
                    i += 3;
                    continue;
                }
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    Ok(out)
}

/// Folds one decoded name onto `stack`: empty names and `.` are dropped, `..` removes the name
/// before it (and stops at the root).
fn push(stack: &mut Vec<Segment>, name: Segment) -> Result<(), PathError> {
    if name.contains(&0) {
        return Err(PathError::InteriorNul);
    }
    match &name[..] {
        b"" | b"." => {}
        b".." => {
            stack.pop();
        }
        _ => stack.push(name),
    }
    Ok(())
}

/// Splits the encoded path of a URI (`/a/b%20c`) into decoded names, folding `.`, `..` and empty
/// names. A name that decodes to one holding `/` is refused: no provider has such names.
pub(crate) fn parse_path(encoded: &str, lenient: bool) -> Result<Vec<Segment>, PathError> {
    let mut out = Vec::new();
    for part in encoded.split('/') {
        let name = decode_text(part, lenient)?;
        if name.contains(&b'/') {
            return Err(PathError::InvalidUri("a name cannot contain `/`"));
        }
        push(&mut out, name)?;
    }
    Ok(out)
}

/// Joins a relative child (which may hold `/`, `.` and `..`) onto `base`. A child that starts with
/// `/` replaces the whole path.
pub(crate) fn join(base: &[Segment], child: &[u8]) -> Result<Vec<Segment>, PathError> {
    let mut out = if child.first() == Some(&b'/') {
        Vec::new()
    } else {
        base.to_vec()
    };
    for part in child.split(|&b| b == b'/') {
        push(&mut out, part.to_vec())?;
    }
    Ok(out)
}

/// The encoded path: `/` for no names, otherwise `/` before each percent-encoded name.
pub(crate) fn render(segments: &[Segment]) -> String {
    if segments.is_empty() {
        return "/".to_owned();
    }
    let mut out = String::new();
    for name in segments {
        out.push('/');
        encode_into(&mut out, name, &[]);
    }
    out
}

/// The path for people: the names decoded, lossy where one is not UTF-8.
pub(crate) fn display(segments: &[Segment]) -> String {
    if segments.is_empty() {
        return "/".to_owned();
    }
    let mut out = String::new();
    for name in segments {
        out.push('/');
        out.push_str(&String::from_utf8_lossy(name));
    }
    out
}

/// The bytes of a native string. On Windows names are Unicode, so a string that is not is refused.
pub(crate) fn os_bytes(text: &OsStr) -> Result<Vec<u8>, PathError> {
    #[cfg(unix)]
    {
        Ok(std::os::unix::ffi::OsStrExt::as_bytes(text).to_vec())
    }
    #[cfg(not(unix))]
    {
        text.to_str()
            .map(|text| text.as_bytes().to_vec())
            .ok_or(PathError::Unrepresentable)
    }
}

/// A name as a native string: exact on Unix, lossy elsewhere for bytes that are not UTF-8.
pub(crate) fn os_string(name: &[u8]) -> OsString {
    #[cfg(unix)]
    {
        <OsString as std::os::unix::ffi::OsStringExt>::from_vec(name.to_vec())
    }
    #[cfg(not(unix))]
    {
        OsString::from(String::from_utf8_lossy(name).into_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_folds_dots_and_empty_names() {
        let parsed = parse_path("/a//./b/../c%20d/", false).unwrap();
        assert_eq!(parsed, vec![b"a".to_vec(), b"c d".to_vec()]);
        assert_eq!(render(&parsed), "/a/c%20d");
        assert_eq!(display(&parsed), "/a/c d");
        assert!(parse_path("/../..", false).unwrap().is_empty());
        assert_eq!(render(&[]), "/");
    }

    #[test]
    fn escaped_slashes_and_nuls_are_refused() {
        assert!(parse_path("/a%2Fb", false).is_err());
        assert_eq!(parse_path("/a%00", false), Err(PathError::InteriorNul));
    }

    #[test]
    fn lenient_decoding_keeps_a_stray_percent() {
        assert_eq!(decode_text("100%", true).unwrap(), b"100%".to_vec());
        assert_eq!(decode_text("a%20b%zz", true).unwrap(), b"a b%zz".to_vec());
        assert!(decode_text("100%", false).is_err());
    }

    #[test]
    fn joining_folds_and_an_absolute_child_replaces() {
        let base = vec![b"a".to_vec(), b"b".to_vec()];
        assert_eq!(
            join(&base, b"../c/d").unwrap(),
            vec![b"a".to_vec(), b"c".to_vec(), b"d".to_vec()]
        );
        assert_eq!(join(&base, b"/x").unwrap(), vec![b"x".to_vec()]);
        assert!(join(&base, b"a\0").is_err());
    }

    #[test]
    fn names_that_are_not_utf8_render_losslessly() {
        let names = vec![b"caf\xe9".to_vec()];
        assert_eq!(render(&names), "/caf%E9");
        assert_eq!(parse_path(&render(&names), false).unwrap(), names);
    }
}
