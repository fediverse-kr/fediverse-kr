//! Authorization surrounds IO; a file hash alone is never a public capability.
use super::{
    db::Database,
    storage::{ObjectRef, ObjectStore},
};
use std::sync::{Arc, OnceLock};
use tokio::sync::Semaphore;
mod svg;
pub(crate) mod validation;

#[derive(Clone)]
pub enum Target {
    Software(String),
    Site(String),
    OwnAvatar,
    OwnEmoji(String),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Unauthorized,
    Unavailable,
    Unsupported,
    Busy,
}
pub struct Image {
    pub mime: &'static str,
    pub bytes: Vec<u8>,
}

pub async fn read(
    db: &Database,
    store: Option<&ObjectStore>,
    target: &Target,
    token: Option<&str>,
) -> Result<Option<Image>, Error> {
    read_with(db, target, token, |reference| async move {
        let store = store.ok_or(Error::Unavailable)?.clone();
        static SLOTS: OnceLock<Arc<Semaphore>> = OnceLock::new();
        let permit = SLOTS
            .get_or_init(|| Arc::new(Semaphore::new(4)))
            .clone()
            .try_acquire_owned()
            .map_err(|_| Error::Busy)?;
        // Cancellation of HTTP must not release the slot while Fs IO still runs.
        tokio::spawn(async move {
            let _permit = permit;
            store.read(&reference).await
        })
        .await
        .map_err(|_| Error::Unavailable)?
        .map_err(|_| Error::Unavailable)
    })
    .await
}

// The storage boundary also allows future remote storage and deterministic IO
// races in tests. Authorization remains here, not inside the byte reader.
pub(crate) async fn read_with<F, Fut>(
    db: &Database,
    target: &Target,
    token: Option<&str>,
    load: F,
) -> Result<Option<Image>, Error>
where
    F: FnOnce(ObjectRef) -> Fut,
    Fut: std::future::Future<Output = Result<Vec<u8>, Error>>,
{
    let Some(reference) = db.media_reference(target, token).await? else {
        return Ok(None);
    };
    let bytes = load(reference.clone()).await?;
    // Recheck deletion, hiding, logout/ban and changed references after file IO.
    if db.media_reference(target, token).await?.as_ref() != Some(&reference) {
        return Ok(None);
    }
    let mime = image_mime(&bytes).ok_or(Error::Unsupported)?;
    Ok(Some(Image { mime, bytes }))
}

pub fn image_mime(bytes: &[u8]) -> Option<&'static str> {
    if let Some(mime) = super::crawler::image_mime(bytes) {
        return Some(mime);
    }
    svg::recognize(bytes).then_some("image/svg+xml")
}

pub fn path_component(value: &str) -> Option<String> {
    let name = decode_component(value, 128)?;
    if name.contains(['/', '\\']) {
        None
    } else {
        Some(name)
    }
}

/// Phoenix retained the original ActivityPub shortcode as the JSON key while
/// sanitizing only its private storage filename. This is not a file path:
/// after decoding it is used solely as an exact, authorized JSON lookup.
pub fn emoji_component(value: &str) -> Option<String> {
    let name = decode_component(value, EMOJI_NAME_MAX_BYTES)?;
    emoji_name(&name).then_some(name)
}

/// Canonical private emoji URLs use a query so literal `.`/`..` and `/` names
/// cannot be changed by URL path normalization. The old path form stays an
/// alias for already-issued simple shortcode URLs.
pub fn emoji_query(value: &str) -> Option<String> {
    const NAME_PREFIX: usize = "name=".len();
    const REVISION_MAX: usize = 20;
    if value.len() > NAME_PREFIX + EMOJI_NAME_MAX_BYTES * 3 + 1 + "v=".len() + REVISION_MAX {
        return None;
    }
    // Reject lossy/malformed input before the URL library applies standard
    // form decoding. The validation result is not parsed or decoded again.
    if !valid_percent_encoding(value)
        || percent_encoding::percent_decode_str(value)
            .decode_utf8()
            .is_err()
    {
        return None;
    }
    let mut name = None;
    let mut revision = false;
    for (key, value) in url::form_urlencoded::parse(value.as_bytes()) {
        match key.as_ref() {
            "name" if name.is_none() => name = Some(value.into_owned()),
            "v" if !revision
                && !value.is_empty()
                && value.len() <= REVISION_MAX
                && value.bytes().all(|b| b.is_ascii_digit()) =>
            {
                revision = true
            }
            _ => return None,
        }
    }
    name.filter(|name| emoji_name(name))
}

