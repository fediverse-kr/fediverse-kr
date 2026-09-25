//! Bounded unsigned public-site reads. Authentication remains AP-only and signed.
use super::{
    directory::{canonical_domain, CrawlJob, Icon, NodeInfo, Observation},
    federation::transport::{pinned_client, safe_url},
};
use chrono::Utc;
use serde_json::Value;
use std::{
    cell::RefCell,
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
const ICON_FETCH_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Clone, Debug)]
struct IconCandidate {
    url: Url,
    declared_area: u64,
    source_priority: u8,
}

#[derive(Default)]
struct IconLinks {
    icons: Vec<IconCandidate>,
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
            if tag.kind != TagKind::StartTag || tag.name.as_ref() != "link" {
                return TokenSinkResult::Continue;
            }
            let attr = |name: &str| {
                tag.attrs
                    .iter()
                    .find(|attribute| attribute.name.local.as_ref() == name)
                    .map(|attribute| attribute.value.to_string())
            };
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
    fallback: Url,
) -> Option<Icon> {
    // The conventional fallback must remain reachable even if a malicious or
    // noisy page advertises more ranked candidates than the request budget.
    candidates.retain(|candidate| candidate.url != fallback);
    candidates.sort_by(|left, right| {
        right
            .declared_area
            .cmp(&left.declared_area)
            .then_with(|| right.source_priority.cmp(&left.source_priority))
    });
    let mut best: Option<((u8, u64), Icon)> = None;
    let fallback = IconCandidate {
        url: fallback,
        declared_area: 0,
        source_priority: 0,
    };
    for candidate in candidates
        .into_iter()
        .take(ICON_CANDIDATE_LIMIT)
        .chain(std::iter::once(fallback))
    {
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
        let score = if mime == "image/svg+xml" {
            (2, u64::MAX)
        } else {
            let Some(area) = crate::backend::media::validation::decoded_raster_area(&bytes) else {
                continue;
            };
            (1, area)
        };
        if best
            .as_ref()
            .is_none_or(|(best_score, _)| score > *best_score)
        {
            best = Some((score, Icon { mime, bytes }));
        }
    }
    best.map(|(_, icon)| icon)
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
        result.icon = best_icon(transport, links.icons, fallback).await;
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
}
