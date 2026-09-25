use crate::Route;
use dioxus::prelude::*;
use lucide_dioxus::{ArrowLeft, ArrowRight, Check, Heart, MessageCircle, Play};

const NEIGHBORHOOD: Asset = asset!(
    "/assets/images/neighborhood.png",
    AssetOptions::image()
        .with_jpg()
        .with_size(ImageSize::Manual {
            width: 960,
            height: 640
        })
);

#[derive(Clone, Copy)]
struct Activity {
    label: &'static str,
    title: &'static str,
    description: &'static str,
    platform: &'static str,
    url: &'static str,
    connection: &'static str,
}
const ACTIVITIES: [Activity; 6] = [
    Activity { label: "독서", title: "혼자 읽은 책이, 함께 나눌 이야기가 돼요.", description: "읽고 있는 책을 기록하고, 감상을 남겨요. 다른 곳의 독자와도 다음 책을 발견해요.", platform: "BookWyrm", url: "https://joinbookwyrm.com/", connection: "공개 독서 기록과 리뷰는 연결된 다른 BookWyrm 서버, Mastodon 등의 팔로워에게도 전해져요." },
    Activity { label: "게시판", title: "관심사가 같으면, 다른 동네 게시판도.", description: "궁금한 것을 묻고, 링크를 모으고, 댓글로 이야기해요. 주제별 커뮤니티를 찾아 구독해요.", platform: "Lemmy", url: "https://join-lemmy.org/", connection: "내 Lemmy 계정으로 연결된 다른 Lemmy 서버의 커뮤니티를 구독하고 토론에 참여해요." },
    Activity { label: "행사·모임", title: "화면에서 만나, 동네에서 함께해요.", description: "독서 모임부터 동네 산책까지. 행사를 만들고, 사람들에게 알리고, 함께할 준비를 해요.", platform: "Mobilizon", url: "https://mobilizon.org/", connection: "연결된 Mobilizon 서버 사이에서 행사와 그룹을 발견해요. 참가 방식은 행사 설정에 따라 달라요." },
    Activity { label: "사진", title: "내가 발견한 순간을, 다른 곳의 친구에게.", description: "사진을 차곡차곡 모으는 나만의 갤러리. 글보다 한 장의 풍경으로 전하고 싶은 날에.", platform: "Pixelfed", url: "https://pixelfed.org/", connection: "공개 사진 게시물은 연결된 다른 서비스의 팔로워에게도 전해질 수 있어요." },
    Activity { label: "영상", title: "내 채널의 다음 이야기를 기다리는 사람들.", description: "직접 만든 영상을 올리고, 좋아하는 채널을 발견해요. 동영상에도 여러 운영자의 공간이 있어요.", platform: "PeerTube", url: "https://joinpeertube.org/", connection: "연결된 PeerTube 서버의 영상을 발견하고, 다른 호환 서비스에서도 채널을 팔로우할 수 있어요." },
    Activity { label: "일상·대화", title: "짧은 안부부터, 하루치 수다까지.", description: "Mastodon의 타임라인, Misskey의 노트와 리액션. 대화를 나누는 모습도 하나만 있지는 않아요.", platform: "Mastodon · Misskey", url: "https://joinmastodon.org/", connection: "서로 연결된 서버의 사람을 팔로우하고 글과 답글을 주고받아요. 지원하는 표현은 플랫폼마다 달라요." },
];

