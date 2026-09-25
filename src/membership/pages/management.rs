use super::*;
use crate::membership::LinkedAccount;

#[cfg(all(test, feature = "server"))]
mod tests;

#[component]
pub(super) fn DisplayNameForm(
    current: String,
    mut pending: Signal<bool>,
    on_changed: EventHandler<()>,
) -> Element {
    let ready = use_context::<Signal<bool>>()();
    let mut name = use_signal(move || current);
    let mut error = use_signal(String::new);
    let mut notice = use_signal(String::new);
    rsx! {
        form { class: "membership-card membership-name-form", aria_busy: pending(),
            onsubmit: move |event| {
                event.prevent_default();
                if pending() || !ready { return; }
                pending.set(true); error.set(String::new()); notice.set(String::new());
                let value = name();
                spawn(async move {
                    match api::update_name(value).await {
                        Ok(_) => { notice.set("표시 이름을 저장했어요.".into()); on_changed.call(()); }
                        Err(failure) => error.set(error_message(failure)),
                    }
                    pending.set(false);
                });
            },
            div { class: "membership-field",
                label { r#for: "member-display-name", "fediverse.kr에서 쓸 이름" }
                input { id: "member-display-name", value: "{name}", required: true, maxlength: 64,
                    disabled: pending() || !ready, oninput: move |event| name.set(event.value()),
                }
                p { class: "membership-hint", "연결한 SNS의 이름은 바뀌지 않아요." }
            }
            button { class: "secondary-button", r#type: "submit", disabled: pending() || !ready, "이름 저장" }
            FormMessage { error: true, text: error() }
            FormMessage { text: notice() }
        }
    }
}

#[component]
pub(super) fn LinkedAccountRow(
    account: LinkedAccount,
    can_unlink: bool,
    mut pending: Signal<bool>,
    on_changed: EventHandler<()>,
) -> Element {
    let ready = use_context::<Signal<bool>>()();
    let mut confirming = use_signal(|| false);
    let mut error = use_signal(String::new);
    let navigator = use_navigator();
    let visibility_id = account.id.clone();
    let unlink_id = account.id.clone();
    let public = account.is_public;
    let consent_help_id = format!("directory-consent-{}", account.id);
    rsx! {
        li { class: "membership-linked-row",
            div { class: "membership-linked-heading",
                strong { "{account.display_name}" }
                if safe_profile_url(&account.profile_url) {
                    a { href: "{account.profile_url}", target: "_blank", rel: "noopener noreferrer", "{account.handle}" }
                } else { span { class: "membership-account-handle", "{account.handle}" } }
                span { class: "membership-visibility", if public { "디렉터리 표시 허용됨" } else { "디렉터리 표시 안 함" } }
            }
            div { id: "{consent_help_id}", class: "membership-consent-help",
                p { class: "membership-hint", "이 계정의 프로필과 연합 주소를 fediverse.kr의 사람 찾기 디렉터리에 표시하도록 허용합니다. 허용하지 않아도 로그인과 서버 등록 자격에는 영향이 없고, 원래 SNS의 공개 범위도 바뀌지 않아요." }
                p { class: "membership-hint", "현재는 동의만 저장하며, 사람 찾기·공개 프로필은 아직 시안이라 실제로 공개되지는 않아요." }
            }
            div { class: "membership-actions",
                button { class: "secondary-button", disabled: pending() || !ready, aria_describedby: "{consent_help_id}",
                    onclick: move |_| {
                        if pending() || !ready { return; }
                        let id = visibility_id.clone();
                        pending.set(true); error.set(String::new());
                        spawn(async move {
                            match api::set_link_visibility(id, !public).await {
                                Ok(()) => on_changed.call(()),
                                Err(failure) => error.set(error_message(failure)),
                            }
                            pending.set(false);
                        });
                    },
                    if public { "디렉터리 표시 허용 취소" } else { "프로필을 디렉터리에 표시 허용" }
                }
                button { class: "secondary-button", disabled: pending() || !ready || !can_unlink,
                    onclick: move |_| confirming.set(!confirming()), "연결 해제"
                }
            }
            if !can_unlink { p { class: "membership-hint", "마지막 로그인 수단이에요. 다른 계정이나 ID와 암호를 먼저 등록하세요." } }
            if confirming() {
                div { class: "membership-confirmation",
                    p { "연결을 해제하면 모든 브라우저에서 로그아웃됩니다. 남은 로그인 방법을 사용할 수 있나요?" }
                    div { class: "membership-actions",
                        button { class: "secondary-button membership-danger", disabled: pending() || !ready,
                            onclick: move |_| {
                                if pending() || !ready { return; }
                                let id = unlink_id.clone();
                                pending.set(true); error.set(String::new());
                                spawn(async move {
                                    match api::unlink_account(id).await {
                                        Ok(()) => { navigator.replace(Route::Login {}); }
                                        Err(failure) => error.set(error_message(failure)),
                                    }
                                    pending.set(false);
                                });
                            }, "해제하고 로그아웃"
                        }
                        button { class: "secondary-button", disabled: pending(), onclick: move |_| confirming.set(false), "취소" }
                    }
                }
            }
            FormMessage { error: true, text: error() }
        }
    }
}

#[component]
pub(super) fn Reauthenticate(
    accounts: Vec<LinkedAccount>,
    available: bool,
    pending: Signal<bool>,
    on_changed: EventHandler<()>,
) -> Element {
    let ready = use_context::<Signal<bool>>()();
    let mut selected = use_signal(String::new);
    let mut notice = use_signal(String::new);
    rsx! {
        section { class: "membership-card membership-management-section",
            h2 { "민감한 변경 전, 다시 인증" }
            p { class: "membership-hint", "연결 해제·탈퇴는 다시 로그인한 뒤 15분 안에 할 수 있어요. 아래 계정을 다시 인증하면 로그인 상태를 유지하며 진행할 수 있습니다." }
            if accounts.is_empty() {
                p { class: "membership-hint", "연결된 연합 계정이 없어요. 로그아웃한 뒤 ID와 암호로 다시 로그인해 주세요. 암호도 잊었다면 자동 복구할 수 없습니다." }
            } else {
                div { class: "membership-reauth-choices",
                    for account in accounts {
                        button { class: "secondary-button", disabled: pending() || !ready,
                            onclick: move |_| { selected.set(account.handle.clone()); notice.set(String::new()); },
                            "{account.handle} 다시 인증"
                        }
                    }
                }
                if !selected().is_empty() {
                    FederatedForm { key: "{selected}", initial_handle: selected(), reauthenticating: true,
                        linking: true, available, pending,
                        on_verified: move |_| {
                            selected.set(String::new()); notice.set("다시 인증했어요. 원하는 변경을 진행해 주세요.".into()); on_changed.call(());
                        }
                    }
                }
            }
            FormMessage { text: notice() }
        }
    }
}

#[component]
pub(super) fn WithdrawForm(mut pending: Signal<bool>) -> Element {
    let ready = use_context::<Signal<bool>>()();
    let mut confirmation = use_signal(String::new);
    let mut error = use_signal(String::new);
    let navigator = use_navigator();
    rsx! {
        details { class: "membership-card membership-management-section membership-disclosure",
            summary { "fediverse.kr 탈퇴" }
            p { class: "membership-hint", "이 사이트의 회원 정보·로그인·계정 연결을 삭제하고 서버 운영자 권한을 해제합니다. 작성한 댓글은 삭제 표시되며 작성자 연결을 끊습니다. 댓글 본문·신고·변경 이력은 관리 목적으로 남습니다. 변경 이력에는 당시 회원 식별자가 남을 수 있어요. 연결한 SNS의 계정이나 인증글은 지워지지 않아요." }
            form { class: "membership-form", aria_busy: pending(),
                onsubmit: move |event| {
                    event.prevent_default();
                    if pending() || !ready { return; }
                    let value = confirmation();
                    pending.set(true); error.set(String::new());
                    spawn(async move {
                        match api::withdraw(value).await {
                            Ok(()) => { navigator.replace(Route::Home {}); }
                            Err(failure) => error.set(error_message(failure)),
                        }
                        pending.set(false);
                    });
                },
                div { class: "membership-field",
                    label { r#for: "withdraw-confirmation", "확인하려면 ‘탈퇴’를 입력하세요." }
                    input { id: "withdraw-confirmation", value: "{confirmation}", required: true,
                        autocomplete: "off", disabled: pending() || !ready, oninput: move |event| confirmation.set(event.value()),
                    }
                }
                FormMessage { error: true, text: error() }
                button { class: "secondary-button membership-danger", r#type: "submit",
                    disabled: pending() || !ready || confirmation() != "탈퇴", "탈퇴하고 로그아웃"
                }
            }
        }
    }
}
