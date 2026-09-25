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
mod catalog;

#[component]
pub fn SoftwareDetail(name: String) -> Element {
    rsx! { SoftwareDetails { key:"{name}", name } }
}
#[component]
fn SoftwareDetails(name: String) -> Element {
    let catalog = use_server_future(use_reactive((&name,), |(name,)| api::software(name)))?;
    rsx! { main { id:"content", class:"detail-page wrap",
        Link { class:"back-link", to:Route::Platforms { filters: Default::default() }, ArrowLeft { size:16 } "소프트웨어 종류" }
        match catalog() {
            Some(Ok(data))=> {
                let item=data.software.as_ref();
                match item {
                    Some(item)=>rsx! {
                        document::Title { "{item.display_name} — 소프트웨어 — fediverse.kr" }
                        PreviewNote { preview:data.preview }
                        header { class:"page-heading", h1 { class:"software-title", if item.logo_available{crate::media_ui::ImageMark{source:crate::media_ui::software_source(&item.name),fallback:item.display_name.chars().next().unwrap_or('·').to_string(),class:"software-mark",size:56}} span { "{item.display_name}" } } if !item.description_html.is_empty() { div { class:"rich-description", dangerous_inner_html:"{item.description_html}" } } }
                        div { class:"detail-layout",
                            section { class:"information-prose",
                                h2 { "이런 종류의 공간이에요" }
                                div { class:"tag-list", for category in &item.categories { span { "{data.categories.iter().find(|c|&c.name==category).map(|c|c.label.as_str()).unwrap_or(category)}" } } }
                                if !item.features.is_empty() { h2 { "주요 기능" } ul { for feature in &item.features { li { "{feature}" } } } }
                                if let Some(tech)=&item.tech_stack { details { summary { "어떤 기술로 만들어졌나요?" } p { "{tech}" } } }
                                h2 { "같은 도구여도, 분위기는 달라요." } p { "규칙과 가입 방식은 서버 운영자가 정해요. 마음에 드는 곳의 안내를 읽어보세요." }
                            }
                            aside { class:"join-panel", h2 { "직접 써보고 싶다면" }
                                Link { class:"primary-button", to:Route::SoftwareServers { name:item.name.clone() }, "이 소프트웨어의 서버 찾기" }
                                if let Some(url)=item.website_url.as_deref().and_then(web_link) { a { class:"text-link", href:url, target:"_blank", rel:"noopener noreferrer ugc", "프로젝트 웹사이트" ArrowRight { size:16 } } }
                                p { "서버 수집 결과에 기록된 소프트웨어 이름을 기준으로 찾아요." }
                            }
                        }
                        div {class:"membership-actions catalog-contribute",Link {class:"secondary-button",to:Route::SoftwareEditor{name:item.name.clone()},"정보 수정"}Link {class:"text-link",to:Route::SoftwareHistory{name:item.name.clone()},"변경 이력"}}
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
    rsx! { main { id:"content", class:"detail-page wrap",
        Link { class:"back-link", to:Route::Servers { filters: Default::default() }, ArrowLeft { size:16 } "서버 목록" }
        match result() {
            Some(Ok(data))=>rsx! {
                PreviewNote { preview:data.preview }
                if let Some(site)=data.site {
                    document::Title { "{site.name} — 서버 정보 — fediverse.kr" }
                    header { class:"page-heading", icon::SiteIcon{site:site.clone()} h1 { "{site.name}" } p { "{site.domain}" } }
                    div { class:"detail-layout",
                        section { class:"information-prose",
                            h2 { "이곳의 소개" }
                            if site.description_html.is_empty() { p { class:"server-description", "아직 수집된 소개가 없어요. 서버에서 직접 확인해 주세요." } } else { div { class:"server-description rich-description", dangerous_inner_html:"{site.description_html}" } }
                            h2 { "최근 확인한 정보" }
                            dl { class:"server-facts",
                                dt { "응답 상태" } dd { "{site.status()}" }
                                dt { "응답 확인 시각" } dd { {site.checked_at.as_deref().unwrap_or("확인 전")} }
                                dt { "소프트웨어" } dd { if let Some(name)=&site.software { Link { to:Route::SoftwareDetail { name:name.clone() }, "{name}" } } else { "확인 전" } }
                                dt { "가입 계정" } dd { {site.users.map(number).unwrap_or_else(||"알 수 없음".into())} }
                                dt { "활동 계정" } dd { {site.active_users.map(number).unwrap_or_else(||"알 수 없음".into())} }
                                dt { "위 정보 수집 시각" } dd { {site.metadata_checked_at.as_deref().unwrap_or("수집 시각 미확인")} }
                                dt { "7일 평균 응답" } dd { {site.average_response_ms.map(|n|format!("{n} ms")).unwrap_or_else(||"자료 부족".into())} }
                            }
                            p { class:"review-note", "계정 수는 사람 수가 아니에요. 활동 계정의 집계 기간은 소프트웨어마다 다르며, 수집 이후 변경되었을 수 있어요." }
                            health::HealthHistory { domain:site.domain.clone() }
                            if !site.guidance.language.is_empty() {h2 {"주로 쓰는 언어"}p {"{site.guidance.language}"}}
                            if !site.guidance.tags.is_empty() {div {class:"tag-list",for tag in &site.guidance.tags {span {"{tag}"}}}}
                            h2 {"이곳의 규칙"}
                            p {class:"server-description",if site.guidance.rules.is_empty() {"등록된 규칙이 없어요. 가입 전 서버에서 직접 확인해 주세요."}else{"{site.guidance.rules}"}}
                            operator::OperatorInformation { owner_comment:site.guidance.owner_comment.clone() }
                        }
                        aside { class:"join-panel", h2 { "{site.registration()}" }
                            if !data.preview && !site.closed {
                                if let Some(url)=site_link(&site.domain) { a { class:"primary-button", href:url, target:"_blank", rel:"noopener noreferrer", "서버 방문하기" } }
                            }
                            p { if data.preview { "예시 주소는 열 수 없어요." } else if site.closed { "운영 종료로 기록된 서버예요." } else { "승인·초대·운영 규칙은 해당 사이트의 안내를 확인하세요." } }
                            Link { class:"text-link", to:Route::Explain { topic:"find".into() }, "다른 서버의 사람을 팔로우하려면" }
                        }
                    }
                    crate::community::pages::Discussion{domain:site.domain.clone()}
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
