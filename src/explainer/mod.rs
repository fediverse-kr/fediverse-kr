mod model;
use crate::social_demo::{
    Avatar, JournalArticle, JournalComment, Person, ProfileCard, Site, SiteWindow, SocialPost,
};
use crate::Route;
use dioxus::prelude::*;
use dioxus_icons::lucide::Lock;
use lucide_dioxus::{ArrowDown, ArrowRight, RotateCcw};
use model::*;

const GREETING: &str = "오늘 저녁, 산책 같이 가실 분?";
const REPLY: &str = "저요! 강변에서 만나요 🌿";

fn route(chapter: Chapter) -> Route {
    match chapter {
        Chapter::Basics => Route::Start {},
        _ => Route::Explain {
            topic: chapter.slug().into(),
        },
    }
}

#[component]
fn ChapterNav(current: Chapter) -> Element {
    rsx! { nav { class: "ex-chapters", aria_label: "설명 주제",
        div { class: "wrap", for chapter in Chapter::ALL {
            Link { to: route(chapter), class: if current == chapter { "is-current" } else { "" }, aria_current: if current == chapter { "page" } else { "false" },
                span { "0{chapter.index() + 1}" } "{chapter.name()}"
            }
        } }
    } }
}

#[component]
pub fn Explain(topic: String) -> Element {
    let chapter = match topic.as_str() {
        "delivery" => Chapter::Delivery,
        "visibility" => Chapter::Visibility,
        "boundaries" => Chapter::Boundaries,
        "find" => Chapter::Find,
        "moving" => Chapter::Moving,
        _ => return rsx! { crate::portal::NotFound { segments: vec!["start".into(), topic] } },
    };
    rsx! { Episode { key: "{chapter.slug()}", chapter } }
}

#[component]
pub fn Basics() -> Element {
    #[allow(unused_mut)]
    let mut step = use_signal(|| 0usize);
    let ready = use_context::<Signal<bool>>()();
    rsx! {
        document::Title { "한 사이트의 바깥 — fediverse.kr" }
        document::Stylesheet { href: asset!("/assets/styling/explainer.css") }
        main { id: "content", class: "ex-page",
            ChapterNav { current: Chapter::Basics }
            noscript { "스크롤 장면 전환에는 JavaScript가 필요해요. 글로 된 설명과 다른 주제 링크는 아래에서 읽을 수 있어요." }
            section { id: "ex-scrolly", class: "ex-scrolly wrap", "data-step": step(),
                onmounted: move |_| {
                    #[cfg(target_arch = "wasm32")]
                    spawn(async move {
                        let mut observer = document::eval(include_str!("../../assets/explainer-scroll.js"));
                        while let Ok(index) = observer.recv::<usize>().await { step.set(index.min(BASICS.len()-1)); }
                    });
                },
                div { class: "ex-story-copy",
                    for (index, beat) in BASICS.iter().enumerate() {
                        article { class: "ex-story-step", id: "story-{index}", "data-scroll-step": index,
                            span { class: "ex-overline", if index == 0 { "한 사이트의 바깥" } else { "0{index+1}" } }
                            if index == 0 { h1 { "{beat.title}" } } else { h2 { "{beat.title}" } }
                            p { "{beat.line}" }
                            if index == 0 { span { class: "ex-scroll-hint", ArrowDown { size: 17 } "천천히 내려보세요" } }
                        }
                    }
                }
                div { class: "ex-stage-wrap",
                    BasicStage { step: step() }
                    div { class:"ex-mobile-caption", aria_hidden:"true", strong { "{BASICS[step()].title}" } p { "{BASICS[step()].line}" } }
                    div { class: "ex-scroll-controls",
                        span { "0{step()+1} / 05" }
                        div { class: "ex-chunks", role: "group", aria_label: "장면 바로 보기",
                            for (i, beat) in BASICS.iter().enumerate() {
                                button { disabled: !ready, aria_label: beat.title.replace('\n', " "), aria_pressed: step()==i, onclick: move |_| scroll_to(i), span {} }
                            }
                        }
                        button { class: "ex-icon-button", disabled: !ready, aria_label: if step()==4 { "처음 장면" } else { "다음 장면" }, onclick: move |_| scroll_to((step()+1)%5), if step()==4 { RotateCcw { size: 17 } } else { ArrowDown { size: 17 } } }
                    }
                }
            }
            div { class: "wrap ex-after-story",
                p { class: "ex-one-line", "다른 곳에 또 가입할 필요 없이, 내 공간에서." }
                ChapterLinks { current: Chapter::Basics }
                FinePrint { chapter: Chapter::Basics }
            }
        }
    }
}