#[component]
pub fn ActivityExplorer(full: bool) -> Element {
    let ready = use_context::<Signal<bool>>()();
    let mut selected = use_signal(|| 0usize);
    let mut alternate = use_signal(|| false);
    let activity = ACTIVITIES[selected()];
    rsx! {
        div { class: "activity-explorer",
            if full { div { class: "activity-tabs", role: "group", aria_label: "연합우주에서 할 일",
                for (index, item) in ACTIVITIES.iter().enumerate() {
                    button { disabled: !ready, aria_pressed: selected() == index, class: if selected() == index { "selected" } else { "" }, onclick: move |_| { selected.set(index); alternate.set(false); }, "{item.label}" }
                }
            } }
            div { class: "activity-stage", "data-activity": selected(),
                div { class: "activity-art",
                    div { class: "activity-art-label", span { "{activity.platform}" } span { "가상 콘텐츠로 만든 기능 예시" } }
                    ActivityScene { kind: selected(), alternate: alternate() }
                    div { class: "activity-art-action",
                        if selected() == 0 { button { disabled: !ready, onclick: move |_| alternate.toggle(), if alternate() { ArrowLeft { size: 15 } "독서 기록으로 돌아가기" } else { "Mastodon에는 어떻게 보일까요?" ArrowRight { size: 15 } } } }
                        else if selected() == 1 { button { disabled: !ready, aria_pressed: alternate(), onclick: move |_| alternate.toggle(), if alternate() { Check { size: 15 } "먼 서버의 게시판 구독 중" } else { "이 게시판 구독해보기" ArrowRight { size: 15 } } } }
                        else if selected() == 2 { button { disabled: !ready, aria_expanded: alternate(), onclick: move |_| alternate.toggle(), if alternate() { "모임 소개로 돌아가기" } else { "어떤 모임인지 알아보기" ArrowRight { size: 15 } } } }
                        else if selected() == 3 || selected() == 5 { button { disabled: !ready, aria_pressed: alternate(), onclick: move |_| alternate.toggle(), Heart { size: 15 } if alternate() { "마음을 남겼어요" } else { "마음 남겨보기" } } }
                        else { span { "영상 표지 예시 · 실제 영상은 없어요" } }
                    }
                }
                div { class: "activity-story", h3 { "{activity.title}" } p { "{activity.description}" }
                    div { class: "activity-connection", span { "어떻게 연결되나요?" } p { "{activity.connection}" } }
                    if full { div { class: "activity-official", a { class: "text-link", href: activity.url, target: "_blank", rel: "noopener noreferrer", "{activity.platform.split('·').next().unwrap().trim()} 공식 소개" ArrowRight { size: 15 } }
                        if selected() == 5 { a { class: "text-link", href: "https://misskey-hub.net/ko/", target: "_blank", rel: "noopener noreferrer", "Misskey 공식 소개" ArrowRight { size: 15 } } }
                    } }
                    else { Link { class: "text-link", to: Route::Platforms { filters: Default::default() }, "플랫폼 더 알아보기" ArrowRight { size: 15 } } }
                }
            }
            p { class: "connection-caveat", "모든 기능이 모든 서비스 사이에서 연결되는 것은 아니에요. 지원 기능과 각 서버의 운영 정책에 따라 달라요." }
            if full {
                section { class: "platform-server-bridge", h2 { "무엇을 할지는 플랫폼," br {} "어디서 할지는 서버." }
                    div { class: "choice-example", div { span { "플랫폼 선택" } strong { "Mastodon" } p { "타임라인에서 일상 나누기" } } ArrowRight { size: 22 } div { span { "서버 선택" } strong { "멍멍.타운" } p { "이곳의 운영 규칙과 분위기로" } } }
                    Link { class: "primary-button", to: Route::Servers { filters: Default::default() }, "가입할 공간 알아보기" ArrowRight { size: 16 } }
                    p { "멍멍.타운은 가상 예시예요. 실제 서버 목록은 연결 예정입니다." }
                }
            }
        }
    }
}

