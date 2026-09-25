use super::ProfileBatch;
#[cfg(feature = "server")]
use crate::{backend::config, membership::api::server::*};
#[cfg(feature = "server")]
use dioxus::fullstack::HeaderMap;
use dioxus::prelude::*;

#[get("/api/member/moderation/profile-batch",headers:HeaderMap)]
pub async fn status() -> Result<Option<ProfileBatch>, ServerFnError> {
    private_response();
    let state = config::state().await.map_err(|_| unavailable())?;
    let session = super::api::session(state, &headers).await?;
    state
        .db
        .profile_batch_status(&session)
        .await
        .map_err(super::api::failure)
}
#[post("/api/member/moderation/profile-batch/start",headers:HeaderMap)]
pub async fn start(confirmed: bool) -> Result<ProfileBatch, ServerFnError> {
    let state = write_state(&headers).await?;
    let session = super::api::session(state, &headers).await?;
    if !confirmed {
        return Err(error(400, "전체 갱신 여부를 확인해 주세요."));
    }
    if state.media.is_none() {
        return Err(error(503, "사진 저장소를 먼저 설정해 주세요."));
    }
    state
        .db
        .start_profile_batch(&session)
        .await
        .map_err(super::api::failure)
}
#[post("/api/member/moderation/profile-batch/cancel",headers:HeaderMap)]
pub async fn cancel(id: String) -> Result<ProfileBatch, ServerFnError> {
    let state = write_state(&headers).await?;
    let session = super::api::session(state, &headers).await?;
    let id = crate::backend::moderation::id(&id).map_err(super::api::failure)?;
    state
        .db
        .cancel_profile_batch(&session, id)
        .await
        .map_err(super::api::failure)
}

#[component]
pub fn ModerationProfiles() -> Element {
    rsx! {
        document::Title { "프로필 유지보수 — fediverse.kr" }
        document::Stylesheet { href: asset!("/assets/styling/backoffice-operations.css") }
        super::layout::BackofficePage {
            section: super::layout::BackofficeSection::Operations,
            title: "프로필 유지보수".to_string(),
            description: "회원 프로필 미디어를 일괄 갱신하되, 기존 신원과 공개 설정은 그대로 둡니다.".to_string(),
            super::workers::pages::OperationsNav { profiles: true }
            ProfileMaintenance {}
        }
    }
}
#[component]
pub fn ProfileMaintenance() -> Element {
    let ready = use_context::<Signal<bool>>()();
    let mut batch = use_server_future(status)?;
    let mut confirmed = use_signal(|| false);
    let mut pending = use_signal(|| false);
    let mut error = use_signal(String::new);
    #[cfg(target_arch = "wasm32")]
    use_future(move || async move {
        loop {
            gloo_timers::future::TimeoutFuture::new(1500).await;
            if batch.peek().as_ref().is_some_and(|r| {
                r.as_ref()
                    .is_ok_and(|v| v.as_ref().is_some_and(|v| v.state == "running"))
            }) {
                batch.restart();
            }
        }
    });
    rsx! {section {class:"backoffice-panel profile-maintenance",aria_label:"전체 프로필 갱신",
        h2 {"전체 프로필 갱신"}
        p {class:"profile-maintenance-scope","회원 사진과 이모지만 갱신합니다. 이름과 공개 범위는 바꾸지 않습니다."}
        p {class:"membership-hint","전체 갱신을 시작하거나 남은 대기 작업을 중단하려면 최근 로그인·재인증이 필요합니다."}
        match batch() {
            Some(Ok(current))=> {
                let running=current.as_ref().is_some_and(|v|v.state=="running");
                rsx! {
                    if let Some(current)=current {
                        p {role:"status",class:"membership-hint",
                            if running {"진행 중 · "} else if current.state=="cancelled" {"중단됨 · "}else{"완료 · "}
                            "전체 {current.total}명 / 성공 {current.succeeded} / 일부 실패 {current.partial} / 실패 {current.failed} / 건너뜀 {current.skipped} / 취소 {current.cancelled}"
                        }
                        if running {
                            progress { max:current.total.max(1).to_string(), value:(current.total-current.pending-current.running).to_string(), aria_label:"프로필 갱신 진행" }
                            button {class:"secondary-button",disabled:!ready||pending(),onclick:move |_| {
                                pending.set(true); error.set(String::new()); let id=current.id.clone();
                                spawn(async move { if let Err(e)=cancel(id).await {error.set(crate::membership::pages::error_message(e));} batch.restart();pending.set(false); });
                            },"남은 갱신 중단"}
                        }
                    }
                    if !running {form { class:"membership-form",onsubmit:move |event| {
                        event.prevent_default(); if !ready||pending()||!confirmed(){return}
                        pending.set(true);error.set(String::new());
                        spawn(async move {if let Err(e)=start(true).await {error.set(crate::membership::pages::error_message(e));}batch.restart();confirmed.set(false);pending.set(false);});
                    },
                        label {class:"owner-checkbox",input {r#type:"checkbox",checked:confirmed(),disabled:!ready||pending(),onchange:move |event|confirmed.set(event.checked())} "모든 회원의 사진·이모지를 다시 가져옵니다."}
                        button {class:"secondary-button",r#type:"submit",disabled:!ready||pending()||!confirmed(),"전체 갱신 시작"}
                    }}
                }
            },
            Some(Err(e))=>rsx!{p {role:"alert",{crate::membership::pages::error_message(e)}}button{class:"secondary-button",disabled:!ready,onclick:move |_|batch.restart(),"상태 다시 확인"}},
            None=>rsx!{p {role:"status","갱신 상태를 확인하고 있어요."}},
        }
        if !error().is_empty(){p {role:"alert",class:"membership-error","{error}"}}
        p {class:"membership-hint","기존에 선택한 출처를 사용합니다. 출처가 없거나 최근 갱신한 회원은 건너뛰며, 창을 닫아도 작업은 계속됩니다."}
    }}
}
