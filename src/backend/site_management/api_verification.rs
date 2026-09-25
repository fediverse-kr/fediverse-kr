//! Read-only operator API compatibility, separate from AP member authentication.
//! Contract reference: fedkr-ref (MIT); no Mastodon/Misskey source is copied.
use super::{auth, AuthenticatedSession, Database, Error};
use crate::backend::federation::{
    transport::{pinned_client, safe_url, FederationTransport, MAX_BODY_BYTES},
    webfinger::AccountHandle,
    FederationClient,
};
use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use std::time::Duration;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Family {
    Mastodon,
    Misskey,
}
impl Family {
    // Not the member-editable software catalog: a fixed compatibility allowlist.
    pub(crate) fn parse(value: &str) -> Result<Self, Error> {
        match value.to_ascii_lowercase().as_str() {
            "mastodon" | "hometown" | "glitch-soc" | "fedibird" | "kmyblue" | "pleroma"
            | "akkoma" | "gotosocial" => Ok(Self::Mastodon),
            "misskey" | "firefish" | "iceshrimp" | "sharkey" | "cherrypick" | "foundkey"
            | "calckey" | "catodon" | "meisskey" => Ok(Self::Misskey),
            _ => Err(Error::ApiUnsupported),
        }
    }
}

/// Built only from a freshly authorized DB read, never a deserialized request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Candidate {
    pub member_id: Uuid,
    pub session_id: Uuid,
    pub account_id: Uuid,
    pub actor_url: String,
    pub handle: String,
    pub verified_at: DateTime<Utc>,
    pub site_id: Uuid,
    pub domain: String,
    pub revision: i64,
    pub family: Family,
}
impl Candidate {
    pub(crate) fn account(&self) -> Result<AccountHandle, Error> {
        let handle = AccountHandle::parse(&self.handle).map_err(|_| Error::ApiNotOwner)?;
        let actor = safe_url(&self.actor_url).map_err(|_| Error::ApiNotOwner)?;
        // WebFinger address domain and actor host may legitimately differ. No
        // arbitrary third server can be claimed by matching its local username.
        if self.domain != handle.domain && Some(self.domain.as_str()) != actor.domain() {
            return Err(Error::ApiNotOwner);
        }
        Ok(handle)
    }
}
pub(crate) struct VerifiedApi {
    candidate: Candidate,
}
impl VerifiedApi {
    pub(crate) fn candidate(&self) -> &Candidate {
        &self.candidate
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Endpoint {
    MastodonV2,
    MastodonV1,
    MisskeyUser,
}
impl Endpoint {
    fn path(self) -> &'static str {
        match self {
            Self::MastodonV2 => "/api/v2/instance",
            Self::MastodonV1 => "/api/v1/instance",
            Self::MisskeyUser => "/api/users/show",
        }
    }
}
pub struct ApiResponse {
    pub status: u16,
    pub body: Value,
}
pub trait OperatorApi {
    async fn read(
        &self,
        domain: &str,
        endpoint: Endpoint,
        username: &str,
    ) -> Result<ApiResponse, Error>;
}
pub struct HttpOperatorApi;
impl OperatorApi for HttpOperatorApi {
    async fn read(
        &self,
        domain: &str,
        endpoint: Endpoint,
        username: &str,
    ) -> Result<ApiResponse, Error> {
        tokio::time::timeout(Duration::from_secs(10), async {
            let domain = super::domain(domain)?;
            let url = safe_url(&format!("https://{domain}{}", endpoint.path()))
                .map_err(|_| Error::ApiUnavailable)?;
            let client = pinned_client(&url)
                .await
                .map_err(|_| Error::ApiUnavailable)?;
            // users/show is a read-only lookup using POST, never a login/write.
            let request = if endpoint == Endpoint::MisskeyUser {
                client.post(url).json(&json!({"username": username}))
            } else {
                client.get(url)
            };
            let mut response = request
                .header("accept", "application/json")
                .header("user-agent", "fediverse.kr/2 operator-verification")
                .send()
                .await
                .map_err(|_| Error::ApiUnavailable)?;
            let status = response.status().as_u16();
            if status != 200 {
                return Ok(ApiResponse {
                    status,
                    body: Value::Null,
                });
            }
            if response
                .content_length()
                .is_some_and(|n| n > MAX_BODY_BYTES as u64)
            {
                return Err(Error::ApiUnavailable);
            }
            let mut bytes = Vec::new();
            while let Some(chunk) = response.chunk().await.map_err(|_| Error::ApiUnavailable)? {
                if bytes.len().saturating_add(chunk.len()) > MAX_BODY_BYTES {
                    return Err(Error::ApiUnavailable);
                }
                bytes.extend_from_slice(&chunk);
            }
            Ok(ApiResponse {
                status,
                body: serde_json::from_slice(&bytes).map_err(|_| Error::ApiUnavailable)?,
            })
        })
        .await
        .map_err(|_| Error::ApiUnavailable)?
    }
}

fn mastodon_contact(value: &Value, username: &str, actor: &str) -> Result<(), Error> {
    if !value["username"]
        .as_str()
        .is_some_and(|s| s.eq_ignore_ascii_case(username))
        || !value["acct"]
            .as_str()
            .is_some_and(|s| s.eq_ignore_ascii_case(username))
    {
        return Err(Error::ApiNotOwner);
    }
    // Older API versions omit uri. In that case the fresh AP resolution plus
    // local username binding still applies; an explicit conflicting uri fails.
    if !value["uri"].is_null() && value["uri"].as_str() != Some(actor) {
        return Err(Error::ApiNotOwner);
    }
    Ok(())
}
pub(crate) async fn verify<T: FederationTransport>(
    candidate: Candidate,
    federation: &FederationClient<T>,
    api: &impl OperatorApi,
) -> Result<VerifiedApi, Error> {
    tokio::time::timeout(Duration::from_secs(40), async {
        let handle = candidate.account()?;
        let actor = federation
            .resolve_account_without_creation_date(&candidate.handle)
            .await
            .map_err(|_| Error::ApiUnavailable)?;
        if actor.id != candidate.actor_url {
            return Err(Error::ApiNotOwner);
        }
        match candidate.family {
            Family::Mastodon => {
                let v2 = api
                    .read(&candidate.domain, Endpoint::MastodonV2, &handle.username)
                    .await?;
                match v2.status {
                    200 => mastodon_contact(
                        &v2.body["contact"]["account"],
                        &handle.username,
                        &actor.id,
                    )?,
                    // Only absent/unsupported v2 permits fallback, never a
                    // contradictory contact, denied access, redirect, or outage.
                    404 | 405 | 501 => {
                        let v1 = api
                            .read(&candidate.domain, Endpoint::MastodonV1, &handle.username)
                            .await?;
                        if v1.status != 200 {
                            return Err(Error::ApiUnavailable);
                        }
                        mastodon_contact(&v1.body["contact_account"], &handle.username, &actor.id)?;
                    }
                    _ => return Err(Error::ApiUnavailable),
                }
            }
            Family::Misskey => {
                let response = api
                    .read(&candidate.domain, Endpoint::MisskeyUser, &handle.username)
                    .await?;
                if response.status != 200 {
                    return Err(Error::ApiUnavailable);
                }
                let user = response.body;
                if user["isAdmin"] != true
                    || !user["host"].is_null()
                    || !user["username"]
                        .as_str()
                        .is_some_and(|s| s.eq_ignore_ascii_case(&handle.username))
                    || (!user["uri"].is_null() && user["uri"].as_str() != Some(actor.id.as_str()))
                {
                    return Err(Error::ApiNotOwner);
                }
            }
        }
        Ok(VerifiedApi { candidate })
    })
    .await
    .map_err(|_| Error::ApiUnavailable)?
}
pub async fn claim<T: FederationTransport>(
    db: &Database,
    session: &AuthenticatedSession,
    account_id: Uuid,
    domain: &str,
    federation: &FederationClient<T>,
    api: &impl OperatorApi,
) -> Result<String, Error> {
    let domain = super::domain(domain)?;
    auth::consume_attempt(
        db,
        "challenge_verify",
        format!("site:api:{}", session.member.id).as_bytes(),
        24,
    )
    .await?;
    auth::consume_attempt(db, "challenge_verify", b"site:api:global", 180).await?;
    let candidate = db.api_owner_candidate(session, account_id, &domain).await?;
    let proof = verify(candidate, federation, api).await?;
    db.claim_site_api(session, &proof).await?;
    Ok(domain)
}

#[cfg(test)]
pub(crate) mod tests;
