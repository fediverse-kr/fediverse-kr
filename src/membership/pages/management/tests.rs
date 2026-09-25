use super::*;
use dioxus_history::{History, MemoryHistory};
use dioxus_router::components::HistoryProvider;
use std::rc::Rc;

#[derive(Clone, Debug, PartialEq, Routable)]
enum TestRoute {
    #[route("/")]
    ConsentPage {},
}

#[component]
fn TestApp(public: bool) -> Element {
    use_context_provider(|| public);
    use_context_provider(|| Signal::new(true));
    rsx! { HistoryProvider {
        history: move |_| Rc::new(MemoryHistory::with_initial_path("/")) as Rc<dyn History>,
        Router::<TestRoute> {}
    } }
}

#[component]
fn ConsentPage() -> Element {
    let public = use_context::<bool>();
    let pending = use_signal(|| false);
    rsx! { ul { LinkedAccountRow {
        account: LinkedAccount {
            id: "consent-example".into(),
            handle: "@reader@social.example".into(),
            profile_url: "https://social.example/@reader".into(),
            display_name: "예시 계정".into(),
            is_public: public,
        },
        can_unlink: true, pending, on_changed: |_| {},
    } } }
}

fn render(public: bool) -> String {
    let mut dom = VirtualDom::new_with_props(TestApp, TestAppProps { public });
    dom.rebuild_in_place();
    dioxus::ssr::render(&dom)
}

#[test]
fn directory_consent_names_scope_without_promising_live_publication() {
    for public in [false, true] {
        let html = render(public);
        for expected in [
            "fediverse.kr의 사람 찾기 디렉터리",
            "이 계정의 프로필과 연합 주소",
            "허용하지 않아도 로그인과 서버 등록",
            "원래 SNS의 공개 범위",
            "현재는 동의만 저장",
            "아직 시안",
            "aria-describedby=\"directory-consent-consent-example\"",
            "id=\"directory-consent-consent-example\"",
        ] {
            assert!(
                html.contains(expected),
                "public={public}: missing {expected}\n{html}"
            );
        }
        if public {
            assert!(html.contains("디렉터리 표시 허용됨"));
            assert!(html.contains("디렉터리 표시 허용 취소"));
            assert!(!html.contains("비공개로 변경"));
        } else {
            assert!(html.contains("디렉터리 표시 안 함"));
            assert!(html.contains("프로필을 디렉터리에 표시 허용"));
        }
        // Optional local preview is real component output, not hand-copied markup.
        if let Ok(directory) = std::env::var("FEDKR_CONSENT_PREVIEW_DIR") {
            std::fs::write(
                std::path::Path::new(&directory).join(format!("consent-{public}.html")),
                html,
            )
            .unwrap();
        }
    }
}
