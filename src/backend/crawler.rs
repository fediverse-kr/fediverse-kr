//! Bounded unsigned public-site reads. Authentication remains AP-only and signed.
use super::{
    directory::{canonical_domain, CrawlJob, Icon, NodeInfo, Observation},
    federation::transport::{pinned_client, safe_url},
};
use chrono::Utc;
use serde_json::Value;
use std::{
    cell::RefCell,
    collections::HashSet,
    future::Future,
    pin::Pin,
    time::{Duration, Instant},
};
use url::Url;

pub const PAGE_LIMIT: usize = 256 * 1024;
pub const ICON_LIMIT: usize = 512 * 1024;
pub type FetchFuture<'a> = Pin<Box<dyn Future<Output = Result<Reply, &'static str>> + Send + 'a>>;
#[derive(Clone, Debug)]
pub struct Reply {
    pub url: Url,
    pub status: u16,
    pub body: Result<Vec<u8>, &'static str>,
}
pub trait SiteTransport: Send + Sync {
    fn get<'a>(&'a self, url: &'a Url, max_bytes: usize) -> FetchFuture<'a>;
}
pub struct PublicHttp;
impl SiteTransport for PublicHttp {
    fn get<'a>(&'a self, input: &'a Url, cap: usize) -> FetchFuture<'a> {
        Box::pin(async move {
            tokio::time::timeout(Duration::from_secs(10), async {
                let mut url = safe_url(input.as_str()).map_err(|_| "unsafe_url")?;
                for redirect in 0..=3 {
                    let client = pinned_client(&url)
                        .await
                        .map_err(|_| "unsafe_or_unreachable_host")?;
                    let mut response = client
                        .get(url.clone())
                        .header("user-agent", "fediverse.kr/2 site-directory")
                        .header("accept", "application/json, text/html, image/*;q=0.8")
                        .send()
                        .await
                        .map_err(|_| "network_error")?;
                    let status = response.status().as_u16();
                    if [301, 302, 303, 307, 308].contains(&status) {
                        if redirect == 3 {
                            return Err("redirect_limit");
                        }
                        let location = response
                            .headers()
                            .get("location")
                            .and_then(|v| v.to_str().ok())
                            .ok_or("invalid_redirect")?;
                        url =
                            safe_url(url.join(location).map_err(|_| "invalid_redirect")?.as_str())
                                .map_err(|_| "unsafe_redirect")?;
                        continue;
                    }
                    // Health is the HTTP response, not whether its page body fits.
                    let mut body = Ok(Vec::new());
                    if response.content_length().is_some_and(|n| n > cap as u64) {
                        body = Err("body_too_large");
                    } else {
                        loop {
                            match response.chunk().await {
                                Ok(Some(chunk)) => {
                                    let bytes = body.as_mut().expect("body read not failed");
                                    if bytes.len().saturating_add(chunk.len()) > cap {
                                        body = Err("body_too_large");
                                        break;
                                    }
                                    bytes.extend_from_slice(&chunk);
                                }
                                Ok(None) => break,
                                Err(_) => {
                                    body = Err("body_read_error");
                                    break;
                                }
                            }
                        }
                    }
                    return Ok(Reply { url, status, body });
                }
                Err("redirect_limit")
            })
            .await
            .map_err(|_| "request_timeout")?
        })
    }
}
fn json(reply: Reply) -> Result<Value, &'static str> {
    if reply.status != 200 {
        return Err("nodeinfo_http_status");
    }
    serde_json::from_slice(&reply.body?).map_err(|_| "invalid_json")
}
fn text(value: &Value, max: usize) -> Option<String> {
    let value = value.as_str()?.trim();
    (!value.is_empty() && value.len() <= max && !value.chars().any(|c| c == '\0'))
        .then(|| value.to_owned())
}
pub(crate) fn parse_nodeinfo(v: &Value) -> Result<NodeInfo, &'static str> {
    if !v.is_object()
        || !matches!(
            v.get("version").and_then(Value::as_str),
            Some("2.0" | "2.1")
        )
    {
        return Err("unsupported_nodeinfo");
    }
    let count = |path: &str| {
        v.pointer(path)
            .and_then(Value::as_u64)
            .and_then(|v| i64::try_from(v).ok())
    };
    Ok(NodeInfo {
        software: text(&v["software"]["name"], 128),
        version: text(&v["software"]["version"], 256),
        name: text(&v["metadata"]["nodeName"], 1024),
        description: text(&v["metadata"]["nodeDescription"], 8192),
        registration_open: v["openRegistrations"].as_bool(),
        users: count("/usage/users/total"),
        active_users: count("/usage/users/activeMonth"),
        posts: count("/usage/localPosts"),
    })
}
async fn nodeinfo<T: SiteTransport>(transport: &T, base: &Url) -> Result<NodeInfo, &'static str> {
    let discovery = json(
        transport
            .get(
                &base
                    .join("/.well-known/nodeinfo")
                    .map_err(|_| "invalid_discovery")?,
                PAGE_LIMIT,
            )
            .await?,
    )?;
    let links = discovery["links"].as_array().ok_or("no_nodeinfo_link")?;
    for version in ["2.1", "2.0"] {
        let rel = format!("http://nodeinfo.diaspora.software/ns/schema/{version}");
        if let Some(link) = links.iter().find(|l| l["rel"].as_str() == Some(&rel)) {
            let url = safe_url(link["href"].as_str().ok_or("invalid_nodeinfo_link")?)
                .map_err(|_| "unsafe_nodeinfo_link")?;
            return parse_nodeinfo(&json(transport.get(&url, PAGE_LIMIT).await?)?);
        }
    }
    Err("no_nodeinfo_link")
}
const MANIFEST_LIMIT: usize = 64 * 1024;
const ICON_CANDIDATE_LIMIT: usize = 8;
const SITE_IMAGE_CANDIDATE_LIMIT: usize = 4;
const ICON_FETCH_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CandidateKind {
    Icon,
    SiteImage,
}