fn scroll_to(index: usize) {
    #[cfg(target_arch = "wasm32")]
    spawn(async move {
        let _ = document::eval(&format!("window.__fedkrExplainerScroll?.go({index});"))
            .join::<()>()
            .await;
    });
    #[cfg(not(target_arch = "wasm32"))]
    let _ = index;
}

#[component]
fn BasicStage(step: usize) -> Element {
    rsx! { div { class: "ex-basic-stage", "data-scene": step, aria_label: "같은 대화의 바깥을 보여주는 장면",
        div { class: "ex-stage-eyebrow", if step==0 { "슈나의 타임라인" } else if step<3 { "각자 가입한 곳에서, 같은 대화" } else { "SNS와 개인 블로그도" } }
        div { class: "ex-basic-dog",
            SiteWindow { density: "compact", site: Site::Dog, viewer: Person::Shuna,
                if step < 3 {
                    SocialPost { author: Person::Shuna.index(), body: GREETING, reveal_address: step>0 }
                    SocialPost { author: Person::Patricia.index(), body: REPLY, reply_to: Person::Shuna.name(), highlighted: step==1, reveal_address: step>0 }
                } else {
                    SocialPost { author: Person::Soup.index(), body: "빨리 걷지 않아도 괜찮은 저녁", reveal_address: true, highlighted: true }
                    SocialPost { author: Person::Shuna.index(), body: "이 글 읽고 저도 산책 나왔어요.", reply_to: Person::Soup.name() }
                }
            }
            div { class: "ex-site-owner", "내가 가입한 곳" }
        }
        div { class: "ex-basic-cat", inert: step==0 || step>=3, aria_hidden: step==0 || step>=3,
            SiteWindow { density: "compact", site: Site::Cat, viewer: Person::Patricia,
                SocialPost { author: Person::Shuna.index(), body: GREETING, reveal_address: true }
                SocialPost { author: Person::Patricia.index(), body: REPLY, reply_to: Person::Shuna.name(), highlighted: step==1, reveal_address: true }
            }
            div { class: "ex-site-owner", "페트리샤가 가입한 곳 · 운영자도 따로" }
        }
        div { class: "ex-basic-blog", inert: step<3, aria_hidden: step<3,
            SiteWindow { density: "compact", site: Site::Blog, viewer: Person::Soup,
                JournalArticle { title: "빨리 걷지 않아도 괜찮은 저녁", body: "길가의 풀도 보고, 바람도 조금 쐬고.", replies: 1,
                    JournalComment { author: Person::Shuna.index(), body: "이 글 읽고 저도 산책 나왔어요." }
                }
            }
            div { class: "ex-site-owner", "곰국 혼자 운영하는 블로그" }
        }
        if step==2 {
            div { class: "ex-address-reveal",
                div { span { "누구" } span { "어디" } }
                p { b { "@patricia" } b { "@cat.tower" } }
                small { "내 사이트에서 이 주소를 검색하면 돼요." }
            }
        }
        if step==4 { div { class: "ex-fedi-reveal", span { "연합우주" } small { "Fediverse" } } }
    } }
}

