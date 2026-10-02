// The built-in generator: decodes common image formats (a JPEG at a reduced scale) and resizes them to fit a thumbnail
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::fs::File;
use std::io::{Cursor, Read};
use std::path::Path;

use fast_image_resize::images::Image;
use fast_image_resize::{FilterType, MulDiv, PixelType, ResizeAlg, ResizeOptions, Resizer};
use image::{DynamicImage, ImageDecoder, ImageFormat};

/// The most pixels the generator will decode, whatever the file size (a small file can still expand to a huge image).
const MAX_PIXELS: u64 = 256_000_000;
/// The most memory an image decoder may allocate.
const MAX_ALLOC: u64 = 1 << 30;

/// Pixels ready to be encoded as a PNG.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rendered {
    pub width: u32,
    pub height: u32,
    /// `Rgb` or `Rgba`, eight bits a channel.
    pub color: png::ColorType,
    pub pixels: Vec<u8>,
}

/// Why the built-in generator made nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GenError {
    /// The file (or the image in it) is larger than the limit for decoding.
    TooLarge,
    /// The file is not a format the generator decodes.
    Unsupported,
    Failed(String),
}

impl GenError {
    fn failed(error: impl std::fmt::Display) -> Self {
        GenError::Failed(error.to_string())
    }
}

/// The formats the generator decodes, found from the file's first bytes and never from its name.
pub fn detect(header: &[u8]) -> Option<ImageFormat> {
    match image::guess_format(header).ok()? {
        format @ (ImageFormat::Png
        | ImageFormat::Jpeg
        | ImageFormat::Gif
        | ImageFormat::WebP
        | ImageFormat::Bmp
        | ImageFormat::Tiff
        | ImageFormat::Ico) => Some(format),
        _ => None,
    }
}

/// The MIME types the generator handles, which the platform checks before it asks an external thumbnailer.
pub fn handles_mime(mime: &str) -> bool {
    matches!(
        mime,
        "image/png"
            | "image/jpeg"
            | "image/gif"
            | "image/webp"
            | "image/bmp"
            | "image/x-bmp"
            | "image/x-ms-bmp"
            | "image/tiff"
            | "image/x-icon"
            | "image/vnd.microsoft.icon"
    )
}

/// Decodes the image at `path` and fits it within `max_side` pixels on each side, without scaling it up. Never reads a file larger than `max_file_bytes`.
pub fn generate(path: &Path, max_side: u32, max_file_bytes: u64) -> Result<Rendered, GenError> {
    let mut file = File::open(path).map_err(GenError::failed)?;
    let len = file.metadata().map_err(GenError::failed)?.len();
    if len > max_file_bytes {
        return Err(GenError::TooLarge);
    }
    let mut bytes = Vec::with_capacity(len as usize);
    // `take` keeps a file that grew since the check inside the limit.
    (&mut file)
        .take(max_file_bytes + 1)
        .read_to_end(&mut bytes)
        .map_err(GenError::failed)?;
    if bytes.len() as u64 > max_file_bytes {
        return Err(GenError::TooLarge);
    }
    generate_from_bytes(&bytes, max_side)
}

/// Like [`generate`], over bytes already read.
pub fn generate_from_bytes(bytes: &[u8], max_side: u32) -> Result<Rendered, GenError> {
    let format = detect(&bytes[..bytes.len().min(32)]).ok_or(GenError::Unsupported)?;
    let image = if format == ImageFormat::Jpeg {
        decode_jpeg(bytes, max_side)?
    } else {
        decode_with_image(bytes, format)?
    };
    fit(image, max_side)
}

fn limits() -> image::Limits {
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(MAX_ALLOC);
    limits
}

fn decode_with_image(bytes: &[u8], format: ImageFormat) -> Result<DynamicImage, GenError> {
    let mut decoder = image::ImageReader::with_format(Cursor::new(bytes), format)
        .into_decoder()
        .map_err(GenError::failed)?;
    let (w, h) = decoder.dimensions();
    if u64::from(w) * u64::from(h) > MAX_PIXELS {
        return Err(GenError::TooLarge);
    }
    decoder.set_limits(limits()).map_err(GenError::failed)?;
    let orientation = decoder
        .orientation()
        .unwrap_or(image::metadata::Orientation::NoTransforms);
    let mut image = DynamicImage::from_decoder(decoder).map_err(GenError::failed)?;
    image.apply_orientation(orientation);
    Ok(image)
}

