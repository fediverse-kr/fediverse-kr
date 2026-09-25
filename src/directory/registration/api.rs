use super::Preview;
#[cfg(feature = "server")]
use crate::backend::{auth::AuthenticatedSession, config, site_registration as registration};
#[cfg(feature = "server")]
use crate::membership::api::server::*;

#[cfg(feature = "server")]
use dioxus::fullstack::HeaderMap;
use dioxus::prelude::*;

#[cfg(feature = "server")]
async fn authorized(
    state: &config::State,
    headers: &HeaderMap,
) -> Result<AuthenticatedSession, ServerFnError> {
    session_details(state, headers)
        .await?
        .ok_or_else(|| auth_error(crate::backend::auth::AuthError::Unauthenticated))
}
#[cfg(feature = "server")]
fn failure(e: registration::Error) -> ServerFnError {
    use registration::Error::*;
    if let Auth(e) = e {
        return auth_error(e);
    }
    error(
        match e {
            InvalidDomain => 400,
            Existing => 409,
            NotFederated => 422,
            Ineligible => 403,
            RateLimited => 429,
            _ => 503,
        },
        &e.to_string(),
    )
}
#[get("/api/member/sites/registration",headers:HeaderMap)]
pub async fn eligibility() -> Result<(), ServerFnError> {
    private_response();
    let state = config::state().await.map_err(|_| unavailable())?;
    state
        .db
        .site_registration_eligible(&authorized(state, &headers).await?)
        .await
        .map_err(failure)
}

/// Explicit owner action used by registration's “다시 확인”. A normal
/// eligibility GET remains read-only; this bounded POST refreshes at most the
/// first four owner-linked rows whose date is still unknown.
#[post("/api/member/sites/registration/recheck", headers: HeaderMap)]
pub async fn recheck_eligibility() -> Result<(), ServerFnError> {
    let state = write_state(&headers).await?;
    let session = authorized(state, &headers).await?;
    crate::backend::auth::consume_attempt(
        &state.db,
        "account_age_retry",
        session.member.id.as_bytes(),
        4,
    )
    .await
    .map_err(auth_error)?;
    crate::backend::auth::consume_attempt(&state.db, "account_age_retry", b"network:global", 40)
        .await
        .map_err(auth_error)?;
    let _permit = network_slot()?;
    let targets = state
        .db
        .creation_date_targets(&session, 4)
        .await
        .map_err(auth_error)?;
    let federation = crate::backend::identity::client_with_signer(state.signer.clone());
    for target in targets {
        let Some(date) = federation
            .creation_date_for(&target.actor_url, &target.handle)
            .await
        else {
            continue;
        };
        match state
            .db
            .persist_creation_date_if_missing(
                &session,
                target.id,
                &target.actor_url,
                &target.handle,
                date,
            )
            .await
        {
            Ok(_) | Err(crate::backend::auth::AuthError::AccountNotFound) => {}
            Err(error) => return Err(auth_error(error)),
        }
    }
    Ok(())
}

#[post("/api/member/sites/registration/preview",headers:HeaderMap)]
pub async fn preview(domain: String) -> Result<Preview, ServerFnError> {
    let state = write_state(&headers).await?;
    let session = authorized(state, &headers).await?;
    let _permit = network_slot()?;
    registration::inspect(
        &state.db,
        &session,
        &domain,
        &crate::backend::crawler::PublicHttp,
    )
    .await
    .map(|c| c.preview())
    .map_err(failure)
}
#[post("/api/member/sites/registration/create",headers:HeaderMap)]
pub async fn create(domain: String) -> Result<String, ServerFnError> {
    let state = write_state(&headers).await?;
    let session = authorized(state, &headers).await?;
    let _permit = network_slot()?;
    // Re-fetch at confirmation. The browser supplies only the confirmed domain,
    // not a cached preview or arbitrary observed/editorial/ownership fields.
    let checked = registration::inspect(
        &state.db,
        &session,
        &domain,
        &crate::backend::crawler::PublicHttp,
    )
    .await
    .map_err(failure)?;
    state
        .db
        .register_checked_site(&session, &checked)
        .await
        .map_err(failure)
}
