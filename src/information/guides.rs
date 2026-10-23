//! Task-first navigation for operating and building federated spaces.
use super::InfoPage;
use crate::Route;
use dioxus::prelude::*;
use lucide_dioxus::ArrowRight;

#[component]
fn GuideLink(to: Route, title: String, description: String) -> Element {
    rsx! {
        Link { class: "guide-link", to,
            div { strong { "{title}" } p { "{description}" } }
            span { class: "guide-link-icon", aria_hidden: "true", ArrowRight { size: 19 } }
        }
    }
}

#[component]
fn GuideGroup(id: String, title: String, intro: String, children: Element) -> Element {
    rsx! {
        section { id, class: "guide-group",
            header { h2 { "{title}" } p { "{intro}" } }
            div { class: "guide-actions", {children} }
        }
    }
}

#[component]
pub fn Operate() -> Element {
    rsx! {
        InfoPage { title: "내 공간을 운영하기", intro: "처음 준비할 때도, 이미 운영 중일 때도.",
            nav { class: "guide-jumps", aria_label: "운영 안내 바로가기",
                a { href: "#before-opening", "처음 열기" }
                a { href: "#already-running", "운영 중 관리" }
                a { href: "#operation-notes", "운영 지식" }
            }
            div { class: "guide-columns",
                GuideGroup { id: "before-opening", title: "처음 열려고 해요", intro: "소프트웨어와 운영 방식을 고르세요.",
                    GuideLink { to: Route::SelfHosting {}, title: "직접 운영할까, 맡길까?", description: "업데이트·백업·이용자 대응, 누가 맡는지 비교해요." }
                    GuideLink { to: Route::Platforms { filters: Default::default() }, title: "소프트웨어 고르기", description: "짧은 글, 사진, 영상, 블로그. 담을 내용을 먼저." }
                    GuideLink { to: Route::HostingList {}, title: "관리형 서비스 비교", description: "제공 범위와 공식 출처를 확인해요." }
                }
                GuideGroup { id: "already-running", title: "이미 운영하고 있어요", intro: "내 서버의 소개와 연결 정책을 살펴보세요.",
                    GuideLink { to: Route::ManagedSites {}, title: "내 서버 정보 관리", description: "DNS TXT로 운영 권한 확인 후 직접 수정해요." }
                    GuideLink { to: Route::Servers { filters: Default::default() }, title: "목록에서 우리 서버 보기", description: "공개된 소개와 수집 상태를 확인해요." }
                    GuideLink { to: Route::Explain { topic: "boundaries".into() }, title: "연결과 차단의 범위", description: "운영자의 설정은 어디까지 영향을 줄까요?" }
                }
            }
            section { id: "operation-notes", class: "guide-group", aria_labelledby: "operation-notes-title",
                header { h2 { id: "operation-notes-title", "운영을 이어 가는 기본 점검" }
                    p { "설치보다 중요한 건, 문제가 생겨도 돌볼 수 있는 준비예요." }
                }
                div { class: "guide-columns",
                    section { class: "guide-checklist",
                        h3 { "업데이트와 복구" }
                        dl {
                            dt { "업데이트 전에" } dd { "사용 중인 버전과 목표 버전의 공식 릴리즈 노트를 읽고, 중간 버전이나 DB 변경 절차가 필요한지 확인하세요." }
                            dt { "백업에 담을 것" } dd { "DB뿐 아니라 업로드 파일과 설정·비밀키도 확인하세요. 필요한 항목은 소프트웨어마다 달라요." }
                            dt { "복구할 수 있나요?" } dd { "같은 디스크 밖에 사본을 두고, 별도 환경에서 복원해 보세요. 파일이 있다는 것과 복구가 된다는 것은 달라요." }
                        }
                        div { class: "guide-doc-links",
                            a { href: "https://docs.joinmastodon.org/admin/backups/", target: "_blank", rel: "noopener noreferrer", "Mastodon 공식 백업 안내" ArrowRight { size: 16 } }
                            a { href: "https://docs.joinmastodon.org/admin/upgrading/", target: "_blank", rel: "noopener noreferrer", "Mastodon 공식 업데이트 안내" ArrowRight { size: 16 } }
                        }
                    }
                    section { class: "guide-checklist",
                        h3 { "이용자와의 약속" }
                        dl {
                            dt { "연락과 신고" } dd { "규칙, 신고 방법, 운영자 연락처를 알아보기 쉽게 두고 부재 시 대응도 정해 두세요." }
                            dt { "장애와 비용" } dd { "디스크 여유와 백업 실패를 확인하고, 장애 공지는 같은 서버 밖에서도 볼 수 있게 준비하세요." }
                            dt { "종료와 이사" } dd { "종료를 결정했다면 충분히 미리 알리고 내보내기 방법을 안내하세요. 팔로워 이동과 게시물 백업은 같은 기능이 아니에요." }
                        }
                    }
                }
                p { class: "guide-footnote", "일반적인 점검 목록입니다. 실제 명령과 복구 절차는 사용하는 소프트웨어와 호스팅 제공자의 공식 안내를 따르세요. 회원 편집형 운영 지식 모음은 아직 제공하지 않아요." }
            }
        }
    }
}