#[component]
fn Episode(chapter: Chapter) -> Element {
    #[allow(unused_mut)]
    let mut step = use_signal(|| 0usize);
    let ready = use_context::<Signal<bool>>()();
    let story = beats(chapter);
    rsx! {
        document::Title { "{chapter.name()} — fediverse.kr" }
        document::Stylesheet { href: asset!("/assets/styling/explainer.css") }
        main { id: "content", class: "ex-page", "data-chapter": chapter.slug(), "data-step": step(),
            ChapterNav { current: chapter }
            noscript { "스크롤 장면 전환에는 JavaScript가 필요해요. 글로 된 설명과 다른 주제 링크는 아래에서 읽을 수 있어요." }
            section { id: "ex-scrolly", class: "ex-scrolly ex-episode-scrolly wrap", "data-step": step(),
                onmounted: move |_| {
                    #[cfg(target_arch = "wasm32")]
                    spawn(async move {
                        let mut observer = document::eval(include_str!("../../assets/explainer-scroll.js"));
                        while let Ok(index) = observer.recv::<usize>().await { step.set(index.min(story.len()-1)); }
                    });
                },
                div { class: "ex-story-copy",
                    for (index, beat) in story.iter().enumerate() {
                        article { class: "ex-story-step", id: "story-{index}", "data-scroll-step": index,
                            span { class: "ex-overline", if index==0 { "{chapter.name()}" } else { "0{index+1}" } }
                            if index==0 { h1 { "{beat.title}" } } else { h2 { "{beat.title}" } }
                            p { "{beat.line}" }
                            if index==0 { span { class: "ex-scroll-hint", ArrowDown { size: 17 } "천천히 내려보세요" } }
                        }
                    }
                }
                div { class: "ex-stage-wrap",
                    div { class: "ex-scenario-board", "data-scene": step(),
                        match chapter {
                            Chapter::Delivery => rsx! { DeliveryScene { step: step() } },
                            Chapter::Visibility => rsx! { VisibilityScene { step: step() } },
                            Chapter::Find => rsx! { FindScene { step: step() } },
                            Chapter::Moving => rsx! { MovingScene { step: step() } },
                            _ => rsx! { BoundaryScene { step: step() } },
                        }
                    }
                    div { class:"ex-mobile-caption", aria_hidden:"true", strong { "{story[step()].title}" } p { "{story[step()].line}" } }
                    div { class: "ex-scroll-controls",
                        span { "{step_label(step(), story.len())}" }
                        div { class: "ex-chunks", role: "group", aria_label: "장면 바로 보기",
                            for (i, beat) in story.iter().enumerate() {
                                button { disabled: !ready, aria_label: beat.title.replace('\n', " "), aria_pressed: step()==i, onclick: move |_| scroll_to(i), span {} }
                            }
                        }
                        button { class: "ex-icon-button", disabled: !ready, aria_label: story[step()].action, onclick: move |_| scroll_to(next_step(step(), story.len())), if step()+1==story.len() { RotateCcw { size: 17 } } else { ArrowDown { size: 17 } } }
                    }
                }
            }
            div { class: "wrap ex-after-story",
                p { class: "ex-scope-note", match chapter {
                    Chapter::Delivery=>"새 공개 글의 대표 경로예요. 멘션·재공유 등 다른 경로도 있어요.",
                    Chapter::Visibility=>"멘션 없는 새 글의 예시예요. 공개범위는 암호화가 아니며, 받은 사람은 복사할 수 있어요.",
                    Chapter::Find=>"연결을 허용하는 서버의 예시예요. 검색과 팔로우가 바로 되지 않을 수도 있어요.",
                    Chapter::Moving=>"이사 기능을 서로 지원하는 계정의 예시예요. 옛 계정을 먼저 삭제하지 마세요.",
                    _=>"여기서 연결을 끊어도 상대 사이트가 삭제되지는 않아요.",
                } }
                FinePrint { chapter }
                ChapterLinks { current: chapter }
            }
        }
    }
}

fn step_label(step: usize, total: usize) -> String {
    format!("{:02} / {:02}", step + 1, total)
}

fn next_step(step: usize, total: usize) -> usize {
    (step + 1) % total
}

#[component]
fn ViewerLabel(person: Person, relation: String) -> Element {
    rsx! { div { class: "ex-viewer-label", Avatar { author: person.index() } div { strong { "{person.name()}의 화면" } span { "{relation}" } } } }
}