/// Decodes a JPEG at the smallest scale (1, 1/2, 1/4 or 1/8) that is still at least as large as the thumbnail, which skips most of the work for a large photo. A JPEG the scaled decoder cannot read (CMYK, say) goes to the general decoder.
fn decode_jpeg(bytes: &[u8], max_side: u32) -> Result<DynamicImage, GenError> {
    match decode_jpeg_scaled(bytes, max_side) {
        Ok(Some(image)) => Ok(image),
        Ok(None) | Err(GenError::Failed(_)) => decode_with_image(bytes, ImageFormat::Jpeg),
        Err(other) => Err(other),
    }
}

fn decode_jpeg_scaled(bytes: &[u8], max_side: u32) -> Result<Option<DynamicImage>, GenError> {
    let mut decoder = jpeg_decoder::Decoder::new(Cursor::new(bytes));
    decoder.read_info().map_err(GenError::failed)?;
    let info = decoder
        .info()
        .ok_or_else(|| GenError::failed("no JPEG header"))?;
    let (w, h) = (u32::from(info.width), u32::from(info.height));
    if u64::from(w) * u64::from(h) > MAX_PIXELS {
        return Err(GenError::TooLarge);
    }
    let (tw, th) = fit_dimensions(w, h, max_side);
    decoder
        .scale(tw.max(1) as u16, th.max(1) as u16)
        .map_err(GenError::failed)?;
    let pixels = decoder.decode().map_err(GenError::failed)?;
    let info = decoder
        .info()
        .ok_or_else(|| GenError::failed("no JPEG header"))?;
    let (w, h) = (u32::from(info.width), u32::from(info.height));
    let image = match info.pixel_format {
        jpeg_decoder::PixelFormat::RGB24 => {
            image::RgbImage::from_raw(w, h, pixels).map(DynamicImage::ImageRgb8)
        }
        jpeg_decoder::PixelFormat::L8 => {
            image::GrayImage::from_raw(w, h, pixels).map(DynamicImage::ImageLuma8)
        }
        _ => None,
    };
    let Some(mut image) = image else {
        return Ok(None);
    };
    // The orientation is in the Exif block, which the general decoder reads from the header alone.
    if let Ok(mut reader) = image::codecs::jpeg::JpegDecoder::new(Cursor::new(bytes)) {
        if let Ok(orientation) = reader.orientation() {
            image.apply_orientation(orientation);
        }
    }
    Ok(Some(image))
}

/// The size that fits `w` by `h` within `max_side` on each side, never larger than the original.
pub fn fit_dimensions(w: u32, h: u32, max_side: u32) -> (u32, u32) {
    let longest = w.max(h);
    if longest <= max_side || longest == 0 {
        return (w, h);
    }
    let scale = |side: u32| {
        ((u64::from(side) * u64::from(max_side) + u64::from(longest) / 2) / u64::from(longest))
            .max(1) as u32
    };
    (scale(w), scale(h))
}

/// Resizes to fit and converts to eight-bit RGB (or RGBA when the image has transparency).
fn fit(image: DynamicImage, max_side: u32) -> Result<Rendered, GenError> {
    let (w, h) = (image.width(), image.height());
    if w == 0 || h == 0 {
        return Err(GenError::failed("the image is empty"));
    }
    let (tw, th) = fit_dimensions(w, h, max_side);
    let alpha = image.color().has_alpha();
    let (pixel_type, color, buffer) = if alpha {
        (
            PixelType::U8x4,
            png::ColorType::Rgba,
            image.into_rgba8().into_raw(),
        )
    } else {
        (
            PixelType::U8x3,
            png::ColorType::Rgb,
            image.into_rgb8().into_raw(),
        )
    };
    if (tw, th) == (w, h) {
        return Ok(Rendered {
            width: w,
            height: h,
            color,
            pixels: buffer,
        });
    }
    let mut source = Image::from_vec_u8(w, h, buffer, pixel_type).map_err(GenError::failed)?;
    let mut target = Image::new(tw, th, pixel_type);
    let mul_div = MulDiv::default();
    // Colours are blended with their weight, or a transparent edge bleeds its hidden colour in.
    if alpha {
        mul_div
            .multiply_alpha_inplace(&mut source)
            .map_err(GenError::failed)?;
    }
    let options = ResizeOptions::new().resize_alg(ResizeAlg::Convolution(FilterType::Lanczos3));
    Resizer::new()
        .resize(&source, &mut target, &options)
        .map_err(GenError::failed)?;
    if alpha {
        mul_div
            .divide_alpha_inplace(&mut target)
            .map_err(GenError::failed)?;
    }
    Ok(Rendered {
        width: tw,
        height: th,
        color,
        pixels: target.into_vec(),
    })
}

#[cfg(test)]
mod tests;
