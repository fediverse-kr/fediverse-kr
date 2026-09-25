use crate::Route;
use dioxus::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackofficeSection {
    Overview,
    Reports,
    Sites,
    Catalog,
    Operations,
}

impl BackofficeSection {
    fn label(self) -> &'static str {
        match self {
            Self::Overview => "운영 개요",
            Self::Reports => "신고",
            Self::Sites => "서버",
            Self::Catalog => "소프트웨어·분류",
            Self::Operations => "작업·유지보수",
        }
    }

    fn description(self) -> &'static str {
        match self {
            Self::Overview => "우선 확인할 운영 상태",
            Self::Reports => "신고 검토와 조치 이력",
            Self::Sites => "등록 서버 상태와 관리",
            Self::Catalog => "소프트웨어와 분류 관리",
            Self::Operations => "수집 작업과 프로필 유지보수",
        }
    }

    fn route(self) -> Route {
        match self {
            Self::Overview => Route::Moderation {},
            Self::Reports => Route::ModerationReports {},
            Self::Sites => Route::ModerationSites {},
            Self::Catalog => Route::ModerationCatalog {},
            Self::Operations => Route::ModerationWorkers {},
        }
    }
}

const SECTIONS: [BackofficeSection; 5] = [
    BackofficeSection::Overview,
    BackofficeSection::Reports,
    BackofficeSection::Sites,
    BackofficeSection::Catalog,
    BackofficeSection::Operations,
];

#[component]
fn SectionLinks(active: BackofficeSection, mobile: bool) -> Element {
    rsx! {
        nav {
            class: if mobile { "backoffice-nav backoffice-nav-mobile" } else { "backoffice-nav backoffice-nav-rail" },
            aria_label: "백오피스 영역",
            for section in SECTIONS {
                Link {
                    class: if section == active { "backoffice-nav-link is-active" } else { "backoffice-nav-link" },
                    to: section.route(),
                    aria_current: (section == active).then_some("page"),
                    span { class: "backoffice-nav-label", "{section.label()}" }
                    span { class: "backoffice-nav-description", "{section.description()}" }
                }
            }
        }
    }
}

#[component]
pub fn BackofficePage(
    section: BackofficeSection,
    title: String,
    description: String,
    children: Element,
) -> Element {
    rsx! {
        main { id: "content", class: "backoffice-page membership-page wrap",
            header { class: "backoffice-heading",
                p { class: "backoffice-eyebrow", "관리자 전용" }
                h1 { "{title}" }
                p { "{description}" }
            }
            details { class: "backoffice-mobile-sections",
                summary { "영역 이동 · {section.label()}" }
                SectionLinks { active: section, mobile: true }
            }
            div { class: "backoffice-workspace",
                aside { class: "backoffice-rail", aria_label: "백오피스 탐색",
                    p { class: "backoffice-rail-title", "백오피스" }
                    SectionLinks { active: section, mobile: false }
                    Link { class: "backoffice-account-link", to: Route::Account {}, "내 계정으로 돌아가기" }
                }
                div { class: "backoffice-body", {children} }
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

    #[derive(Clone, Debug, PartialEq, Routable)]
    enum LayoutTestRoute {
        #[route("/")]
        TestPage {},
    }

    #[component]
    fn TestApp() -> Element {
        rsx! {
            HistoryProvider {
                history: move |_| Rc::new(MemoryHistory::with_initial_path("/")) as Rc<dyn History>,
                Router::<LayoutTestRoute> {}
            }
        }
    }

    #[component]
    fn TestPage() -> Element {
        rsx! {
            BackofficePage {
                section: BackofficeSection::Reports,
                title: "신고".to_string(),
                description: "신고 검토 작업".to_string(),
                p { id: "test-child", "본문" }
            }
        }
    }

    #[test]
    fn shell_owns_one_main_heading_and_accessible_five_section_navigation() {
        let mut dom = VirtualDom::new(|| rsx! { TestApp {} });
        dom.rebuild_in_place();
        let html = dioxus::ssr::render(&dom);

        assert_eq!(html.matches("<main").count(), 1, "{html}");
        assert!(html.contains("id=\"content\""), "{html}");
        assert!(html.contains("<h1>신고</h1>"), "{html}");
        assert!(html.contains("aria-current=\"page\""), "{html}");
        for href in [
            "/account/moderation",
            "/account/moderation/reports",
            "/account/moderation/sites",
            "/account/moderation/catalog",
            "/account/moderation/workers",
        ] {
            assert!(html.contains(&format!("href=\"{href}\"")), "{html}");
        }
        assert!(html.contains("<details"), "{html}");
        assert!(html.contains("<summary"), "{html}");
        assert!(html.contains("id=\"test-child\""), "{html}");
    }
}
