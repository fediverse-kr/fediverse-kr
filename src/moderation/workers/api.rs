use super::WorkerOverview;
#[cfg(feature = "server")]
use crate::{backend::config, membership::api::server::*};
#[cfg(feature = "server")]
use dioxus::fullstack::HeaderMap;
use dioxus::prelude::*;

#[get("/api/member/moderation/workers", headers: HeaderMap)]
pub async fn overview() -> Result<WorkerOverview, ServerFnError> {
    private_response();
    let state = config::state().await.map_err(|_| unavailable())?;
    let session = crate::moderation::api::session(state, &headers).await?;
    state
        .db
        .worker_overview(&session)
        .await
        .map_err(crate::moderation::api::failure)
}
