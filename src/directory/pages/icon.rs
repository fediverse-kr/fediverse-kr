use super::*;
#[component]
pub fn SiteIcon(site: PublicSite) -> Element {
    let source = site_link(&site.domain)
        .and_then(|s| url::Url::parse(&s).ok())
        .and_then(|u| u.domain().map(|d| format!("/api/public/server-icon/{d}")));
    rsx! {crate::media_ui::ImageMark{source:if site.icon_available{source}else{None},fallback:site.name.chars().next().unwrap_or('·').to_string(),class:"server-mark violet"}}
}