#[component]
fn DeliveryScene(step: usize) -> Element {
    rsx! { div { class: "ex-scene-pair",
        div { class: "ex-perspective",
            ViewerLabel { person: Person::Shuna, relation: "슈나의 서버 · 글을 쓴 곳" }
            SiteWindow { density: "compact", site: Site::Dog, viewer: Person::Shuna,
                SocialPost { author: Person::Shuna.index(), body: GREETING, reveal_address: true, highlighted: step<2 }
                div { class: "ex-small-status", if step==0 { "새 공개 글 활동 생성" } else { "슈나의 서버에 있는 원본 글" } }
                SocialPost { author: Person::Golden.index(), body: "산책 다녀와서 먹는 간식이 최고." }
            }
            div { class: "ex-delivery-route", "data-active": step==1,
                span { "슈나의 서버" }
                strong { if step==1 { "팔로워 관계를 따라 배달" } else if step==2 { "팔로워 관계 없음 · 자동 배달 안 함" } else if step==3 { "글 주소로 별도 요청" } else { "새 글을 보낼 곳 확인" } }
            }
        }
        div { class: "ex-perspective ex-perspective-result", key: "delivery-{step}",
            if step==0 {
                ViewerLabel { person: Person::Shuna, relation: "새 공개 글의 출발점" }
                SiteWindow { density: "compact", site: Site::Dog, viewer: Person::Shuna, heading: "새 글 활동",
                    SocialPost { author: Person::Shuna.index(), body: GREETING, reveal_address: true, highlighted: true }
                    div { class: "ex-small-status", "관련 팔로워·수신자 관계가 있는 서버를 확인해요" }
                }
            } else if step==1 {
                ViewerLabel { person: Person::Patricia, relation: "다른 서버 · 슈나를 팔로우함" }
                SiteWindow { density: "compact", site: Site::Cat, viewer: Person::Patricia,
                    SocialPost { author: Person::Shuna.index(), body: GREETING, reveal_address: true, highlighted: true }
                    div { class: "ex-small-status", "냥냥.타워에 새 글 배달" }
                    SocialPost { author: Person::Patricia.index(), body: "바람이 선선해서 좋다." }
                }
            } else if step==2 {
                ViewerLabel { person: Person::Film, relation: "다른 서버 · 슈나를 팔로우하지 않음" }
                SiteWindow { density: "compact", site: Site::Photo, viewer: Person::Film, heading: "홈",
                    SocialPost { author: Person::Film.index(), body: "빛이 좋은 오후", photo: true, photo_site: true }
                    div { class: "ex-small-status", "이 서버에는 배달되지 않음" }
                    div { class: "ex-fetch-box", p { "홈에는 슈나의 글이 없어요." } }
                }
            } else {
                ViewerLabel { person: Person::Film, relation: "주소로 연 공개 글 · 홈 배달과 별도" }
                SiteWindow { density: "compact", site: Site::Photo, viewer: Person::Film, heading: "주소로 연 공개 글",
                    div { class: "ex-url-preview", "dog.town/@shuna/산책" }
                    SocialPost { author: Person::Shuna.index(), body: GREETING, reveal_address: true, highlighted: true }
                    div { class: "ex-small-status", "공개 글을 주소로 가져옴" }
                }
            }
        }
    } }
}

#[component]
fn VisibilityScene(step: usize) -> Element {
    let scope = [Visibility::Public, Visibility::Followers, Visibility::Quiet][step];
    let scope_name = ["공개", "팔로워", "조용한 공개"][step];
    rsx! {
        div { class: "ex-setting-strip", div { Avatar { author: Person::Shuna.index() } span { "슈나가 새 글을 쓴다면" } strong { "{scope_name}" } }
        }
        div { class: "ex-scene-pair", for (person, follows) in [(Person::Golden,false),(Person::Patricia,true)] {
            div { class: "ex-perspective", "data-viewer": person.username(), "data-can-read": can_read(scope,follows),
                ViewerLabel { person, relation: if follows { "다른 서버 · 슈나를 팔로우함" } else { "같은 서버 · 슈나를 팔로우하지 않음" } }
                SiteWindow { density: "compact", site: person.site(), viewer: person, heading: "슈나의 글",
                    if can_read(scope, follows) { SocialPost { author: Person::Shuna.index(), body: GREETING, reveal_address: true, scope: scope_name, highlighted: step>0 } }
                    else { div { class: "ex-unavailable", Lock { size: 26 } EmptyPost { text: "이 글을 볼 수 없어요" } } }
                    div { class: "ex-read-result", "data-allowed": can_read(scope,follows), if can_read(scope,follows) { "읽을 수 있어요" } else { "같은 서버여도, 읽을 수 없어요" } }
                }
            }
        } }
    }
}

