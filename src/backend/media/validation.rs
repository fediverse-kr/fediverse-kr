//! Library decoding shared by uploaded logos and fetched AP profile images.
//! Animation checks cover the first frame, not every frame of an animation.
use image::{
    codecs::{gif::GifDecoder, jpeg::JpegDecoder, png::PngDecoder, webp::WebPDecoder},
    ImageDecoder, Limits,
};
use std::io::Cursor;

const MAX_IMAGE_SIDE: u32 = 4096;
const MAX_IMAGE_BYTES: u64 = 64 * 1024 * 1024;

fn limits() -> Limits {
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_IMAGE_SIDE);
    limits.max_image_height = Some(MAX_IMAGE_SIDE);
    limits.max_alloc = Some(MAX_IMAGE_BYTES);
    limits
}

fn raster_area(mut decoder: impl ImageDecoder) -> Option<u64> {
    decoder.set_limits(limits()).ok()?;
    let (width, height) = decoder.dimensions();
    if width > MAX_IMAGE_SIDE || height > MAX_IMAGE_SIDE || decoder.total_bytes() > MAX_IMAGE_BYTES
    {
        return None;
    }
    // Decode under the shared allocation limits: dimensions alone do not prove
    // a compressed remote raster is valid or safe for an image consumer.
    image::DynamicImage::from_decoder(decoder).ok()?;
    u64::from(width).checked_mul(u64::from(height))
}

fn area_for_mime(mime: &str, bytes: &[u8], animations: bool) -> Option<u64> {
    let cursor = Cursor::new(bytes);
    match mime {
        "image/png" => {
            let decoder = PngDecoder::with_limits(cursor, limits()).ok()?;
            if !animations && decoder.is_apng().ok()? {
                return None;
            }
            raster_area(decoder)
        }
        "image/jpeg" => raster_area(JpegDecoder::new(cursor).ok()?),
        "image/webp" => {
            let decoder = WebPDecoder::new(cursor).ok()?;
            if !animations && decoder.has_animation() {
                return None;
            }
            raster_area(decoder)
        }
        "image/gif" if animations => raster_area(GifDecoder::new(cursor).ok()?),
        _ => None,
    }
}

/// Decode a crawler candidate with the shared 4096px/64MiB guard and return
/// its true raster area. `sizes` metadata is not trustworthy enough to replace
/// this verification.
pub(crate) fn decoded_raster_area(bytes: &[u8]) -> Option<u64> {
    let mime = super::image_mime(bytes)?;
    area_for_mime(mime, bytes, true)
}

pub(crate) fn validate(bytes: &[u8], animations: bool) -> Option<&'static str> {
    let mime = super::image_mime(bytes)?;
    match mime {
        // Recognized XML only; served as an isolated image, never inline HTML.
        "image/svg+xml" => Some(mime),
        _ => area_for_mime(mime, bytes, animations).map(|_| mime),
    }
}
