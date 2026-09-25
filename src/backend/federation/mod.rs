//! Read-only, bounded ActivityPub account/post client.
//! Behavioral reference: fedkr-ref (MIT, copyright Fediverse.kr contributors 2025–2026).
//! No inbound activity processing, federation delivery, OAuth, or MiAuth implementation.
//! `/actor/inbox` only preserves Phoenix's bounded, no-op 202 compatibility contract.
pub mod account_age;
pub mod activitypub;
pub mod http_signature;
pub mod profile;
mod proof_fallback;
pub mod transport;
pub mod webfinger;

use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use std::{fmt, sync::Arc};
use transport::FederationTransport;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FederationError {
    InvalidHandle,
    UnsafeUrl,
    UnsafeAddress,
    Dns,
    Timeout,
    Network,
    HttpStatus(u16),
    RedirectNotAllowed,
    BodyTooLarge,
    InvalidJson,
    InvalidWebFinger,
    InvalidActor,
    InvalidOutbox,
    InvalidChallenge,
    ChallengeExpired,
    ProofNotFound,
    ScanLimitReached,
    InvalidSigningKey,
}

impl fmt::Display for FederationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Exclude remote HTML, verification codes, and private account data.
        f.write_str(match self {
            Self::InvalidHandle => "@이름@서버 주소를 확인해 주세요.",
            Self::ProofNotFound | Self::ScanLimitReached => {
                "최근 공개 글에서 인증 문구를 찾지 못했습니다."
            }
            Self::ChallengeExpired => "인증 시간이 지났습니다. 새 문구로 다시 시작해 주세요.",
            Self::InvalidSigningKey => "서버 인증 키가 준비되지 않았습니다.",
            Self::HttpStatus(401 | 403) => "해당 서버에서 ActivityPub 조회를 허용하지 않았습니다.",
            _ => "연합우주 계정 정보를 확인하지 못했습니다. 잠시 후 다시 시도해 주세요.",
        })
    }
}
impl std::error::Error for FederationError {}

/// Authoritative profile resolution is NOT ownership proof for this browser.
#[derive(Clone, Debug)]
pub struct ResolvedActor {
    pub id: String,
    pub handle: String,
    pub name: Option<String>,
    pub published: Option<DateTime<Utc>>,
    pub outbox: String,
    pub profile_url: Option<String>,
    pub media: profile::ActorMedia,
}

/// No public constructor/deserializer. The service must still atomically
/// consume the exact challenge before attaching identity or issuing a session.
pub struct VerifiedIdentity {
    actor: ResolvedActor,
    post_id: String,
    verified_at: DateTime<Utc>,
    challenge_code_hash: [u8; 32],
    challenge_issued_at: DateTime<Utc>,
}
impl VerifiedIdentity {
    pub fn actor(&self) -> &ResolvedActor {
        &self.actor
    }
    pub fn post_id(&self) -> &str {
        &self.post_id
    }
    pub fn verified_at(&self) -> DateTime<Utc> {
        self.verified_at
    }
    /// Match both values to the locked DB challenge before consuming it.
    /// An actor match alone must not allow proof reuse for another challenge.
    pub fn challenge_code_hash(&self) -> [u8; 32] {
        self.challenge_code_hash
    }
    pub fn challenge_issued_at(&self) -> DateTime<Utc> {
        self.challenge_issued_at
    }
}

