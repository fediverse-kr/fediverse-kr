//! Reviewable editorial drafts, separate from membership permissions and storage.
use crate::Route;
use dioxus::prelude::*;
use lucide_dioxus::ArrowRight;
mod guides;
pub use guides::{Develop, Operate, SelfHosting};

pub const REGISTRATION_REQUEST_URL: &str =
    "https://github.com/fediverse-kr/fediverse-kr/issues/new";

#[component]
fn InfoPage(title: String, intro: String, children: Element) -> Element {
    rsx! {
        document::Title { "{title} — fediverse.kr" }
        document::Stylesheet { href: asset!("/assets/styling/information.css") }
        main { id:"content", class:"information-page wrap",
            header { class:"page-heading", h1 { "{title}" } p { "{intro}" } }
            {children}
        }
    }
}

#[component]
pub fn People() -> Element {
    let mut mode = use_signal(|| 0usize);
    let ready = use_context::<Signal<bool>>()();
    rsx! { InfoPage { title:"다른 곳의 나, 여기의 이웃.", intro:"계정의 주인임을 확인하는 일과, 나를 소개하는 일은 따로예요.",
        div { class:"data-notice", span { "공개 범위 검토안" } p { "아래는 공개 방식의 예시입니다. 선택해도 저장되거나 실제 회원이 공개되지 않아요." } }
        div { class:"people-preview",
            div { class:"choice-list", role:"group", aria_label:"공개 방식 예시",
                for (i,title,description) in [(0,"인증만 하고 싶어요","이 사이트에서만 계정의 주인을 확인"),(1,"링크로 나를 소개해요","주소를 아는 사람에게 공개 프로필 공유"),(2,"목록에서 발견되고 싶어요","공개 프로필을 사람 찾기에도 노출")] {
                    button { disabled:!ready, aria_pressed:mode()==i, onclick:move |_|mode.set(i), strong { "{title}" } p { "{description}" } }
                }
            }
            div { class:"identity-preview", "data-mode":mode(),
                p { class:"catalog-kind", "다른 사람이 보는 화면" }
                if mode()==0 { div { class:"private-identity", span { "◌" } h2 { "공개 프로필 없음" } p { "연동한 계정 목록도 공개하지 않아요." } } }
                else { div { class:"public-identity", span { class:"server-mark violet", "나" } h2 { "내 소개 이름" } p { "이름과 소개, 공개로 고른 계정만." } ul { li { "@me@photo.example" } li { "@me@notes.example" } } span { class:"identity-listing-state", if mode()==2 { "사람 찾기에 표시" } else { "사람 찾기에는 표시하지 않음" } } } }
            }
        }
        div { class:"information-prose",
            p { "첫 인증에 쓰는 글은 원래 연합 계정에 공개로 게시해요. 위 설정은 fediverse.kr의 프로필·목록 공개 여부이며, 인증글 자체를 비공개로 만드는 설정이 아니에요." }
            p { "‘목록에 없음’은 비공개가 아니에요. 공개 프로필 주소는 전달되거나 검색엔진에 알려질 수 있어요." }
            Link { class:"primary-button", to:Route::Account{}, "내 계정 연동 확인하기" }
            p { class:"review-note", "연동과 계정별 표시 허용은 저장할 수 있어요. 공개 프로필과 사람 목록은 아직 시안입니다." }
        }
        section { class:"directory-help", h2 { "친구의 연합 주소를 이미 안다면" } p { "여기서 찾지 않아도 돼요. 내가 가입한 사이트에서 검색하세요." } Link { class:"text-link", to:Route::Explain { topic:"find".into() }, "주소로 찾아가는 장면 보기" ArrowRight { size:16 } } }
    } }
}

