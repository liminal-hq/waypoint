// Percent-encoding for the `file://` URIs locations carry.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::PathError;

const HEX: &[u8; 16] = b"0123456789ABCDEF";

fn is_unreserved(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~')
}

/// Appends `bytes` to `out`, percent-encoding everything except unreserved characters and the
/// bytes listed in `keep`.
pub(crate) fn encode_into(out: &mut String, bytes: &[u8], keep: &[u8]) {
    for &byte in bytes {
        if is_unreserved(byte) || keep.contains(&byte) {
            out.push(byte as char);
        } else {
            out.push('%');
            out.push(HEX[usize::from(byte >> 4)] as char);
            out.push(HEX[usize::from(byte & 0x0f)] as char);
        }
    }
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

/// Decodes percent-escapes to the raw bytes they stand for, which need not be valid UTF-8.
pub(crate) fn decode(text: &str) -> Result<Vec<u8>, PathError> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let high = bytes.get(i + 1).copied().and_then(hex_value);
            let low = bytes.get(i + 2).copied().and_then(hex_value);
            match (high, low) {
                (Some(high), Some(low)) => out.push(high << 4 | low),
                _ => return Err(PathError::InvalidUri("a percent-escape is incomplete")),
            }
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_reserved_and_non_ascii_bytes() {
        let mut out = String::new();
        encode_into(&mut out, "a b#é/".as_bytes(), b"/");
        assert_eq!(out, "a%20b%23%C3%A9/");
    }

    #[test]
    fn decodes_bytes_that_are_not_utf8() {
        assert_eq!(decode("a%FFb").unwrap(), vec![b'a', 0xff, b'b']);
    }

    #[test]
    fn rejects_truncated_escapes() {
        assert!(decode("a%2").is_err());
        assert!(decode("a%zz").is_err());
    }

    #[test]
    fn round_trips_every_byte() {
        let all: Vec<u8> = (0..=255).collect();
        let mut out = String::new();
        encode_into(&mut out, &all, b"");
        assert_eq!(decode(&out).unwrap(), all);
    }
}
