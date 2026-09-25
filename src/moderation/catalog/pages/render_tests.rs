use super::*;
use dioxus_history::{History, MemoryHistory};
use dioxus_router::components::HistoryProvider;
use std::rc::Rc;

#[derive(Clone, Debug, PartialEq, Routable)]
enum TestRoute {
    #[route("/")]
    TestPage {},
}

#[component]
fn TestApp(categories: bool) -> Element {
    use_context_provider(|| categories);
    rsx! { HistoryProvider {
        history: move |_| Rc::new(MemoryHistory::with_initial_path("/")) as Rc<dyn History>,
        Router::<TestRoute> {}
    } }
}

#[component]
fn TestPage() -> Element {
    let categories = use_context::<bool>();
    rsx! { CatalogPage { title: "작업 제목", description: "작업 설명", categories,
        p { id: "actual-content", "작업 본문" }
    } }
}

#[test]
fn catalog_shell_renders_single_landmarks_and_correct_active_subnavigation() {
    for categories in [false, true] {
        let mut dom = VirtualDom::new_with_props(TestApp, TestAppProps { categories });
        dom.rebuild_in_place();
        let html = dioxus::ssr::render(&dom);
        assert_eq!(html.matches("<main").count(), 1, "{html}");
        assert_eq!(html.matches("<h1").count(), 1, "{html}");
        assert!(html.contains("id=\"actual-content\""));
        let start = html.find("class=\"catalog-subnav\"").unwrap();
        let nav = html[start..].split("</nav>").next().unwrap();
        assert_eq!(nav.matches("aria-current=\"page\"").count(), 1, "{nav}");
        let href = if categories {
            "/account/moderation/catalog/categories"
        } else {
            "/account/moderation/catalog"
        };
        let active = nav
            .split("<a ")
            .find(|a| a.contains("aria-current=\"page\""))
            .unwrap();
        assert!(active.contains(&format!("href=\"{href}\"")), "{active}");
    }
}
