// Tests the built-in generator on small images it encodes itself
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;
use image::{ImageEncoder, Rgb, RgbImage, Rgba, RgbaImage};

fn jpeg(w: u32, h: u32) -> Vec<u8> {
    let image = RgbImage::from_fn(w, h, |x, y| Rgb([(x % 256) as u8, (y % 256) as u8, 128]));
    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 80)
        .write_image(image.as_raw(), w, h, image::ExtendedColorType::Rgb8)
        .unwrap();
    out
}

fn encode(image: &DynamicImage, format: ImageFormat) -> Vec<u8> {
    let mut out = Cursor::new(Vec::new());
    image.write_to(&mut out, format).unwrap();
    out.into_inner()
}

#[test]
fn dimensions_fit_without_scaling_up() {
    assert_eq!(fit_dimensions(4000, 3000, 256), (256, 192));
    assert_eq!(fit_dimensions(3000, 4000, 256), (192, 256));
    assert_eq!(fit_dimensions(100, 50, 256), (100, 50));
    assert_eq!(fit_dimensions(10_000, 1, 256), (256, 1));
}

#[test]
fn a_jpeg_is_scaled_to_fit() {
    let out = generate_from_bytes(&jpeg(1600, 1200), 128).unwrap();
    assert_eq!((out.width, out.height), (128, 96));
    assert_eq!(out.color, png::ColorType::Rgb);
    assert_eq!(out.pixels.len(), 128 * 96 * 3);
}

#[test]
fn a_small_image_is_not_scaled_up() {
    let out = generate_from_bytes(&jpeg(40, 30), 256).unwrap();
    assert_eq!((out.width, out.height), (40, 30));
}

#[test]
fn a_png_with_transparency_keeps_its_alpha() {
    let image = DynamicImage::ImageRgba8(RgbaImage::from_fn(400, 200, |x, _| {
        if x < 200 {
            Rgba([255, 0, 0, 255])
        } else {
            Rgba([0, 0, 255, 0])
        }
    }));
    let out = generate_from_bytes(&encode(&image, ImageFormat::Png), 128).unwrap();
    assert_eq!((out.width, out.height), (128, 64));
    assert_eq!(out.color, png::ColorType::Rgba);
    assert_eq!(&out.pixels[..4], &[255, 0, 0, 255]);
    let last = out.pixels.len() - 4;
    assert_eq!(out.pixels[last + 3], 0);
}

#[test]
fn every_decodable_format_is_recognised_by_content() {
    let image = DynamicImage::ImageRgb8(RgbImage::from_pixel(32, 16, Rgb([9, 99, 199])));
    for format in [
        ImageFormat::Png,
        ImageFormat::Gif,
        ImageFormat::Bmp,
        ImageFormat::Tiff,
        ImageFormat::Ico,
        ImageFormat::Jpeg,
    ] {
        let bytes = if matches!(format, ImageFormat::Ico | ImageFormat::Gif) {
            encode(&DynamicImage::ImageRgba8(image.to_rgba8()), format)
        } else {
            encode(&image, format)
        };
        let out = generate_from_bytes(&bytes, 128).unwrap_or_else(|e| panic!("{format:?}: {e:?}"));
        assert_eq!((out.width, out.height), (32, 16), "{format:?}");
    }
}

#[test]
fn text_and_garbage_are_unsupported_whatever_the_name() {
    assert_eq!(
        generate_from_bytes(b"hello, world", 128),
        Err(GenError::Unsupported)
    );
    assert_eq!(generate_from_bytes(b"", 128), Err(GenError::Unsupported));
}

#[test]
fn a_truncated_image_fails_rather_than_panics() {
    let bytes = jpeg(800, 600);
    assert!(matches!(
        generate_from_bytes(&bytes[..bytes.len() / 3], 128),
        Err(GenError::Failed(_)) | Ok(_)
    ));
    let png = encode(
        &DynamicImage::ImageRgb8(RgbImage::new(64, 64)),
        ImageFormat::Png,
    );
    assert!(matches!(
        generate_from_bytes(&png[..40], 128),
        Err(GenError::Failed(_))
    ));
}

#[test]
fn a_file_over_the_cap_is_never_read() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("big.jpg");
    std::fs::write(&path, jpeg(64, 64)).unwrap();
    assert_eq!(generate(&path, 128, 10), Err(GenError::TooLarge));
    assert!(generate(&path, 128, 1 << 20).is_ok());
    assert!(matches!(
        generate(&tmp.path().join("missing.jpg"), 128, 1 << 20),
        Err(GenError::Failed(_))
    ));
}

#[test]
fn mime_types_the_generator_handles() {
    assert!(handles_mime("image/jpeg"));
    assert!(handles_mime("image/vnd.microsoft.icon"));
    assert!(!handles_mime("image/heic"));
    assert!(!handles_mime("application/pdf"));
}
