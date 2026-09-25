use crate::branding::BrandMark;
use crate::landing::{LandingDemo, RandomHeadline};
use crate::Route;
use dioxus::prelude::*;
use lucide_dioxus::{Network, PanelsTopLeft, Server};

#[component]
pub fn NotFound(segments: Vec<String>) -> Element {
    response_status(404);
    rsx! { document::Title { "페이지를 찾을 수 없어요 — fediverse.kr" } main { id: "content", class: "empty-state wrap not-found", h1 { "아직 없는 페이지예요." } p { "주소를 확인하거나 첫 화면으로 돌아가 주세요." } Link { class: "primary-button", to: Route::Home {}, "첫 화면으로" } } }
}

pub fn response_status(code: u16) {
    #[cfg(feature = "server")]
    if let Ok(status) = dioxus::fullstack::http::StatusCode::from_u16(code) {
        dioxus::fullstack::FullstackContext::commit_http_status(status, None);
    }
    #[cfg(not(feature = "server"))]
    let _ = code;
}

#[component]
pub fn Shell() -> Element {
    let route = use_route::<Route>();
    let mut mobile_menu_open = use_signal(|| false);
    let start_active = matches!(route, Route::Start {} | Route::Explain { .. });
    let platforms_active = matches!(
        route,
        Route::Platforms { .. } | Route::SoftwareDetail { .. }
    );
    let servers_active = matches!(
        route,
        Route::Servers { .. } | Route::ServerDetail { .. } | Route::SoftwareServers { .. }
    );
    let community_active = matches!(route, Route::Community {});
    let operate_active = matches!(route, Route::Operate {} | Route::SelfHosting {});
    let develop_active = matches!(route, Route::Develop {});
    let account_active = matches!(route, Route::Login {} | Route::Account {});

    rsx! {
        a { class: "skip-link", href: "#content", "본문으로 건너뛰기" }
        header { class: "site-header",
            nav { class: "site-nav wrap", aria_label: "주요 메뉴",
                button {
                    id: "mobile-menu-toggle",
                    class: "mobile-menu-toggle",
                    r#type: "button",
                    aria_label: "주요 메뉴 열기",
                    aria_expanded: mobile_menu_open(),
                    onclick: move |_| mobile_menu_open.set(true),
                    span { aria_hidden: "true" }
                    span { aria_hidden: "true" }
                    span { aria_hidden: "true" }
                }
                Link { class: "brand", to: Route::Home {}, BrandMark {} span { "fediverse.kr" } }
                div { class: "nav-links",
                    PrimaryNavLink { active: start_active, to: Route::Start {}, label: "연합우주 이해하기" }
                    PrimaryNavLink { active: platforms_active, to: Route::Platforms { filters: Default::default() }, label: "소프트웨어" }
                    PrimaryNavLink { active: servers_active, to: Route::Servers { filters: Default::default() }, label: "서버 찾기" }
                    PrimaryNavLink { active: community_active, to: Route::Community {}, label: "커뮤니티" }
                    PrimaryNavLink { active: operate_active, to: Route::Operate {}, label: "서버 운영" }
                    PrimaryNavLink { active: develop_active, to: Route::Develop {}, label: "개발" }
                    PrimaryNavLink { active: account_active, to: Route::Account {}, label: "내 계정" }
                }
            }
        }
        if mobile_menu_open() {
            dialog {
                id: "mobile-navigation",
                class: "mobile-navigation",
                role: "dialog",
                aria_modal: "true",
                aria_label: "주요 메뉴",
                onmounted: move |_| open_mobile_navigation(),
                oncancel: move |event| {
                    event.prevent_default();
                    close_mobile_navigation();
                    mobile_menu_open.set(false);
                    restore_mobile_menu_focus();
                },
                div { class: "mobile-navigation-heading",
                    h2 { "메뉴" }
                    button {
                        id: "mobile-menu-close",
                        class: "mobile-menu-close",
                        r#type: "button",
                        aria_label: "주요 메뉴 닫기",
                        onclick: move |_| {
                            close_mobile_navigation();
                            mobile_menu_open.set(false);
                            restore_mobile_menu_focus();
                        },
                        span { aria_hidden: "true", "×" }
                    }
                }
                nav { class: "mobile-navigation-links", aria_label: "주요 메뉴",
                    PrimaryNavLink { active: start_active, to: Route::Start {}, label: "연합우주 이해하기", mobile_menu_open: Some(mobile_menu_open) }
                    PrimaryNavLink { active: platforms_active, to: Route::Platforms { filters: Default::default() }, label: "소프트웨어", mobile_menu_open: Some(mobile_menu_open) }
                    PrimaryNavLink { active: servers_active, to: Route::Servers { filters: Default::default() }, label: "서버 찾기", mobile_menu_open: Some(mobile_menu_open) }
                    PrimaryNavLink { active: community_active, to: Route::Community {}, label: "커뮤니티", mobile_menu_open: Some(mobile_menu_open) }
                    PrimaryNavLink { active: operate_active, to: Route::Operate {}, label: "서버 운영", mobile_menu_open: Some(mobile_menu_open) }
                    PrimaryNavLink { active: develop_active, to: Route::Develop {}, label: "개발", mobile_menu_open: Some(mobile_menu_open) }
                    PrimaryNavLink { active: account_active, to: Route::Account {}, label: "내 계정", mobile_menu_open: Some(mobile_menu_open) }
                }
            }
        }
        SuspenseBoundary {
            fallback: |_| rsx! { RouteLoadingFallback {} },
            Outlet::<Route> {}
        }
        footer { class: "site-footer wrap",
            Link { to:Route::About{}, "fediverse.kr 소개" }
            p { "하나의 앱이 아닌, 서로 연결된 우리의 공간." }
            Link { to:Route::Apps{}, "앱으로 이용하기" }
            a { href: "https://github.com/fediverse-kr/fediverse-kr", "GitHub · 의견과 기여" }
        }
    }
}

