//! Official project assets supplement uploaded catalog logos.
//! This is editorial identification, not a replacement brand or an endorsement.
use super::*;

#[component]
pub(super) fn SoftwareMark(item: Software, #[props(default = 64)] size: u32) -> Element {
    let (placeholder, tone) = bundled(&item.name);
    rsx! { crate::media_ui::ImageMark {
        source: if item.logo_available { crate::media_ui::software_source(&item.name) } else { None },
        placeholder_source: placeholder.map(|asset| asset.to_string()),
        fallback: item.display_name.chars().next().unwrap_or('·').to_string(),
        class: format!("software-emblem brand-{tone}"),
        size,
    } }
}

fn bundled(name: &str) -> (Option<Asset>, &'static str) {
    // Exact names only: forks must not inherit another project's branding.
    match name.to_ascii_lowercase().as_str() {
        "mastodon" => (
            Some(asset!("/assets/software-media/mastodon-logo-purple.svg")),
            "mastodon",
        ),
        "misskey" => (
            Some(asset!("/assets/software-media/misskey-icon.png")),
            "misskey",
        ),
        "pixelfed" => (
            Some(asset!("/assets/software-media/pixelfed-icon-white.png")),
            "pixelfed",
        ),
        "peertube" => (
            Some(asset!("/assets/software-media/peertube-logo.svg")),
            "peertube",
        ),
        "bookwyrm" => (
            Some(asset!("/assets/software-media/bookwyrm-logo-small.png")),
            "bookwyrm",
        ),
        "mobilizon" => (
            Some(asset!("/assets/software-media/mobilizon-logo-favicon.svg")),
            "mobilizon",
        ),
        _ => (None, "neutral"),
    }
}

pub(super) fn artwork(name: &str) -> Option<Asset> {
    match name.to_ascii_lowercase().as_str() {
        "mastodon" => Some(asset!("/assets/software-media/mastodon-home-timeline.png")),
        "misskey" => Some(asset!("/assets/software-media/misskey-timeline-light.webp")),
        "pixelfed" => Some(asset!("/assets/software-media/pixelfed-web-ui.webp")),
        "peertube" => Some(asset!("/assets/software-media/peertube-mascot-banner.jpg")),
        "bookwyrm" => Some(asset!("/assets/software-media/bookwyrm-product-phone.png")),
        "mobilizon" => Some(asset!("/assets/software-media/mobilizon-product-main.jpg")),
        _ => None,
    }
}

pub(super) fn artwork_source(name: &str) -> Option<&'static str> {
    match name.to_ascii_lowercase().as_str() {
        "mastodon" => Some("https://joinmastodon.org/branding"),
        "misskey" => Some("https://misskey-hub.net/en/"),
        "pixelfed" => Some("https://pixelfed.org/"),
        "peertube" => Some("https://joinpeertube.org/"),
        "bookwyrm" => Some("https://joinbookwyrm.com/"),
        "mobilizon" => Some("https://mobilizon.org/"),
        _ => None,
    }
}

#[component]
pub(super) fn SoftwareArtwork(item: Software, #[props(default = false)] compact: bool) -> Element {
    let ready = use_context::<Signal<bool>>()();
    let (_, tone) = bundled(&item.name);
    let media = artwork(&item.name);
    let mut failed = use_signal(|| None::<String>);
    let has_image = media.is_some() && failed().as_deref() != Some(item.name.as_str());
    let fallback_background =
        crate::directory::header_art::background_image_for(&format!("software:{}", item.name));
    rsx! {
        div { class:format!("software-artwork artwork-{tone} {}", if compact {"compact"} else {"expanded"}),
            if has_image {
                // Reconcile a fresh keyed image after hydration: an SSR load failure
                // may happen before Rust's error listener exists. Cached successes
                // stay cached; failures now reliably reach the fallback handler.
                for mounted in [ready] {
                img {
                    key:"{item.name}-{mounted}",
                    src:media.unwrap(),
                    alt:if compact {String::new()} else {format!("{} 공식 소개 이미지",item.display_name)},
                    loading:if compact {"lazy"} else {"eager"},
                    decoding:"async",
                    onerror:{let name=item.name.clone();move |_| failed.set(Some(name.clone()))},
                }
                }
            } else {
                div { class:"software-artwork-fallback", style:fallback_background,
                    span { "{item.display_name}" }
                }
            }
        }
    }
}

#[component]
pub(super) fn SoftwareStatus(item: Software) -> Element {
    rsx! {
        if let Some(status) = item.development_status {
            span { class:"software-status", "{status.label()}" }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    #[test]
    fn official_media_matches_recorded_original_bytes() {
        let directory =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/software-media");
        let manifest: serde_json::Value = serde_json::from_str(include_str!(
            "../../../assets/software-media/extra-provenance.json"
        ))
        .unwrap();
        let mut names = std::collections::HashSet::new();
        for entry in manifest["assets"].as_array().unwrap() {
            let name = entry["file"].as_str().unwrap();
            assert!(names.insert(name), "duplicate asset: {name}");
            let bytes = std::fs::read(directory.join(name)).unwrap();
            assert_eq!(
                format!("{:x}", Sha256::digest(bytes)),
                entry["sha256"].as_str().unwrap(),
                "{name}"
            );
            assert!(entry["source_url"]
                .as_str()
                .unwrap()
                .starts_with("https://"));
        }
    }

    #[test]
    fn official_assets_cover_known_projects_but_never_their_forks() {
        for name in [
            "mastodon",
            "misskey",
            "pixelfed",
            "peertube",
            "bookwyrm",
            "mobilizon",
        ] {
            assert!(bundled(name).0.is_some(), "{name}");
            assert!(artwork(name).is_some(), "{name}");
            assert!(artwork_source(name).is_some(), "{name}");
        }
        for name in ["mastodon-fork", "misskey-fork", "unknown", "lemmy", "ghost"] {
            assert!(bundled(name).0.is_none(), "{name}");
            assert!(artwork(name).is_none(), "{name}");
        }
        assert!(bundled("Mastodon").0.is_some());
    }
}
