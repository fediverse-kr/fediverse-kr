//! Image parsing is a library responsibility, not magic-byte-only validation.
//! Byte preservation, SVG-as-image isolation and catalog authorization remain
//! separate. New uploads accept static PNG/JPEG/WebP and image-context SVG.
use super::{
    auth::AuthenticatedSession,
    catalog_editing::Error,
    catalog_moderation,
    db::Database,
    media,
    storage::{ObjectRef, ObjectStore},
};
use crate::moderation::catalog::{LogoRequest, Software, LOGO_MAX_BYTES};
use base64::{engine::general_purpose::STANDARD, Engine};
use std::sync::{Arc, OnceLock};
use tokio::sync::Semaphore;

pub(crate) struct Logo {
    pub object: ObjectRef,
    pub mime: &'static str,
    pub bytes: Vec<u8>,
}
fn validate(encoded: String) -> Result<Logo, Error> {
    if encoded.len() > LOGO_MAX_BYTES.div_ceil(3) * 4 {
        return Err(Error::Invalid);
    }
    let bytes = STANDARD.decode(encoded).map_err(|_| Error::Invalid)?;
    if bytes.is_empty() || bytes.len() > LOGO_MAX_BYTES {
        return Err(Error::Invalid);
    }
    let mime = media::validation::validate(&bytes, false).ok_or(Error::Invalid)?;
    Ok(Logo {
        object: ObjectRef::from_bytes(&bytes).map_err(|_| Error::Invalid)?,
        mime,
        bytes,
    })
}

pub async fn change(
    db: &Database,
    store: Option<&ObjectStore>,
    session: &AuthenticatedSession,
    request: LogoRequest,
) -> Result<Software, Error> {
    let request = catalog_moderation::logo(request)?;
    // Reject non-admin requests before using a decoder. Final auth/CAS happens
    // inside both publication transactions, including after image decoding.
    let before = db.moderation_software(session, &request.name).await?;
    if before.revision != request.revision {
        return Err(Error::Conflict);
    }
    let store = store.ok_or(Error::Unavailable)?.clone();
    static SLOTS: OnceLock<Arc<Semaphore>> = OnceLock::new();
    let permit = SLOTS
        .get_or_init(|| Arc::new(Semaphore::new(2)))
        .clone()
        .try_acquire_owned()
        .map_err(|_| Error::RateLimited)?;
    let db = db.clone();
    let session = session.clone();
    // Connection cancellation must not release capacity while decoding or file
    // publication continues. No detached timeout can acknowledge success.
    tokio::spawn(async move {
        let _permit = permit;
        let mut request = request;
        let logo = match request.data.take() {
            Some(encoded) => Some(
                tokio::task::spawn_blocking(move || validate(encoded))
                    .await
                    .map_err(|_| Error::Unavailable)??,
            ),
            None => None,
        };
        db.moderate_software_logo(&session, &store, request, logo)
            .await
    })
    .await
    .map_err(|_| Error::Unavailable)?
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    #[test]
    fn static_rasters_are_decoded_and_original_bytes_are_kept() {
        for format in [
            image::ImageFormat::Png,
            image::ImageFormat::Jpeg,
            image::ImageFormat::WebP,
        ] {
            let image = image::DynamicImage::ImageRgb8(image::RgbImage::new(2, 3));
            let mut bytes = Cursor::new(Vec::new());
            image.write_to(&mut bytes, format).unwrap();
            let bytes = bytes.into_inner();
            let logo = validate(STANDARD.encode(&bytes)).unwrap();
            assert_eq!(logo.bytes, bytes);
            logo.object.matches(&logo.bytes).unwrap();
            assert!(validate(STANDARD.encode(&bytes[..bytes.len() / 2])).is_err());
        }
    }
    #[test]
    fn rejects_size_bombs_fake_headers_and_unsupported_types() {
        for bytes in [
            b"\x89PNG\r\n\x1a\n".as_slice(),
            b"\xff\xd8\xfffake",
            b"GIF89a",
            b"<html>logo</html>",
            b"",
        ] {
            assert!(validate(STANDARD.encode(bytes)).is_err());
        }
        assert!(validate("!not-base64!".into()).is_err());
        assert!(validate(STANDARD.encode(vec![0; LOGO_MAX_BYTES + 1])).is_err());
        let image = image::DynamicImage::ImageRgb8(image::RgbImage::new(4097, 1));
        let mut bytes = Cursor::new(Vec::new());
        image.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
        assert!(validate(STANDARD.encode(bytes.into_inner())).is_err());
        let svg = b"<svg xmlns='http://www.w3.org/2000/svg'><path d='M0 0'/></svg>";
        assert_eq!(validate(STANDARD.encode(svg)).unwrap().bytes, svg);
    }
}
