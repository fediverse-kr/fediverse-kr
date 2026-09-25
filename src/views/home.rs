use dioxus::prelude::*;
use lucide_dioxus::{Network, PanelsTopLeft, Server};

use crate::components::mastodon_demo::MastodonDemo;

/// The fediverse.kr landing page.
#[component]
pub fn Home() -> Element {
    rsx! {
        main {
            id: "main-content",
            section {
                class: "mx-auto grid min-h-[calc(100svh-73px)] w-full max-w-[1180px] grid-cols-1 items-center gap-14 px-6 py-16 sm:px-8 lg:grid-cols-[minmax(0,0.88fr)_minmax(440px,1.12fr)] lg:gap-20 lg:px-10 lg:py-20",
                section {
                    class: "max-w-[560px]",
                    aria_labelledby: "landing-title",
                    h1 {
                        id: "landing-title",
                        class: "text-balance text-[clamp(3.35rem,7vw,6.5rem)] font-black leading-[1.08] tracking-[-0.065em] text-slate-950",
                        span { class: "block", "다른 서버의" }
                        span { class: "block", "내 친구를" }
                        span { class: "block text-violet-600", "구독해요." }
                    }
                    p {
                        class: "mt-8 max-w-md text-pretty text-base leading-7 text-slate-500 sm:text-lg",
                        "fediverse.kr에서 연합우주에 대해 알아보세요."
                    }
                }

                section {
                    class: "w-full lg:justify-self-end",
                    aria_label: "fediverse.kr 서비스 데모 영역",
                    div {
                        class: "overflow-hidden rounded-[1.75rem] border border-slate-200 bg-white shadow-[0_28px_80px_-36px_rgba(71,51,130,0.35)]",
                        div {
                            class: "flex h-11 items-center gap-2 border-b border-slate-200 bg-slate-50 px-5",
                            span { class: "size-2.5 rounded-full bg-slate-300" }
                            span { class: "size-2.5 rounded-full bg-slate-300" }
                            span { class: "size-2.5 rounded-full bg-slate-300" }
                        }
                        MastodonDemo {}
                    }
                }
            }

            section {
                class: "border-y border-slate-200 bg-slate-50/80",
                aria_labelledby: "explore-heading",
                div {
                    class: "mx-auto w-full max-w-[1180px] px-6 py-16 sm:px-8 sm:py-20 lg:px-10",
                    h2 {
                        id: "explore-heading",
                        class: "text-3xl font-extrabold tracking-[-0.04em] text-slate-950 sm:text-4xl",
                        "무엇을 알아볼까요?"
                    }
                    div {
                        class: "mt-9 grid grid-cols-1 gap-4 md:grid-cols-3",
                        a {
                            id: "platforms",
                            class: "group min-h-56 rounded-2xl border border-slate-200 bg-white p-7 text-slate-950 no-underline transition duration-200 hover:-translate-y-0.5 hover:border-violet-200 hover:bg-violet-50/60 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-violet-500 focus-visible:ring-offset-2",
                            href: "#platforms",
                            div {
                                class: "grid size-11 place-items-center rounded-xl bg-violet-50 text-violet-700 transition group-hover:bg-violet-100",
                                PanelsTopLeft { size: 22, stroke_width: 2 }
                            }
                            h3 { class: "mt-8 text-xl font-extrabold tracking-[-0.025em]", "플랫폼 둘러보기" }
                            p {
                                class: "mt-3 text-[0.95rem] leading-6 text-slate-500",
                                "나에게 맞는 연합우주 소프트웨어를 알아보아요."
                            }
                        }

                        a {
                            id: "servers",
                            class: "group min-h-56 rounded-2xl border border-slate-200 bg-white p-7 text-slate-950 no-underline transition duration-200 hover:-translate-y-0.5 hover:border-violet-200 hover:bg-violet-50/60 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-violet-500 focus-visible:ring-offset-2",
                            href: "#servers",
                            div {
                                class: "grid size-11 place-items-center rounded-xl bg-violet-50 text-violet-700 transition group-hover:bg-violet-100",
                                Server { size: 22, stroke_width: 2 }
                            }
                            h3 { class: "mt-8 text-xl font-extrabold tracking-[-0.025em]", "가입할 서버 알아보기" }
                            p {
                                class: "mt-3 text-[0.95rem] leading-6 text-slate-500",
                                span { class: "font-bold text-slate-700", "한국 서버 78곳" }
                                " · 이용자 95,849명"
                            }
                        }

                        a {
                            id: "federation",
                            class: "group min-h-56 rounded-2xl border border-slate-200 bg-white p-7 text-slate-950 no-underline transition duration-200 hover:-translate-y-0.5 hover:border-violet-200 hover:bg-violet-50/60 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-violet-500 focus-visible:ring-offset-2",
                            href: "#federation",
                            div {
                                class: "grid size-11 place-items-center rounded-xl bg-violet-50 text-violet-700 transition group-hover:bg-violet-100",
                                Network { size: 22, stroke_width: 2 }
                            }
                            h3 { class: "mt-8 text-xl font-extrabold tracking-[-0.025em]", "연합우주 이해하기" }
                            p {
                                class: "mt-3 text-[0.95rem] leading-6 text-slate-500",
                                "서로 다른 서버가 연결되는 원리를 쉽게 알려드려요."
                            }
                        }
                    }
                }
            }
        }
    }
}