#[component]
pub fn SelfHosting() -> Element {
    rsx! {
        InfoPage { title: "직접 운영할까, 맡길까?", intro: "어디에 설치하든, 누가 무엇을 맡는지가 중요해요.",
            Link { class: "back-link", to: Route::Operate {}, "서버 운영 안내" }
            div { class: "guide-compare", role: "region", aria_label: "운영 방식 비교", tabindex: "0",
                table {
                    caption { "운영 방식별 책임" }
                    thead { tr { th { scope: "col", "맡을 일" } th { scope: "col", "직접 운영" } th { scope: "col", "관리형 서비스" } } }
                    tbody {
                        for (task, own, managed) in [
                            ("설치 · 업데이트", "내가 설정하고 관리", "제공 범위 확인"),
                            ("백업 · 장애 복구", "내가 준비하고 복구", "백업 주기·복구 지원 확인"),
                            ("규칙 · 이용자 대응", "내가 정하고 대응", "별도 지원이 없다면 내가 담당"),
                            ("데이터 · 이사", "내보내기·복구 직접 준비", "내보내기·해지 조건 확인"),
                        ] {
                            tr { th { scope: "row", "{task}" } td { "{own}" } td { "{managed}" } }
                        }
                    }
                }
            }
            p { class: "guide-footnote", "‘관리형’에 정해진 서비스 범위는 없어요. 계약과 공식 안내에서 확인하세요." }
            div { class: "guide-columns",
                GuideGroup { id: "hosting-next", title: "방식을 골랐다면", intro: "어떤 공간을 만들지 구체화해요.",
                    GuideLink { to: Route::Platforms { filters: Default::default() }, title: "소프트웨어 둘러보기", description: "만들고 싶은 공간에 맞는 도구 찾기" }
                    GuideLink { to: Route::HostingList {}, title: "관리형 서비스 살펴보기", description: "제공 범위·출처·확인 날짜 비교" }
                }
                section { class: "guide-checklist", aria_labelledby: "before-opening-title",
                    h2 { id: "before-opening-title", "열기 전, 네 가지만" }
                    dl {
                        for (question, answer) in [
                            ("누구의 공간인가요?", "개인용 · 초대제 · 공개 가입"),
                            ("누가 돌보나요?", "문제가 생겼을 때 연락받을 사람"),
                            ("무엇을 남길 수 있나요?", "백업·복구·내보내기 방법"),
                            ("끝낼 때는요?", "종료 안내와 이사할 시간"),
                        ] { div { dt { "{question}" } dd { "{answer}" } } }
                    }
                }
            }
        }
    }
}