#[derive(Clone, Debug)]
struct IconCandidate {
    url: Url,
    declared_area: u64,
    source_priority: u8,
    kind: CandidateKind,
}

#[derive(Default)]
struct IconLinks {
    icons: Vec<IconCandidate>,
    site_images: Vec<IconCandidate>,
    manifests: Vec<Url>,
}

fn safe_link(base: &Url, href: &str) -> Option<Url> {
    base.join(href)
        .ok()
        .and_then(|url| safe_url(url.as_str()).ok())
}

fn declared_icon_area(value: Option<&str>) -> u64 {
    value
        .into_iter()
        .flat_map(str::split_ascii_whitespace)
        .filter_map(|size| {
            if size.eq_ignore_ascii_case("any") {
                return Some(u64::MAX / 2);
            }
            let (width, height) = size.split_once('x')?;
            let width = width.parse::<u64>().ok()?;
            let height = height.parse::<u64>().ok()?;
            width.checked_mul(height)
        })
        .max()
        .unwrap_or_default()
}

fn push_icon_candidate(links: &mut IconLinks, url: Url, declared_area: u64, source_priority: u8) {
    if !links.icons.iter().any(|candidate| candidate.url == url) {
        links.icons.push(IconCandidate {
            url,
            declared_area,
            source_priority,
            kind: CandidateKind::Icon,
        });
    }
}

fn push_site_image_candidate(links: &mut IconLinks, url: Url, source_priority: u8) {
    if !links
        .site_images
        .iter()
        .any(|candidate| candidate.url == url)
    {
        links.site_images.push(IconCandidate {
            url,
            declared_area: 0,
            source_priority,
            kind: CandidateKind::SiteImage,
        });
    }
}

fn icon_links(html: &[u8], base: &Url) -> IconLinks {
    use html5ever::{
        tendril::StrTendril,
        tokenizer::{BufferQueue, TagKind, Token, TokenSink, TokenSinkResult, Tokenizer},
    };
    struct Links {
        base: Url,
        links: RefCell<IconLinks>,
    }
    impl TokenSink for Links {
        type Handle = ();
        fn process_token(&self, token: Token, _: u64) -> TokenSinkResult<()> {
            let Token::TagToken(tag) = token else {
                return TokenSinkResult::Continue;
            };
            if tag.kind != TagKind::StartTag {
                return TokenSinkResult::Continue;
            }
            let attr = |name: &str| {
                tag.attrs
                    .iter()
                    .find(|attribute| attribute.name.local.as_ref() == name)
                    .map(|attribute| attribute.value.to_string())
            };
            if tag.name.as_ref() == "meta" {
                let key = attr("property").or_else(|| attr("name"));
                let Some(key) = key.map(|key| key.to_ascii_lowercase()) else {
                    return TokenSinkResult::Continue;
                };
                if !matches!(
                    key.as_str(),
                    "og:image"
                        | "og:image:url"
                        | "og:image:secure_url"
                        | "twitter:image"
                        | "twitter:image:src"
                ) {
                    return TokenSinkResult::Continue;
                }
                let Some(content) = attr("content") else {
                    return TokenSinkResult::Continue;
                };
                if let Some(url) = safe_link(&self.base, &content) {
                    push_site_image_candidate(
                        &mut self.links.borrow_mut(),
                        url,
                        u8::from(key.starts_with("og:")) + 1,
                    );
                }
                return TokenSinkResult::Continue;
            }
            if tag.name.as_ref() != "link" {
                return TokenSinkResult::Continue;
            }
            let Some(rel) = attr("rel") else {
                return TokenSinkResult::Continue;
            };
            let Some(href) = attr("href") else {
                return TokenSinkResult::Continue;
            };
            let Some(url) = safe_link(&self.base, &href) else {
                return TokenSinkResult::Continue;
            };
            let tokens: Vec<_> = rel.split_ascii_whitespace().collect();
            let mut links = self.links.borrow_mut();
            if tokens
                .iter()
                .any(|token| token.eq_ignore_ascii_case("manifest"))
            {
                if !links.manifests.iter().any(|manifest| manifest == &url) {
                    links.manifests.push(url.clone());
                }
            }
            let standard_icon = tokens
                .iter()
                .any(|token| token.eq_ignore_ascii_case("icon"));
            let apple_icon = tokens.iter().any(|token| {
                token.eq_ignore_ascii_case("apple-touch-icon")
                    || token.eq_ignore_ascii_case("apple-touch-icon-precomposed")
            });
            if standard_icon || apple_icon {
                push_icon_candidate(
                    &mut links,
                    url,
                    declared_icon_area(attr("sizes").as_deref()),
                    u8::from(standard_icon) + u8::from(apple_icon),
                );
            }
            TokenSinkResult::Continue
        }
    }
    let input = BufferQueue::default();
    input.push_back(StrTendril::from(String::from_utf8_lossy(html).as_ref()));
    let tokenizer = Tokenizer::new(
        Links {
            base: base.clone(),
            links: RefCell::new(IconLinks::default()),
        },
        Default::default(),
    );
    let _ = tokenizer.feed(&input);
    tokenizer.end();
    tokenizer.sink.links.into_inner()
}