#[component]
fn BoundaryScene(step: usize) -> Element {
    let boundary = [Boundary::Mute, Boundary::Block, Boundary::Suspend][step];
    rsx! {
        div { class: "ex-setting-strip", "data-admin": step==2,
            div { span { if step==2 { "멍멍.개집 운영자" } else { "슈나의 설정" } } strong { match boundary { Boundary::Mute=>"페트리샤 뮤트", Boundary::Block=>"페트리샤 차단", Boundary::Suspend=>"냥냥.타워 연결 중단" } } }
        }
        div { class: "ex-scene-pair", for (person,is_actor) in [(Person::Shuna,true),(Person::Golden,false)] {
            div { class: "ex-perspective", "data-viewer": person.username(), "data-receives": shows_remote_post(boundary,is_actor),
                ViewerLabel { person, relation: if is_actor { "내 화면" } else { "같은 서버의 다른 사람" } }
                SiteWindow { density: "compact", site: Site::Dog, viewer: person,
                    div { class: "ex-relationship", "data-following": keeps_follow(boundary,is_actor), Avatar { author: Person::Patricia.index() } div { strong { "페트리샤 · cat.tower" } span { if keeps_follow(boundary,is_actor) { "팔로우 유지" } else { "팔로우 관계 끊김" } } } }
                    if shows_remote_post(boundary,is_actor) { SocialPost { author: Person::Patricia.index(), body: "산책 사진, 또 한 장! 🌿", reveal_address: true, highlighted: true } }
                    else { EmptyPost { text: if step==2 { "이 서버의 글을 받지 않아요" } else { "내 홈에서 이 사람의 글을 숨겨요" } } }
                    SocialPost { author: Person::Soup.index(), body: "오늘도 느긋한 하루를.", reveal_address: true }
                }
            }
        } }
    }
}

#[component]
fn ChapterLinks(current: Chapter) -> Element {
    rsx! { section { class: "ex-next-topics", aria_label: "다른 상황 살펴보기",
        h2 { if current==Chapter::Basics { "그럼, 이런 때는요?" } else { "다른 상황도 궁금하다면" } }
        div { for chapter in Chapter::ALL {
            if chapter != current {
                Link { to: route(chapter), span { "0{chapter.index()+1}" } div { strong { match chapter { Chapter::Basics=>"처음의 큰 그림으로", Chapter::Delivery=>"내 글, 전 세계에 뿌려질까요?", Chapter::Visibility=>"같은 서버면 다 볼 수 있나요?", Chapter::Boundaries=>"연결하고 싶지 않다면?", Chapter::Find=>"다른 곳의 친구는 어떻게 찾죠?", Chapter::Moving=>"나중에 옮길 수도 있나요?" } } small { "{chapter.name()}" } } ArrowRight { size: 18 } }
            }
        } }
        p { "감이 왔다면 " Link { to: Route::Servers { filters: Default::default() }, "가입할 서버 찾기" } span { " · " } Link { to: Route::Platforms { filters: Default::default() }, "소프트웨어 둘러보기" } }
    } }
}

