//! Shared SSR-safe image loading and fallback for logos, site icons and profile media.
use dioxus::prelude::*;
#[component]
pub fn ImageMark(
    source: Option<String>,
    fallback: String,
    class: String,
    #[props(default = 40)] size: u32,
    #[props(default = true)] decorative: bool,
) -> Element {
    let ready = use_context::<Signal<bool>>()();
    let mut failed = use_signal(|| false);
    use_effect(use_reactive((&source,), move |_| failed.set(false)));
    rsx! {span{class:"media-mark {class}",aria_hidden:if decorative{Some("true")}else{None},
     if ready && !failed(){
      if let Some(src)=source {img{src,alt:if decorative{String::new()}else{fallback.clone()},width:size,height:size,loading:"lazy",referrerpolicy:"no-referrer",onerror:move |_|failed.set(true)}}else{"{fallback}"}
     }else{"{fallback}"}
    }}
}
pub fn software_source(name: &str) -> Option<String> {
    let mut url = url::Url::parse("https://fediverse.kr/api/public/software-logo/").ok()?;
    url.path_segments_mut().ok()?.pop_if_empty().push(name);
    Some(url.path().to_owned())
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
