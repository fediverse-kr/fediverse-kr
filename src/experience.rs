use dioxus::prelude::*;
use lucide_dioxus::{
    ArrowLeft, ArrowRight, Check, Globe, Heart, MessageCircle, Pause, Play, Repeat2, RotateCcw,
};

// THESIS: one authored action, not a slideshow: publish here, receive a reply from there.
// WORLD: the pinned light interface, violet home and restrained rose remote domain.
// STORY: retain the same identity and the same message as it crosses the boundary.
// FIRST VIEWPORT: editable composer on the left; a friend's independently operated home on the right.
// FORM: code-led extension. Mobile stays at home; a compact remote receipt carries the missing view.
// FINISH: unreviewed and undocumented is unfinished; this build ends with the finish review,
// the verdict, DESIGN.md, and every shipping raster carrying its provenance.

const CAPTIONS: [&str; 7] = [
    "슈나가 되어, 첫 인사를 보내보세요.",
    "멍멍.타운에 인사를 보내고 있어요.",
    "내 글이 멍멍.타운에 올라왔어요.",
    "나를 팔로우한 패트리샤에게도 도착했어요.",
    "패트리샤가 냥냥.타워에서 답장을 쓰고 있어요.",
    "냥냥.타워에서 보낸 답장이 건너와요.",
    "다른 곳에 가입한 친구와, 내 공간에서 대화했어요.",
];
// Durations in 200ms ticks. No hidden time elapses while paused.
const DURATIONS: [u8; 7] = [0, 4, 9, 11, 9, 8, 0];