fn decode_component(value: &str, max_bytes: usize) -> Option<String> {
    if value.len() > max_bytes * 3 {
        return None;
    }
    // The standard decoder leaves malformed escapes literal. Our route policy
    // is stricter: reject them before decoding, but do not reimplement decoding.
    if !valid_percent_encoding(value) {
        return None;
    }
    let name = percent_encoding::percent_decode_str(value)
        .decode_utf8()
        .ok()?
        .into_owned();
    if !valid_name(&name, max_bytes) {
        None
    } else {
        Some(name)
    }
}

fn valid_percent_encoding(value: &str) -> bool {
    for (index, byte) in value.bytes().enumerate() {
        if byte == b'%'
            && !value
                .as_bytes()
                .get(index + 1..index + 3)
                .is_some_and(|s| s.iter().all(u8::is_ascii_hexdigit))
        {
            return false;
        }
    }
    true
}

pub fn emoji_name(name: &str) -> bool {
    valid_name(name, EMOJI_NAME_MAX_BYTES)
}

pub const EMOJI_NAME_MAX_BYTES: usize = 1024;

fn valid_name(name: &str, max_bytes: usize) -> bool {
    !name.is_empty() && name.len() <= max_bytes && !name.chars().any(char::is_control)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn svg_is_recognized_without_treating_html_or_entities_as_an_image() {
        for svg in [
            "<svg xmlns='http://www.w3.org/2000/svg'></svg>",
            "\u{feff}<?xml version='1.0'?> <!--logo--> <svg onload='alert(1)'></svg>",
        ] {
            assert_eq!(image_mime(svg.as_bytes()), Some("image/svg+xml"));
        }
        for other in [
            "<html><svg></svg></html>",
            "<svgscript>",
            "<!DOCTYPE svg><svg/>",
            "<svg><!ENTITY x SYSTEM 'file:///secret'></svg>",
            "not a file",
        ] {
            assert_eq!(image_mime(other.as_bytes()), None);
        }
    }
    #[test]
    fn media_routes_decode_safe_components_without_treating_emoji_names_as_paths() {
        assert_eq!(
            path_component("test%20software").as_deref(),
            Some("test software")
        );
        assert_eq!(path_component("a+b").as_deref(), Some("a+b"));
        assert_eq!(
            path_component("%EC%97%B0%ED%95%A9%EC%9A%B0%EC%A3%BC").as_deref(),
            Some("연합우주")
        );
        assert_eq!(path_component("a%25b").as_deref(), Some("a%b"));
        for input in [
            "", "a%2fb", "%00", "%zz", "%ff", "../a", "a\\b", "%", "%2", "%z0", "a%%20",
        ] {
            assert!(path_component(input).is_none());
        }
        for (encoded, name) in [
            ("Blob_cat2", "Blob_cat2"),
            (
                "blob-cat.%40%2F%ED%95%9C%EA%B5%AD%EC%96%B4",
                "blob-cat.@/한국어",
            ),
            ("..", ".."),
        ] {
            assert_eq!(emoji_component(encoded).as_deref(), Some(name));
        }
        let too_long = "a".repeat(EMOJI_NAME_MAX_BYTES + 1);
        for name in ["", "%00", "%zz", "a\nname", &too_long] {
            assert!(emoji_component(name).is_none());
        }
    }

    #[test]
    fn canonical_emoji_query_keeps_legacy_logical_names_and_bounds_encoding() {
        let encoded = "name=blob-cat.%40%2F%ED%95%9C%EA%B5%AD%EC%96%B4&v=7";
        assert_eq!(emoji_query(encoded).as_deref(), Some("blob-cat.@/한국어"));
        assert_eq!(emoji_query("name=..").as_deref(), Some(".."));
        assert_eq!(emoji_query("name=%252F").as_deref(), Some("%2F"));
        for query in [
            "",
            "name=wave&name=again",
            "name=wave&other=1",
            "name=wave&v=-1",
            "name=wave&v=",
            "name=%zz",
            "name=%ff",
        ] {
            assert!(emoji_query(query).is_none());
        }
        assert!(emoji_query(&format!("name={}", "a".repeat(EMOJI_NAME_MAX_BYTES + 1))).is_none());
        assert!(emoji_query(&format!("name={}", "%41".repeat(EMOJI_NAME_MAX_BYTES + 1))).is_none());
    }
}