#[component]
pub fn Apps() -> Element {
    rsx! { InfoPage { title:"서버는 그대로, 쓰는 앱은 다르게.", intro:"먼저 내가 가입한 서버가 어떤 소프트웨어인지 확인하세요.",
        div { class:"information-prose", h2 { "앱은 들어가는 문이에요." } p { "같은 계정으로 다른 앱을 쓸 수도 있어요. 다만 앱마다 지원하는 서버 소프트웨어와 기능은 다릅니다." }
            ol { class:"operator-checks", li { strong { "내 서버의 소프트웨어 확인" } p { "서버 소개나 하단 안내에서 이름을 찾아요." } } li { strong { "호환되는 앱 고르기" } p { "서버의 앱 안내 또는 소프트웨어 공식 사이트를 확인해요." } } li { strong { "앱에서 내 서버 주소 입력" } p { "다른 서버에 새로 가입하는 것이 아니라, 기존 계정으로 들어가요." } } }
            section { class:"directory-help", aria_label:"소프트웨어별 공식 앱 안내",
                h2 { "내 서버에서 쓸 앱 찾기" }
                p { "내 서버의 소프트웨어를 골라 공식 앱 안내를 열어보세요." }
                p { a { class:"text-link", href:"https://joinmastodon.org/apps", target:"_blank", rel:"noopener noreferrer", "Mastodon 공식 앱 안내 ↗" } }
                p { a { class:"text-link", href:"https://misskey-hub.net/en/docs/apps.html", target:"_blank", rel:"noopener noreferrer", "Misskey 공식 앱 안내 ↗" } }
                p { a { class:"text-link", href:"https://pixelfed.org/mobile-apps", target:"_blank", rel:"noopener noreferrer", "Pixelfed 공식 모바일 앱 안내 ↗" } }
                p { class:"review-note", "앱마다 지원하는 기기와 서버가 달라요. 설치 전 공식 안내를 확인하세요." }
            }
            section { class:"directory-help", aria_label:"내 서버를 찾거나 소프트웨어를 모를 때",
                h2 { "아직 내 서버를 모르겠다면" }
                p { "서버 소개에서 소프트웨어 이름을 확인하세요. 가입할 곳을 찾는 중이라면 서버 목록부터 보세요." }
                Link { class:"text-link", to:Route::Servers { filters: Default::default() }, "서버 목록 둘러보기" }
                span { " · " }
                Link { class:"text-link", to:Route::Start {}, "연합우주 설명 읽기" }
            }
        }
    } }
}

#[component]
pub fn Migration() -> Element {
    rsx! { crate::explainer::Explain { topic:"moving" } }
}

#[component]
pub fn Contact() -> Element {
    rsx! { InfoPage { title:"문의하기", intro:"서버 등록 제안, 정보 수정, 목록 노출에 관한 요청을 보내 주세요.",
        div { class:"information-prose contact-content",
            section { class:"contact-section",
                h2 { "공개해도 되는 제안과 오류 제보" }
                p { "목록에 없는 서버를 알려주시거나 잘못된 정보를 제보할 때는 GitHub를 이용해 주세요. 서버 주소와 간단한 설명, 문제가 생긴 경우에는 오류 내용을 함께 적어 주세요." }
                a { class:"text-link", href:REGISTRATION_REQUEST_URL, target:"_blank", rel:"noopener noreferrer", "GitHub에 등록 요청·제보하기" ArrowRight { size:16 } }
                p { "GitHub에 작성한 내용은 누구나 볼 수 있는 공개 이슈로 남습니다." }
            }
            section { class:"contact-section",
                h2 { "운영자에게 직접 문의하기" }
                p { "GitHub를 쓰지 않거나 공개 이슈로 남기기 어려운 내용은 아래 연합 계정으로 문의해 주세요. 내가 사용하는 서버에서 계정 주소를 검색해 메시지를 보낼 수 있어요." }
                a { class:"text-link contact-handle", href:"https://lake.naru.cafe/@narucafe", target:"_blank", rel:"noopener noreferrer", "narucafe@lake.naru.cafe" ArrowRight { size:16 } }
                p { "메시지를 보내기 전에 수신자와 공개 범위를 확인해 주세요. 비밀번호나 인증 코드 등 비밀정보는 보내지 마세요." }
            }
            section { class:"contact-section",
                h2 { "목록에 노출되기를 원하지 않으시나요?" }
                p { "서버 주소와 요청 내용을 운영자에게 알려 주세요. 관리자가 확인한 뒤 목록에서 내립니다." }
                p { "NodeInfo 미지원 등으로 직접 등록하기 어려운 서버도 문의할 수 있어요. 등재 조건을 확인한 뒤 수동 등록 가능 여부를 검토합니다." }
            }
        }
    } }
}