#[component]
fn FinePrint(chapter: Chapter) -> Element {
    rsx! { details { class: "ex-fine-print",
        summary { "조금 더 정확히 알아두려면" }
        match chapter {
            Chapter::Basics => rsx! {
                p { "사이트들이 공통 규약으로 글과 활동을 주고받아요. 대표적인 규약이 ActivityPub입니다. 일반 홈페이지가 저절로 연결되는 건 아니고, 연합 기능과 연결을 허용하는 운영 정책이 필요해요." }
                a { href: "https://www.w3.org/TR/activitypub/", target: "_blank", rel: "noopener noreferrer", "ActivityPub 표준" }
            },
            Chapter::Delivery => rsx! {
                p { "여기서는 팔로워에게 새 글을 보내는 경로와 공개 글 주소로 가져오는 경로만 비교했어요. 멘션·답장·재공유·릴레이 등으로도 글이 알려질 수 있어요. 서버에 도착했다고 그 서버의 모든 사람 홈에 뜨는 것은 아니에요." }
                a { href: "https://docs.joinmastodon.org/user/network/", target: "_blank", rel: "noopener noreferrer", "Mastodon의 검색과 타임라인" }
            },
            Chapter::Visibility => rsx! {
                p { "Mastodon의 공개·팔로워·조용한 공개를 단순화한 예시예요. 팔로워 글에 멘션된 사람도 읽을 수 있으므로 이 장면에는 멘션을 넣지 않았어요. 원치 않는 팔로우를 막으려면 승인 설정도 확인하세요." }
                p { "지정한 사람에게 보내는 비공개 멘션도 종단간 암호화된 비밀 편지가 아니에요. 운영자의 기술적 접근이나 수신자의 복사를 막지 못해요. 이 데모는 게시 전 선택이며 이미 공개한 글을 회수하는 기능이 아닙니다." }
                a { href: "https://docs.joinmastodon.org/user/posting/", target: "_blank", rel: "noopener noreferrer", "Mastodon 공개범위 설명" }
            },
            Chapter::Boundaries => rsx! {
                p { "Mastodon 기준의 뮤트·계정 차단·운영자 Suspend 예시예요. 개인의 서버 숨김과 운영자의 연결 중단은 범위가 달라요. 차단은 상대 서버나 인터넷 전체에서 글을 지우지 않으며, 공개 웹 열람까지 완전히 막는 보장도 아니에요." }
                p { "운영자에게 신고하는 일은 차단과 별개예요. 서버 제한에는 여러 강도가 있고, 연결 중단을 해제해도 기존 팔로우가 자동 복구되지는 않아요. 되감기는 설정 해제가 아니라 예시 장면을 다시 보는 기능입니다." }
                a { href: "https://docs.joinmastodon.org/user/moderating/", target: "_blank", rel: "noopener noreferrer", "개인 설정" } span { " · " } a { href: "https://docs.joinmastodon.org/admin/moderation/", target: "_blank", rel: "noopener noreferrer", "운영자 설정" }
            },
            Chapter::Find => rsx! {
                p { "여기서는 전체 계정 주소로 검색한 뒤 팔로우하는 예시를 보여줘요. 이름만 검색하면 내 서버가 아직 모르는 계정은 못 찾을 수 있어요. 상대 프로필 URL로 검색할 수도 있고, 연결 제한이나 상대 서버의 상태에 따라 실패할 수 있어요." }
                p { "승인하지 않는 계정은 바로 팔로우가 성립할 수 있어요. 팔로우 전의 옛 글까지 홈으로 모두 복사되는 것은 아닙니다." }
                a { href:"https://docs.joinmastodon.org/user/network/", target:"_blank", rel:"noopener noreferrer", "Mastodon의 주소 검색과 팔로우" }
            },
            Chapter::Moving => rsx! {
                p { "Mastodon의 계정 이사를 단순화한 장면이에요. 새 계정에 옛 계정의 별칭을 등록하고, 옛 계정에서 이사를 실행합니다. 상대 소프트웨어의 지원 여부와 처리 시간에 따라 팔로워 이동은 달라져요." }
                p { "게시물은 새 계정으로 이동하지 않아요. 내가 팔로우하는 목록 등은 별도 내보내기·가져오기가 필요하고, 백업 파일이 모든 것을 복원해 준다는 뜻도 아닙니다. 서버가 종료되기 전에 준비하세요. 실제 실행 전에는 양쪽 소프트웨어의 최신 안내를 확인해야 해요." }
                a { href:"https://docs.joinmastodon.org/user/moving/", target:"_blank", rel:"noopener noreferrer", "Mastodon의 이사·내보내기 안내" }
            },
        }
        p { class: "ex-method-note", "설명용 가상 화면입니다. 실제 전송이나 설정 변경은 없으며, 지원 기능과 정책은 소프트웨어·서버마다 달라요." }
    } }
}

#[component]
fn EmptyPost(#[props(default = "표시할 글이 없어요".into())] text: String) -> Element {
    rsx! { div { class: "ex-empty", span { aria_hidden: "true", "· · ·" } p { "{text}" } } }
}

#[component]
fn FindScene(step: usize) -> Element {
    rsx! { div { class:"ex-scene-pair ex-find-scene",
        div { class:"ex-perspective", ViewerLabel { person:Person::Shuna, relation:"내가 가입한 곳" }
            SiteWindow { site:Site::Dog, viewer:Person::Shuna, density:"compact", heading:if step==2 {"홈"}else{"검색"},
                if step<2 {
                    div { class:"ex-search-field", span { "검색" } strong { "@patricia@cat.tower" } }
                    if step==0 { EmptyPost { text:"다른 사이트의 주소를 찾는 중" } }
                    else { ProfileCard { person:Person::Patricia, site:Site::Cat, state:"팔로우 요청 중", highlighted:true } }
                } else { SocialPost { author:Person::Patricia.index(), body:"오늘도 산책! 같이 가요 🌿", highlighted:true } div { class:"ex-small-status", "내 홈에 도착한 새 글" } }
            }
        }
        div { class:"ex-perspective", ViewerLabel { person:Person::Patricia, relation:"친구가 가입한 곳 · 승인제 계정" }
            SiteWindow { site:Site::Cat, viewer:Person::Patricia, density:"compact", heading:if step==1 {"알림"}else{"프로필"},
                if step==1 { div { class:"ex-follow-request", Avatar { author:Person::Shuna.index() } strong { "슈나의 팔로우 요청" } small { "@shuna@dog.town" } span { class:"ex-demo-action", "수락" } } }
                else if step==0 { ProfileCard { person:Person::Patricia, site:Site::Cat, state:"내 프로필" } }
                else { SocialPost { author:Person::Patricia.index(), body:"오늘도 산책! 같이 가요 🌿", highlighted:true } div { class:"ex-small-status", "내 사이트에서 쓴 글" } }
            }
        }
    } }
}

