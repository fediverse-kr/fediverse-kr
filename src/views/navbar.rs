use crate::Route;
use dioxus::prelude::*;
use lucide_dioxus::{Globe, LogIn};

const NAVBAR_CSS: Asset = asset!("/assets/styling/navbar.css");

/// The global navigation layout shared by all routes.
#[component]
pub fn Navbar() -> Element {
    rsx! {
        document::Link { rel: "stylesheet", href: NAVBAR_CSS }

        header {
            class: "sticky top-0 z-50 border-b border-slate-200 bg-white/95 backdrop-blur",
            nav {
                class: "mx-auto flex h-[72px] w-full max-w-[1180px] items-center justify-between px-6 sm:px-8 lg:px-10",
                aria_label: "주요 메뉴",
                Link {
                    class: "flex items-center gap-3 text-slate-950 no-underline",
                    to: Route::Home {},
                    span {
                        class: "grid size-9 place-items-center rounded-xl bg-violet-600 text-white shadow-sm",
                        Globe { size: 20, stroke_width: 2 }
                    }
                    span { class: "text-lg font-extrabold tracking-[-0.03em]", "fediverse.kr" }
                }

                div {
                    class: "flex items-center gap-2 sm:gap-6",
                    div {
                        class: "hidden items-center gap-6 md:flex",
                        a { class: "nav-link", href: "#main-content", "연합우주" }
                        a { class: "nav-link", href: "#main-content", "서버 찾기" }
                        a { class: "nav-link", href: "#main-content", "가이드" }
                    }
                    a {
                        class: "inline-flex h-10 items-center gap-2 rounded-xl border border-slate-200 bg-white px-3.5 text-sm font-bold text-slate-800 no-underline transition hover:border-violet-200 hover:bg-violet-50 hover:text-violet-700 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-violet-500 focus-visible:ring-offset-2",
                        href: "#login",
                        LogIn { size: 17, stroke_width: 2 }
                        span { "로그인" }
                    }
                }
            }
        }

        Outlet::<Route> {}
    }
}
