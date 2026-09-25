use super::{DnsChallenge, OwnedPage, SiteEdit};
#[cfg(feature = "server")]
use crate::{
    backend::{auth::AuthenticatedSession, config, site_management as owners},
    membership::api::server::*,
};
#[cfg(feature = "server")]
use dioxus::fullstack::HeaderMap;
use dioxus::prelude::*;

#[get("/api/member/sites/refresh?domain", headers: HeaderMap)]
pub async fn refresh_status(domain: String) -> Result<Option<super::RefreshStatus>, ServerFnError> {
    private_response();
    let state = config::state().await.map_err(|_| unavailable())?;
    let session = authorized(state, &headers).await?;
    let domain = owners::domain(&domain).map_err(failure)?;
    state
        .db
        .site_refresh_status(&session, &domain)
        .await
        .map_err(failure)
}

#[cfg(feature = "server")]
fn failure(e: owners::Error) -> ServerFnError {
    use owners::Error::*;
    if let Auth(e) = e {
        return auth_error(e);
    }
    error(
        match e {
            InvalidDomain | InvalidText => 400,
            NotOwned => 404,
            Conflict | InvalidChallenge | DnsPriority => 409,
            DnsNotFound | ApiNotOwner | ApiUnsupported => 422,
            DnsUnavailable | Unavailable | ApiUnavailable => 503,
            RefreshTooSoon => 429,
            Auth(_) => 401,
        },
        &e.to_string(),
    )
}
#[post("/api/member/sites/refresh", headers: HeaderMap)]
pub async fn refresh(domain: String) -> Result<(), ServerFnError> {
    let state = write_state(&headers).await?;
    let session = authorized(state, &headers).await?;
    let domain = owners::domain(&domain).map_err(failure)?;
    state
        .db
        .request_site_refresh(&session, &domain)
        .await
        .map_err(failure)
}
#[post("/api/member/sites/operator/verify", headers: HeaderMap)]
pub async fn verify_operator(account_id: String, domain: String) -> Result<String, ServerFnError> {
    let state = write_state(&headers).await?;
    let session = authorized(state, &headers).await?;
    let account_id =
        uuid::Uuid::parse_str(&account_id).map_err(|_| failure(owners::Error::ApiNotOwner))?;
    let _permit = network_slot()?;
    owners::api_verification::claim(
        &state.db,
        &session,
        account_id,
        &domain,
        &crate::backend::identity::client_with_signer(state.signer.clone()),
        &owners::api_verification::HttpOperatorApi,
    )
    .await
    .map_err(failure)
}
#[cfg(feature = "server")]
async fn authorized(
    state: &config::State,
    headers: &HeaderMap,
) -> Result<AuthenticatedSession, ServerFnError> {
    session_details(state, headers)
        .await?
        .ok_or_else(|| auth_error(crate::backend::auth::AuthError::Unauthenticated))
}

#[get("/api/member/sites?page", headers: HeaderMap)]
pub async fn owned(page: u32) -> Result<OwnedPage, ServerFnError> {
    private_response();
    let state = config::state().await.map_err(|_| unavailable())?;
    state
        .db
        .owned_sites(&authorized(state, &headers).await?, page)
        .await
        .map_err(failure)
}
#[post("/api/member/sites/dns/begin", headers: HeaderMap)]
pub async fn begin_dns(domain: String) -> Result<DnsChallenge, ServerFnError> {
    let state = write_state(&headers).await?;
    owners::begin(&state.db, &authorized(state, &headers).await?, &domain)
        .await
        .map_err(failure)
}
#[post("/api/member/sites/dns/finish", headers: HeaderMap)]
pub async fn finish_dns(id: String, value: String) -> Result<String, ServerFnError> {
    let state = write_state(&headers).await?;
    let session = authorized(state, &headers).await?;
    let _permit = network_slot()?;
    let id = uuid::Uuid::parse_str(&id).map_err(|_| failure(owners::Error::InvalidChallenge))?;
    owners::finish(&state.db, &session, id, &value, &owners::SystemDns)
        .await
        .map_err(failure)
}
#[post("/api/member/sites/edit", headers: HeaderMap)]
pub async fn edit(domain: String, revision: i64, changes: SiteEdit) -> Result<(), ServerFnError> {
    let state = write_state_limit(&headers, 32768).await?;
    let session = authorized(state, &headers).await?;
    let domain = owners::domain(&domain).map_err(failure)?;
    let value = owners::validate_edit(changes).map_err(failure)?;
    state
        .db
        .edit_owned_site(&session, &domain, revision, &value)
        .await
        .map_err(failure)
}
#[post("/api/member/sites/resign", headers: HeaderMap)]
pub async fn resign(
    domain: String,
    revision: i64,
    confirmation: String,
) -> Result<(), ServerFnError> {
    let state = write_state(&headers).await?;
    let session = authorized(state, &headers).await?;
    let domain = owners::domain(&domain).map_err(failure)?;
    if confirmation != domain {
        return Err(error(400, "확인란에 서버 도메인을 입력해 주세요."));
    }
    state
        .db
        .resign_site(&session, &domain, revision)
        .await
        .map_err(failure)
}
