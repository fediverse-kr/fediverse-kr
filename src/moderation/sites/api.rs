use super::{DeleteImpact, DeleteRequest, History, Request, Site, SitePage};
#[cfg(feature = "server")]
use crate::{
    backend::{config, moderation as service},
    membership::api::server::*,
    moderation::api::{failure, session},
};
#[cfg(feature = "server")]
use dioxus::fullstack::HeaderMap;
use dioxus::prelude::*;

#[get("/api/member/moderation/sites?query&status&sort&page",headers:HeaderMap)]
pub async fn sites(
    query: String,
    status: String,
    sort: String,
    page: u32,
) -> Result<SitePage, ServerFnError> {
    private_response();
    let state = config::state().await.map_err(|_| unavailable())?;
    let s = session(state, &headers).await?;
    state
        .db
        .moderation_sites(&s, &query, &status, &sort, page)
        .await
        .map_err(failure)
}
#[get("/api/member/moderation/site?id",headers:HeaderMap)]
pub async fn site(id: String) -> Result<Site, ServerFnError> {
    private_response();
    let state = config::state().await.map_err(|_| unavailable())?;
    let s = session(state, &headers).await?;
    state
        .db
        .moderation_site(&s, service::id(&id).map_err(failure)?)
        .await
        .map_err(failure)
}
#[get("/api/member/moderation/site/history?id&page",headers:HeaderMap)]
pub async fn history(id: String, page: u32) -> Result<History, ServerFnError> {
    private_response();
    let state = config::state().await.map_err(|_| unavailable())?;
    let s = session(state, &headers).await?;
    state
        .db
        .moderation_site_history(&s, service::id(&id).map_err(failure)?, page)
        .await
        .map_err(failure)
}
#[post("/api/member/moderation/site/act",headers:HeaderMap)]
pub async fn act(request: Request) -> Result<Site, ServerFnError> {
    let state = write_state(&headers).await?;
    let s = session(state, &headers).await?;
    state.db.moderate_site(&s, request).await.map_err(failure)
}

#[get("/api/member/moderation/site/delete-impact?id",headers:HeaderMap)]
pub async fn delete_impact(id: String) -> Result<DeleteImpact, ServerFnError> {
    private_response();
    let state = config::state().await.map_err(|_| unavailable())?;
    let s = session(state, &headers).await?;
    state
        .db
        .moderation_site_delete_impact(&s, service::id(&id).map_err(failure)?)
        .await
        .map_err(failure)
}

#[post("/api/member/moderation/site/delete",headers:HeaderMap)]
pub async fn delete_site(request: DeleteRequest) -> Result<(), ServerFnError> {
    let state = write_state(&headers).await?;
    let s = session(state, &headers).await?;
    state
        .db
        .delete_moderated_site(&s, request)
        .await
        .map_err(failure)
}
