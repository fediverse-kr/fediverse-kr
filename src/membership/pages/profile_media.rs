use super::*;
use crate::membership::{LinkedAccount, ProfileMedia};

#[component]
pub(super) fn ProfileMediaForm(
    accounts: Vec<LinkedAccount>,
    media: ProfileMedia,
    mut pending: Signal<bool>,
    on_changed: EventHandler<()>,
) -> Element {
    let ready = use_context::<Signal<bool>>()();
    let mut selected = use_signal(String::new);
    let selection = if accounts.iter().any(|a| a.id == selected()) {
        selected()
    } else {
        media
            .source_account_id
            .clone()
            .or_else(|| accounts.first().map(|a| a.id.clone()))
            .unwrap_or_default()
    };
    let chosen = selection.clone();
    let mut error = use_signal(String::new);
    let mut notice = use_signal(String::new);
    let mut fetching = use_signal(|| false);
    let blocked = pending() || !ready || media.refreshing || accounts.is_empty();
    rsx! {
        form { class: "membership-card membership-form", aria_label: "프로필 사진과 이모지", aria_busy: fetching() || media.refreshing,
            onsubmit: move |event| {
                event.prevent_default();
                if blocked { return; }
                let account = chosen.clone();
                pending.set(true); fetching.set(true); error.set(String::new()); notice.set(String::new());
                spawn(async move {
                    match api::refresh_profile_media(account).await {
                        Ok(0) => notice.set("사진과 이모지를 가져왔어요.".into()),
                        Ok(_) => notice.set("일부 이미지는 가져오지 못해 기존 이미지를 유지했어요.".into()),
                        Err(e) => error.set(error_message(e)),
                    }
                    pending.set(false); fetching.set(false); on_changed.call(());
                });
            },
            h2 { "사진과 이모지" }
            p { class: "membership-hint", "연합 계정에서 가져옵니다. 여기서 정한 이름과 공개 설정은 바뀌지 않아요." }
            if accounts.is_empty() {
                p { class: "membership-hint", "연합 계정을 연결하면 가져올 수 있어요." }
            } else {
                div { class: "membership-field",
                    label { r#for: "profile-media-source", "가져올 계정" }
                    select { id: "profile-media-source", value: selection, disabled: blocked,
                        oninput: move |event| selected.set(event.value()),
                        for account in &accounts { option { value: account.id.clone(), "{account.handle}" } }
                    }
                }
                if let Some(source) = accounts.iter().find(|a| Some(&a.id) == media.source_account_id.as_ref()) {
                    p { class: "membership-hint", "현재 사진 출처: {source.handle}" }
                }
                button { class: "secondary-button", r#type: "submit", disabled: blocked,
                    if fetching() || media.refreshing { "이미지 가져오는 중…" } else { "사진·이모지 가져오기" }
                }
            }
            if media.refreshing { p { role: "status", "인증한 계정의 이미지를 가져오고 있어요." } }
            if media.refresh_failed && notice().is_empty() && error().is_empty() {
                p { role: "status", class: "membership-hint", "마지막 갱신을 모두 마치지 못했어요. 기존 이미지는 유지합니다. 잠시 후 다시 가져올 수 있어요." }
            }
            FormMessage { error: true, text: error() }
            FormMessage { text: notice() }
        }
    }
}
