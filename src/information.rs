//! Reviewable editorial drafts, separate from membership permissions and storage.
use crate::Route;
use dioxus::prelude::*;
use lucide_dioxus::ArrowRight;

#[component]
fn InfoPage(title: String, intro: String, children: Element) -> Element {
    rsx! {
        document::Title { "{title} — fediverse.kr" }
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
pub fn Operate() -> Element {
    rsx! { InfoPage { title:"내 공간을 운영한다면.", intro:"처음 열 때의 선택과, 매일 돌보는 일을 나눠 생각해요.",
        div { class:"information-grid",
            Link { class:"software-card", to:Route::SelfHosting{}, span { class:"catalog-kind", "처음 시작" } h2 { "직접 운영할까, 맡길까?" } p { "서버를 열기 전에 맡게 될 일부터 살펴보세요." } span { class:"text-link", "운영 방식 살펴보기" ArrowRight { size:16 } } }
            Link { class:"software-card", to:Route::Explain { topic:"boundaries".into() }, span { class:"catalog-kind", "우리 공간의 규칙" } h2 { "어디와 연결할까요?" } p { "운영자의 연결 정책은 이웃에게도 영향을 줘요." } span { class:"text-link", "차단의 범위 보기" ArrowRight { size:16 } } }
            Link { class:"software-card", to:Route::Servers { filters: Default::default() }, span { class:"catalog-kind", "이미 운영 중" } h2 { "우리 서버의 정보" } p { "수집된 상태와 소개가 어떻게 표시되는지 확인하세요." } span { class:"text-link", "서버 찾기" ArrowRight { size:16 } } }
            Link { class:"software-card", to:Route::ManagedSites{}, span { class:"catalog-kind", "정보 수정" } h2 { "내 서버 정보 관리" } p { "DNS TXT로 운영 권한을 확인하고, 서버 소개와 운영 정보를 직접 고치세요." } span { class:"text-link", "인증하고 수정하기" ArrowRight { size:16 } } }
        }
        section { class:"information-prose", h2 { "운영 지식은 함께 쌓는 공간으로." } p { "업데이트 경험, 백업과 복구, 신고 대응, 운영 종료 안내. 실제로 운영하며 알게 된 내용을 모으려 합니다." }
            div { class:"topic-stubs", span { "업데이트 · 복구" } span { "운영 규칙 · 신고" } span { "종료 · 이사 안내" } }
            p { class:"review-note", "회원이 직접 글을 쓰고 수정하며, 충돌이 생기면 관리자가 개입하는 구조를 준비 중입니다. 지금은 안내 초안이며 투고 기능은 없습니다." }
        }
    } }
}

#[component]
pub fn SelfHosting() -> Element {
    rsx! { InfoPage { title:"서버를 연다는 건, 공간을 돌보는 일.", intro:"혼자 쓰는 공간도, 많은 사람이 모이는 공간도 만들 수 있어요.",
        div { class:"information-grid two",
            section { class:"software-card", span { class:"catalog-kind", "직접 운영" } h2 { "도구와 환경을 직접 고르기" } p { "설정의 자유와 함께 업데이트, 백업, 장애 복구도 직접 맡아요." } }
            section { class:"software-card", span { class:"catalog-kind", "관리형 서비스" } h2 { "기술 운영의 일부를 맡기기" } p { "어디까지 맡기는지는 서비스마다 달라요. 커뮤니티의 규칙과 이용자 대응도 대신해 주는지 따로 확인하세요." } }
        }
        section { class:"information-prose", h2 { "열기 전에 정해둘 네 가지" }
            ol { class:"operator-checks", li { strong { "누구의 공간인가요?" } p { "개인용인지, 초대제인지, 누구나 가입하는 공간인지." } } li { strong { "누가 돌보나요?" } p { "문제가 생겼을 때 연락받고 대응할 사람." } } li { strong { "무엇을 남길 수 있나요?" } p { "백업을 받을 수 있는지, 복구와 이동은 어떻게 하는지." } } li { strong { "끝낼 때는요?" } p { "종료 안내와 이용자가 이사할 시간을 어떻게 마련할지." } } }
            h2 { "서비스 소개에는 근거와 날짜를." } p { "가격이나 ‘운영을 다 해준다’는 표현만으로 비교하지 않으려 합니다. 제공 범위·이용 조건·원문 링크·확인 날짜를 따로 기록하고, 제공자도 직접 바로잡을 수 있게 할 예정이에요." }
            p { class:"review-note", "사업자 목록과 편집 기능은 준비 중입니다. 확인하지 않은 서비스 내용이나 가격은 싣지 않았어요." }
        }
        Link { class:"text-link", to:Route::Platforms { filters: Default::default() }, "어떤 소프트웨어로 만들지 살펴보기" ArrowRight { size:16 } }
    } }
}

#[component]
pub fn Develop() -> Element {
    rsx! { InfoPage { title:"다른 공간과 통하는 것을 만들어요.", intro:"새로운 앱, 새로운 서버, 연결을 돕는 도구. 만들고 싶은 쪽에서 시작하세요.",
        div { class:"information-grid",
            section { class:"software-card", span { class:"catalog-kind", "사용하는 화면" } h2 { "클라이언트 앱" } p { "어떤 서버 소프트웨어의 API를 지원할지 정하고, 그 소프트웨어의 개발 문서에서 시작해요." } }
            section { class:"software-card", span { class:"catalog-kind", "새로운 공간" } h2 { "연합 소프트웨어" } p { "다른 서버와 어떤 활동을 주고받을지 정해요. 계정 조회·전달·접근 범위는 각각의 설계 대상이에요." } a { class:"text-link", href:"https://www.w3.org/TR/activitypub/", target:"_blank", rel:"noopener noreferrer", "ActivityPub 표준" ArrowRight { size:16 } } }
            section { class:"software-card", span { class:"catalog-kind", "이미 만들었다면" } h2 { "소프트웨어 소개" } p { "종류, 지원하는 활동, 프로젝트 웹사이트와 함께 소개해 주세요. 회원이 직접 등록하고 고칠 수 있어요." } Link { class:"text-link", to:Route::SoftwareNew{}, "내 소프트웨어 소개하기" ArrowRight { size:16 } } }
        }
        section { class:"information-prose", h2 { "하나의 규약, 완전히 같지는 않은 구현." } p { "연결된다고 모든 기능이 똑같이 보이지는 않아요. 대상 소프트웨어와 실제로 주고받으며 확인할 호환성 사례를 모으려 합니다." } p { class:"review-note", "구현 경험·호환성 기록·도구 소개를 회원이 직접 채우는 공간은 준비 중입니다. 아직 비어 있는 곳을 완성된 개발 가이드처럼 표시하지 않았어요." } }
    } }
}

#[component]
pub fn Apps() -> Element {
    rsx! { InfoPage { title:"서버는 그대로, 쓰는 앱은 다르게.", intro:"먼저 내가 가입한 서버가 어떤 소프트웨어인지 확인하세요.",
        div { class:"information-prose", h2 { "앱은 들어가는 문이에요." } p { "같은 계정으로 다른 앱을 쓸 수도 있어요. 다만 앱마다 지원하는 서버 소프트웨어와 기능은 다릅니다." }
            ol { class:"operator-checks", li { strong { "내 서버의 소프트웨어 확인" } p { "서버 소개나 하단 안내에서 이름을 찾아요." } } li { strong { "호환되는 앱 고르기" } p { "서버의 앱 안내 또는 소프트웨어 공식 사이트를 확인해요." } } li { strong { "앱에서 내 서버 주소 입력" } p { "다른 서버에 새로 가입하는 것이 아니라, 기존 계정으로 들어가요." } } }
            Link { class:"primary-button", to:Route::Platforms { filters: Default::default() }, "소프트웨어 공식 안내 찾기" }
            p { class:"review-note", "앱별 지원 환경·가격·언어 정보는 확인 날짜를 갖춘 목록으로 다시 채울 예정이에요. 오래된 앱 추천을 그대로 옮기지는 않았습니다." }
        }
    } }
}

#[component]
pub fn Migration() -> Element {
    rsx! { crate::explainer::Explain { topic:"moving" } }
}

#[component]
pub fn About() -> Element {
    rsx! { InfoPage { title:"연합우주를 만나보는 곳.", intro:"fediverse.kr은 서로 연결된 공간을 이해하고, 머물 곳을 찾도록 돕습니다.",
        div { class:"information-prose", h2 { "이 사이트가 연합우주 전체는 아니에요." } p { "여기에 가입하지 않아도 다른 서버를 이용할 수 있어요. 서버 목록도 연합우주 전체 목록이 아니라, 이곳에 등록된 한국어권 공간을 다룹니다." }
            h2 { "정보는 함께 고치고, 기록은 남기기." } p { "회원이 소프트웨어 소개를 직접 등록·수정하고, 서버 오너는 자신의 서버 정보를 관리해요. 소프트웨어 변경 이력을 비교하고 이전 내용으로 되돌릴 수 있습니다." } p { class:"review-note", "운영 자료의 공동 편집과 관리자의 분쟁 개입 화면은 준비 중입니다." }
            h2 { "오류나 의견이 있다면" } p { "사이트 의견은 GitHub Issues로 모을 예정입니다. 공개 저장소 이전 전이라 아직 연결할 주소는 없습니다." }
            h2 { "지금 검토 중인 버전" } p { "소개 페이지와 메뉴는 검토용 초안입니다. DB 미연결 화면에는 가상 예시가 표시되고, 운영 데이터로 전환되면 수집된 공개 정보를 사용합니다." }
            Link { class:"text-link", to:Route::Start{}, "연합우주, 화면으로 이해하기" ArrowRight { size:16 } }
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
    }
}
