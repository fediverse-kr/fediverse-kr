use super::{EditableSoftware, History, Kinds, SoftwareEdit};
#[cfg(feature = "server")]
use crate::{
    backend::{catalog_editing as service, config},
    membership::api::server::*,
};
#[cfg(feature = "server")]
use dioxus::fullstack::HeaderMap;
use dioxus::prelude::*;

#[cfg(feature = "server")]
pub(crate) fn failure(e: service::Error) -> ServerFnError {
    use service::Error::*;
    if let Auth(e) = e {
        return auth_error(e);
    }
    error(
        match e {
            Invalid => 400,
            Forbidden => 403,
            Missing => 404,
            Duplicate | Conflict => 409,
            Locked => 423,
            RateLimited => 429,
            _ => 503,
        },
        &e.to_string(),
    )
}
#[cfg(feature = "server")]
async fn authorized(
    state: &config::State,
    headers: &HeaderMap,
) -> Result<crate::backend::auth::AuthenticatedSession, ServerFnError> {
    session_details(state, headers)
        .await?
        .ok_or_else(|| auth_error(crate::backend::auth::AuthError::Unauthenticated))
}
#[get("/api/public/software-edit?name")]
pub async fn current(name: String) -> Result<EditableSoftware, ServerFnError> {
    service::existing_name(&name).map_err(failure)?;
    if std::env::var_os("FEDKR_DATABASE_URL").is_none() {
        let s = super::super::preview::catalog()
            .software
            .into_iter()
            .find(|s| s.name == name)
            .ok_or_else(|| failure(service::Error::Missing))?;
        return Ok(EditableSoftware {
            name: s.name,
            revision: 0,
            locked: false,
            edit: SoftwareEdit {
                display_name: s.display_name,
                family: String::new(),
                description: s.description,
                categories: s.categories,
                features: s.features,
                website_url: s.website_url.unwrap_or_default(),
                tech_stack: s.tech_stack.unwrap_or_default(),
            },
        });
    }
    config::state()
        .await
        .map_err(|_| unavailable())?
        .db
        .editable_software(&name)
        .await
        .map_err(failure)
}
#[get("/api/public/software-kinds")]
pub async fn kinds() -> Result<Kinds, ServerFnError> {
    if std::env::var_os("FEDKR_DATABASE_URL").is_none() {
        return Ok(Kinds {
            items: super::super::preview::catalog().categories,
            truncated: false,
        });
    }
    config::state()
        .await
        .map_err(|_| unavailable())?
        .db
        .catalog_kinds()
        .await
        .map_err(failure)
}
#[get("/api/public/software-history?name&page")]
pub async fn history(name: String, page: u32) -> Result<History, ServerFnError> {
    service::existing_name(&name).map_err(failure)?;
    if page > 10_000 {
        return Err(failure(service::Error::Invalid));
    }
    if std::env::var_os("FEDKR_DATABASE_URL").is_none() {
        current(name.clone()).await?;
        return Ok(History {
            name,
            current_revision: 0,
            locked: false,
            entries: vec![],
            page,
            has_next: false,
        });
    }
    config::state()
        .await
        .map_err(|_| unavailable())?
        .db
        .software_history(&name, page)
        .await
        .map_err(failure)
}
#[get("/api/public/software-revision?name&revision")]
pub async fn read_revision(name: String, revision: i64) -> Result<SoftwareEdit, ServerFnError> {
    service::existing_name(&name).map_err(failure)?;
    if revision < 0 {
        return Err(failure(service::Error::Invalid));
    }
    if std::env::var_os("FEDKR_DATABASE_URL").is_none() {
        return Err(failure(service::Error::Missing));
    }
    config::state()
        .await
        .map_err(|_| unavailable())?
        .db
        .software_revision(&name, revision)
        .await
        .map_err(failure)
}
#[post("/api/member/software/create",headers:HeaderMap)]
pub async fn create(
    name: String,
    edit: SoftwareEdit,
    summary: String,
) -> Result<EditableSoftware, ServerFnError> {
    let state = write_state_limit(&headers, 65536).await?;
    let session = authorized(state, &headers).await?;
    let name = service::new_name(&name).map_err(failure)?;
    let edit = service::validate(edit, summary).map_err(failure)?;
    state
        .db
        .create_software(&session, &name, &edit)
        .await
        .map_err(failure)
}
#[post("/api/member/software/save",headers:HeaderMap)]
pub async fn save(
    name: String,
    revision: i64,
    edit: SoftwareEdit,
    summary: String,
) -> Result<EditableSoftware, ServerFnError> {
    let state = write_state_limit(&headers, 65536).await?;
    let session = authorized(state, &headers).await?;
    let edit = service::validate(edit, summary).map_err(failure)?;
    state
        .db
        .edit_software(&session, &name, revision, &edit)
        .await
        .map_err(failure)
}
#[post("/api/member/software/restore",headers:HeaderMap)]
pub async fn restore(
    name: String,
    current: i64,
    revision: i64,
    summary: String,
) -> Result<EditableSoftware, ServerFnError> {
    let state = write_state(&headers).await?;
    let session = authorized(state, &headers).await?;
    state
        .db
        .restore_software(&session, &name, current, revision, summary)
        .await
        .map_err(failure)
}
