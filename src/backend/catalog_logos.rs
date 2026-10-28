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
use crate::{
    directory::SoftwareMediaMetadata,
    moderation::catalog::{
        LogoRequest, MediaRequest, MediaRole, Software, HEADER_MAX_BYTES, LOGO_MAX_BYTES,
    },
};
use base64::{engine::general_purpose::STANDARD, Engine};
use std::sync::{Arc, OnceLock};
use tokio::sync::Semaphore;

pub(crate) struct Logo {
    pub object: ObjectRef,
    pub mime: &'static str,
    pub bytes: Vec<u8>,
}

fn validate(encoded: String) -> Result<Logo, Error> {
    validate_role(encoded, false)
}

pub(crate) fn validate_role(encoded: String, header: bool) -> Result<Logo, Error> {
    let max = if header {
        HEADER_MAX_BYTES
    } else {
        LOGO_MAX_BYTES
    };
    if encoded.len() > max.div_ceil(3) * 4 {
        return Err(Error::Invalid);
    }
    let bytes = STANDARD.decode(encoded).map_err(|_| Error::Invalid)?;
    if bytes.is_empty() || bytes.len() > max {
        return Err(Error::Invalid);
    }
    let mime = media::validation::validate_catalog(&bytes, header).ok_or(Error::Invalid)?;
    Ok(Logo {
        object: ObjectRef::from_bytes(&bytes).map_err(|_| Error::Invalid)?,
        mime,
        bytes,
    })
}

fn bounded_text(value: &str, max: usize) -> bool {
    value.chars().count() <= max && !value.chars().any(|c| c.is_control())
}

pub(crate) fn validate_metadata(
    metadata: Option<SoftwareMediaMetadata>,
    required: bool,
) -> Result<Option<SoftwareMediaMetadata>, Error> {
    let Some(mut metadata) = metadata else {
        return if required {
            Err(Error::Invalid)
        } else {
            Ok(None)
        };
    };
    metadata.source_url = metadata
        .source_url
        .map(|value| crate::directory::web_link(value.trim()).ok_or(Error::Invalid))
        .transpose()?;
    metadata.license_url = metadata
        .license_url
        .map(|value| crate::directory::web_link(value.trim()).ok_or(Error::Invalid))
        .transpose()?;
    metadata.attribution = metadata.attribution.trim().to_owned();
    metadata.license = metadata.license.trim().to_owned();
    metadata.caption = metadata.caption.trim().to_owned();
    if !bounded_text(&metadata.attribution, 500)
        || !bounded_text(&metadata.license, 2000)
        || !bounded_text(&metadata.caption, 1000)
        || metadata.attribution.is_empty()
        || metadata.license.is_empty()
        || metadata
            .license_url
            .as_deref()
            .is_some_and(|v| !bounded_text(v, 2048))
        || metadata
            .source_url
            .as_deref()
            .is_some_and(|v| !bounded_text(v, 2048))
        || metadata.background.as_deref().is_some_and(|value| {
            value.len() != 7
                || !value.starts_with('#')
                || !value.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit)
        })
    {
        return Err(Error::Invalid);
    }
    Ok(Some(metadata))
}

pub async fn change(
    db: &Database,
    store: Option<&ObjectStore>,
    session: &AuthenticatedSession,
    request: LogoRequest,
) -> Result<Software, Error> {
    let request = catalog_moderation::logo(request)?;
    change_media(
        db,
        store,
        session,
        MediaRequest {
            name: request.name,
            revision: request.revision,
            role: MediaRole::Logo,
            note: request.note,
            data: request.data,
            metadata: request.metadata,
        },
    )
    .await
}

pub async fn change_media(
    db: &Database,
    store: Option<&ObjectStore>,
    session: &AuthenticatedSession,
    request: MediaRequest,
) -> Result<Software, Error> {
    let mut request = catalog_moderation::media(request)?;
    let header = request.role == MediaRole::Header;
    request.metadata = validate_metadata(request.metadata, false)?;
    let store = store.ok_or(Error::Unavailable)?.clone();
    static SLOTS: OnceLock<Arc<Semaphore>> = OnceLock::new();
    let permit = SLOTS
        .get_or_init(|| Arc::new(Semaphore::new(2)))
        .clone()
        .try_acquire_owned()
        .map_err(|_| Error::RateLimited)?;
    let db = db.clone();
    let session = session.clone();
    tokio::spawn(async move {
        let _permit = permit;
        let media = match request.data.take() {
            Some(encoded) => Some(
                tokio::task::spawn_blocking(move || validate_role(encoded, header))
                    .await
                    .map_err(|_| Error::Unavailable)??,
            ),
            None => None,
        };
        db.moderate_software_media(&session, &store, request, media)
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
    #[test]
    fn header_validation_allows_the_existing_large_screenshot_limit() {
        let image = image::DynamicImage::ImageRgb8(image::RgbImage::new(5760, 3240));
        let mut bytes = Cursor::new(Vec::new());
        image
            .write_to(&mut bytes, image::ImageFormat::Jpeg)
            .unwrap();
        assert!(validate_role(STANDARD.encode(bytes.into_inner()), true).is_ok());
    }
}