#[component]
pub fn FederationDemo() -> Element {
    let ready = use_context::<Signal<bool>>()();
    let mut stage = use_signal(|| 0usize);
    let mut ticks = use_signal(|| 0u8);
    let mut paused = use_signal(|| false);
    let mut side = use_signal(|| false);
    let mut draft = use_signal(|| "안녕하세요 처음이에요".to_string());
    let mut posted = use_signal(String::new);
    let mut liked = use_signal(|| false);
    #[cfg(target_arch = "wasm32")]
    use_future(move || async move {
        loop {
            gloo_timers::future::TimeoutFuture::new(200).await;
            let current = stage();
            if (1..6).contains(&current) && !paused() {
                let next = ticks() + 1;
                if next >= DURATIONS[current] {
                    ticks.set(0);
                    stage.set(current + 1);
                } else {
                    ticks.set(next);
                }
            }
        }
    });
    rsx! {
        section { class: "experience", "data-stage": stage(), "data-paused": paused(), aria_label: "다른 서버와 인사 나누기 체험",
            div { class: "experience-top", span { "직접 해보세요" } span { "가상 인물 · 실제로 전송되지 않아요" } }
            div { class: "experience-perspectives", role: "group", aria_label: "데모에서 볼 공간",
                button { disabled: !ready, aria_pressed: !side(), class: if !side() { "chosen" } else { "" }, onclick: move |_| side.set(false), "나의 멍멍.타운" }
                button { disabled: !ready, aria_pressed: side(), class: if side() { "chosen remote" } else { "remote" }, onclick: move |_| side.set(true), "친구의 냥냥.타워" }
            }
            div { class: "experience-pair",
                section { class: if side() { "experience-window hometown" } else { "experience-window hometown on-mobile" }, aria_label: "멍멍.타운의 슈나 화면",
                    WindowHeader { remote: false }
                    div { class: "experience-feed",
                        div { class: "reply-slot",
                            if stage() >= 6 {
                                div { class: "reply-in", ConversationPost { remote: true, body: "반가워요! 냥냥.타워에서 인사해요.", reply: true } }
                            } else {
                                div { class: "slot-message",
                                    if stage() < 2 { MessageCircle { size: 25 } strong { "처음 온 동네에," br {} "인사 한마디." } }
                                    else if stage() < 5 { span { class: "quiet-orbit" } strong { "답장은 여기로 와요." } }
                                    else { span { class: "incoming-label" , "다른 서버에서" } }
                                }
                            }
                        }
                        if stage() < 2 {
                            form { class: "demo-composer", onsubmit: move |event| {
                                event.prevent_default();
                                if stage() == 0 && !draft().trim().is_empty() { posted.set(draft().trim().to_string()); ticks.set(0); paused.set(false); stage.set(1); }
                            },
                                div { class: "composer-author", CharacterAvatar { remote: false } strong { "슈나" } span { "당신" } }
                                label { class: "sr-only", r#for: "demo-greeting", "보낼 인사" }
                                // Static trusted initial text avoids SSR hydration markers in this raw-text element.
                                // User input only ever goes through the escaped value property, never inner HTML.
                                textarea { id: "demo-greeting", aria_label: "보낼 인사", maxlength: "90", rows: "2", dangerous_inner_html: "안녕하세요 처음이에요", value: "{draft}", disabled: !ready || stage() != 0, oninput: move |event| draft.set(event.value()) }
                                div { class: "composer-bottom", span { Globe { size: 13 } "공개 글" } button { r#type: "submit", disabled: !ready || stage() != 0 || draft().trim().is_empty(),
                                    if stage() == 1 { span { class: "send-spinner", aria_hidden: "true" } "보내는 중" } else { "인사 보내기" ArrowRight { size: 15 } }
                                } }
                            }
                        } else { div { class: "own-post-in", ConversationPost { remote: false, body: posted(), reply: false } } }
                    }
                    div { class: "home-identity", CharacterAvatar { remote: false } div { strong { "내 계정은 그대로" } span { "@슈나" b { class: "dog-text", "@멍멍.타운" } } } }
                }
                div { class: "crossing-track", aria_hidden: "true",
                    span { class: if stage() == 3 { "travel-dot outbound" } else if stage() == 5 { "travel-dot inbound" } else { "rest-dot" } }
                }
                section { class: if side() { "experience-window awaytown on-mobile" } else { "experience-window awaytown" }, aria_label: "냥냥.타워의 패트리샤 화면",
                    WindowHeader { remote: true }
                    div { class: "experience-feed remote-feed",
                        div { class: "reply-slot",
                            if stage() >= 5 { div { class: "own-post-in", ConversationPost { remote: true, body: "반가워요! 냥냥.타워에서 인사해요.", reply: true } } }
                            else if stage() == 4 { div { class: "remote-writing", CharacterAvatar { remote: true } div { strong { "패트리샤" } p { "반가워요! 냥냥.타워에서…" } span { class: "typing-dots", i {} i {} i {} } } } }
                            else { div { class: "remote-person", CharacterAvatar { remote: true } strong { "패트리샤" } span { "슈나를 팔로우하고 있어요" } } }
                        }
                        if stage() >= 3 { div { class: "remote-post-in", ConversationPost { remote: false, body: posted(), reply: false } } }
                        else { div { class: "remote-wait", Globe { size: 22 } p { "다른 곳의 친구도" br {} "내 홈에서 만나요." } } }
                    }
                    div { class: "home-identity remote-identity", CharacterAvatar { remote: true } div { strong { "친구는 자기 계정으로" } span { "@패트리샤" b { class: "cat-text", "@냥냥.타워" } } } }
                }
            }
            // This receipt preserves the off-screen cause on mobile; it never swaps the visitor's home away.
            div { class: "mobile-receipt", "data-active": stage() >= 3, aria_hidden: "true",
                if stage() < 3 { span { "냥냥.타워" } span { "패트리샤가 슈나를 팔로우 중" } }
                else if stage() < 5 { Check { size: 14 } span { "냥냥.타워에 인사 도착" } }
                else { ArrowLeft { size: 14 } span { "냥냥.타워 → 멍멍.타운" } }
            }
            div { class: "experience-caption", role: "status", aria_atomic: "true", p { "{CAPTIONS[stage()]}" } }
            div { class: "experience-bottom",
                if stage() == 6 {
                    button { class: if liked() { "appreciate liked" } else { "appreciate" }, aria_pressed: liked(), onclick: move |_| liked.toggle(), Heart { size: 16 } if liked() { "답장에 마음을 보냈어요" } else { "답장에 마음 보내기" } }
                } else { span { class: "demo-explanation", "두 서버가 글과 답장을 서로 전달해요." } }
                if (1..6).contains(&stage()) { button { class: "playback-control", aria_label: if paused() { "체험 계속 보기" } else { "체험 일시정지" }, onclick: move |_| paused.toggle(), if paused() { Play { size: 15 } "계속" } else { Pause { size: 15 } "잠시 멈춤" } } }
                if stage() > 0 { button { class: "playback-control", aria_label: "인사 체험 다시 하기", onclick: move |_| { stage.set(0); ticks.set(0); paused.set(false); liked.set(false); side.set(false); }, RotateCcw { size: 15 } "다시" } }
            }
        }
    }
}

#[component]
fn WindowHeader(remote: bool) -> Element {
    rsx! {
        div { class: "browser-bar", div { class: "browser-dots", i {} i {} i {} } span { if remote { "냥냥.타워" } else { "멍멍.타운" } } }
        header { class: "experience-window-header", span { class: "server-symbol", if remote { "냥" } else { "멍" } } div { strong { if remote { "냥냥.타워" } else { "멍멍.타운" } } span { if remote { "패트리샤의 타임라인" } else { "나의 타임라인" } } } }
    }
}

#[component]
fn ConversationPost(remote: bool, body: String, reply: bool) -> Element {
    rsx! {
        article { class: "conversation-post",
            CharacterAvatar { remote }
            div { class: "conversation-body", strong { if remote { "패트리샤" } else { "슈나" } }
                div { class: "conversation-address", if remote { "@패트리샤" span { class: "cat-text", "@냥냥.타워" } } else { "@슈나" span { class: "dog-text", "@멍멍.타운" } } }
                if reply { span { class: "conversation-reply", MessageCircle { size: 11 } "슈나에게 답글" } }
                p { "{body}" }
                div { class: "conversation-actions", aria_hidden: "true", MessageCircle { size: 14 } Repeat2 { size: 15 } Heart { size: 14 } }
            }
        }
    }
}

#[component]
fn CharacterAvatar(remote: bool) -> Element {
    let source = if remote {
        asset!(
            "/assets/images/patricia.png",
            AssetOptions::image()
                .with_jpg()
                .with_size(ImageSize::Manual {
                    width: 128,
                    height: 128
                })
        )
    } else {
        asset!(
            "/assets/images/shuna.png",
            AssetOptions::image()
                .with_jpg()
                .with_size(ImageSize::Manual {
                    width: 128,
                    height: 128
                })
        )
    };
    rsx! { img { class: if remote { "character-avatar remote-avatar" } else { "character-avatar" }, src: source, alt: "", width: "36", height: "36" } }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn story_has_bounded_nonzero_active_frames() {
        assert!(DURATIONS[1..6].iter().all(|d| *d > 0));
        assert_eq!(CAPTIONS.len(), DURATIONS.len());
        assert_eq!(
            DURATIONS.iter().map(|n| u32::from(*n) * 200).sum::<u32>(),
            8200
        );
    }
}
