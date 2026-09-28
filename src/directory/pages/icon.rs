use super::*;
fn source(site: &PublicSite, role: &str) -> Option<String> {
    site_link(&site.domain)
        .and_then(|s| url::Url::parse(&s).ok())
        .and_then(|u| u.domain().map(|d| format!("/api/public/server-{role}/{d}")))
}

#[component]
pub fn SiteIcon(site: PublicSite) -> Element {
    rsx! {crate::media_ui::ImageMark {
        source: if site.icon_available { source(&site,"icon") } else { None },
        fallback: site.name.chars().next().unwrap_or('·').to_string(),
        class: if site.icon_available { "server-mark site-icon" } else { "server-mark site-icon violet" },
        size: 32,
    }}
}

#[component]
pub fn SiteHeader(site: PublicSite) -> Element {
    let ready = use_context::<Signal<bool>>()();
    let mut failed = use_signal(|| false);
    let image = if site.header_available {
        source(&site, "header")
    } else {
        None
    };
    use_effect(use_reactive((&image,), move |_| failed.set(false)));
    // Render the same fallback in SSR and WASM, including before a real image loads.
    use base64::Engine;
    let svg = crate::directory::header_art::svg_for(&site.domain);
    let encoded = base64::engine::general_purpose::STANDARD.encode(svg.as_bytes());
    let background = format!("background-image:url(data:image/svg+xml;base64,{encoded})");
    rsx! {
        div {class:"server-cover",style:background,aria_hidden:"true",
            if ready && !failed() {
                if let Some(src) = image {
                    img {src,alt:"",width:800,height:420,loading:"lazy",referrerpolicy:"no-referrer",onerror:move |_|failed.set(true)}
                }
            }
        }
    }
}
