// Makes the unguessable names and the per-launch token of the Windows pipes, from a source of random bytes the caller supplies
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::io;

/// Where named pipes live on Windows.
pub const PIPE_NAMESPACE: &str = r"\\.\pipe\";

/// Bytes of the random part of a pipe name (128 bits).
const NAME_BYTES: usize = 16;
/// Bytes of the per-launch token (256 bits).
const TOKEN_BYTES: usize = 32;

/// A source of random bytes. The real one is the operating system's; tests use a counter.
pub trait Rng {
    fn fill(&mut self, buf: &mut [u8]) -> io::Result<()>;
}

/// The names of the two one-direction pipes of a launch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipeNames {
    /// The pipe the app writes and the helper reads.
    pub to: String,
    /// The pipe the helper writes and the app reads.
    pub from: String,
}

/// Lower-case hexadecimal text of `bytes`.
pub fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        text.push(DIGITS[(byte >> 4) as usize] as char);
        text.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    text
}

/// The two pipe names for one launch: `\\.\pipe\<prefix>-<128 random bits as hex>-to` and `...-from`. The prefix is the app's, and only letters, digits, `-` and `_` are accepted so that it cannot change the name's shape.
pub fn pipe_names(prefix: &str, rng: &mut dyn Rng) -> io::Result<PipeNames> {
    let valid = !prefix.is_empty()
        && prefix.len() <= 64
        && prefix
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if !valid {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid pipe name prefix",
        ));
    }
    let mut id = [0u8; NAME_BYTES];
    rng.fill(&mut id)?;
    let base = format!("{PIPE_NAMESPACE}{prefix}-{}", hex(&id));
    Ok(PipeNames {
        to: format!("{base}-to"),
        from: format!("{base}-from"),
    })
}

/// A new per-launch token: 256 random bits as 64 hex digits.
pub fn token(rng: &mut dyn Rng) -> io::Result<String> {
    let mut bytes = [0u8; TOKEN_BYTES];
    rng.fill(&mut bytes)?;
    Ok(hex(&bytes))
}

/// Compares two byte strings in time that depends only on their lengths.
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut difference = 0u8;
    for (x, y) in a.iter().zip(b) {
        difference |= x ^ y;
    }
    difference == 0
}

/// The longest handshake line worth reading: the ready line, a space, the token and a line ending.
pub fn max_handshake_len(ready_line: &str) -> usize {
    ready_line.len() + 1 + TOKEN_BYTES * 2 + 2
}

/// True when `line` (with or without its line ending) is exactly `<ready line> <token>`. The token is compared in constant time.
pub fn is_handshake(line: &[u8], ready_line: &str, token: &str) -> bool {
    let line = line.strip_suffix(b"\n").unwrap_or(line);
    let line = line.strip_suffix(b"\r").unwrap_or(line);
    let Some(rest) = line.strip_prefix(ready_line.as_bytes()) else {
        return false;
    };
    let Some(presented) = rest.strip_prefix(b" ") else {
        return false;
    };
    constant_time_eq(presented, token.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Counts up, so every call gives different, predictable bytes.
    struct Counter(u8);

    impl Rng for Counter {
        fn fill(&mut self, buf: &mut [u8]) -> io::Result<()> {
            for byte in buf {
                *byte = self.0;
                self.0 = self.0.wrapping_add(1);
            }
            Ok(())
        }
    }

    struct Broken;

    impl Rng for Broken {
        fn fill(&mut self, _: &mut [u8]) -> io::Result<()> {
            Err(io::Error::other("no entropy"))
        }
    }

    #[test]
    fn hex_is_lower_case_and_two_digits_per_byte() {
        assert_eq!(hex(&[0x00, 0x0f, 0xa5, 0xff]), "000fa5ff");
        assert_eq!(hex(&[]), "");
    }

    #[test]
    fn names_carry_the_prefix_128_bits_and_the_direction() {
        let names = pipe_names("tool-elevate", &mut Counter(0)).unwrap();
        assert_eq!(
            names.to,
            r"\\.\pipe\tool-elevate-000102030405060708090a0b0c0d0e0f-to"
        );
        assert_eq!(
            names.from,
            r"\\.\pipe\tool-elevate-000102030405060708090a0b0c0d0e0f-from"
        );
    }

    #[test]
    fn two_launches_get_different_names_and_tokens() {
        let mut rng = Counter(0);
        let first = pipe_names("p", &mut rng).unwrap();
        let second = pipe_names("p", &mut rng).unwrap();
        assert_ne!(first, second);
        let token_a = token(&mut rng).unwrap();
        let token_b = token(&mut rng).unwrap();
        assert_eq!(token_a.len(), 64);
        assert_ne!(token_a, token_b);
        assert!(token_a.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn a_prefix_that_could_change_the_name_is_refused() {
        for prefix in ["", "a b", r"a\b", "a/b", "a\"b", "..", "é", &"x".repeat(65)] {
            assert!(
                pipe_names(prefix, &mut Counter(0)).is_err(),
                "accepted {prefix:?}"
            );
        }
    }

    #[test]
    fn a_failing_source_of_randomness_is_an_error_not_a_weak_name() {
        assert!(pipe_names("p", &mut Broken).is_err());
        assert!(token(&mut Broken).is_err());
    }

    #[test]
    fn the_handshake_is_the_ready_line_then_the_token() {
        assert!(is_handshake(b"ready abc", "ready", "abc"));
        assert!(is_handshake(b"ready abc\n", "ready", "abc"));
        assert!(is_handshake(b"ready abc\r\n", "ready", "abc"));
        assert!(!is_handshake(b"ready abd", "ready", "abc"));
        assert!(!is_handshake(b"ready abcd", "ready", "abc"));
        assert!(!is_handshake(b"ready ab", "ready", "abc"));
        assert!(!is_handshake(b"ready", "ready", "abc"));
        assert!(!is_handshake(b"ready  abc", "ready", "abc"));
        assert!(!is_handshake(b"readyabc", "ready", "abc"));
        assert!(!is_handshake(b"other abc", "ready", "abc"));
        assert!(!is_handshake(b"", "ready", "abc"));
    }

    #[test]
    fn the_comparison_needs_equal_bytes_and_equal_lengths() {
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(!constant_time_eq(b"abc", b"ab"));
        assert!(constant_time_eq(b"", b""));
    }
}