#[component]
fn RouteLoadingFallback() -> Element {
    rsx! {
        div {
            class: "route-loading-bar",
            role: "progressbar",
            aria_label: "페이지를 불러오는 중",
            aria_valuetext: "페이지를 불러오는 중",
        }
        main {
            id: "content",
            class: "route-loading-content wrap",
            aria_busy: "true",
            p { class: "route-loading-message", role: "status", "페이지를 불러오는 중이에요." }
        }
    }
}

#[component]
fn PrimaryNavLink(
    active: bool,
    to: Route,
    label: &'static str,
    mobile_menu_open: Option<Signal<bool>>,
) -> Element {
    rsx! {
        Link {
            class: if active { "active" } else { "" },
            to,
            onclick: move |_| {
                if let Some(mut menu_open) = mobile_menu_open {
                    close_mobile_navigation();
                    menu_open.set(false);
                }
            },
            "{label}"
        }
    }
}

fn open_mobile_navigation() {
    let _ = document::eval(
        r#"
        const dialog = document.getElementById('mobile-navigation');
        if (dialog && !dialog.open) dialog.showModal();
        document.getElementById('mobile-menu-close')?.focus();

        const controller = new AbortController();
        const dismissForDesktop = () => {
            if (window.matchMedia('(min-width: 1101px)').matches) {
                document.getElementById('mobile-menu-close')?.click();
            }
        };
        window.addEventListener('resize', dismissForDesktop, { signal: controller.signal });
        dialog?.addEventListener('close', () => controller.abort(), { once: true });
        "#,
    );
}

fn close_mobile_navigation() {
    let _ = document::eval(r#"document.getElementById("mobile-navigation")?.close();"#);
}

fn restore_mobile_menu_focus() {
    let _ = document::eval(
        r#"requestAnimationFrame(() => {
            requestAnimationFrame(() => document.getElementById("mobile-menu-toggle")?.focus());
        });"#,
    );
}