pub struct FederationClient<T> {
    transport: T,
    creation_cache: Arc<account_age::CreationDateCache>,
    proof_discovery: Arc<proof_fallback::DiscoveryState>,
}
impl<T: FederationTransport> FederationClient<T> {
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            creation_cache: Arc::new(account_age::CreationDateCache::new()),
            proof_discovery: Arc::new(proof_fallback::DiscoveryState::new()),
        }
    }
    pub fn new_shared(transport: T) -> Self {
        Self {
            transport,
            creation_cache: account_age::shared_cache(),
            proof_discovery: proof_fallback::shared_state(),
        }
    }
    pub async fn resolve_account(&self, handle: &str) -> Result<ResolvedActor, FederationError> {
        self.resolve_account_mode(handle, true).await
    }
    /// Resolve only the canonical ActivityPub identity. Owner/API claims use
    /// this path so an unrelated date enrichment never becomes claim traffic.
    pub(crate) async fn resolve_account_without_creation_date(
        &self,
        handle: &str,
    ) -> Result<ResolvedActor, FederationError> {
        self.resolve_account_mode(handle, false).await
    }
    async fn resolve_account_mode(
        &self,
        handle: &str,
        enrich_creation_date: bool,
    ) -> Result<ResolvedActor, FederationError> {
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(20);
        let account = webfinger::AccountHandle::parse(handle)?;
        let actor = tokio::time::timeout_at(deadline, async {
            let actor_url = webfinger::lookup(&self.transport, &account).await?;
            activitypub::fetch_actor(&self.transport, &account, &actor_url).await
        })
        .await
        .map_err(|_| FederationError::Timeout)??;
        Ok(if enrich_creation_date {
            self.enrich_actor(actor, &account, deadline).await
        } else {
            actor
        })
    }
    pub(crate) async fn creation_date_for(
        &self,
        actor_url: &str,
        handle: &str,
    ) -> Option<DateTime<Utc>> {
        transport::safe_url(actor_url).ok()?;
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(20);
        let account = webfinger::AccountHandle::parse(handle).ok()?;
        let actor = self
            .resolve_account_without_creation_date(handle)
            .await
            .ok()?;
        if actor.id != actor_url {
            return None;
        }
        self.enrich_actor(actor, &account, deadline).await.published
    }
    async fn enrich_actor(
        &self,
        mut actor: ResolvedActor,
        account: &webfinger::AccountHandle,
        deadline: tokio::time::Instant,
    ) -> ResolvedActor {
        if actor.published.is_none() {
            if let Ok(url) = transport::safe_url(&actor.id) {
                // An optional lookup never cancels an already resolved identity
                // or proof when the overall request's remaining budget expires.
                actor.published = tokio::time::timeout_at(
                    deadline,
                    account_age::lookup(&self.transport, &url, account, &self.creation_cache),
                )
                .await
                .ok()
                .flatten();
            }
        }
        actor
    }
    pub async fn verify_public_post(
        &self,
        actor: &ResolvedActor,
        code: &str,
        issued_at: DateTime<Utc>,
    ) -> Result<VerifiedIdentity, FederationError> {
        self.verify_public_post_at(actor, code, issued_at, Utc::now())
            .await
    }

    /// Explicit clock for deterministic tests; production uses above.
    pub async fn verify_public_post_at(
        &self,
        actor: &ResolvedActor,
        code: &str,
        issued_at: DateTime<Utc>,
        now: DateTime<Utc>,
    ) -> Result<VerifiedIdentity, FederationError> {
        activitypub::validate_challenge(code, issued_at, now)?;
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(30);
        let account = webfinger::AccountHandle::parse(&actor.handle)?;
        let (current, post_id) = tokio::time::timeout_at(deadline, async {
            // Refresh the canonical source; do not trust browser-provided outboxes.
            let actor_url = webfinger::lookup(&self.transport, &account).await?;
            if actor_url.as_str() != actor.id {
                return Err(FederationError::InvalidActor);
            }
            let current = activitypub::fetch_actor(&self.transport, &account, &actor_url).await?;
            let post_id = match activitypub::find_public_proof(
                &self.transport,
                &current,
                code,
                issued_at,
                now,
            )
            .await
            {
                Ok(id) => id,
                // Only a completed AP search without a match may use native
                // discovery. Never bypass AP errors, rate limits or scan budgets.
                Err(FederationError::ProofNotFound) => match proof_fallback::find(
                    &self.transport,
                    &current,
                    code,
                    issued_at,
                    now,
                    &self.proof_discovery,
                )
                .await
                {
                    Err(
                        FederationError::HttpStatus(404 | 405 | 501) | FederationError::InvalidJson,
                    ) => {
                        return Err(FederationError::ProofNotFound);
                    }
                    result => result?,
                },
                Err(error) => return Err(error),
            };
            Ok::<_, FederationError>((current, post_id))
        })
        .await
        .map_err(|_| FederationError::Timeout)??;
        let current = self.enrich_actor(current, &account, deadline).await;
        Ok(VerifiedIdentity {
            actor: current,
            post_id,
            verified_at: now,
            challenge_code_hash: Sha256::digest(code.as_bytes()).into(),
            challenge_issued_at: issued_at,
        })
    }
}

#[cfg(test)]
mod proof_fallback_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use transport::JsonFuture;
    use url::Url;

    #[tokio::test]
    async fn successful_proof_is_bound_to_exact_code_and_issued_at() {
        const CODE: &str = "fedkr-proof-0123456789abcdef0123456789abcdef";
        const ACTOR: &str = "https://social.example.com/users/alice";
        struct Fixture;
        impl FederationTransport for Fixture {
            fn get_json<'a>(&'a self, url: &'a Url, _: bool) -> JsonFuture<'a> {
                Box::pin(async move {
                    Ok(match url.path() {
                        "/.well-known/webfinger" => {
                            json!({"subject":"acct:alice@social.example.com",
                            "links":[{"rel":"self","type":"application/activity+json","href":ACTOR}]})
                        }
                        "/users/alice" => {
                            json!({"id":ACTOR,"type":"Person","preferredUsername":"alice",
                            "outbox":"https://social.example.com/users/alice/outbox",
                            "icon":{"type":"Image","url":"https://cdn.example.com/alice.png"},
                            "tag":[{"type":"Emoji","name":":wave:","icon":{"url":"https://cdn.example.com/wave.gif"}}]})
                        }
                        "/users/alice/outbox" => {
                            json!({"type":"OrderedCollection","orderedItems":[{
                            "id":"https://social.example.com/notes/proof","type":"Note","attributedTo":ACTOR,
                            "to":[activitypub::PUBLIC],"published":"2026-09-12T12:00:30Z","content":CODE}]})
                        }
                        _ => return Err(FederationError::HttpStatus(404)),
                    })
                })
            }
        }
        let client = FederationClient::new(Fixture);
        let actor = client
            .resolve_account("alice@social.example.com")
            .await
            .unwrap();
        let issued_at = DateTime::parse_from_rfc3339("2026-09-12T12:00:00.123456Z")
            .unwrap()
            .with_timezone(&Utc);
        let now = issued_at + chrono::Duration::minutes(1);
        let proof = client
            .verify_public_post_at(&actor, CODE, issued_at, now)
            .await
            .unwrap();
        let expected: [u8; 32] = Sha256::digest(CODE.as_bytes()).into();
        let other: [u8; 32] =
            Sha256::digest(b"different-proof-0123456789abcdef0123456789abcdef").into();
        assert_eq!(proof.challenge_code_hash(), expected);
        assert_ne!(proof.challenge_code_hash(), other);
        assert_eq!(proof.challenge_issued_at(), issued_at);
        assert_eq!(
            proof.actor().media.avatar.as_deref(),
            Some("https://cdn.example.com/alice.png")
        );
        assert_eq!(proof.actor().media.emojis[0].0, "wave");
        assert_ne!(
            proof.challenge_issued_at(),
            issued_at + chrono::Duration::microseconds(1)
        );
    }
}