#[component]
fn MovingScene(step: usize) -> Element {
    rsx! { div { class:"ex-scene-pair ex-moving-scene",
        div { class:"ex-perspective", ViewerLabel { person:Person::Shuna, relation:"옛 계정 · dog.town" }
            SiteWindow { site:Site::Dog, viewer:Person::Shuna, density:"compact", heading:"프로필",
                ProfileCard { person:Person::Shuna, site:Site::Dog, state:if step==2 {"새 주소로 이사했어요"}else{"지금 쓰는 계정"}, highlighted:step==1 }
                if step==2 { div { class:"ex-address-note", "@shuna@cat.tower" } }
                div { class:"ex-move-inventory", strong { "옛 글" } span { "이곳에 남아요" } }
                if step==0 { div { class:"ex-follower-row", Avatar { author:Person::Golden.index() } Avatar { author:Person::Patricia.index() } span { "나를 팔로우하는 이웃" } } }
            }
        }
        div { class:"ex-perspective", ViewerLabel { person:Person::Shuna, relation:"새 계정 · cat.tower" }
            SiteWindow { site:Site::Cat, viewer:Person::Shuna, density:"compact", heading:"프로필",
                ProfileCard { person:Person::Shuna, site:Site::Cat, state:if step==0 {"새로 준비한 계정"}else if step==1 {"옛 계정을 내 별칭으로 등록"}else{"이제 여기에서 만나요"}, highlighted:step>0 }
                if step==1 { div { class:"ex-address-note", "옛 주소: @shuna@dog.town" } }
                if step==2 { div { class:"ex-follower-row", Avatar { author:Person::Golden.index() } Avatar { author:Person::Patricia.index() } span { "새 주소를 팔로우" } } }
                div { class:"ex-move-inventory", strong { "이전 게시물" } span { "자동으로 옮겨지지 않아요" } }
            }
        }
    } }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render_delivery(step: usize) -> String {
        let mut dom = VirtualDom::new_with_props(DeliveryScene, DeliverySceneProps { step });
        dom.rebuild_in_place();
        dioxus::ssr::render(&dom)
    }

    #[test]
    fn delivery_scenes_separate_origin_delivery_missing_home_and_url_fetch() {
        let origin = render_delivery(0);
        assert!(origin.contains("슈나의 서버"), "{origin}");

        let delivered = render_delivery(1);
        assert!(delivered.contains("슈나를 팔로우함"), "{delivered}");
        assert!(delivered.contains("냥냥.타워에 새 글 배달"), "{delivered}");

        let not_delivered = render_delivery(2);
        assert!(
            not_delivered.contains("슈나를 팔로우하지 않음"),
            "{not_delivered}"
        );
        assert!(
            not_delivered.contains("이 서버에는 배달되지 않음"),
            "{not_delivered}"
        );
        assert!(
            not_delivered.contains("홈에는 슈나의 글이 없어요"),
            "{not_delivered}"
        );

        let fetched = render_delivery(3);
        assert!(fetched.contains("주소로 연 공개 글"), "{fetched}");
        assert!(fetched.contains("공개 글을 주소로 가져옴"), "{fetched}");
    }

    #[test]
    fn explainer_styles_keep_delivery_legible_and_functional_with_reduced_motion() {
        let css = include_str!("../../assets/styling/explainer.css");

        assert!(css.contains(".ex-delivery-route"));
        assert!(css.contains("@media(max-width:760px)"));
        assert!(css.contains(".ex-page[data-chapter=delivery] .ex-delivery-route"));
        assert!(!css.contains("@media(prefers-reduced-motion:reduce)"));
    }

    #[test]
    fn step_navigation_uses_each_storys_actual_length() {
        assert_eq!(step_label(3, 4), "04 / 04");
        assert_eq!(next_step(2, 4), 3);
        assert_eq!(next_step(3, 4), 0);
        assert_eq!(next_step(2, 3), 0);
    }
}