fn manifest_icon_candidates(bytes: &[u8], base: &Url) -> Vec<IconCandidate> {
    serde_json::from_slice::<Value>(bytes)
        .ok()
        .and_then(|manifest| manifest.get("icons")?.as_array().cloned())
        .into_iter()
        .flatten()
        .filter_map(|icon| {
            let url = safe_link(base, icon.get("src")?.as_str()?)?;
            Some(IconCandidate {
                url,
                declared_area: declared_icon_area(icon.get("sizes").and_then(Value::as_str)),
                source_priority: 0,
                kind: CandidateKind::Icon,
            })
        })
        .collect()
}

async fn icon_reply<T: SiteTransport>(transport: &T, url: &Url, limit: usize) -> Option<Reply> {
    tokio::time::timeout(ICON_FETCH_TIMEOUT, transport.get(url, limit))
        .await
        .ok()?
        .ok()
}

async fn best_icon<T: SiteTransport>(
    transport: &T,
    mut candidates: Vec<IconCandidate>,
    mut site_images: Vec<IconCandidate>,
    fallback: Url,
) -> (Option<Icon>, Option<Icon>) {
    // The conventional fallback must remain reachable even if a malicious or
    // noisy page advertises more ranked candidates than the request budget.
    candidates.retain(|candidate| candidate.url != fallback);
    candidates.sort_by(|left, right| {
        right
            .declared_area
            .cmp(&left.declared_area)
            .then_with(|| right.source_priority.cmp(&left.source_priority))
    });
    site_images.sort_by(|left, right| {
        right
            .source_priority
            .cmp(&left.source_priority)
            .then_with(|| right.declared_area.cmp(&left.declared_area))
    });
    candidates.retain(|candidate| candidate.url != fallback);
    site_images.retain(|candidate| candidate.url != fallback);
    let mut selected = Vec::new();
    let mut seen = HashSet::new();
    for candidate in site_images.iter().take(SITE_IMAGE_CANDIDATE_LIMIT).chain(
        candidates
            .iter()
            .take(ICON_CANDIDATE_LIMIT - SITE_IMAGE_CANDIDATE_LIMIT),
    ) {
        if seen.insert(candidate.url.clone()) {
            selected.push(candidate.clone());
        }
    }
    for candidate in site_images.iter().chain(candidates.iter()) {
        if selected.len() >= ICON_CANDIDATE_LIMIT {
            break;
        }
        if seen.insert(candidate.url.clone()) {
            selected.push(candidate.clone());
        }
    }
    let mut best_icon: Option<((u8, u64), Icon)> = None;
    let mut best_header: Option<(u64, Icon)> = None;
    let fallback = IconCandidate {
        url: fallback,
        declared_area: 0,
        source_priority: 0,
        kind: CandidateKind::Icon,
    };
    for candidate in selected.into_iter().chain(std::iter::once(fallback)) {
        let Some(Reply {
            status: 200,
            body: Ok(bytes),
            ..
        }) = icon_reply(transport, &candidate.url, ICON_LIMIT).await
        else {
            continue;
        };
        if bytes.is_empty() || bytes.len() > ICON_LIMIT {
            continue;
        }
        let Some(mime) = crate::backend::media::image_mime(&bytes) else {
            continue;
        };
        // One URL can be advertised for both roles. Fetch once, preserve the
        // declarations, and select each role independently.
        let is_icon = candidate.kind == CandidateKind::Icon
            || candidates.iter().any(|item| item.url == candidate.url);
        let is_header = candidate.kind == CandidateKind::SiteImage
            || site_images.iter().any(|item| item.url == candidate.url);
        if is_header && mime != "image/svg+xml" {
            if let Some((width, height)) =
                crate::backend::media::validation::decoded_raster_dimensions(&bytes)
            {
                let area = u64::from(width) * u64::from(height);
                if site_image_dimensions_suitable(width, height)
                    && best_header.as_ref().is_none_or(|(old, _)| area > *old)
                {
                    best_header = Some((
                        area,
                        Icon {
                            mime,
                            bytes: bytes.clone(),
                        },
                    ));
                }
            }
        }
        if is_icon {
            let score = if mime == "image/svg+xml" {
                (2, u64::MAX)
            } else {
                let Some(area) = crate::backend::media::validation::decoded_raster_area(&bytes)
                else {
                    continue;
                };
                (1, area)
            };
            if best_icon.as_ref().is_none_or(|(old, _)| score > *old) {
                best_icon = Some((score, Icon { mime, bytes }));
            }
        }
    }
    let icon = best_icon.map(|(_, icon)| icon);
    let header = best_header
        .map(|(_, image)| image)
        .filter(|image| icon.as_ref().is_none_or(|icon| icon.bytes != image.bytes));
    (icon, header)
}