#[component]
pub fn Community() -> Element {
    rsx! {
        document::Title { "한국어 연합우주 커뮤니티 — fediverse.kr" }
        main { id: "content", class: "community-page wrap",
            header { class: "page-heading",
                h1 { "한국어 연합우주 커뮤니티" }
                p { "fediverse.kr와 별도로 운영되는 독립적인 한국어 연합우주 커뮤니티와 프로젝트입니다. 각자의 활동과 안내를 직접 확인해 보세요." }
            }
            section { class: "community-resource-list", aria_label: "독립 외부 커뮤니티와 프로젝트",
                a { class: "community-resource", href: "https://fedidev.kr", target: "_blank", rel: "noopener noreferrer",
                    span { class: "community-resource-kind", "독립 커뮤니티" }
                    h2 { "한국 연합우주 개발자 모임" }
                    p { "연합우주를 만드는 개발자들이 소통하고 정보를 나누는 모임" }
                    span { class: "community-resource-link", "fedidev.kr →" }
                }
                a { class: "community-resource", href: "https://joinfediverse.kr", target: "_blank", rel: "noopener noreferrer",
                    span { class: "community-resource-kind", "독립 프로젝트" }
                    h2 { "연합우주 둘러보기 프로젝트" }
                    p { "연합우주를 처음 둘러볼 곳을 찾는 프로젝트" }
                    span { class: "community-resource-link", "joinfediverse.kr →" }
                }
            }
        }
    }
}

#[component]
pub fn Home() -> Element {
    rsx! {
        document::Title { "fediverse.kr — 연합우주를 만나보세요" }
        main { id: "content", class: "landing-page",
            section { class: "landing-hero wrap",
                div { class: "landing-copy",
                    RandomHeadline {}
                    p { "fediverse.kr에서 " span { "연합우주에 대해 알아보세요." } }
                    div { class: "landing-actions",
                        Link { class: "primary-button landing-learn-link", to: Route::Start {}, aria_label: "연합우주 알아보기", "연합우주 알아보기" }
                        Link { class: "landing-join-link", to: "/servers", "가입할 곳을 찾고 있나요? →" }
                    }
                }
                LandingDemo {}
            }
            crate::directory::pages::LandingStatistics {}
            section { class: "entry-section", div { class: "wrap",
                h2 { "어디서부터 알아볼까요?" }
                div { class: "entry-links",
                    Link { to: Route::Platforms { filters: Default::default() }, class: "entry-link", PanelsTopLeft { size: 23 } h3 { "플랫폼 둘러보기" } p { "다양한 소프트웨어를 알아보아요." } }
                    Link { to: Route::Servers { filters: Default::default() }, class: "entry-link", Server { size: 23 } h3 { "가입할 서버 알아보기" } p { "내가 머물 공간을 찾아보세요." } }
                    Link { to: Route::Start {}, class: "entry-link", Network { size: 23 } h3 { "연합우주 이해하기" } p { "동작 원리를 쉽게 알려드려요." } }
                }
            } }
        }
    }
}

#[component]
pub fn Start() -> Element {
    rsx! { crate::explainer::Basics {} }
}

#[cfg(test)]
mod tests {
    use super::*;

    use dioxus_history::{History, MemoryHistory};
    use dioxus_router::components::HistoryProvider;
    use std::rc::Rc;

    #[component]
    fn PortalTestApp(path: String) -> Element {
        let ready = use_signal(|| true);
        use_context_provider(move || ready);
        rsx! {
            HistoryProvider {
                history: move |_| Rc::new(MemoryHistory::with_initial_path(&path)) as Rc<dyn History>,
                Router::<Route> {}
            }
        }
    }

    fn render_path(path: &str) -> String {
        let mut dom = VirtualDom::new_with_props(
            PortalTestApp,
            PortalTestAppProps {
                path: path.to_string(),
            },
        );
        dom.rebuild_in_place();
        dioxus::ssr::render(&dom)
    }

    #[test]
    fn route_loading_fallback_exposes_an_accessible_indeterminate_progress_bar() {
        let mut dom = VirtualDom::new(|| rsx! { RouteLoadingFallback {} });
        dom.rebuild_in_place();
        let html = dioxus::ssr::render(&dom);

        assert!(html.contains("class=\"route-loading-bar\""), "{html}");
        assert!(html.contains("role=\"progressbar\""), "{html}");
        assert!(
            html.contains("aria-label=\"페이지를 불러오는 중\""),
            "{html}"
        );
        assert!(html.contains("id=\"content\""), "{html}");
        assert!(html.contains("aria-busy=\"true\""), "{html}");
        assert!(html.contains("페이지를 불러오는 중이에요."), "{html}");
    }

