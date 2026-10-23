use super::*;
use crate::Route;
use dioxus::prelude::*;
use lucide_dioxus::{ArrowLeft, ArrowRight, Search};

#[component]
fn DataError() -> Element {
    crate::portal::response_status(503);
    rsx! { div { class:"data-notice", role:"alert", p { "정보를 불러오지 못했어요. 잠시 후 새로고침해 주세요." } } }
}

#[component]
fn MissingRecord(title: String) -> Element {
    crate::portal::response_status(404);
    rsx! { document::Title { "정보를 찾을 수 없어요 — fediverse.kr" } h1 { "{title}" } p { "주소를 확인하거나 목록에서 다시 찾아보세요." } }
}

#[component]
fn PreviewNote(preview: bool) -> Element {
    rsx! { if preview { div { class:"data-notice", span { "검토용 예시" } p { "DB가 연결되지 않은 미리보기입니다. 서버는 가상의 주소이며 가입할 수 없어요." } } } }
}

#[component]
pub fn Platforms(filters: super::catalog::CatalogQuery) -> Element {
    rsx! { catalog::CatalogListing { key:"{filters}", criteria:filters } }
}
mod brand;
mod catalog;
mod discovery;
mod families;
mod software;

#[component]
fn CatalogPreviewNote(preview: bool) -> Element {
    rsx! {if preview {div {class:"data-notice",
        span {"운영 목록으로 미리보기"}
        p {"{super::preview::catalog_snapshot_date()} 공개 소프트웨어 목록을 가져왔어요. 운영 DB와 연결되지 않았으며, 서버 목록은 가상의 예시입니다."}
    }}}
}

#[component]
pub fn SoftwareDetail(name: String) -> Element {
    rsx! { SoftwareDetails { key:"{name}", name } }
}
#[component]
fn SoftwareDetails(name: String) -> Element {
    let catalog = use_server_future(use_reactive((&name,), |(name,)| api::software(name)))?;
    // Optional navigation: a catalog failure must not hide the requested product.
    let related = use_server_future(|| async { api::catalog().await.ok() })?;
    rsx! { main { id:"content", class:"detail-page software-detail-page wrap",
        Link { class:"back-link", to:Route::Platforms { filters: Default::default() }, ArrowLeft { size:16 } "소프트웨어 목록" }
        match catalog() {
            Some(Ok(data))=> {
                let item=data.software.as_ref();
                match item {
                    Some(item)=>rsx! {
                        document::Title { "{item.display_name} — 소프트웨어 — fediverse.kr" }
                        CatalogPreviewNote { preview:data.preview }
                        software::Overview { item:item.clone(), categories:data.categories.clone(),related:related().flatten().map(|all|all.software).unwrap_or_default() }
                    },
                    None=>rsx! { MissingRecord { title:"아직 등록되지 않은 소프트웨어예요." } Link { class:"text-link", to:Route::Platforms { filters: Default::default() }, "전체 목록 보기" } },
                }
            },
            Some(Err(_))=>rsx!{ DataError{} },
            None=>rsx!{ p { "불러오는 중…" } },
        }
    } }
}

mod health;
mod icon;
mod listing;
mod operator;
mod site;
#[component]
pub fn Servers(filters: search::ServerQuery) -> Element {
    rsx! { listing::ServerListing { key:"{filters}", criteria:filters } }
}
#[component]
pub fn SoftwareServers(name: String) -> Element {
    let criteria = search::ServerQuery {
        software: name,
        ..Default::default()
    };
    rsx! { listing::ServerListing { key:"{criteria}", criteria } }
}

#[component]
pub fn ServerDetail(slug: String) -> Element {
    rsx! { SiteDetails { key:"{slug}", domain:slug } }
}
#[component]
fn SiteDetails(domain: String) -> Element {
    let result = use_server_future(use_reactive((&domain,), |(domain,)| api::server(domain)))?;
    rsx! { main { id:"content", class:"detail-page wrap server-detail-shell",
        Link { class:"back-link", to:Route::Servers { filters: Default::default() }, ArrowLeft { size:16 } "서버 목록" }
        match result() {
            Some(Ok(data))=>rsx! {
                if let Some(site)=data.site {
                    document::Title { "{site.name} — 서버 정보 — fediverse.kr" }
                    site::SiteDetailPage { site:site.clone(), preview:data.preview }
                } else { MissingRecord { title:"표시할 수 있는 서버 정보가 없어요." } }
            },
            Some(Err(_))=>rsx!{ DataError{} },
            None=>rsx!{ p { "불러오는 중…" } },
        }
    } }
}

#[component]
fn FediverseDefinition() -> Element {
    rsx! {
        header { class: "landing-definition",
            h2 { class: "landing-definition-title", "연합우주(Fediverse)는 각자 운영하는 여러 소셜 서버가 연결된 네트워크입니다." }
            p { "한 곳에 가입해도, 다른 서버의 사람들과 만나고 대화할 수 있어요." }
        }
    }
}

#[component]
fn LandingStatisticsFrame(children: Element) -> Element {
    rsx! {
        section { class: "landing-stats", aria_label: "연합우주와 등록된 한국어 서버 현황",
            div { class: "wrap",
                FediverseDefinition {}
                {children}
            }
        }
    }
}

#[component]
pub fn LandingStatistics() -> Element {
    let result = use_server_future(api::statistics)?;
    rsx! { LandingStatisticsFrame {
        match result() {
            Some(Ok(data))=>rsx! {
                dl { class:"landing-stat-values",
                    div { dt { "등록된 한국어 서버" } dd { "{number(data.sites)}" span { "개" } } }
                    div { dt { "가입 계정" } dd { {data.accounts.map(number).unwrap_or_else(||"—".into())} span { "개" } } }
                }
                p { class:"landing-stat-source",
                    if data.preview { "2026.09.10 기존 사이트 집계 · 실시간 아님" }
                    else { "공개 목록 중 운영 종료 제외 · 계정 수 확인 {data.counted_sites}곳" }
                }
            },
            _=>rsx! { p { class:"landing-stat-source", "현재 서버 통계를 불러오지 못했어요." } },
        }
    } }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn landing_statistics_explains_the_fediverse_before_presenting_counts() {
        let mut dom = VirtualDom::new(|| {
            rsx! {
                LandingStatisticsFrame {
                    dl { class: "landing-stat-values",
                        div { dt { "등록된 한국어 서버" } dd { "77개" } }
                        div { dt { "가입 계정" } dd { "94,730개" } }
                    }
                }
            }
        });
        dom.rebuild_in_place();
        let html = dioxus::ssr::render(&dom);

        assert!(
            html.contains("<h2 class=\"landing-definition-title\""),
            "{html}"
        );
        assert!(
            html.contains(
                "연합우주(Fediverse)는 각자 운영하는 여러 소셜 서버가 연결된 네트워크입니다."
            ),
            "{html}"
        );
        assert!(html.contains("한 곳에 가입해도, 다른 서버의 사람들과 만나고 대화할 수 있어요."));
        let definition = html.find("landing-definition").expect("definition");
        let statistics = html.find("landing-stat-values").expect("statistics");
        assert!(definition < statistics, "{html}");
    }
}
