// The pure parts of drawing the shell's icons: which system image list serves a size, and turning the bitmap an icon gives into straight RGBA
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The Windows backend asks the shell for an icon's index in the system image list and then for the icon at one of four fixed sizes. Nothing here touches the shell, so it is compiled and tested everywhere.

#![cfg_attr(not(windows), allow(dead_code))]

/// The system image lists the shell keeps, by the size of their icons.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageList {
    /// 16 pixels (`SHIL_SMALL`).
    Small,
    /// 32 pixels (`SHIL_LARGE`).
    Large,
    /// 48 pixels (`SHIL_EXTRALARGE`).
    ExtraLarge,
    /// 256 pixels (`SHIL_JUMBO`).
    Jumbo,
}

impl ImageList {
    /// The smallest list whose icons are at least `pixels` across, so a picture is only ever scaled down.
    pub fn for_pixels(pixels: u32) -> ImageList {
        match pixels {
            0..=16 => ImageList::Small,
            17..=32 => ImageList::Large,
            33..=48 => ImageList::ExtraLarge,
            _ => ImageList::Jumbo,
        }
    }

    /// The `SHIL_*` constant for `SHGetImageList`.
    pub fn shil(self) -> i32 {
        match self {
            ImageList::Large => 0,
            ImageList::Small => 1,
            ImageList::ExtraLarge => 2,
            ImageList::Jumbo => 4,
        }
    }

    /// The edge of the icons in the list, in pixels.
    pub fn edge(self) -> u32 {
        match self {
            ImageList::Small => 16,
            ImageList::Large => 32,
            ImageList::ExtraLarge => 48,
            ImageList::Jumbo => 256,
        }
    }
}

/// Turns the 32-bit colour bitmap of an icon (blue, green, red, alpha per pixel, as `GetDIBits` writes it) into straight RGBA in place.
///
/// Icons made before alpha existed have a colour bitmap whose alpha is all zero and a separate mask (a set bit is transparent); `mask` is that mask as 32-bit pixels, and is used only when the colour bitmap has no alpha at all.
pub fn bgra_to_rgba(pixels: &mut [u8], mask: Option<&[u8]>) {
    let has_alpha = pixels.as_chunks::<4>().0.iter().any(|pixel| pixel[3] != 0);
    for (index, pixel) in pixels.as_chunks_mut::<4>().0.iter_mut().enumerate() {
        pixel.swap(0, 2);
        if !has_alpha {
            let transparent = mask
                .and_then(|mask| mask.get(index * 4))
                .is_some_and(|&bit| bit != 0);
            pixel[3] = if transparent { 0 } else { 255 };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_smallest_list_that_is_big_enough_serves_a_size() {
        let cases = [
            (16, ImageList::Small),
            (17, ImageList::Large),
            (32, ImageList::Large),
            (33, ImageList::ExtraLarge),
            (48, ImageList::ExtraLarge),
            (49, ImageList::Jumbo),
            (512, ImageList::Jumbo),
        ];
        for (pixels, list) in cases {
            assert_eq!(ImageList::for_pixels(pixels), list, "{pixels}");
            assert!(list.edge() >= pixels.min(256), "{pixels}");
        }
    }

    #[test]
    fn the_shell_constants_are_the_documented_ones() {
        assert_eq!(ImageList::Large.shil(), 0);
        assert_eq!(ImageList::Small.shil(), 1);
        assert_eq!(ImageList::ExtraLarge.shil(), 2);
        assert_eq!(ImageList::Jumbo.shil(), 4);
    }

    #[test]
    fn colour_is_reordered_and_alpha_kept() {
        let mut pixels = vec![1, 2, 3, 200, 10, 20, 30, 0];
        bgra_to_rgba(&mut pixels, None);
        assert_eq!(pixels, [3, 2, 1, 200, 30, 20, 10, 0]);
    }

    #[test]
    fn an_icon_with_no_alpha_takes_it_from_the_mask() {
        let mut pixels = vec![1, 2, 3, 0, 4, 5, 6, 0];
        let mask = [0, 0, 0, 0, 255, 255, 255, 0];
        bgra_to_rgba(&mut pixels, Some(&mask));
        assert_eq!(pixels, [3, 2, 1, 255, 6, 5, 4, 0]);
        // With no mask at all it is opaque.
        let mut plain = vec![1, 2, 3, 0];
        bgra_to_rgba(&mut plain, None);
        assert_eq!(plain, [3, 2, 1, 255]);
    }
}