    #[test]
    fn footer_links_to_github_contributions() {
        let html = render_path("/");
        let footer = html.split("<footer").nth(1).expect("site footer");
        let footer = footer.split("</footer>").next().unwrap();
        assert!(footer.contains("href=\"https://github.com/fediverse-kr/fediverse-kr\""));
        assert!(footer.contains(">GitHub · 의견과 기여</a>"));
    }

    #[test]
    fn home_hero_offers_explanation_and_server_finding_actions() {
        let html = render_path("/");
        let primary = html
            .split("<a ")
            .find(|anchor| anchor.contains("landing-learn-link"))
            .expect("Home hero must include a primary explanation link");
        assert!(primary.contains("href=\"/start\""));
        assert!(primary.contains("aria-label=\"연합우주 알아보기\""));
        assert!(primary.contains(">연합우주 알아보기</a>"));

        let secondary = html
            .split("<a ")
            .find(|anchor| anchor.contains("landing-join-link"))
            .expect("Home hero must include a server-finding link");
        assert!(
            secondary.contains("href=\"/servers\""),
            "secondary anchor: {secondary}"
        );
        assert!(secondary.contains(">가입할 곳을 찾고 있나요? →</a>"));
        assert!(!html.contains(">자세히 알아보기</a>"));
    }

    #[test]
    fn community_has_its_own_primary_navigation_route_and_leaves_home_focused() {
        let home = render_path("/");
        assert!(
            home.contains("href=\"/community\"") && home.contains(">커뮤니티</a>"),
            "global navigation must link to the community page: {home}"
        );
        assert!(
            !home.contains("id=\"community-resources\"")
                && !home.contains("landing-community-link")
                && !home.contains("한국 연합우주 개발자 모임"),
            "community resources and CTA must no longer render on home: {home}"
        );

        let community = render_path("/community");
        assert!(
            community.contains("<h1>한국어 연합우주 커뮤니티</h1>"),
            "community route must expose the approved page heading: {community}"
        );
        let active_nav = community
            .split("<a ")
            .find(|anchor| anchor.contains("href=\"/community\""))
            .expect("community route must remain reachable from primary navigation");
        assert!(active_nav.contains("class=\"active\""), "{active_nav}");
        assert!(
            !community.contains(">사람 찾기</a>"),
            "people must be removed from primary navigation: {community}"
        );
    }

    #[test]
    fn community_identifies_independent_projects_and_opens_them_safely() {
        let html = render_path("/community");

        assert!(
            html.contains("fediverse.kr와 별도로 운영되는"),
            "the page must identify the listed destinations as independent: {html}"
        );
        for (href, label, purpose) in [
            (
                "https://fedidev.kr",
                "한국 연합우주 개발자 모임",
                "연합우주를 만드는 개발자들이 소통하고 정보를 나누는 모임",
            ),
            (
                "https://joinfediverse.kr",
                "연합우주 둘러보기 프로젝트",
                "연합우주를 처음 둘러볼 곳을 찾는 프로젝트",
            ),
        ] {
            let link = html
                .split("<a ")
                .find(|anchor| anchor.contains(href))
                .unwrap_or_else(|| panic!("missing resource link {href}: {html}"));
            assert!(link.contains("target=\"_blank\""), "{link}");
            assert!(link.contains("rel=\"noopener noreferrer\""), "{link}");
            assert!(link.contains(label), "{link}");
            assert!(link.contains(purpose), "{link}");
        }
    }

    #[test]
    fn people_route_remains_available_outside_primary_navigation() {
        let html = render_path("/people");

        assert!(
            html.contains("<h1>다른 곳의 나, 여기의 이웃.</h1>"),
            "{html}"
        );
        assert!(html.contains("공개 범위 검토안"), "{html}");
        assert!(
            !html.contains("href=\"/people\""),
            "people remains a route but must not appear in primary navigation: {html}"
        );
    }
}
