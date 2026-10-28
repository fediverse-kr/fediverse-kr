//! Library decoding shared by uploaded logos and fetched AP profile images.
//! Animation checks cover the first frame, not every frame of an animation.
use image::{
    codecs::{gif::GifDecoder, jpeg::JpegDecoder, png::PngDecoder, webp::WebPDecoder},
    ImageDecoder, Limits,
};
use std::io::Cursor;

const MAX_IMAGE_SIDE: u32 = 4096;
const HEADER_IMAGE_SIDE: u32 = 8192;
const MAX_IMAGE_AREA: u64 = (MAX_IMAGE_SIDE as u64) * (MAX_IMAGE_SIDE as u64);
const HEADER_IMAGE_AREA: u64 = 32_000_000;
const MAX_IMAGE_BYTES: u64 = 64 * 1024 * 1024;

fn limits(max_side: u32) -> Limits {
    let mut limits = Limits::default();
    limits.max_image_width = Some(max_side);
    limits.max_image_height = Some(max_side);
    limits.max_alloc = Some(MAX_IMAGE_BYTES);
    limits
}

fn raster_dimensions(
    mut decoder: impl ImageDecoder,
    max_side: u32,
    max_area: u64,
) -> Option<(u32, u32)> {
    decoder.set_limits(limits(max_side)).ok()?;
    let (width, height) = decoder.dimensions();
    let area = u64::from(width).checked_mul(u64::from(height))?;
    if width > max_side
        || height > max_side
        || area > max_area
        || decoder.total_bytes() > MAX_IMAGE_BYTES
    {
        return None;
    }
    // Decode under the shared allocation limits: dimensions alone do not prove
    // a compressed remote raster is valid or safe for an image consumer.
    image::DynamicImage::from_decoder(decoder).ok()?;
    Some((width, height))
}

fn dimensions_for_mime(
    mime: &str,
    bytes: &[u8],
    animations: bool,
    max_side: u32,
    max_area: u64,
) -> Option<(u32, u32)> {
    let cursor = Cursor::new(bytes);
    match mime {
        "image/png" => {
            let decoder = PngDecoder::with_limits(cursor, limits(max_side)).ok()?;
            if !animations && decoder.is_apng().ok()? {
                return None;
            }
            raster_dimensions(decoder, max_side, max_area)
        }
        "image/jpeg" => raster_dimensions(JpegDecoder::new(cursor).ok()?, max_side, max_area),
        "image/webp" => {
            let decoder = WebPDecoder::new(cursor).ok()?;
            if !animations && decoder.has_animation() {
                return None;
            }
            raster_dimensions(decoder, max_side, max_area)
        }
        "image/gif" if animations => {
            raster_dimensions(GifDecoder::new(cursor).ok()?, max_side, max_area)
        }
        _ => None,
    }
}

fn area_for_mime(
    mime: &str,
    bytes: &[u8],
    animations: bool,
    max_side: u32,
    max_area: u64,
) -> Option<u64> {
    let (width, height) = dimensions_for_mime(mime, bytes, animations, max_side, max_area)?;
    u64::from(width).checked_mul(u64::from(height))
}

/// Decode a crawler candidate with the shared 4096px/64MiB guard and return
/// its true raster dimensions. Metadata declarations are not trusted.
pub(crate) fn decoded_raster_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    let mime = super::image_mime(bytes)?;
    dimensions_for_mime(mime, bytes, true, MAX_IMAGE_SIDE, MAX_IMAGE_AREA)
}

/// Decode a crawler candidate with the shared 4096px/64MiB guard and return
/// its true raster area. `sizes` metadata is not trustworthy enough to replace
/// this verification.
pub(crate) fn decoded_raster_area(bytes: &[u8]) -> Option<u64> {
    let mime = super::image_mime(bytes)?;
    area_for_mime(mime, bytes, true, MAX_IMAGE_SIDE, MAX_IMAGE_AREA)
}

pub(crate) fn validate(bytes: &[u8], animations: bool) -> Option<&'static str> {
    validate_with_limits(bytes, animations, MAX_IMAGE_SIDE, MAX_IMAGE_AREA)
}

/// Validate catalog media without applying the smaller logo/fetched-image
/// dimensions. Headers may be up to 8192px on an edge and 32 megapixels while
/// preserving the original bytes.
pub(crate) fn validate_catalog(bytes: &[u8], header: bool) -> Option<&'static str> {
    let (max_side, max_area) = if header {
        (HEADER_IMAGE_SIDE, HEADER_IMAGE_AREA)
    } else {
        (MAX_IMAGE_SIDE, MAX_IMAGE_AREA)
    };
    validate_with_limits(bytes, false, max_side, max_area)
}

fn validate_with_limits(
    bytes: &[u8],
    animations: bool,
    max_side: u32,
    max_area: u64,
) -> Option<&'static str> {
    let mime = super::image_mime(bytes)?;
    match mime {
        // Recognized XML only; served as an isolated image, never inline HTML.
        "image/svg+xml" => Some(mime),
        _ => area_for_mime(mime, bytes, animations, max_side, max_area).map(|_| mime),
    }
}