#[component]
pub fn About() -> Element {
    rsx! { InfoPage { title:"연합우주를 만나보는 곳.", intro:"fediverse.kr은 서로 연결된 공간을 이해하고, 머물 곳을 찾도록 돕습니다.",
        div { class:"information-prose", h2 { "이 사이트가 연합우주 전체는 아니에요." } p { "여기에 가입하지 않아도 다른 서버를 이용할 수 있어요. 서버 목록도 연합우주 전체 목록이 아니라, 이곳에 등록된 한국어권 공간을 다룹니다." }
            h2 { "정보는 함께 고치고, 기록은 남기기." } p { "회원이 소프트웨어 소개를 직접 등록·수정하고, 서버 오너는 자신의 서버 정보를 관리해요. 소프트웨어 변경 이력을 비교하고 이전 내용으로 되돌릴 수 있습니다." } p { class:"review-note", "운영 자료의 공동 편집과 관리자의 분쟁 개입 화면은 준비 중입니다." }
            h2 { "오류나 의견이 있다면" } p { "서버 등록 제안, 정보 수정, 목록 노출에 관한 요청은 문의 페이지의 GitHub 또는 운영 문의 계정으로 보내 주세요." }
            Link { class:"text-link", to:Route::Contact{}, "문의하기" ArrowRight { size:16 } }
            h2 { "목록을 읽을 때" } p { "서버마다 운영자와 가입 방식, 규칙이 달라요. 이곳의 목록과 수집 정보는 참고하되, 가입 전에는 해당 서버의 최신 안내를 확인해 주세요." }
            Link { class:"text-link", to:Route::Start{}, "연합우주, 화면으로 이해하기" ArrowRight { size:16 } }
            section {id:"software-marks",
                h2 {"소프트웨어 로고와 이미지 출처"}
                p {"각 프로젝트의 공식 웹사이트·브랜드킷·보도자료에서 제공하는 로고와 소개 이미지를 사용합니다. 제품을 식별하고 소개하기 위한 자료이며, fediverse.kr과의 제휴나 보증을 뜻하지 않아요."}
                ul {
                    li {a {href:"https://joinmastodon.org/branding",target:"_blank",rel:"noopener noreferrer","Mastodon 브랜드킷"} " — Mastodon gGmbH. 공식 마크와 Android 앱 화면."}
                    li {a {href:"https://misskey-hub.net/en/brand-assets/",target:"_blank",rel:"noopener noreferrer","Misskey 브랜드 자산"} " — syuilo / Misskey Project. 아이콘 CC BY-SA 4.0, 소개 화면은 " a {href:"https://misskey-hub.net/en/",target:"_blank",rel:"noopener noreferrer","Misskey Hub"} "."}
                    li {a {href:"https://pixelfed.org/",target:"_blank",rel:"noopener noreferrer","Pixelfed"} " — Pixelfed Project. 공식 아이콘과 제품 소개 화면."}
                    li {a {href:"https://joinpeertube.org/press-kit.zip",target:"_blank",rel:"noopener noreferrer","PeerTube 보도자료"} " — 로고: Framasoft, " a {href:"https://creativecommons.org/licenses/by-sa/4.0/",target:"_blank",rel:"noopener noreferrer","CC BY-SA 4.0"} ". 배너의 마스코트·삽화: David Revoy, CC BY (원본 키트에 버전 미기재)."}
                    li {a {href:"https://joinbookwyrm.com/",target:"_blank",rel:"noopener noreferrer","BookWyrm"} " — BookWyrm Project. 공식 헤더 로고와 책 소개 화면."}
                    li {a {href:"https://mobilizon.org/",target:"_blank",rel:"noopener noreferrer","Mobilizon"} " — Mobilizon / Kaihuri. 공식 아이콘과 제품 소개 화면."}
                }
                p {"원본 파일은 수정하지 않았으며, 화면에 맞춰 크기를 조절하거나 일부를 잘라 표시합니다. 별도 자유 이용 조건이 없는 자료의 권리는 원저작자에게 있습니다. 자산별 출처·조건·원본 해시는 저장소에 기록합니다."}
                p {a {href:"https://creativecommons.org/licenses/by-sa/4.0/",target:"_blank",rel:"noopener noreferrer","CC BY-SA 4.0 이용 조건"} " · " a {href:"https://creativecommons.org/licenses/by/4.0/",target:"_blank",rel:"noopener noreferrer","CC BY 이용 조건 안내"}}
                p {"등록된 로고가 있으면 우선 표시합니다. 확인된 공식 자산이 없는 경우 임의로 로고를 만들지 않고 이름 또는 이니셜을 표시해요."}
            }
        }
    } }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dioxus_history::{History, MemoryHistory};
    use dioxus_router::components::HistoryProvider;
    use std::rc::Rc;

    #[component]
    fn OperateTestApp() -> Element {
        rsx! {
            HistoryProvider {
                history: move |_| Rc::new(MemoryHistory::with_initial_path("/operate")) as Rc<dyn History>,
                Router::<Route> {}
            }
        }
    }

    #[test]
    fn operate_page_links_directly_to_owned_server_management() {
        let mut dom = VirtualDom::new(|| rsx! { OperateTestApp {} });
        dom.rebuild_in_place();
        let html = dioxus::ssr::render(&dom);

        assert!(html.contains("href=\"/account/sites\""), "{html}");
        assert!(html.contains("내 서버 정보 관리"), "{html}");
        assert!(html.contains("DNS TXT"), "{html}");
        assert!(html.contains("href=\"#before-opening\""), "{html}");
        assert!(html.contains("id=\"already-running\""), "{html}");
        assert!(html.contains("href=\"/hosting\""), "{html}");
        assert!(!html.contains("class=\"information-grid\""), "{html}");
    }

    #[component]
    fn SelfHostingTestApp() -> Element {
        rsx! { HistoryProvider {
            history: move |_| Rc::new(MemoryHistory::with_initial_path("/guides/self-hosting")) as Rc<dyn History>,
            Router::<Route> {}
        } }
    }

    #[test]
    fn hosting_comparison_has_semantic_headers_and_real_next_steps() {
        let mut dom = VirtualDom::new(SelfHostingTestApp);
        dom.rebuild_in_place();
        let html = dioxus::ssr::render(&dom);
        assert!(html.contains("<caption>운영 방식별 책임</caption>"));
        assert_eq!(html.matches("scope=\"row\"").count(), 4);
        assert!(html.contains("href=\"/hosting\""));
        assert!(html.contains("href=\"/platforms"));
        assert!(html.contains("해지 조건 확인"));
    }

    #[component]
    fn DevelopTestApp() -> Element {
        rsx! { HistoryProvider {
            history: move |_| Rc::new(MemoryHistory::with_initial_path("/develop")) as Rc<dyn History>,
            Router::<Route> {}
        } }
    }

    #[test]
    fn development_paths_distinguish_client_api_from_federation() {
        let mut dom = VirtualDom::new(DevelopTestApp);
        dom.rebuild_in_place();
        let html = dioxus::ssr::render(&dom);
        assert!(html.contains("서버 API"));
        assert!(html.contains("ActivityPub"));
        assert!(html.contains("https://docs.joinmastodon.org/client/intro/"));
        assert!(html.contains("https://www.w3.org/TR/activitypub/"));
        assert!(html.contains("href=\"/account/software/new\""));
    }

    #[component]
    fn AppsTestApp() -> Element {
        rsx! { HistoryProvider {
            history: move |_| Rc::new(MemoryHistory::with_initial_path("/apps")) as Rc<dyn History>,
            Router::<Route> {}
        } }
    }

    #[test]
    fn apps_page_links_to_official_software_guides_and_local_next_steps() {
        let mut dom = VirtualDom::new(AppsTestApp);
        dom.rebuild_in_place();
        let html = dioxus::ssr::render(&dom);

        assert_eq!(html.matches("<h1>").count(), 1, "{html}");
        for (url, label) in [
            ("https://joinmastodon.org/apps", "Mastodon 공식 앱 안내"),
            (
                "https://misskey-hub.net/en/docs/apps.html",
                "Misskey 공식 앱 안내",
            ),
            (
                "https://pixelfed.org/mobile-apps",
                "Pixelfed 공식 모바일 앱 안내",
            ),
        ] {
            let anchor = html
                .split("<a ")
                .find(|anchor| anchor.contains(url))
                .unwrap_or_else(|| panic!("missing official app guide {url}: {html}"));
            assert!(anchor.contains("target=\"_blank\""), "{anchor}");
            assert!(anchor.contains("rel=\"noopener noreferrer\""), "{anchor}");
            assert!(anchor.contains(label), "{anchor}");
        }
        // Dioxus serializes an empty query struct as a trailing question mark.
        assert!(html.contains("href=\"/servers?\""), "{html}");
        assert!(html.contains("href=\"/start\""), "{html}");
        assert!(html.contains("설치 전 공식 안내를 확인하세요"), "{html}");
    }
}