#[component]
fn ActivityScene(kind: usize, alternate: bool) -> Element {
    rsx! {
        div { class: "activity-scene",
            if kind == 0 {
                if alternate { div { class: "book-echo", span { class: "echo-platform", "Mastodon · 내 타임라인" }
                    div { class: "echo-author", span { class: "avatar cat-avatar", "소" } div { strong { "소담" } span { "@소담@책갈피.숲" } } }
                    p { "『느리게 걷는 마음』을 읽었어요." } blockquote { "익숙한 길에서도 새로운 것을 찾는 법." }
                    div { class: "echo-book", BookCover { small: true } span { "느리게 걷는 마음" small { "가상 도서 · 공개 독서 기록" } } }
                    div { class: "echo-foot", MessageCircle { size: 16 } "내 타임라인에서 감상을 읽고 답할 수 있어요." }
                } } else { div { class: "reading-scene",
                    div { class: "reading-shelf-label", "소담의 책장" span { "읽은 책" } }
                    div { class: "reading-book", BookCover { small: false } div { span { class: "read-state", Check { size: 13 } "다 읽었어요" } h4 { "느리게" br {} "걷는 마음" } p { "가상 도서 · 소담의 독서 기록" } } }
                    blockquote { "익숙한 길에서도 새로운 것을 찾는 법." } div { class: "reading-byline", "소담" span { "@책갈피.숲" } }
                } }
            } else if kind == 1 { div { class: "forum-scene", div { class: "forum-heading", strong { "초록생활" } span { "@숲속.게시판" } }
                div { class: "forum-row", span { class: "forum-votes", "24" } div { strong { "햇빛이 적은 방, 어떤 식물이 좋을까요?" } p { "새싹 · 질문" span { "댓글 8" } } } }
                div { class: "forum-row", span { class: "forum-votes", "18" } div { strong { "커피 찌꺼기 퇴비, 직접 해봤어요" } p { "골댕이 · 경험 나눔" span { "댓글 5" } } } }
                div { class: "forum-row", span { class: "forum-votes", "12" } div { strong { "이번 주 씨앗 나눔" } p { "소담 · 나눔" span { "댓글 3" } } } }
                div { class: "forum-subscription", if alternate { Check { size: 16 } "내 구독 목록에 초록생활이 생겼어요." } else { "다른 서버에도 관심사가 같은 사람들이 있어요." } }
            } }
            else if kind == 2 { div { class: "event-scene", img { src: NEIGHBORHOOD, alt: "햇살이 드는 골목의 화분과 고양이, 생성한 예시 사진", loading: "lazy" }
                div { class: "event-description", div { class: "event-date", span { "예시" } strong { "토" } } div { h4 { "카메라 들고 동네 한 바퀴" } p { "오후 3시 · 동네 산책 모임" } } }
                if alternate { div { class: "event-agenda", strong { "누구나, 어떤 카메라든." } p { "한 시간쯤 천천히 걸어요. 각자 발견한 풍경을 나누고 헤어져요." } span { "가상 행사 · 실제 모집이 아닙니다" } } }
                else { p { class: "event-intro", "혼자서는 지나쳤을 풍경을 함께 찾아요." } }
            } }
            else if kind == 3 { div { class: "photo-scene", div { class: "photo-author", span { class: "avatar cat-avatar", "패" } strong { "패트리샤" } span { "사진 기록" } } img { src: NEIGHBORHOOD, alt: "화분 옆 양지에 앉은 고양이, 생성한 예시 사진", loading: "lazy" } div { class: "photo-caption", Heart { size: 17, class: if alternate { "heart-active" } else { "" } } p { "오후의 빛을 모으는 중" } } } }
            else if kind == 4 { div { class: "video-scene", div { class: "video-poster", img { src: NEIGHBORHOOD, alt: "동네 산책 영상의 가상 표지", loading: "lazy" } span { class: "video-play", Play { size: 26 } } span { class: "video-time", "03:24" } } h4 { "우리 동네를 천천히 걷는 시간" } p { "골댕이의 산책 채널 · 영상 표지 예시" } div { class: "video-channel", span { class: "avatar dog-avatar", "골" } "골댕이의 산책" } } }
            else { div { class: "notes-scene", div { class: "note-author", span { class: "avatar dog-avatar", "골" } div { strong { "골댕이" } span { "@골댕이@멍멍.타운" } } } p { "오늘도 수고했어요." br {} "다들 뭐 하고 있나요?" } div { class: "note-reactions", span { "수고했어요  3" } span { "저녁 먹는 중  2" } if alternate { span { class: "heart-active", Heart { size: 14 } "1" } } } small { "Misskey의 리액션을 단순화한 예시" } } }
        }
    }
}

#[component]
fn BookCover(small: bool) -> Element {
    rsx! { div { class: if small { "book-cover small" } else { "book-cover" }, aria_label: "가상 도서 느리게 걷는 마음 표지", span { "느리게" br {} "걷는 마음" } i {} small { "산책의 기록" } } }
}
