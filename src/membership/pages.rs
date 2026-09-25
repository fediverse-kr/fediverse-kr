//! Membership screens. Verification and credentials live only in component memory;
//! server functions own identity checks, authorization, sessions, and persistence.
use super::{api, AccountState, Challenge, Member};
use crate::Route;
use dioxus::prelude::*;

mod management;
mod profile_media;
use management::{DisplayNameForm, LinkedAccountRow, Reauthenticate, WithdrawForm};

const PASSWORD_HELP: &str = "15~128자로 입력해 주세요. 다른 사이트에서 쓰는 암호는 피하세요.";

pub(crate) fn error_message(error: ServerFnError) -> String {
    // API ServerError.message is an intentionally sanitized user-facing message.
    // Never display Display/Debug: those also contain transport internals/details.
    match error {
        ServerFnError::ServerError { message, .. } if !message.trim().is_empty() => message,
        _ => "요청을 완료하지 못했어요. 연결을 확인하고 다시 시도해 주세요.".into(),
    }
}

fn validate_password(password: &str, confirmation: &str) -> Result<(), &'static str> {
    if !(15..=128).contains(&password.chars().count()) {
        return Err("암호는 15~128자로 입력해 주세요.");
    }
    if password != confirmation {
        return Err("두 암호가 일치하지 않아요.");
    }
    Ok(())
}

fn verification_post(code: &str) -> String {
    format!("fediverse.kr 계정 소유 인증\n{code}")
}

fn safe_profile_url(url: &str) -> bool {
    url.starts_with("https://") && !url.chars().any(char::is_control)
}

#[component]
pub fn Login() -> Element {
    let ready = use_context::<Signal<bool>>()();
    let pending = use_signal(|| false);
    let navigator = use_navigator();
    let mut account = use_server_future(api::session)?;

    rsx! {
        document::Title { "로그인 — fediverse.kr" }
        main { id: "content", class: "membership-page membership-login wrap",
            header { class: "membership-heading", h1 { "내 연합 계정으로 시작해요." } p { "이미 쓰고 있는 계정의 주인인지 확인하면 됩니다." } }
            match account() {
                Some(Ok(state)) if state.member.is_some() => rsx! {
                    section { class: "membership-card",
                        h2 { "이미 로그인되어 있어요." }
                        Link { class: "primary-button", to: Route::Account {}, "내 계정으로" }
                    }
                },
                Some(Ok(state)) => rsx! {
                    p { class: "membership-hint", "기존 회원이라면 예전에 인증한 연합 계정 주소를 사용해 주세요. 새 공개 인증글로 확인하면 기존 정보가 이어집니다." }
                    section { class: "membership-card", aria_label: "연합 계정으로 로그인",
                        FederatedForm {
                            available: state.federation_available && ready,
                            pending,
                            on_verified: move |_| { navigator.push(Route::Account {}); },
                        }
                    }
                    details { class: "membership-card membership-disclosure",
                        summary { "fediverse.kr ID와 암호가 있나요?" }
                        p { class: "membership-hint", "연합 계정 인증 후 별도로 설정한 ID와 암호로 로그인할 수 있어요." }
                        PasswordLogin { enabled: ready, pending, on_logged_in: move |_| { navigator.push(Route::Account {}); } }
                    }
                },
                Some(Err(error)) => rsx! {
                    section { class: "membership-card",
                        FormMessage { error: true, text: error_message(error) }
                        button { class: "secondary-button", disabled: !ready, onclick: move |_| account.restart(), "다시 불러오기" }
                    }
                },
                None => rsx! { p { role: "status", "로그인 정보를 확인하고 있어요." } },
            }
        }
    }
}

