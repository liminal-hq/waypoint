// The pure side of the Windows backend: file attributes to shell flags, shell errors to outcomes and bitmap pixels to PNG pixels
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Nothing here calls the operating system, so it is tested on every platform; `windows.rs` supplies the values.

use crate::builtin::Rendered;
use crate::models::SkipWhy;

/// `FILE_ATTRIBUTE_RECALL_ON_OPEN`: opening the file brings its content down from the cloud.
pub const FILE_ATTRIBUTE_RECALL_ON_OPEN: u32 = 0x0004_0000;
/// `FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS`: reading the file's content brings it down from the cloud.
pub const FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS: u32 = 0x0040_0000;
/// `FILE_ATTRIBUTE_OFFLINE`: the content has been moved to offline storage.
pub const FILE_ATTRIBUTE_OFFLINE: u32 = 0x0000_1000;

/// Whether reading the file would download it (a OneDrive-style placeholder, or offline storage).
pub fn is_placeholder(attributes: u32) -> bool {
    attributes
        & (FILE_ATTRIBUTE_RECALL_ON_OPEN
            | FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS
            | FILE_ATTRIBUTE_OFFLINE)
        != 0
}

/// How the shell may be asked for a thumbnail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellMode {
    /// Ask for a thumbnail, letting the shell make one.
    Make,
    /// Ask only for one the shell already has (`SIIGBF_INCACHEONLY`): a placeholder's thumbnail is never made, because making it would download the file.
    CacheOnly,
}

pub fn shell_mode(attributes: u32) -> ShellMode {
    if is_placeholder(attributes) {
        ShellMode::CacheOnly
    } else {
        ShellMode::Make
    }
}

/// Whether the path is on a local drive: `C:\…` or `C:/…` (also with the `\\?\` prefix). A network share (`\\server\share`), a `\\?\UNC\` path and anything with a scheme are remote.
pub fn is_local(path: &str) -> bool {
    let path = path.strip_prefix(r"\\?\").unwrap_or(path);
    let bytes = path.as_bytes();
    bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && (bytes[2] == b'\\' || bytes[2] == b'/')
        && !path.contains("://")
}

/// What a failed `GetImage` means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellFailure {
    /// The shell has no thumbnail handler for this kind of file, or its handler found nothing to show.
    NoThumbnail,
    /// The file is not there.
    NotFound,
    /// Anything else.
    Other,
}

const E_FAIL: i32 = 0x8000_4005_u32 as i32;
const E_PENDING: i32 = 0x8000_000A_u32 as i32;
const WTS_E_FIRST: u32 = 0x8004_B200;
const WTS_E_LAST: u32 = 0x8004_B20F;

/// Classifies the `HRESULT` of a failed `GetImage`: the thumbnail cache's own codes (`WTS_E_*`), `E_FAIL` and `E_PENDING` mean the shell has nothing to show for this file; `ERROR_FILE_NOT_FOUND` and `ERROR_PATH_NOT_FOUND` mean it is gone.
pub fn classify_failure(hresult: i32) -> ShellFailure {
    let code = hresult as u32;
    if (WTS_E_FIRST..=WTS_E_LAST).contains(&code) || hresult == E_FAIL || hresult == E_PENDING {
        return ShellFailure::NoThumbnail;
    }
    // `HRESULT_FROM_WIN32(2)` and `(3)`.
    if code == 0x8007_0002 || code == 0x8007_0003 {
        return ShellFailure::NotFound;
    }
    ShellFailure::Other
}

/// What to report for a failed ask: a cache-only miss on a placeholder is "cloud" (nothing is wrong, nothing was downloaded); a file the shell cannot show is `NoGenerator`; the rest is a failure to explain.
pub fn skip_for(mode: ShellMode, failure: ShellFailure) -> Option<SkipWhy> {
    match (mode, failure) {
        (ShellMode::CacheOnly, ShellFailure::NoThumbnail | ShellFailure::Other) => {
            Some(SkipWhy::Cloud)
        }
        (ShellMode::Make, ShellFailure::NoThumbnail) => Some(SkipWhy::NoGenerator),
        _ => None,
    }
}

/// Turns a top-down 32-bit `BGRA` bitmap into PNG pixels. A bitmap whose alpha bytes are all zero has no alpha channel (a thumbnail of an opaque picture), so it becomes opaque RGB; otherwise the alpha is kept as `RGBA`.
pub fn bitmap_to_rendered(width: u32, height: u32, bgra: &[u8]) -> Option<Rendered> {
    let expected = (width as usize)
        .checked_mul(height as usize)?
        .checked_mul(4)?;
    if width == 0 || height == 0 || bgra.len() < expected {
        return None;
    }
    let bgra = &bgra[..expected];
    let has_alpha = bgra.as_chunks::<4>().0.iter().any(|pixel| pixel[3] != 0);
    if has_alpha {
        let mut pixels = Vec::with_capacity(expected);
        for pixel in bgra.as_chunks::<4>().0 {
            pixels.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
        }
        Some(Rendered {
            width,
            height,
            color: png::ColorType::Rgba,
            pixels,
        })
    } else {
        let mut pixels = Vec::with_capacity(expected / 4 * 3);
        for pixel in bgra.as_chunks::<4>().0 {
            pixels.extend_from_slice(&[pixel[2], pixel[1], pixel[0]]);
        }
        Some(Rendered {
            width,
            height,
            color: png::ColorType::Rgb,
            pixels,
        })
    }
}

#[cfg(test)]
mod tests;
