use super::History;
use super::*;
use crate::directory::catalog_editing::{EditableSoftware, SoftwareEdit};
#[cfg(feature = "server")]
use crate::{
    backend::{catalog_editing as domain, config},
    directory::catalog_editing::api::failure,
    membership::api::server::*,
    moderation::api::session,
};
#[cfg(feature = "server")]
use dioxus::fullstack::HeaderMap;
use dioxus::prelude::*;
#[get("/api/member/moderation/software?query&locked&page",headers:HeaderMap)]
pub async fn list(query: String, locked: bool, page: u32) -> Result<SoftwarePage, ServerFnError> {
    private_response();
    let state = config::state().await.map_err(|_| unavailable())?;
    let s = session(state, &headers).await?;
    state
        .db
        .moderation_software_list(&s, &query, locked, page)
        .await
        .map_err(failure)
}
#[get("/api/member/moderation/software/item?name",headers:HeaderMap)]
pub async fn item(name: String) -> Result<Software, ServerFnError> {
    private_response();
    let state = config::state().await.map_err(|_| unavailable())?;
    let s = session(state, &headers).await?;
    state
        .db
        .moderation_software(&s, &name)
        .await
        .map_err(failure)
}
#[get("/api/member/moderation/software/editable?name",headers:HeaderMap)]
pub async fn current(name: String) -> Result<EditableSoftware, ServerFnError> {
    private_response();
    let state = config::state().await.map_err(|_| unavailable())?;
    let s = session(state, &headers).await?;
    state
        .db
        .moderation_software_editable(&s, &name)
        .await
        .map_err(failure)
}
#[post("/api/member/moderation/software/save",headers:HeaderMap)]
pub async fn save(
    name: String,
    revision: i64,
    edit: SoftwareEdit,
    summary: String,
) -> Result<EditableSoftware, ServerFnError> {
    let state = write_state_limit(&headers, 65536).await?;
    let s = session(state, &headers).await?;
    let edit = domain::validate(edit, summary).map_err(failure)?;
    state
        .db
        .moderation_software_save(&s, &name, revision, &edit)
        .await
        .map_err(failure)
}
#[post("/api/member/moderation/software/act",headers:HeaderMap)]
pub async fn act(request: Request) -> Result<Software, ServerFnError> {
    let state = write_state(&headers).await?;
    let s = session(state, &headers).await?;
    state
        .db
        .moderate_software(&s, request)
        .await
        .map_err(failure)
}
#[post("/api/member/moderation/software/logo",headers:HeaderMap)]
pub async fn logo(request: LogoRequest) -> Result<Software, ServerFnError> {
    let state = write_state_limit(&headers, LOGO_BODY_LIMIT as u64).await?;
    let s = session(state, &headers).await?;
    crate::backend::catalog_logos::change(&state.db, state.media.as_ref(), &s, request)
        .await
        .map_err(failure)
}
#[get("/api/member/moderation/categories?query&page",headers:HeaderMap)]
pub async fn categories(query: String, page: u32) -> Result<CategoryPage, ServerFnError> {
    private_response();
    let state = config::state().await.map_err(|_| unavailable())?;
    let s = session(state, &headers).await?;
    state
        .db
        .moderation_categories(&s, &query, page)
        .await
        .map_err(failure)
}
#[get("/api/member/moderation/category?name",headers:HeaderMap)]
pub async fn category(name: String) -> Result<Category, ServerFnError> {
    private_response();
    let state = config::state().await.map_err(|_| unavailable())?;
    let s = session(state, &headers).await?;
    state
        .db
        .moderation_category(&s, &name)
        .await
        .map_err(failure)
}
#[post("/api/member/moderation/category/save",headers:HeaderMap)]
pub async fn category_save(request: CategoryRequest) -> Result<Category, ServerFnError> {
    let state = write_state(&headers).await?;
    let s = session(state, &headers).await?;
    state
        .db
        .moderate_category(&s, request)
        .await
        .map_err(failure)
}
#[get("/api/member/moderation/catalog/history?name&category&page",headers:HeaderMap)]
pub async fn history(name: String, category: bool, page: u32) -> Result<History, ServerFnError> {
    private_response();
    let state = config::state().await.map_err(|_| unavailable())?;
    let s = session(state, &headers).await?;
    state
        .db
        .moderation_catalog_history(&s, &name, category, page)
        .await
        .map_err(failure)
}