/// The exact same public-post challenge is used for first login and adding an account.
#[component]
fn FederatedForm(
    available: bool,
    mut pending: Signal<bool>,
    #[props(default)] linking: bool,
    #[props(default)] initial_handle: String,
    #[props(default)] reauthenticating: bool,
    on_verified: EventHandler<Member>,
) -> Element {
    let mut handle = use_signal(move || initial_handle);
    let mut challenge = use_signal(|| None::<Challenge>);
    let mut error = use_signal(String::new);
    let ready = use_context::<Signal<bool>>()();
    let blocked = pending() || !available || !ready;
    let prefix = if reauthenticating {
        "reauthenticate"
    } else if linking {
        "link-account"
    } else {
        "federated-login"
    };

    rsx! {
        if !available && ready {
            p { class: "membership-notice", role: "status", "지금은 연합 계정 인증을 사용할 수 없어요. 잠시 후 다시 시도해 주세요." }
        }
        if let Some(issued) = challenge() {
            div { class: "membership-verification",
                h2 { "이 문구를 공개 글로 올려 주세요." }
                p { class: "membership-hint", strong { "{issued.handle}" } " 계정에서 작성해 주세요. 글은 자동으로 게시되지 않아요." }
                label { r#for: "{prefix}-post", "복사할 인증 문구" }
                textarea {
                    id: "{prefix}-post", class: "membership-proof", readonly: true,
                    rows: 3, spellcheck: false, value: verification_post(&issued.code),
                    aria_describedby: "{prefix}-proof-help",
                    onmounted: move |event| async move { let _ = event.data().set_focus(true).await; },
                }
                p { id: "{prefix}-proof-help", class: "membership-hint",
                    "공개 범위를 ‘공개’로 선택하고 게시한 뒤, 아래 버튼을 눌러 주세요."
                }
                p { class: "membership-expiry", "유효 기한: " time { datetime: "{issued.expires_at}", "{issued.expires_at}" } }
                FormMessage { error: true, text: error() }
                div { class: "membership-actions",
                    button {
                        class: "primary-button", r#type: "button", disabled: blocked,
                        onclick: move |_| {
                            if pending() || !available || !ready { return; }
                            let Some(issued) = challenge() else { return; };
                            pending.set(true);
                            error.set(String::new());
                            spawn(async move {
                                match api::finish_federated(issued.id, issued.code).await {
                                    Ok(member) => {
                                        challenge.set(None);
                                        handle.set(String::new());
                                        pending.set(false);
                                        on_verified.call(member);
                                    }
                                    Err(failure) => {
                                        error.set(error_message(failure));
                                        pending.set(false);
                                    }
                                }
                            });
                        },
                        if pending() { "공개 글 확인 중…" } else { "게시했어요 · 확인하기" }
                    }
                    button {
                        class: "secondary-button", r#type: "button", disabled: pending(),
                        onclick: move |_| {
                            if pending() { return; }
                            challenge.set(None);
                            error.set(String::new());
                        },
                        "다시 시작"
                    }
                }
                p { class: "membership-hint", "이 화면을 새로고침하면 인증 문구를 다시 발급받아야 해요." }
            }
        } else {
            form {
                class: "membership-form", aria_busy: pending(),
                onsubmit: move |event| {
                    event.prevent_default();
                    if pending() || !available || !ready { return; }
                    let input = handle().trim().to_string();
                    if input.is_empty() {
                        error.set("연합 계정 주소를 입력해 주세요.".into());
                        return;
                    }
                    pending.set(true);
                    error.set(String::new());
                    spawn(async move {
                        match api::begin_federated(input).await {
                            Ok(issued) => challenge.set(Some(issued)),
                            Err(failure) => error.set(error_message(failure)),
                        }
                        pending.set(false);
                    });
                },
                h2 { if reauthenticating { "내 계정 다시 인증" } else if linking { "다른 계정 연결" } else { "연합 계정 확인" } }
                div { class: "membership-field",
                    label { r#for: "{prefix}-handle", "연합 계정 주소" }
                    input {
                        id: "{prefix}-handle", name: "federated-handle", r#type: "text",
                        autocomplete: "username", autocapitalize: "none", spellcheck: false,
                        required: true, disabled: blocked, readonly: reauthenticating, value: "{handle}",
                        placeholder: "@이름@서버주소", aria_describedby: "{prefix}-help",
                        oninput: move |event| handle.set(event.value()),
                    }
                    p { id: "{prefix}-help", class: "membership-hint", "예: @naru@social.example · 프로필에서 계정 주소를 확인할 수 있어요." }
                }
                FormMessage { error: true, text: error() }
                button { class: "primary-button", r#type: "submit", disabled: blocked,
                    if pending() { "계정 확인 중…" } else { "인증 문구 받기" }
                }
                p { class: "membership-hint", "다음 단계에서 인증 문구를 내 계정의 공개 글로 올리면 됩니다. SNS 암호는 필요 없어요." }
            }
        }
    }
}

#[component]
fn PasswordLogin(
    enabled: bool,
    mut pending: Signal<bool>,
    on_logged_in: EventHandler<Member>,
) -> Element {
    let mut login_id = use_signal(String::new);
    let mut password = use_signal(String::new);
    let mut error = use_signal(String::new);
    rsx! {
        form { class: "membership-form", aria_busy: pending(),
            onsubmit: move |event| {
                event.prevent_default();
                if pending() || !enabled { return; }
                let id = login_id().trim().to_string();
                let secret = password();
                if id.is_empty() || secret.is_empty() {
                    error.set("ID와 암호를 입력해 주세요.".into());
                    return;
                }
                pending.set(true);
                error.set(String::new());
                password.set(String::new());
                spawn(async move {
                    match api::password_login(id, secret).await {
                        Ok(member) => {
                            pending.set(false);
                            on_logged_in.call(member);
                        }
                        Err(failure) => {
                            error.set(error_message(failure));
                            pending.set(false);
                        }
                    }
                });
            },
            div { class: "membership-field",
                label { r#for: "password-login-id", "fediverse.kr ID" }
                input { id: "password-login-id", name: "username", autocomplete: "username",
                    required: true, disabled: pending() || !enabled, autocapitalize: "none", spellcheck: false,
                    value: "{login_id}", oninput: move |event| login_id.set(event.value()),
                }
            }
            div { class: "membership-field",
                label { r#for: "password-login-password", "암호" }
                input { id: "password-login-password", name: "password", r#type: "password", autocomplete: "current-password",
                    required: true, disabled: pending() || !enabled,
                    value: "{password}", oninput: move |event| password.set(event.value()),
                }
            }
            FormMessage { error: true, text: error() }
            button { class: "primary-button", r#type: "submit", disabled: pending() || !enabled,
                if pending() { "로그인 중…" } else { "ID로 로그인" }
            }
        }
    }
}

#[component]
pub fn Account() -> Element {
    let ready = use_context::<Signal<bool>>()();
    let pending = use_signal(|| false);
    let mut account = use_server_future(api::session)?;
    rsx! {
        document::Title { "내 계정 — fediverse.kr" }
        main { id: "content", class: "membership-page wrap",
            header { class: "membership-heading", h1 { "내 계정" } p { "연결한 연합 계정과 로그인 방법을 관리해요." }
                nav { class:"membership-actions", aria_label:"내 활동", Link {class:"text-link",to:Route::MyComments{},"내 댓글"} Link {class:"text-link",to:Route::ManagedSites{},"내 서버 관리"} }
            }
            match account() {
                Some(Ok(state)) if state.member.is_some() => rsx! {
                    AccountPanel { state, pending, on_changed: move |_| account.restart() }
                },
                Some(Ok(_)) => rsx! {
                    section { class: "membership-card", h2 { "먼저 로그인해 주세요." }
                        Link { class: "primary-button", to: Route::Login {}, "로그인" }
                    }
                },
                Some(Err(error)) => rsx! {
                    section { class: "membership-card",
                        FormMessage { error: true, text: error_message(error) }
                        button { class: "secondary-button", disabled: !ready, onclick: move |_| account.restart(), "다시 불러오기" }
                    }
                },
                None => rsx! { p { role: "status", "계정 정보를 확인하고 있어요." } },
            }
        }
    }
}

#[component]
fn AccountPanel(
    state: AccountState,
    mut pending: Signal<bool>,
    on_changed: EventHandler<()>,
) -> Element {
    let ready = use_context::<Signal<bool>>()();
    let mut error = use_signal(String::new);
    let mut linked_notice = use_signal(String::new);
    let mut profile = use_server_future(api::profile_media)?;
    let media = profile().and_then(Result::ok).unwrap_or_default();
    // Only a known active background refresh is polled; no constant session fetch.
    use_future(move || async move {
        #[cfg(target_arch = "wasm32")]
        loop {
            gloo_timers::future::TimeoutFuture::new(1500).await;
            if profile
                .peek()
                .as_ref()
                .is_some_and(|r| r.as_ref().is_ok_and(|p| p.refreshing))
            {
                profile.restart();
            }
        }
    });
    let navigator = use_navigator();
    let Some(member) = state.member else {
        return rsx! {};
    };
    let has_credentials = member.login_id.is_some();
    let can_unlink = has_credentials || state.linked_accounts.len() > 1;
    let linked_accounts = state.linked_accounts.clone();
    let credential_mode = if has_credentials {
        "change-password"
    } else {
        "set-credentials"
    };
    rsx! {
        div { class: "membership-account-bar",
            p { super::profile::PrivateIdentity{key:"{member.id}",name:member.display_name.clone(),media:media.clone()} }
            button { class: "secondary-button", disabled: pending() || !ready,
                onclick: move |_| {
                    if pending() || !ready { return; }
                    pending.set(true);
                    error.set(String::new());
                    spawn(async move {
                        match api::logout().await {
                            Ok(()) => {
                                pending.set(false);
                                navigator.replace(Route::Login {});
                            }
                            Err(failure) => {
                                error.set(error_message(failure));
                                pending.set(false);
                            }
                        }
                    });
                },
                "로그아웃"
            }
        }
        FormMessage { error: true, text: error() }
        DisplayNameForm { current: member.display_name.clone(), pending, on_changed }
        profile_media::ProfileMediaForm { accounts: linked_accounts.clone(), media, pending,
            on_changed: move |_| { profile.restart(); on_changed.call(()); }
        }
        crate::moderation::pages::AdminEntry {}
        div { class: "membership-account-grid",
            section { class: "membership-card", aria_label: "연결한 연합 계정",
                h2 { "연결한 연합 계정" }
                if state.linked_accounts.is_empty() {
                    p { class: "membership-hint", "아직 표시할 연결 계정이 없어요." }
                } else {
                    ul { class: "membership-accounts",
                        for linked in state.linked_accounts {
                            LinkedAccountRow { key: "{linked.id}", account: linked, can_unlink, pending, on_changed }
                        }
                    }
                }
                FormMessage { text: linked_notice() }
                details { class: "membership-add-account",
                    summary { "다른 연합 계정 연결하기" }
                    FederatedForm {
                        available: state.federation_available && ready, linking: true, pending,
                        on_verified: move |_| {
                            linked_notice.set("연합 계정을 연결했어요.".into());
                            on_changed.call(());
                        },
                    }
                }
                p { class: "membership-hint", "인증글은 원래 SNS에 공개되며, 디렉터리 표시 허용을 취소해도 비공개로 바뀌지는 않아요." }
            }
            section { class: "membership-card", aria_label: "ID와 암호 관리",
                h2 { if has_credentials { "암호 변경" } else { "ID와 암호 설정" } }
                if let Some(id) = member.login_id {
                    div { key: "{credential_mode}", class: "membership-credential-group",
                        p { class: "membership-hint", "fediverse.kr ID: " strong { "{id}" } }
                        CredentialForm { current_id: id.clone(), has_credentials: true, pending, on_saved: move |_| on_changed.call(()) }
                        details { class: "membership-add-account",
                            summary { "현재 암호를 잊었나요?" }
                            p { class: "membership-hint", "아래에서 이미 연결된 계정으로 다시 인증한 뒤 새 암호를 설정하세요. 다른 브라우저는 로그아웃됩니다." }
                            CredentialForm { current_id: id, has_credentials: true, recovery: true, pending, on_saved: move |_| on_changed.call(()) }
                        }
                    }
                } else {
                    div { key: "{credential_mode}", class: "membership-credential-group",
                        p { class: "membership-hint", "다음부터 공개 글 인증 없이 로그인하고 싶다면 설정해 주세요. 필수는 아니에요." }
                        CredentialForm { pending, on_saved: move |_| on_changed.call(()) }
                    }
                }
            }
        }
        Reauthenticate { accounts: linked_accounts, available: state.federation_available, pending, on_changed }
        WithdrawForm { pending }
    }
}

#[component]
fn CredentialForm(
    mut pending: Signal<bool>,
    on_saved: EventHandler<Member>,
    #[props(default)] current_id: String,
    #[props(default)] has_credentials: bool,
    #[props(default)] recovery: bool,
) -> Element {
    let ready = use_context::<Signal<bool>>()();
    let mut login_id = use_signal(String::new);
    let mut current_password = use_signal(String::new);
    let mut password = use_signal(String::new);
    let mut confirmation = use_signal(String::new);
    let mut error = use_signal(String::new);
    let mut notice = use_signal(String::new);
    let prefix = if recovery { "recovery" } else { "account" };
    rsx! {
        form { class: "membership-form", aria_busy: pending(),
            onsubmit: move |event| {
                event.prevent_default();
                if pending() || !ready { return; }
                error.set(String::new());
                notice.set(String::new());
                let id = login_id().trim().to_string();
                let new_password = password();
                if !has_credentials && id.is_empty() {
                    error.set("사용할 ID를 입력해 주세요.".into());
                    return;
                }
                if let Err(message) = validate_password(&new_password, &confirmation()) {
                    error.set(message.into());
                    return;
                }
                let old_password = current_password();
                if has_credentials && !recovery && old_password.is_empty() {
                    error.set("현재 암호를 입력해 주세요.".into());
                    return;
                }
                pending.set(true);
                current_password.set(String::new());
                password.set(String::new());
                confirmation.set(String::new());
                spawn(async move {
                    let result = if recovery {
                        api::recover_password(new_password).await
                    } else if has_credentials {
                        api::change_password(old_password, new_password).await
                    } else {
                        api::set_credentials(id, new_password).await
                    };
                    match result {
                        Ok(member) => {
                            notice.set(if has_credentials { "암호를 변경했어요." } else { "ID와 암호를 설정했어요." }.into());
                            pending.set(false);
                            on_saved.call(member);
                        }
                        Err(failure) => {
                            error.set(error_message(failure));
                            pending.set(false);
                        }
                    }
                });
            },
            if has_credentials {
                // Password managers can associate the change with the existing ID.
                input { r#type: "hidden", name: "username", autocomplete: "username", value: "{current_id}" }
            }
            if has_credentials && !recovery {
                div { class: "membership-field",
                    label { r#for: "account-current-password", "현재 암호" }
                    input { id: "account-current-password", name: "current-password", r#type: "password", autocomplete: "current-password",
                        required: true, disabled: pending() || !ready,
                        value: "{current_password}", oninput: move |event| current_password.set(event.value()),
                    }
                }
            } else if !has_credentials {
                div { class: "membership-field",
                    label { r#for: "account-login-id", "사용할 fediverse.kr ID" }
                    input { id: "account-login-id", name: "username", autocomplete: "username",
                        required: true, disabled: pending() || !ready, autocapitalize: "none", spellcheck: false,
                        value: "{login_id}", oninput: move |event| login_id.set(event.value()),
                    }
                }
            }
            div { class: "membership-field",
                label { r#for: "{prefix}-new-password", "새 암호" }
                input { id: "{prefix}-new-password", name: "new-password", r#type: "password", autocomplete: "new-password",
                    required: true, disabled: pending() || !ready,
                    aria_describedby: "{prefix}-password-help", value: "{password}", oninput: move |event| password.set(event.value()),
                }
                p { id: "{prefix}-password-help", class: "membership-hint", "{PASSWORD_HELP}" }
            }
            div { class: "membership-field",
                label { r#for: "{prefix}-confirm-password", "새 암호 확인" }
                input { id: "{prefix}-confirm-password", name: "confirm-password", r#type: "password", autocomplete: "new-password",
                    required: true, disabled: pending() || !ready,
                    value: "{confirmation}", oninput: move |event| confirmation.set(event.value()),
                }
            }
            FormMessage { error: true, text: error() }
            FormMessage { text: notice() }
            button { class: "primary-button", r#type: "submit", disabled: pending() || !ready,
                if pending() { "저장 중…" } else if recovery { "인증한 계정으로 암호 재설정" } else if has_credentials { "암호 변경" } else { "ID와 암호 설정" }
            }
        }
    }
}

#[component]
fn FormMessage(text: String, #[props(default)] error: bool) -> Element {
    rsx! {
        if !text.is_empty() {
            p {
                class: if error { "membership-message membership-error" } else { "membership-message membership-success" },
                role: if error { "alert" } else { "status" },
                "{text}"
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_password_length_uses_characters_without_trimming() {
        assert!(validate_password(&"가".repeat(15), &"가".repeat(15)).is_ok());
        assert!(validate_password(&"a".repeat(128), &"a".repeat(128)).is_ok());
        assert!(validate_password(&"a".repeat(14), &"a".repeat(14)).is_err());
        assert!(validate_password(&"a".repeat(129), &"a".repeat(129)).is_err());
        assert!(validate_password(" fourteen chars", " fourteen chars").is_ok());
    }

    #[test]
    fn confirmation_must_match() {
        assert!(validate_password("long-enough-password", "long-enough-other").is_err());
    }

    #[test]
    fn public_post_contains_server_issued_code_verbatim() {
        assert_eq!(
            verification_post("example-code"),
            "fediverse.kr 계정 소유 인증\nexample-code"
        );
    }

    #[test]
    fn stored_profile_urls_cannot_be_script_links() {
        assert!(safe_profile_url("https://social.example/@naru"));
        assert!(!safe_profile_url("javascript:alert(1)"));
        assert!(!safe_profile_url("https://social.example/\n"));
        assert!(!safe_profile_url("//social.example/@naru"));
    }

    #[test]
    fn user_errors_never_include_server_details_or_transport_errors() {
        let failure = ServerFnError::ServerError {
            message: "다시 확인해 주세요.".into(),
            code: 400,
            details: Some(serde_json::json!({ "private": "must-not-render" })),
        };
        assert_eq!(error_message(failure), "다시 확인해 주세요.");
        let transport = error_message(ServerFnError::Deserialization("private-response".into()));
        assert!(!transport.contains("private-response"));
    }
}
