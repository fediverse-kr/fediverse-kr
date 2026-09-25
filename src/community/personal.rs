use super::api;
use crate::{
    membership::{api as members, pages::error_message},
    Route,
};
use dioxus::prelude::*;

#[component]
pub fn MyComments() -> Element {
    let mut account = use_server_future(members::session)?;
    let ready = use_context::<Signal<bool>>()();
    rsx! {
        document::Title { "내 댓글 — fediverse.kr" }
        main { id:"content", class:"membership-page wrap",
            Link { class:"back-link", to:Route::Account{}, "내 계정" }
            header { class:"membership-heading", h1 { "내 댓글" } p { "서버에 남긴 댓글과 답글을 모았어요. 나에게만 보이는 목록입니다." } }
            match account() {
                Some(Ok(state)) if state.member.is_some() => rsx! { OwnList { key:"{state.member.unwrap().id}" } },
                Some(Ok(_)) => rsx! { section { class:"membership-card", h2 { "먼저 로그인해 주세요." } Link { class:"primary-button", to:Route::Login{}, "로그인" } } },
                Some(Err(e)) => rsx! { p { role:"alert", {error_message(e)} } button { class:"secondary-button", disabled:!ready, onclick:move |_| account.restart(), "다시 불러오기" } },
                None => rsx! { p { role:"status", "로그인 확인 중…" } },
            }
        }
    }
}

#[component]
fn OwnList() -> Element {
    let ready = use_context::<Signal<bool>>()();
    let mut page = use_signal(|| 0u32);
    let mut result = use_server_future(move || api::own_comments(page()))?;
    rsx! {
        section { class:"own-comments", aria_label:"내가 남긴 댓글",
            match result() {
                Some(Ok(data)) => rsx! {
                    if data.comments.is_empty() { p { class:"membership-notice", if data.page==0 { "아직 작성한 댓글이 없어요." } else { "이 페이지에는 댓글이 없어요." } } }
                    ul { class:"own-comment-list",
                        for comment in data.comments {
                            li { key:"{comment.id}", class:"membership-card",
                                header { class:"own-comment-meta",
                                    if let Some(domain)=comment.domain.as_ref() {
                                        if comment.server_available {
                                            Link { class:"text-link", to:Route::ServerDetail{slug:domain.clone()}, "{domain}" }
                                        } else { span { "{domain}" } }
                                    } else { span { "연결된 서버 없음" } }
                                    span { if comment.reply { "답글" } else { "댓글" } }
                                    time { datetime:comment.created_at.clone(), {format!("{} UTC", comment.created_at.get(..16).unwrap_or(&comment.created_at).replace('T', " "))} }
                                }
                                p { class:"community-body", "{comment.body}" }
                                if comment.truncated { p { class:"membership-hint", "긴 댓글의 앞부분만 표시했어요." } }
                                if !comment.server_available && comment.domain.is_some() { p { class:"membership-hint", "현재 목록에서 숨겨진 서버입니다. 내 댓글만 확인할 수 있어요." } }
                            }
                        }
                    }
                    nav { class:"directory-pagination", aria_label:"내 댓글 목록 페이지",
                        button { class:"secondary-button", disabled:!ready || data.page==0, onclick:move |_| page.set(page().saturating_sub(1)), "이전" }
                        span { "{data.page+1}페이지" }
                        button { class:"secondary-button", disabled:!ready || !data.has_next, onclick:move |_| page.set(page()+1), "다음" }
                    }
                },
                Some(Err(e)) => rsx! { p { class:"membership-message membership-error", role:"alert", {error_message(e)} } },
                None => rsx! { p { role:"status", "내 댓글을 불러오고 있어요." } },
            }
            button { class:"secondary-button", disabled:!ready, onclick:move |_| result.restart(), "목록 새로고침" }
        }
    }
}