pub(crate) fn site_image_dimensions_suitable(width: u32, height: u32) -> bool {
    const MIN_EDGE: u32 = 160;
    const MIN_AREA: u64 = 40_000;
    if width < MIN_EDGE || height < MIN_EDGE {
        return false;
    }
    let (width, height) = (u64::from(width), u64::from(height));
    let Some(area) = width.checked_mul(height) else {
        return false;
    };
    area >= MIN_AREA && width * 5 >= height * 4 && width * 5 <= height * 12
}
pub(crate) fn image_mime(bytes: &[u8]) -> Option<&'static str> {
    use image::ImageFormat;
    // Imported bytes are kept intact, not transcoded. Signature recognition is
    // independent of enabled decoders and does not validate a complete image.
    // Keep an explicit raster allowlist rather than trusting remote MIME types
    // or silently serving every format the library may learn in the future.
    let format = image::guess_format(bytes).ok()?;
    match format {
        ImageFormat::Png
        | ImageFormat::Jpeg
        | ImageFormat::Gif
        | ImageFormat::WebP
        | ImageFormat::Ico
        | ImageFormat::Avif
        | ImageFormat::Bmp
        | ImageFormat::Tiff => Some(format.to_mime_type()),
        _ => None, // SVG requires the separate bounded XML + sandbox path.
    }
}
pub async fn collect<T: SiteTransport>(transport: &T, job: &CrawlJob) -> Observation {
    let Ok(domain) = canonical_domain(&job.domain) else {
        return Observation::failed("invalid_domain");
    };
    let base = Url::parse(&format!("https://{domain}/")).expect("validated domain");
    let start = Instant::now();
    let root = match transport.get(&base, PAGE_LIMIT).await {
        Ok(r) => r,
        Err(e) => return Observation::failed(e),
    };
    let mut result = Observation {
        alive: (200..400).contains(&root.status),
        response_ms: Some(start.elapsed().as_millis().min(i32::MAX as u128) as i32),
        status: Some(i32::from(root.status)),
        health_error: None,
        checked_at: Utc::now(),
        nodeinfo: None,
        nodeinfo_error: None,
        icon: None,
        header: None,
        icon_collection_complete: false,
    };
    if !result.alive {
        return result;
    }
    match nodeinfo(transport, &base).await {
        Ok(info) => result.nodeinfo = Some(info),
        Err(e) => result.nodeinfo_error = Some(e.to_owned()),
    }
    if job.needs_icon {
        let fallback = base.join("/favicon.ico").expect("static path");
        let mut links = root
            .body
            .as_ref()
            .ok()
            .map(|html| icon_links(html, &root.url))
            .unwrap_or_default();
        for manifest in std::mem::take(&mut links.manifests).into_iter().take(2) {
            if let Some(Reply {
                url,
                status: 200,
                body: Ok(bytes),
            }) = icon_reply(transport, &manifest, MANIFEST_LIMIT).await
            {
                for candidate in manifest_icon_candidates(&bytes, &url) {
                    push_icon_candidate(
                        &mut links,
                        candidate.url,
                        candidate.declared_area,
                        candidate.source_priority,
                    );
                }
            }
        }
        (result.icon, result.header) =
            best_icon(transport, links.icons, links.site_images, fallback).await;
        result.icon_collection_complete = true;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::HashMap;
    struct Fake(HashMap<String, Reply>);
    impl SiteTransport for Fake {
        fn get<'a>(&'a self, url: &'a Url, _: usize) -> FetchFuture<'a> {
            Box::pin(async move { self.0.get(url.as_str()).cloned().ok_or("offline") })
        }
    }
    fn job() -> CrawlJob {
        CrawlJob {
            id: uuid::Uuid::new_v4(),
            site_id: uuid::Uuid::new_v4(),
            domain: "social.example.com".into(),
            lease_token: uuid::Uuid::new_v4(),
            needs_icon: false,
        }
    }
    fn valid_png() -> Vec<u8> {
        use std::io::Cursor;

        let mut bytes = Vec::new();
        image::DynamicImage::new_rgba8(32, 32)
            .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)
            .unwrap();
        bytes
    }
    #[test]
    fn missing_or_malformed_values_are_unknown_not_zero_or_false() {
        let n=parse_nodeinfo(&json!({"version":"2.0","usage":{"users":{"total":-1,"activeMonth":"12"}},"openRegistrations":"true"})).unwrap();
        assert_eq!(n.users, None);
        assert_eq!(n.active_users, None);
        assert_eq!(n.registration_open, None);
        assert!(parse_nodeinfo(&json!({"version":"3.0"})).is_err());
        let n = parse_nodeinfo(
            &json!({"version":"2.1","usage":{"users":{"total":0}},"openRegistrations":false}),
        )
        .unwrap();
        assert_eq!(n.users, Some(0));
        assert_eq!(n.registration_open, Some(false));
    }
    #[tokio::test]
    async fn nodeinfo_failure_does_not_turn_a_healthy_site_offline() {
        let url = Url::parse("https://social.example.com/").unwrap();
        let t = Fake(HashMap::from([(
            url.to_string(),
            Reply {
                url,
                status: 200,
                body: Err("body_too_large"),
            },
        )]));
        let r = collect(&t, &job()).await;
        assert!(r.alive);
        assert_eq!(r.status, Some(200));
        assert!(r.nodeinfo.is_none());
        assert!(r.nodeinfo_error.is_some());
        let r = collect(&Fake(HashMap::new()), &job()).await;
        assert!(!r.alive);
        assert_eq!(r.response_ms, None);
    }
    #[tokio::test]
    async fn favicon_collection_accepts_safe_svg_and_falls_back_after_bad_svg() {
        let root = Url::parse("https://social.example.com/").unwrap();
        let primary = Url::parse("https://social.example.com/site.svg").unwrap();
        let fallback = Url::parse("https://social.example.com/favicon.ico").unwrap();
        let mut svg_job = job();
        svg_job.needs_icon = true;
        let svg = b"<svg><style>rect { fill: red }</style><rect/></svg>".to_vec();
        let collected = collect(
            &Fake(HashMap::from([
                (
                    root.to_string(),
                    Reply {
                        url: root.clone(),
                        status: 200,
                        body: Ok(b"<link rel='icon' href='/site.svg'>".to_vec()),
                    },
                ),
                (
                    primary.to_string(),
                    Reply {
                        url: primary.clone(),
                        status: 200,
                        body: Ok(svg.clone()),
                    },
                ),
            ])),
            &svg_job,
        )
        .await;
        assert_eq!(
            collected.icon.as_ref().map(|icon| icon.mime),
            Some("image/svg+xml")
        );
        assert_eq!(collected.icon.unwrap().bytes, svg);

        let collected = collect(
            &Fake(HashMap::from([
                (
                    root.to_string(),
                    Reply {
                        url: root,
                        status: 200,
                        body: Ok(b"<link rel='icon' href='/site.svg'>".to_vec()),
                    },
                ),
                (
                    primary.to_string(),
                    Reply {
                        url: primary,
                        status: 200,
                        body: Ok(b"<!DOCTYPE svg><svg/>".to_vec()),
                    },
                ),
                (
                    fallback.to_string(),
                    Reply {
                        url: fallback,
                        status: 200,
                        body: Ok(valid_png()),
                    },
                ),
            ])),
            &svg_job,
        )
        .await;
        assert_eq!(
            collected.icon.as_ref().map(|icon| icon.mime),
            Some("image/png")
        );
    }
    #[tokio::test]
    async fn favicon_collection_prefers_largest_valid_declared_candidate() {
        use std::io::Cursor;
        fn png(width: u32, height: u32) -> Vec<u8> {
            let mut bytes = Vec::new();
            image::DynamicImage::new_rgba8(width, height)
                .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)
                .unwrap();
            bytes
        }

        let root = Url::parse("https://social.example.com/").unwrap();
        let tiny = Url::parse("https://social.example.com/favicon-16.png").unwrap();
        let touch = Url::parse("https://social.example.com/touch-512.png").unwrap();
        let manifest = Url::parse("https://social.example.com/manifest.webmanifest").unwrap();
        let app = Url::parse("https://social.example.com/app-1024.png").unwrap();
        let mut icon_job = job();
        icon_job.needs_icon = true;
        let selected = png(1024, 1024);
        let collected = collect(
            &Fake(HashMap::from([
                (
                    root.to_string(),
                    Reply {
                        url: root,
                        status: 200,
                        body: Ok(b"<link rel='icon' sizes='16x16' href='/favicon-16.png'><link rel='apple-touch-icon' sizes='512x512' href='/touch-512.png'><link rel='manifest' href='/manifest.webmanifest'>".to_vec()),
                    },
                ),
                (
                    tiny.to_string(),
                    Reply { url: tiny, status: 200, body: Ok(png(16, 16)) },
                ),
                (
                    touch.to_string(),
                    Reply { url: touch, status: 200, body: Ok(png(512, 512)) },
                ),
                (
                    manifest.to_string(),
                    Reply {
                        url: manifest,
                        status: 200,
                        body: Ok(br#"{"icons":[{"src":"/app-1024.png","sizes":"1024x1024","type":"image/png"}]}"#.to_vec()),
                    },
                ),
                (
                    app.to_string(),
                    Reply { url: app, status: 200, body: Ok(selected.clone()) },
                ),
            ])),
            &icon_job,
        )
        .await;
        assert_eq!(collected.icon.unwrap().bytes, selected);
    }

    #[tokio::test]
    async fn favicon_collection_uses_decoded_area_not_a_lie_in_sizes() {
        use std::io::Cursor;
        fn png(width: u32, height: u32) -> Vec<u8> {
            let mut bytes = Vec::new();
            image::DynamicImage::new_rgba8(width, height)
                .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)
                .unwrap();
            bytes
        }

        let root = Url::parse("https://area.example.com/").unwrap();
        let claimed = root.join("/claimed-large.png").unwrap();
        let actual_large = root.join("/actual-large.png").unwrap();
        let large = png(512, 512);
        let mut icon_job = job();
        icon_job.domain = "area.example.com".to_owned();
        icon_job.needs_icon = true;
        let collected = collect(
            &Fake(HashMap::from([
                (
                    root.to_string(),
                    Reply {
                        url: root,
                        status: 200,
                        body: Ok(b"<link rel='icon' sizes='4096x4096' href='/claimed-large.png'><link rel='apple-touch-icon' sizes='512x512' href='/actual-large.png'>".to_vec()),
                    },
                ),
                (
                    claimed.to_string(),
                    Reply { url: claimed, status: 200, body: Ok(png(16, 16)) },
                ),
                (
                    actual_large.to_string(),
                    Reply { url: actual_large, status: 200, body: Ok(large.clone()) },
                ),
            ])),
            &icon_job,
        )
        .await;
        assert_eq!(collected.icon.unwrap().bytes, large);
    }

    #[tokio::test]
    async fn favicon_collection_rejects_rasters_over_the_shared_decode_limits() {
        use std::io::Cursor;
        fn png(width: u32, height: u32) -> Vec<u8> {
            let mut bytes = Vec::new();
            image::DynamicImage::new_rgba8(width, height)
                .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)
                .unwrap();
            bytes
        }

        let root = Url::parse("https://limits.example.com/").unwrap();
        let oversized = root.join("/too-wide.png").unwrap();
        let fallback = root.join("/favicon.ico").unwrap();
        let fallback_bytes = png(32, 32);
        let mut icon_job = job();
        icon_job.domain = "limits.example.com".to_owned();
        icon_job.needs_icon = true;
        let collected = collect(
            &Fake(HashMap::from([
                (
                    root.to_string(),
                    Reply {
                        url: root,
                        status: 200,
                        body: Ok(b"<link rel='icon' sizes='4097x1' href='/too-wide.png'>".to_vec()),
                    },
                ),
                (
                    oversized.to_string(),
                    Reply {
                        url: oversized,
                        status: 200,
                        body: Ok(png(4097, 1)),
                    },
                ),
                (
                    fallback.to_string(),
                    Reply {
                        url: fallback,
                        status: 200,
                        body: Ok(fallback_bytes.clone()),
                    },
                ),
            ])),
            &icon_job,
        )
        .await;
        assert_eq!(collected.icon.unwrap().bytes, fallback_bytes);
    }

    #[tokio::test]
    async fn favicon_collection_times_out_a_candidate_then_tries_fallback() {
        struct Slow(HashMap<String, Reply>);
        impl SiteTransport for Slow {
            fn get<'a>(&'a self, url: &'a Url, _: usize) -> FetchFuture<'a> {
                Box::pin(async move {
                    if url.path() == "/slow.png" {
                        return std::future::pending::<Result<Reply, &'static str>>().await;
                    }
                    self.0.get(url.as_str()).cloned().ok_or("offline")
                })
            }
        }

        let root = Url::parse("https://timeout.example.com/").unwrap();
        let fallback = root.join("/favicon.ico").unwrap();
        let fallback_bytes = valid_png();
        let mut icon_job = job();
        icon_job.domain = "timeout.example.com".to_owned();
        icon_job.needs_icon = true;
        let collected = tokio::time::timeout(
            Duration::from_millis(2_500),
            collect(
                &Slow(HashMap::from([
                    (
                        root.to_string(),
                        Reply {
                            url: root,
                            status: 200,
                            body: Ok(b"<link rel='icon' href='/slow.png'>".to_vec()),
                        },
                    ),
                    (
                        fallback.to_string(),
                        Reply {
                            url: fallback,
                            status: 200,
                            body: Ok(fallback_bytes.clone()),
                        },
                    ),
                ])),
                &icon_job,
            ),
        )
        .await
        .expect("bounded candidate fetch must leave time for fallback");
        assert_eq!(collected.icon.map(|icon| icon.bytes), Some(fallback_bytes));
    }

    #[tokio::test]
    async fn favicon_collection_always_tries_fallback_after_ranked_candidates_fail() {
        let root = Url::parse("https://fallback.example.com/").unwrap();
        let fallback = root.join("/favicon.ico").unwrap();
        let mut responses = HashMap::new();
        let html = (0..9)
            .map(|index| format!("<link rel='icon' sizes='1024x1024' href='/missing-{index}.png'>"))
            .collect::<String>();
        responses.insert(
            root.to_string(),
            Reply {
                url: root.clone(),
                status: 200,
                body: Ok(html.into_bytes()),
            },
        );
        for index in 0..9 {
            let url = root.join(&format!("/missing-{index}.png")).unwrap();
            responses.insert(
                url.to_string(),
                Reply {
                    url,
                    status: 404,
                    body: Ok(Vec::new()),
                },
            );
        }
        let fallback_bytes = valid_png();
        responses.insert(
            fallback.to_string(),
            Reply {
                url: fallback,
                status: 200,
                body: Ok(fallback_bytes.clone()),
            },
        );
        let mut icon_job = job();
        icon_job.domain = "fallback.example.com".to_owned();
        icon_job.needs_icon = true;
        let collected = collect(&Fake(responses), &icon_job).await;
        assert_eq!(collected.icon.map(|icon| icon.bytes), Some(fallback_bytes));
    }

    #[test]
    fn icons_use_html_tokens_and_safe_urls_not_remote_content_types() {
        let base = Url::parse("https://social.example.com/sub/").unwrap();
        let links = icon_links(br#"<LINK href='../logo.png' REL='SHORTCUT ICON'>"#, &base);
        assert_eq!(
            links.icons[0].url.as_str(),
            "https://social.example.com/logo.png"
        );
        assert!(icon_links(
            br#"<link rel='icon' href='http://127.0.0.1/private'>"#,
            &base
        )
        .icons
        .is_empty());
        assert!(icon_links(
            br#"<meta property='og:image' content='http://127.0.0.1/private'><meta name='twitter:image' content='javascript:alert(1)'><meta property='og:image' content='data:image/png;base64,AA=='>"#,
            &base
        )
        .site_images
        .is_empty());
        assert_eq!(image_mime(b"<svg onload='alert(1)'/>"), None);
        assert_eq!(image_mime(b"<html>not an icon"), None);
    }

    #[test]
    fn image_mime_preserves_legacy_raster_formats_without_enabling_decoders() {
        // Fixed signatures exercise MIME dispatch, not complete image decoding.
        let signatures: &[(&[u8], &str)] = &[
            (b"\x89PNG\r\n\x1a\n", "image/png"),
            (b"\xff\xd8\xff", "image/jpeg"),
            (b"GIF87a", "image/gif"),
            (b"GIF89a", "image/gif"),
            (b"RIFF\x12\x34\x56\x78WEBP", "image/webp"),
            (b"\0\0\x01\0", "image/x-icon"),
            (b"\0\0\0\x20ftypavif", "image/avif"),
            (b"BM\0\0\0\0", "image/bmp"),
            (b"II*\0", "image/tiff"),
            (b"MM\0*", "image/tiff"),
        ];
        for (bytes, mime) in signatures {
            assert_eq!(image_mime(bytes), Some(*mime));
            assert_eq!(crate::backend::media::image_mime(bytes), Some(*mime));
        }
        for unsupported in [
            b"".as_slice(),
            b"RIFF\0\0\0\0WAVE",
            b"\0\0\0\x20ftypmp42",
            b"<html><img src='/private'></html>",
            b"DDS ",
            b"qoif",
        ] {
            assert_eq!(image_mime(unsupported), None);
        }
    }

    #[tokio::test]
    async fn open_graph_header_does_not_replace_the_identity_icon() {
        use std::io::Cursor;
        fn png(width: u32, height: u32) -> Vec<u8> {
            let mut bytes = Vec::new();
            image::DynamicImage::new_rgba8(width, height)
                .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)
                .unwrap();
            bytes
        }

        let root = Url::parse("https://site-image.example.com/").unwrap();
        let card_url = root.join("/preview.png").unwrap();
        let favicon_url = root.join("/favicon.svg").unwrap();
        let card = png(1200, 630);
        let html = b"<meta property='og:image' content='/preview.png'><link rel='icon' href='/favicon.svg'>";
        assert_eq!(
            icon_links(html, &root).site_images[0].url,
            root.join("/preview.png").unwrap()
        );
        assert_eq!(
            crate::backend::media::validation::decoded_raster_dimensions(&card),
            Some((1200, 630))
        );
        assert!(site_image_dimensions_suitable(1200, 630));
        let mut icon_job = job();
        icon_job.domain = "site-image.example.com".into();
        icon_job.needs_icon = true;
        let result = collect(
            &Fake(HashMap::from([
                (
                    root.to_string(),
                    Reply {
                        url: root,
                        status: 200,
                        body: Ok(b"<meta property='og:image' content='/preview.png'><link rel='icon' href='/favicon.svg'>".to_vec()),
                    },
                ),
                (
                    card_url.to_string(),
                    Reply {
                        url: card_url,
                        status: 200,
                        body: Ok(card.clone()),
                    },
                ),
                (
                    favicon_url.to_string(),
                    Reply {
                        url: favicon_url,
                        status: 200,
                        body: Ok(b"<svg><circle cx='8' cy='8' r='8'/></svg>".to_vec()),
                    },
                ),
            ])),
            &icon_job,
        )
        .await;

        assert_eq!(
            result
                .icon
                .as_ref()
                .map(|icon| (icon.mime, icon.bytes.len())),
            Some((
                "image/svg+xml",
                b"<svg><circle cx='8' cy='8' r='8'/></svg>".len()
            ))
        );
        assert_eq!(result.header.unwrap().bytes, card);
    }

    #[test]
    fn site_images_require_large_usable_aspect_ratio_but_allow_square_and_wide() {
        assert!(site_image_dimensions_suitable(1200, 630));
        assert!(site_image_dimensions_suitable(512, 512));
        assert!(site_image_dimensions_suitable(320, 160));
        assert!(site_image_dimensions_suitable(200, 200));
        assert!(!site_image_dimensions_suitable(159, 512));
        assert!(!site_image_dimensions_suitable(160, 159));
        assert!(!site_image_dimensions_suitable(400, 700));
        assert!(!site_image_dimensions_suitable(1200, 300));
        assert!(!site_image_dimensions_suitable(199, 199));
    }

    #[tokio::test]
    async fn unsuitable_or_malformed_metadata_images_fall_back_to_real_icons() {
        use std::io::Cursor;
        fn png(width: u32, height: u32) -> Vec<u8> {
            let mut bytes = Vec::new();
            image::DynamicImage::new_rgba8(width, height)
                .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)
                .unwrap();
            bytes
        }

        let root = Url::parse("https://fallback-shapes.example.com/").unwrap();
        let mut entries = Vec::new();
        let mut html = String::new();
        for (path, bytes) in [
            ("/portrait.png", png(400, 700)),
            ("/banner.png", png(1200, 300)),
            ("/small.png", png(159, 512)),
            ("/malformed.png", b"not an image".to_vec()),
        ] {
            let url = root.join(path).unwrap();
            html.push_str(&format!("<meta property='og:image' content='{path}'>"));
            entries.push((
                url.to_string(),
                Reply {
                    url,
                    status: 200,
                    body: Ok(bytes),
                },
            ));
        }
        let icon_url = root.join("/touch.png").unwrap();
        let icon = png(64, 64);
        html.push_str("<link rel='apple-touch-icon' href='/touch.png'>");
        entries.push((
            icon_url.to_string(),
            Reply {
                url: icon_url,
                status: 200,
                body: Ok(icon.clone()),
            },
        ));
        entries.push((
            root.to_string(),
            Reply {
                url: root.clone(),
                status: 200,
                body: Ok(html.into_bytes()),
            },
        ));
        let fallback = root.join("/favicon.ico").unwrap();
        entries.push((
            fallback.to_string(),
            Reply {
                url: fallback,
                status: 404,
                body: Ok(Vec::new()),
            },
        ));
        let mut icon_job = job();
        icon_job.domain = "fallback-shapes.example.com".into();
        icon_job.needs_icon = true;

        let result = collect(&Fake(HashMap::from_iter(entries)), &icon_job).await;
        assert_eq!(
            result.icon.as_ref().map(|icon| icon.bytes.as_slice()),
            Some(icon.as_slice())
        );
    }

    #[tokio::test]
    async fn metadata_candidate_overflow_keeps_candidate_budget_and_favicon_fallback() {
        let root = Url::parse("https://meta-budget.example.com/").unwrap();
        let mut html = String::new();
        let mut entries = Vec::new();
        for index in 0..9 {
            let path = format!("/share-{index}.png");
            html.push_str(&format!("<meta property='og:image' content='{path}'>"));
            let url = root.join(&path).unwrap();
            entries.push((
                url.to_string(),
                Reply {
                    url,
                    status: 404,
                    body: Ok(Vec::new()),
                },
            ));
        }
        let icon = root.join("/favicon-32.png").unwrap();
        html.push_str("<link rel='icon' sizes='32x32' href='/favicon-32.png'>");
        entries.push((
            icon.to_string(),
            Reply {
                url: icon,
                status: 404,
                body: Ok(Vec::new()),
            },
        ));
        entries.push((
            root.to_string(),
            Reply {
                url: root.clone(),
                status: 200,
                body: Ok(html.into_bytes()),
            },
        ));
        let fallback = root.join("/favicon.ico").unwrap();
        let fallback_bytes = valid_png();
        entries.push((
            fallback.to_string(),
            Reply {
                url: fallback,
                status: 200,
                body: Ok(fallback_bytes.clone()),
            },
        ));
        let mut icon_job = job();
        icon_job.domain = "meta-budget.example.com".into();
        icon_job.needs_icon = true;

        let result = collect(&Fake(HashMap::from_iter(entries)), &icon_job).await;
        assert_eq!(result.icon.map(|icon| icon.bytes), Some(fallback_bytes));
    }

    #[tokio::test]
    async fn image_roles_support_header_only_icon_only_absent_and_duplicate_urls() {
        let root = Url::parse("https://roles.example.com/").unwrap();
        let mut bytes = Vec::new();
        image::DynamicImage::new_rgba8(800, 420)
            .write_to(
                &mut std::io::Cursor::new(&mut bytes),
                image::ImageFormat::Png,
            )
            .unwrap();
        for (html, icon, header) in [
            (
                "<meta property='og:image' content='/same.png'>",
                false,
                true,
            ),
            ("<link rel='icon' href='/same.png'>", true, false),
            (
                "<meta property='og:image' content='/same.png'><link rel='icon' href='/same.png'>",
                true,
                false,
            ),
            ("", false, false),
        ] {
            let fake = Fake(HashMap::from([
                (
                    root.to_string(),
                    Reply {
                        url: root.clone(),
                        status: 200,
                        body: Ok(html.as_bytes().to_vec()),
                    },
                ),
                (
                    root.join("/same.png").unwrap().to_string(),
                    Reply {
                        url: root.join("/same.png").unwrap(),
                        status: 200,
                        body: Ok(bytes.clone()),
                    },
                ),
            ]));
            let mut j = job();
            j.domain = "roles.example.com".into();
            j.needs_icon = true;
            let result = collect(&fake, &j).await;
            assert_eq!(result.icon.is_some(), icon, "icon role for {html}");
            assert_eq!(result.header.is_some(), header, "header role for {html}");
        }
    }
}
