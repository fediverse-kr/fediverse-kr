use dioxus::prelude::*;
use lucide_dioxus::{Ellipsis, Heart, MessageCircle, Repeat2};

/// A static, light-theme Mastodon timeline used in the landing-page demo.
#[component]
pub fn MastodonDemo() -> Element {
    rsx! {
        document::Stylesheet {
            href: asset!("/assets/styling/mastodon-demo.css"),
        }

        div {
            class: "flex h-[430px] flex-col bg-[#f4f4f5] sm:h-[470px] lg:h-[500px]",
            div {
                class: "flex shrink-0 items-center justify-between border-b border-slate-200 bg-white px-5 py-3.5",
                div {
                    class: "flex min-w-0 items-center gap-3",
                    div {
                        class: "grid size-9 shrink-0 place-items-center rounded-xl bg-[#6364ff] text-xl font-black text-white shadow-sm",
                        aria_hidden: "true",
                        "m"
                    }
                    div {
                        class: "min-w-0",
                        p { class: "truncate text-[0.95rem] font-extrabold tracking-[-0.02em] text-slate-950", "멍멍.타운" }
                        p { class: "text-[0.7rem] font-medium text-slate-400", "홈 타임라인" }
                    }
                }
                button {
                    r#type: "button",
                    class: "grid size-8 place-items-center rounded-full text-slate-400 transition hover:bg-slate-100 hover:text-slate-700",
                    aria_label: "타임라인 메뉴",
                    Ellipsis { size: 19, stroke_width: 2 }
                }
            }

            div {
                class: "mastodon-demo-scroll relative min-h-0 flex-1 overflow-y-auto overscroll-contain bg-white",
                role: "feed",
                aria_label: "멍멍.타운 모의 타임라인",
                tabindex: "0",

                div {
                    class: "mastodon-scenario-compose",
                    aria_hidden: "true",
                    div {
                        class: "mx-4 my-3 flex gap-2.5 rounded-xl border border-violet-200 bg-violet-50/50 p-3 shadow-sm",
                        div {
                            class: "grid size-9 shrink-0 place-items-center rounded-full bg-violet-100 text-sm ring-1 ring-violet-200/70",
                            "🐾"
                        }
                        div {
                            class: "min-w-0 flex-1",
                            div {
                                class: "rounded-lg border border-slate-200 bg-white px-3 py-2 text-sm text-slate-700",
                                span { class: "mastodon-compose-typing", "안녕하세요 처음이에요" }
                            }
                            div {
                                class: "mt-2 flex justify-end",
                                div {
                                    class: "mastodon-compose-action relative grid h-7 w-12 place-items-center rounded-lg bg-violet-600 text-[0.68rem] font-extrabold text-white",
                                    span { class: "mastodon-compose-send-label", "게시" }
                                    span { class: "mastodon-compose-spinner", aria_label: "게시 중" }
                                }
                            }
                        }
                    }
                }

                div {
                    class: "mastodon-scenario-stage mastodon-scenario-stage--remote",
                    p { class: "mastodon-scenario-cue mastodon-scenario-cue--remote", aria_hidden: "true", "다른 서버에서" }
                    div {
                        class: "mastodon-scenario-post mastodon-scenario-post--fly",
                        TimelinePost {
                            name: "패트리샤",
                            handle: "@패트리샤@냥냥.타워",
                            time: "2분",
                            content: "멍멍이가 또 하나 늘었네용",
                            avatar: "🐕",
                            avatar_class: "bg-amber-100 ring-amber-200/70",
                            remote: true,
                        }
                    }
                }
                div {
                    class: "mastodon-scenario-stage mastodon-scenario-stage--same",
                    p { class: "mastodon-scenario-cue mastodon-scenario-cue--same", aria_hidden: "true", "같은 서버에서" }
                    div {
                        class: "mastodon-scenario-post mastodon-scenario-post--fade",
                        TimelinePost {
                            name: "골댕이",
                            handle: "@골댕@멍멍.타운",
                            time: "3분",
                            content: "오 반가워요!",
                            avatar: "🐶",
                            avatar_class: "bg-violet-100 ring-violet-200/70",
                            reply_to: "슈나",
                        }
                    }
                }
                div {
                    class: "mastodon-scenario-stage mastodon-scenario-stage--first",
                    div {
                        class: "mastodon-scenario-post mastodon-scenario-post--first",
                        TimelinePost {
                            name: "슈나",
                            handle: "@슈나@멍멍.타운",
                            time: "5분",
                            content: "안녕하세요 처음이에요",
                            avatar: "🐾",
                            avatar_class: "bg-violet-100 ring-violet-200/70",
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn TimelinePost(
    name: String,
    handle: String,
    time: String,
    content: String,
    avatar: String,
    avatar_class: String,
    #[props(default = false)] remote: bool,
    #[props(default)] reply_to: Option<String>,
    #[props(default)] boosted_by: Option<String>,
    #[props(default = false)] boosted: bool,
) -> Element {
    rsx! {
        article {
            class: "h-full border-b border-slate-200 bg-white px-5 py-3",
            aria_label: "{name}의 게시물",
            if let Some(boosted_by) = boosted_by {
                div {
                    class: "mb-1.5 ml-12 flex items-center gap-2 text-xs font-bold text-slate-500",
                    Repeat2 { size: 15, stroke_width: 2 }
                    span { "{boosted_by}" }
                }
            }
            div {
                class: "flex gap-2.5",
                div {
                    class: "grid size-10 shrink-0 place-items-center rounded-full text-base shadow-sm ring-1 {avatar_class}",
                    aria_hidden: "true",
                    "{avatar}"
                }
                div {
                    class: "min-w-0 flex-1",
                    PostHeader { name: name.clone(), handle, time, remote }
                    if let Some(reply_to) = reply_to {
                        p {
                            class: "mt-1 text-xs text-slate-500",
                            span { class: "text-violet-600", "{reply_to}" }
                            " 님에게"
                        }
                    }
                    p { class: "mt-1 text-[0.93rem] leading-6 text-slate-800", "{content}" }
                    PostActions { boosted }
                }
            }
        }
    }
}

#[component]
fn PostHeader(name: String, handle: String, time: String, remote: bool) -> Element {
    rsx! {
        div {
            class: "flex min-w-0 items-baseline gap-1.5 text-[0.79rem]",
            strong { class: "shrink-0 font-extrabold text-slate-950", "{name}" }
            span {
                class: if remote { "size-1.5 shrink-0 rounded-full bg-amber-400" } else { "size-1.5 shrink-0 rounded-full bg-violet-400" },
                aria_hidden: "true"
            }
            span {
                class: "truncate text-slate-400",
                "{handle}"
            }
            span { class: "shrink-0 text-slate-400", "· {time}" }
        }
    }
}

#[component]
fn PostActions(#[props(default = false)] boosted: bool) -> Element {
    rsx! {
        div {
            class: "mt-2 flex max-w-[210px] items-center justify-between text-slate-400",
            button {
                r#type: "button",
                class: "grid size-6 place-items-center rounded-full transition hover:bg-sky-50 hover:text-sky-600",
                aria_label: "답글",
                MessageCircle { size: 15, stroke_width: 2 }
            }
            button {
                r#type: "button",
                class: if boosted { "grid size-6 place-items-center rounded-full bg-emerald-50 text-emerald-600" } else { "grid size-6 place-items-center rounded-full transition hover:bg-emerald-50 hover:text-emerald-600" },
                aria_label: "공유",
                Repeat2 { size: 15, stroke_width: 2 }
            }
            button {
                r#type: "button",
                class: "grid size-6 place-items-center rounded-full transition hover:bg-rose-50 hover:text-rose-500",
                aria_label: "좋아요",
                Heart { size: 15, stroke_width: 2 }
            }
            button {
                r#type: "button",
                class: "grid size-6 place-items-center rounded-full transition hover:bg-slate-100 hover:text-slate-700",
                aria_label: "더 보기",
                Ellipsis { size: 15, stroke_width: 2 }
            }
        }
    }
}
