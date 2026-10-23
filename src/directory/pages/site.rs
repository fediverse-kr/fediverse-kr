use super::operator::{OperatorEditFooter, OperatorInformation};
use super::*;

#[component]
pub(super) fn SiteDetailPage(site: PublicSite, preview: bool) -> Element {
    let health_domain = site.domain.clone();
    let discussion_domain = site.domain.clone();
    rsx! {
        SiteDetailFrame {
            site,
            preview,
            health_history: rsx! { health::HealthHistory { domain:health_domain } },
            opinions: rsx! { crate::community::pages::Discussion { domain:discussion_domain } }
        }
    }
}

#[component]
fn SiteDetailFrame(
    site: PublicSite,
    preview: bool,
    health_history: Element,
    opinions: Element,
) -> Element {
    let visit_url = if preview || site.closed {
        None
    } else {
        site_link(&site.domain)
    };
    rsx! {
        document::Stylesheet { href: asset!("/assets/styling/site-detail.css") }
        div { class:"server-detail-page",
            PreviewNote { preview }
            header { class:"server-detail-hero",
                icon::SiteHeader { site:site.clone() }
                div { class:"server-detail-identity",
                    icon::SiteIcon { site:site.clone() }
                    div { h1 { "{site.name}" } p { class:"server-detail-domain", "{site.domain}" } }
                    if !site.guidance.tags.is_empty() {
                        div { class:"tag-list server-identity-tags", aria_label:"서버 주제",
                            for tag in &site.guidance.tags { span { "{tag}" } }
                        }
                    }
                }
            }
            nav { class:"server-detail-nav", aria_label:"이 페이지에서 보기",
                a { href:"#site-intro", "소개" }
                a { href:"#site-guidance", "가입 전 확인" }
                a { href:"#site-collected", "운영 현황" }
                a { href:"#site-opinions", "의견" }
            }
            div { class:"server-detail-content",
                section { id:"site-intro", class:"server-detail-section", aria_labelledby:"site-intro-title",
                    h2 { id:"site-intro-title", "이곳의 소개" }
                    if site.description_html.is_empty() {
                        p { class:"server-description", "아직 수집된 소개가 없어요. 서버에서 직접 확인해 주세요." }
                    } else {
                        div { class:"server-description rich-description", dangerous_inner_html:"{site.description_html}" }
                    }
                }
                aside { class:"server-decision-rail", aria_label:"가입과 서버 정보",
                    section { id:"site-join-check", class:"server-join-status", aria_labelledby:"site-join-title",
                        span { class:"server-section-eyebrow", "이곳에서 시작하기" }
                        h2 { id:"site-join-title", "{site.registration()}" }
                        p {
                            if preview { "검토용 예시라 실제로 방문하거나 가입할 수 없어요." }
                            else if site.closed { "운영 종료로 기록된 서버예요." }
                            else { "가입하기 전에 이곳의 규칙과 최신 안내를 확인해 주세요." }
                        }
                        if let Some(url)=visit_url {
                            a { class:"primary-button", href:url, target:"_blank", rel:"noopener noreferrer", "서버 방문하기" ArrowRight { size:18 } }
                        }
                    }
                    div { class:"server-glance",
                        p { class:if site.alive==Some(true) && !site.closed {"server-response responding"} else {"server-response"},
                            span { aria_hidden:"true" } "응답 상태 · {site.status()}"
                        }
                        dl { class:"server-glance-software",
                            dt { "사용하는 소프트웨어" }
                            dd { if let Some(name)=&site.software { Link { to:Route::SoftwareDetail { name:name.clone() }, "{name}" ArrowRight { size:16 } } } else { "확인 전" } }
                        }
                        dl { class:"server-population",
                            div { dt { "가입 계정" } dd { {site.users.map(number).unwrap_or_else(||"알 수 없음".into())} } }
                            div { dt { "활동 계정" } dd { {site.active_users.map(number).unwrap_or_else(||"알 수 없음".into())} } }
                        }
                        p { class:"server-collected-note", "계정 수는 사람 수가 아니에요. 활동 계정의 집계 기간은 소프트웨어마다 다를 수 있어요." }
                    }
                    Link { class:"server-follow-tip", to:Route::Explain { topic:"find".into() },
                        strong { "이미 다른 서버에 계신가요?" }
                        span { "다시 가입하지 않고 팔로우하기" ArrowRight { size:16 } }
                    }
                }
                section { id:"site-guidance", class:"server-detail-section", aria_labelledby:"site-guidance-title",
                    h2 { id:"site-guidance-title", "가입 전 확인" }
                    div { class:"server-guidance-row server-rules",
                        strong { "이곳의 규칙" }
                        p { class:"server-description", if site.guidance.rules.is_empty() { "등록된 규칙이 없어요. 가입 전 서버에서 직접 확인해 주세요." } else { "{site.guidance.rules}" } }
                    }
                    if !site.guidance.language.is_empty() {
                        div { class:"server-guidance-row", strong { "주로 쓰는 언어" } p { "{site.guidance.language}" } }
                    }
                }
                OperatorInformation { owner_comment:site.guidance.owner_comment.clone() }
                section { id:"site-collected", class:"server-detail-section server-collected", aria_labelledby:"site-collected-title",
                    h2 { id:"site-collected-title", "운영 현황" }
                    p { class:"server-collected-intro", "서버에서 공개한 정보와 자동 확인 결과예요. 수집 이후 바뀌었을 수 있어요." }
                    div { class:"server-operations-grid",
                        div { class:"server-history-panel", {health_history} }
                        dl { class:"server-collected-facts",
                        dt { "응답 상태" } dd { "{site.status()}" }
                        dt { "응답 확인 시각" } dd { {site.checked_at.as_deref().unwrap_or("확인 전")} }
                        dt { "정보 수집 시각" } dd { {site.metadata_checked_at.as_deref().unwrap_or("수집 시각 미확인")} }
                        dt { "7일 평균 응답" } dd { {site.average_response_ms.map(|n|format!("{n} ms")).unwrap_or_else(||"자료 부족".into())} }
                        if let Some(version)=&site.version { dt { "소프트웨어 버전" } dd { "{version}" } }
                        }
                    }
                }
            }
            div { id:"site-opinions", class:"server-detail-opinions",
                {opinions}
            }
            footer { class:"server-operator-actions",
                div {
                    h2 { "이 서버를 운영하시나요?" }
                    p { "DNS TXT로 도메인 제어권을 확인하면 소개와 규칙, 운영 안내를 직접 수정할 수 있어요." }
                }
                OperatorEditFooter {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dioxus_history::{History, MemoryHistory};
    use dioxus_router::components::HistoryProvider;
    use std::rc::Rc;

    fn fixture(closed: bool) -> PublicSite {
        PublicSite {
            icon_available: false,
            header_available: false,
            guidance: SiteGuidance {
                rules: "서로 존중해 주세요.".into(),
                language: "한국어".into(),
                tags: vec!["대화".into()],
                owner_comment: "운영 안내 예시".into(),
                invite_only: Some(false),
                approval_required: Some(false),
            },
            domain: "example.social".into(),
            name: "예시 서버".into(),
            description: "간단한 소개".into(),
            description_html: "<p>간단한 소개</p>".into(),
            software: Some("mastodon".into()),
            version: None,
            closed,
            alive: Some(true),
            registration_open: Some(true),
            users: None,
            active_users: None,
            checked_at: Some("2026-10-23 10:00 KST".into()),
            metadata_checked_at: Some("2026-10-23 10:00 KST".into()),
            average_response_ms: None,
        }
    }

    #[derive(Clone, Debug, PartialEq, Routable)]
    enum TestRoute {
        #[route("/")]
        Fixture {},
    }

    #[component]
    fn TestApp(site: PublicSite, preview: bool) -> Element {
        use_context_provider(|| Signal::new(false));
        use_context_provider(|| Signal::new((site, preview)));
        rsx! {
            HistoryProvider {
                history: move |_| Rc::new(MemoryHistory::with_initial_path("/")) as Rc<dyn History>,
                Router::<TestRoute> {}
            }
        }
    }

    #[component]
    fn Fixture() -> Element {
        let (site, preview) = use_context::<Signal<(PublicSite, bool)>>()();
        let health = rsx! { p { "최근 응답 기록 테스트 자리" } };
        let opinions = rsx! { p { "의견 테스트 자리" } };
        rsx! { SiteDetailFrame { site, preview, health_history:health, opinions } }
    }

    fn render(site: PublicSite, preview: bool) -> String {
        let mut dom = VirtualDom::new_with_props(TestApp, TestAppProps { site, preview });
        dom.rebuild_in_place();
        dioxus::ssr::render(&dom)
    }

    #[test]
    fn visitor_guidance_precedes_collected_technical_information_and_has_visible_anchors() {
        let html = render(fixture(false), false);
        let intro = html.find("id=\"site-intro-title\"").unwrap();
        let rules = html.find("id=\"site-guidance-title\"").unwrap();
        let operator = html.find("aria-label=\"운영 정보\"").unwrap();
        let collected = html.find("id=\"site-collected-title\"").unwrap();
        assert!(
            intro < rules && rules < operator && operator < collected,
            "{html}"
        );
        for id in [
            "site-intro",
            "site-guidance",
            "site-collected",
            "site-opinions",
        ] {
            assert!(
                html.contains(&format!("href=\"#{id}\"")),
                "missing anchor {id}: {html}"
            );
        }
        assert!(html.contains("id=\"site-join-check\""), "{html}");
        assert_eq!(html.matches("<h1").count(), 1, "{html}");
    }

    #[test]
    fn preview_and_closed_servers_never_render_a_real_visit_link() {
        let live = render(fixture(false), false);
        assert!(live.contains("서버 방문하기"), "{live}");
        assert!(live.contains("href=\"https://example.social/\""), "{live}");

        let preview = render(fixture(false), true);
        assert!(
            preview.contains("검토용 예시라 실제로 방문하거나 가입할 수 없어요."),
            "{preview}"
        );
        assert!(
            !preview.contains("href=\"https://example.social"),
            "{preview}"
        );

        let closed = render(fixture(true), false);
        assert!(closed.contains("운영 종료로 기록된 서버예요."), "{closed}");
        assert!(
            !closed.contains("href=\"https://example.social"),
            "{closed}"
        );
    }

    #[test]
    fn unknown_counts_are_not_rendered_as_zero() {
        let html = render(fixture(false), false);
        assert!(html.contains("가입 계정</dt><dd>알 수 없음"), "{html}");
        assert!(html.contains("활동 계정</dt><dd>알 수 없음"), "{html}");
        assert!(!html.contains("가입 계정</dt><dd>0"), "{html}");
        assert!(!html.contains("활동 계정</dt><dd>0"), "{html}");
    }
}
