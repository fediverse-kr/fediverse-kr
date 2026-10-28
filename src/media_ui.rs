//! Shared SSR-safe image loading and fallback for logos, site icons and profile media.
use dioxus::prelude::*;
#[component]
pub fn ImageMark(
    source: Option<String>,
    #[props(default)] placeholder_source: Option<String>,
    fallback: String,
    class: String,
    #[props(default = 40)] size: u32,
    #[props(default = true)] decorative: bool,
    #[props(default)] public: bool,
    #[props(default)] style: String,
    #[props(default)] image_fit: Option<String>,
) -> Element {
    let ready = use_context::<Signal<bool>>()();
    let mut failed = use_signal(|| false);
    let mut placeholder_failed = use_signal(|| false);
    use_effect(use_reactive((&source, &placeholder_source), move |_| {
        failed.set(false);
        placeholder_failed.set(false);
    }));
    let primary = (ready || public) && !failed() && source.is_some();
    let image = if primary {
        source
    } else if !placeholder_failed() {
        placeholder_source
    } else {
        None
    };
    rsx! {span{class:"media-mark {class}",style,aria_hidden:if decorative{Some("true")}else{None},
     if let Some(src)=image {img{src,object_fit:image_fit,alt:if decorative{String::new()}else{fallback.clone()},width:size,height:size,loading:"lazy",referrerpolicy:"no-referrer",onerror:move |_|{if primary {failed.set(true)} else {placeholder_failed.set(true)}}}}else{"{fallback}"}
    }}
}
/// The logical shortcode is preserved in the member's private media mapping.
/// Keep it in a query instead of a path segment so `.`/`..` cannot normalize.
pub fn emoji_source(name: &str, revision: i64) -> Option<String> {
    let mut url = url::Url::parse("https://fediverse.kr/api/member/emoji").ok()?;
    url.query_pairs_mut().append_pair("name", name);
    url.query_pairs_mut()
        .append_pair("v", &revision.to_string());
    Some(format!("{}?{}", url.path(), url.query()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_placeholder_is_visible_before_hydration_without_loading_private_media() {
        let mut dom = VirtualDom::new(|| {
            use_context_provider(|| Signal::new(false));
            rsx! { ImageMark {
                source:Some("/api/public/software-logo/demo".into()),
                placeholder_source:Some("/assets/local-demo.svg".into()),
                fallback:"D",class:"test-mark",
            } }
        });
        dom.rebuild_in_place();
        let html = dioxus::ssr::render(&dom);
        assert!(html.contains("src=\"/assets/local-demo.svg\""));
        assert!(!html.contains("src=\"/api/public/software-logo/demo\""));
    }

    #[test]
    fn uploaded_logo_is_preferred_when_the_app_is_ready() {
        let mut dom = VirtualDom::new(|| {
            use_context_provider(|| Signal::new(true));
            rsx! { ImageMark {
                source:Some("/api/public/software-logo/demo".into()),
                placeholder_source:Some("/assets/local-demo.svg".into()),
                fallback:"D",class:"test-mark",
            } }
        });
        dom.rebuild_in_place();
        let html = dioxus::ssr::render(&dom);
        assert!(html.contains("src=\"/api/public/software-logo/demo\""));
        assert!(!html.contains("src=\"/assets/local-demo.svg\""));
    }

    #[test]
    fn ordinary_member_media_keeps_its_existing_ssr_text_fallback() {
        let mut dom = VirtualDom::new(|| {
            use_context_provider(|| Signal::new(false));
            rsx! { ImageMark {source:Some("/api/member/avatar".into()),fallback:"나",class:"profile-avatar"} }
        });
        dom.rebuild_in_place();
        let html = dioxus::ssr::render(&dom);
        assert!(html.contains("나"));
        assert!(!html.contains("<img"));
    }

    #[test]
    fn emoji_source_uses_a_percent_encoded_query_for_legacy_shortcodes() {
        let source = emoji_source("blob-cat.@/한국어", 7).unwrap();
        assert!(source.starts_with("/api/member/emoji?name=blob-cat."));
        assert!(source.contains("%2F"));
        assert!(source.contains("%ED%95%9C%EA%B5%AD%EC%96%B4"));
        assert!(source.ends_with("&v=7"));
        assert_eq!(
            emoji_source("..", 0).as_deref(),
            Some("/api/member/emoji?name=..&v=0")
        );
    }
}
