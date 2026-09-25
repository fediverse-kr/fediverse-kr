use super::{ActionRequest, EventPage, ReportDetail, ReportPage};
#[cfg(feature = "server")]
use crate::{
    backend::{config, moderation as service},
    membership::api::server::*,
};
#[cfg(feature = "server")]
use dioxus::fullstack::HeaderMap;
use dioxus::prelude::*;
#[cfg(feature = "server")]
pub(super) fn failure(e: service::Error) -> ServerFnError {
    use service::Error::*;
    if let Auth(e) = e {
        return auth_error(e);
    }
    error(
        match e {
            Forbidden | Protected => 403,
            Missing => 404,
            Invalid => 400,
            Conflict => 409,
            RefreshTooSoon => 429,
            _ => 503,
        },
        &e.to_string(),
    )
}
#[cfg(feature = "server")]
pub(super) async fn session(
    state: &config::State,
    headers: &HeaderMap,
) -> Result<crate::backend::auth::AuthenticatedSession, ServerFnError> {
    session_details(state, headers)
        .await?
        .ok_or_else(|| auth_error(crate::backend::auth::AuthError::Unauthenticated))
}
#[get("/api/member/moderation/access",headers:HeaderMap)]
pub async fn access() -> Result<bool, ServerFnError> {
    private_response();
    if std::env::var_os("FEDKR_DATABASE_URL").is_none() {
        return Ok(false);
    }
    let state = config::state().await.map_err(|_| unavailable())?;
    let Some(s) = session_details(state, &headers).await? else {
        return Ok(false);
    };
    state.db.moderation_access(&s).await.map_err(failure)
}
#[get("/api/member/moderation/reports?status&page",headers:HeaderMap)]
pub async fn reports(status: String, page: u32) -> Result<ReportPage, ServerFnError> {
    private_response();
    let state = config::state().await.map_err(|_| unavailable())?;
    let s = session(state, &headers).await?;
    state
        .db
        .moderation_reports(&s, &status, page)
        .await
        .map_err(failure)
}
#[get("/api/member/moderation/report?id",headers:HeaderMap)]
pub async fn report(id: String) -> Result<ReportDetail, ServerFnError> {
    private_response();
    let state = config::state().await.map_err(|_| unavailable())?;
    let s = session(state, &headers).await?;
    state
        .db
        .moderation_report(&s, service::id(&id).map_err(failure)?)
        .await
        .map_err(failure)
}
#[get("/api/member/moderation/events?id&page",headers:HeaderMap)]
pub async fn events(id: String, page: u32) -> Result<EventPage, ServerFnError> {
    private_response();
    let state = config::state().await.map_err(|_| unavailable())?;
    let s = session(state, &headers).await?;
    state
        .db
        .moderation_events(&s, service::id(&id).map_err(failure)?, page)
        .await
        .map_err(failure)
}
#[post("/api/member/moderation/act",headers:HeaderMap)]
pub async fn act(request: ActionRequest) -> Result<ReportDetail, ServerFnError> {
    let state = write_state(&headers).await?;
    let s = session(state, &headers).await?;
    state.db.moderate_report(&s, request).await.map_err(failure)
}