#[component]
pub fn Develop() -> Element {
    rsx! {
        InfoPage { title: "무엇을 만들고 싶으세요?", intro: "사용하는 화면을 만들지, 연결되는 서버를 만들지부터.",
            div { class: "guide-columns",
                section { class: "guide-build-path",
                    span { class: "catalog-kind", "기존 계정을 사용하는 도구" }
                    h2 { "앱 · 봇 · 클라이언트" }
                    div { class: "guide-connection", role: "group", aria_label: "내 앱이 가입한 서버의 API를 이용",
                        span { "내 앱" } small { "서버 API" } span { "가입한 서버" }
                    }
                    p { "지원할 소프트웨어를 정하고, 그 서버의 API로 시작하세요." }
                    div { class: "guide-doc-links",
                        a { href: "https://docs.joinmastodon.org/client/intro/", target: "_blank", rel: "noopener noreferrer", "Mastodon API 문서" ArrowRight { size: 16 } }
                        a { href: "https://misskey-hub.net/en/docs/for-developers/api/", target: "_blank", rel: "noopener noreferrer", "Misskey API 문서" ArrowRight { size: 16 } }
                    }
                }
                section { class: "guide-build-path",
                    span { class: "catalog-kind", "새로운 공간을 만드는 도구" }
                    h2 { "연합 서버 · 소프트웨어" }
                    div { class: "guide-connection", role: "group", aria_label: "내 서버가 다른 서버와 ActivityPub으로 통신",
                        span { "내 서버" } small { "ActivityPub" } span { "다른 서버" }
                    }
                    p { "계정과 글을 표현하고, 다른 서버와 활동을 주고받아요." }
                    div { class: "guide-doc-links",
                        a { href: "https://www.w3.org/TR/activitypub/", target: "_blank", rel: "noopener noreferrer", "ActivityPub 표준" ArrowRight { size: 16 } }
                        a { href: "https://www.w3.org/TR/activitystreams-vocabulary/", target: "_blank", rel: "noopener noreferrer", "ActivityStreams 활동과 객체" ArrowRight { size: 16 } }
                    }
                }
            }
            section { class: "guide-contribute",
                div { h2 { "이미 만들고 있나요?" } p { "소프트웨어의 종류와 기능을 직접 등록하고 고칠 수 있어요." } }
                Link { class: "primary-button", to: Route::SoftwareNew {}, "내 소프트웨어 소개하기" }
            }
            section { class: "guide-checklist", aria_labelledby: "compatibility-title",
                h2 { id: "compatibility-title", "작은 왕복부터 확인하세요" }
                dl {
                    dt { "어떤 API인가요?" } dd { "앱이 쓰는 Mastodon·Misskey API와 서버끼리 쓰는 ActivityPub은 달라요. 같은 ActivityPub 지원 서버라도 클라이언트 API까지 같지는 않아요." }
                    dt { "보내고 받기" } dd { "시험 계정으로 팔로우·수락, 글 전달, 답글, 수정·삭제를 하나씩 확인하세요. 한쪽에서 보낸 요청의 성공만으로 상대 화면에 반영됐다고 판단하지 마세요." }
                    dt { "공개 범위와 실패" } dd { "공개·팔로워 한정 등 받는 사람의 범위와 차단을 확인하고, 중복 전달·일시 장애·재시도도 시험하세요. 비공개 글과 토큰은 로그에 남기지 마세요." }
                    dt { "기록할 것" } dd { "상대 소프트웨어와 버전, 보낸 활동, 확인된 결과를 적어 두세요. 실제 계정으로 반복 시험하기보다 명시적으로 허용된 시험 환경을 사용하세요." }
                }
                p { class: "guide-footnote", "ActivityPub 표준과 각 프로젝트의 구현 문서를 함께 확인하세요. 회원 편집형 호환성 기록 공간은 아직 제공하지 않아요." }
            }
        }
    }
}
