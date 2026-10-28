//! Runtime-managed software media with a generic visual fallback.
use super::*;

#[component]
pub(super) fn SoftwareMark(item: Software, #[props(default = 64)] size: u32) -> Element {
    let source = item.logo.as_ref().map(|media| media.url.clone());
    let style = item
        .logo
        .as_ref()
        .and_then(|m| m.metadata.background.as_ref())
        .map(|color| format!("background-color:{color};"))
        .unwrap_or_default();
    let image_fit = item.logo.as_ref().map(|m| match m.metadata.fit {
        crate::directory::MediaFit::Contain => "contain".to_owned(),
        crate::directory::MediaFit::Cover => "cover".to_owned(),
    });
    rsx! { crate::media_ui::ImageMark {
        source,
        public: true, style, image_fit,
        fallback: item.display_name.chars().next().unwrap_or('·').to_string(),
        class: "software-emblem".to_owned(),
        size,
    } }
}

#[component]
pub(super) fn SoftwareArtwork(item: Software, #[props(default = false)] compact: bool) -> Element {
    let ready = use_context::<Signal<bool>>()();
    let media = item.header.clone();
    let mut failed = use_signal(|| false);
    let source = media.as_ref().map(|m| m.url.clone());
    use_effect(use_reactive((&source,), move |_| failed.set(false)));
    let has_image = media.is_some() && !failed();
    let fallback_background =
        crate::directory::header_art::background_image_for(&format!("software:{}", item.name));
    let fit_class = media
        .as_ref()
        .map(|m| match m.metadata.fit {
            crate::directory::MediaFit::Contain => "media-fit-contain",
            crate::directory::MediaFit::Cover => "media-fit-cover",
        })
        .unwrap_or("media-fit-cover");
    let background = media
        .as_ref()
        .and_then(|m| m.metadata.background.as_deref())
        .map(|color| format!("background-color:{color};"))
        .unwrap_or_default();
    rsx! {
        div { class:format!("software-artwork {fit_class} {}", if compact {"compact"} else {"expanded"}), style: background,
            if has_image {
                for mounted in [ready] {
                    img {
                        class: "software-artwork-image",
                        key:"{item.name}-{mounted}",
                        src:media.as_ref().map(|m| m.url.clone()).unwrap_or_default(),
                        alt:if compact {String::new()} else {format!("{} 공식 소개 이미지",item.display_name)},
                        loading:if compact {"lazy"} else {"eager"},
                        decoding:"async",
                        onerror:move |_| failed.set(true),
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
            span { class: "software-status", "{status.label()}" }
        }
    }
}
