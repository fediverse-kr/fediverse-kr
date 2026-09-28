use super::{HostingEdit, HostingHistory, HostingPage, HostingService};
#[cfg(feature = "server")]
use crate::{
    backend::{config, hosting as domain},
    membership::api::server::*,
};
#[cfg(feature = "server")]
use dioxus::fullstack::HeaderMap;
use dioxus::prelude::*;

#[cfg(feature = "server")]
fn failure(e: domain::Error) -> ServerFnError {
    use domain::Error::*;
    if let Auth(e) = e {
        return auth_error(e);
    }
    error(
        match e {
            Invalid => 400,
            Missing => 404,
            Duplicate | Conflict => 409,
            RateLimited => 429,
            _ => 503,
        },
        &e.to_string(),
    )
}
#[get("/api/public/hosting?page")]
pub async fn list(page: u32) -> Result<HostingPage, ServerFnError> {
    if std::env::var_os("FEDKR_DATABASE_URL").is_none() {
        return Ok(HostingPage {
            items: vec![],
            page,
            has_next: false,
        });
    }
    config::state()
        .await
        .map_err(|_| unavailable())?
        .db
        .hosting_list(page)
        .await
        .map_err(failure)
}
#[get("/api/public/hosting/detail?slug")]
pub async fn detail(slug: String) -> Result<HostingService, ServerFnError> {
    config::state()
        .await
        .map_err(|_| unavailable())?
        .db
        .hosting(&slug)
        .await
        .map_err(failure)
}
#[get("/api/public/hosting/history?slug&page")]
pub async fn history(slug: String, page: u32) -> Result<HostingHistory, ServerFnError> {
    config::state()
        .await
        .map_err(|_| unavailable())?
        .db
        .hosting_history(&slug, page)
        .await
        .map_err(failure)
}
#[get("/api/public/hosting/revision?slug&revision")]
pub async fn read_revision(slug: String, revision: i64) -> Result<HostingEdit, ServerFnError> {
    config::state()
        .await
        .map_err(|_| unavailable())?
        .db
        .hosting_revision(&slug, revision)
        .await
        .map_err(failure)
}
#[post("/api/member/hosting/create",headers:HeaderMap)]
pub async fn create(
    slug: String,
    edit: HostingEdit,
    summary: String,
) -> Result<HostingService, ServerFnError> {
    let state = write_state_limit(&headers, 32768).await?;
    let session = session_details(state, &headers)
        .await?
        .ok_or_else(|| auth_error(crate::backend::auth::AuthError::Unauthenticated))?;
    state
        .db
        .create_hosting(&session, &slug, edit, summary)
        .await
        .map_err(failure)
}
#[post("/api/member/hosting/save",headers:HeaderMap)]
pub async fn save(
    slug: String,
    revision: i64,
    edit: HostingEdit,
    summary: String,
) -> Result<HostingService, ServerFnError> {
    let state = write_state_limit(&headers, 32768).await?;
    let session = session_details(state, &headers)
        .await?
        .ok_or_else(|| auth_error(crate::backend::auth::AuthError::Unauthenticated))?;
    state
        .db
        .save_hosting(&session, &slug, revision, edit, summary)
        .await
        .map_err(failure)
}
#[post("/api/member/hosting/restore",headers:HeaderMap)]
pub async fn restore(
    slug: String,
    current: i64,
    previous: i64,
    summary: String,
) -> Result<HostingService, ServerFnError> {
    let state = write_state_limit(&headers, 32768).await?;
    let session = session_details(state, &headers)
        .await?
        .ok_or_else(|| auth_error(crate::backend::auth::AuthError::Unauthenticated))?;
    state
        .db
        .restore_hosting(&session, &slug, current, previous, summary)
        .await
        .map_err(failure)
}
