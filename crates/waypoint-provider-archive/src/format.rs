// Telling which kind of archive a file is, from its first bytes and then its name.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::io::{self, Read};

/// How a tar archive is compressed as a whole.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TarCompression {
    None,
    Gzip,
    Bzip2,
    Xz,
    Zstd,
}

/// The kinds of archive that browse as folders.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArchiveFormat {
    Zip,
    Tar(TarCompression),
    SevenZ,
}

impl ArchiveFormat {
    /// A short name for messages and logs.
    pub fn label(self) -> &'static str {
        match self {
            Self::Zip => "zip",
            Self::SevenZ => "7z",
            Self::Tar(TarCompression::None) => "tar",
            Self::Tar(TarCompression::Gzip) => "tar.gz",
            Self::Tar(TarCompression::Bzip2) => "tar.bz2",
            Self::Tar(TarCompression::Xz) => "tar.xz",
            Self::Tar(TarCompression::Zstd) => "tar.zst",
        }
    }

    /// Whether listing has to read the file from its start (no index at the end or the start).
    pub fn sequential_listing(self) -> bool {
        matches!(self, Self::Tar(compression) if compression != TarCompression::None)
    }
}

/// How many bytes `detect` needs to see.
pub(crate) const SNIFF: usize = 512;

fn compression_of(head: &[u8]) -> Option<TarCompression> {
    if head.starts_with(&[0x1f, 0x8b]) {
        Some(TarCompression::Gzip)
    } else if head.starts_with(b"BZh") {
        Some(TarCompression::Bzip2)
    } else if head.starts_with(&[0xfd, b'7', b'z', b'X', b'Z', 0]) {
        Some(TarCompression::Xz)
    } else if head.starts_with(&[0x28, 0xb5, 0x2f, 0xfd]) {
        Some(TarCompression::Zstd)
    } else {
        None
    }
}

/// Whether a 512-byte block is a tar header: the checksum field matches the block.
pub(crate) fn is_tar_header(block: &[u8]) -> bool {
    if block.len() < 512 || block.iter().all(|&byte| byte == 0) {
        return false;
    }
    let stored = &block[148..156];
    let text: String = stored
        .iter()
        .take_while(|&&byte| byte != 0 && byte != b' ')
        .map(|&byte| byte as char)
        .collect();
    let Ok(stored) = u32::from_str_radix(text.trim(), 8) else {
        return false;
    };
    let sum: u32 = block
        .iter()
        .enumerate()
        .map(|(at, &byte)| {
            if (148..156).contains(&at) {
                u32::from(b' ')
            } else {
                u32::from(byte)
            }
        })
        .sum();
    sum == stored
}

/// Reads up to `SNIFF` bytes.
pub(crate) fn read_head(reader: &mut dyn Read) -> io::Result<Vec<u8>> {
    let mut head = vec![0u8; SNIFF];
    let mut filled = 0;
    while filled < SNIFF {
        let read = reader.read(&mut head[filled..])?;
        if read == 0 {
            break;
        }
        filled += read;
    }
    head.truncate(filled);
    Ok(head)
}

/// What the file's first bytes say, before the name is consulted: a format, or a compression
/// wrapper whose content `inner` must still be sniffed.
pub(crate) enum Magic {
    Format(ArchiveFormat),
    /// A compression wrapper: it is a tar archive if what it holds is.
    Wrapped(TarCompression),
    Unknown,
}

pub(crate) fn magic(head: &[u8]) -> Magic {
    if head.starts_with(b"7z\xbc\xaf\x27\x1c") {
        Magic::Format(ArchiveFormat::SevenZ)
    } else if head.starts_with(b"PK\x03\x04") || head.starts_with(b"PK\x05\x06") {
        Magic::Format(ArchiveFormat::Zip)
    } else if is_tar_header(head) {
        Magic::Format(ArchiveFormat::Tar(TarCompression::None))
    } else if let Some(compression) = compression_of(head) {
        Magic::Wrapped(compression)
    } else {
        Magic::Unknown
    }
}

/// What the file's name suggests, for archives whose first bytes do not say (a zip with a program
/// in front of it, a tar that starts with an empty block).
pub(crate) fn from_name(name: &str) -> Option<ArchiveFormat> {
    let lower = name.to_lowercase();
    let ends = |suffixes: &[&str]| suffixes.iter().any(|suffix| lower.ends_with(suffix));
    Some(
        if ends(&[
            ".zip", ".jar", ".apk", ".epub", ".docx", ".xlsx", ".pptx", ".odt", ".ods",
        ]) {
            ArchiveFormat::Zip
        } else if ends(&[".7z"]) {
            ArchiveFormat::SevenZ
        } else if ends(&[".tar.gz", ".tgz"]) {
            ArchiveFormat::Tar(TarCompression::Gzip)
        } else if ends(&[".tar.bz2", ".tbz", ".tbz2"]) {
            ArchiveFormat::Tar(TarCompression::Bzip2)
        } else if ends(&[".tar.xz", ".txz"]) {
            ArchiveFormat::Tar(TarCompression::Xz)
        } else if ends(&[".tar.zst", ".tzst", ".tar.zstd"]) {
            ArchiveFormat::Tar(TarCompression::Zstd)
        } else if ends(&[".tar"]) {
            ArchiveFormat::Tar(TarCompression::None)
        } else {
            return None;
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_say_the_format() {
        assert_eq!(from_name("Photos.ZIP"), Some(ArchiveFormat::Zip));
        assert_eq!(
            from_name("a.tar.gz"),
            Some(ArchiveFormat::Tar(TarCompression::Gzip))
        );
        assert_eq!(
            from_name("a.tgz"),
            Some(ArchiveFormat::Tar(TarCompression::Gzip))
        );
        assert_eq!(
            from_name("a.tar.zst"),
            Some(ArchiveFormat::Tar(TarCompression::Zstd))
        );
        assert_eq!(from_name("a.7z"), Some(ArchiveFormat::SevenZ));
        assert_eq!(from_name("notes.txt.gz"), None);
        assert_eq!(from_name("notes.txt"), None);
    }

    #[test]
    fn magic_numbers_say_the_format() {
        assert!(matches!(
            magic(b"PK\x03\x04rest"),
            Magic::Format(ArchiveFormat::Zip)
        ));
        assert!(matches!(
            magic(b"7z\xbc\xaf\x27\x1c\0\x04"),
            Magic::Format(ArchiveFormat::SevenZ)
        ));
        assert!(matches!(
            magic(&[0x1f, 0x8b, 8, 0]),
            Magic::Wrapped(TarCompression::Gzip)
        ));
        assert!(matches!(magic(b"hello"), Magic::Unknown));
    }
}
