//! Browser/session-bound public-post authentication. Remote verification is
//! read-only; only this transaction consumes its exact challenge and creates a
//! membership, attaches an identity, or issues a local session.

use super::db::{Database, StoreError};
use super::{
    auth::{self, AuthError, AuthenticatedSession, SessionGrant},
    federation::{
        activitypub::MAX_CHALLENGE_AGE_MINUTES, transport::safe_url, webfinger::AccountHandle,
    },
    identity::{ResolvedActor, VerifiedIdentity},
};
use chrono::{DateTime, Duration, Utc};
use rand::{rngs::OsRng, RngCore};
use sha2::{Digest, Sha256};
use std::fmt;
use uuid::Uuid;

const CODE_PREFIX: &str = "fedkr-proof-";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FlowError {
    InvalidChallenge,
    Expired,
    Unauthenticated,
    AlreadyLinked,
    LegacyIdentity,
    RateLimited,
    Unavailable,
    Auth(AuthError),
}

impl fmt::Display for FlowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidChallenge => {
                f.write_str("인증 요청을 확인할 수 없습니다. 다시 시작해 주세요.")
            }
            Self::Expired => f.write_str("인증 시간이 지났습니다. 새 문구로 다시 시작해 주세요."),
            Self::Unauthenticated => f.write_str("다시 로그인해 주세요."),
            Self::AlreadyLinked => f.write_str("이미 다른 회원에게 연결된 계정입니다."),
            Self::LegacyIdentity => f.write_str(
                "기존 회원의 연결 상태가 달라 이 계정으로 로그인할 수 없습니다. 연결을 해제했다면 남아 있는 로그인 수단으로 로그인해 주세요.",
            ),
            Self::RateLimited => f.write_str("시도가 많습니다. 잠시 후 다시 시도해 주세요."),
            Self::Unavailable => {
                f.write_str("지금은 인증을 처리할 수 없습니다. 잠시 후 다시 시도해 주세요.")
            }
            Self::Auth(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for FlowError {}
impl From<StoreError> for FlowError {
    fn from(_: StoreError) -> Self {
        Self::Unavailable
    }
}
impl From<AuthError> for FlowError {
    fn from(error: AuthError) -> Self {
        match error {
            AuthError::RateLimited => Self::RateLimited,
            AuthError::Unauthenticated => Self::Unauthenticated,
            AuthError::Unavailable => Self::Unavailable,
            _ => Self::Auth(error),
        }
    }
}

pub struct IssuedChallenge {
    pub id: Uuid,
    pub code: String,
    pub handle: String,
    pub expires_at: DateTime<Utc>,
}
impl fmt::Debug for IssuedChallenge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IssuedChallenge")
            .field("id", &self.id)
            .field("code", &"[REDACTED]")
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

#[derive(Debug)]
pub struct PendingChallenge {
    pub actor: ResolvedActor,
    pub created_at: DateTime<Utc>,
}

/// Owner-only rows. Callers must authorize member_id from the cookie, never a
/// user-supplied member ID. A future public endpoint must filter is_public.
#[derive(Debug)]
pub struct LinkedAccount {
    pub id: Uuid,
    pub actor_url: String,
    pub handle: String,
    pub display_name: String,
    pub profile_url: String,
    pub is_public: bool,
    pub verified_at: DateTime<Utc>,
    pub account_created_at: Option<DateTime<Utc>>,
}

pub(crate) struct ProofChallenge {
    pub(crate) purpose: String,
    pub(crate) member_id: Option<Uuid>,
    pub(crate) session_id: Option<Uuid>,
    pub(crate) browser_binding_hash: Vec<u8>,
    pub(crate) code_hash: Vec<u8>,
    pub(crate) actor_url: String,
    pub(crate) handle: String,
    pub(crate) display_name: String,
    pub(crate) profile_url: String,
    pub(crate) outbox_url: String,
    pub(crate) actor_published_at: Option<DateTime<Utc>>,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) expires_at: DateTime<Utc>,
    pub(crate) consumed_at: Option<DateTime<Utc>>,
}
impl ProofChallenge {
    fn pending(&self) -> PendingChallenge {
        PendingChallenge {
            actor: ResolvedActor {
                id: self.actor_url.clone(),
                handle: self.handle.clone(),
                name: Some(self.display_name.clone()),
                published: self.actor_published_at,
                outbox: self.outbox_url.clone(),
                profile_url: Some(self.profile_url.clone()),
                media: Default::default(),
            },
            created_at: self.created_at,
        }
    }
}

fn hex_secret() -> Result<String, FlowError> {
    let mut bytes = [0u8; 32];
    OsRng
        .try_fill_bytes(&mut bytes)
        .map_err(|_| FlowError::Unavailable)?;
    let mut value = String::with_capacity(64);
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in bytes {
        value.push(HEX[usize::from(byte >> 4)] as char);
        value.push(HEX[usize::from(byte & 15)] as char);
    }
    Ok(value)
}
fn is_hex_secret(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
fn digest(value: &str) -> Vec<u8> {
    Sha256::digest(value.as_bytes()).to_vec()
}

// Inputs are fixed-length digests; avoid early-exit byte comparisons for browser
// binding and proof secrets. Database equality alone must not be treated as auth.
fn same_hash(left: &[u8], right: &[u8]) -> bool {
    use subtle::ConstantTimeEq;
    left.len() == 32 && right.len() == 32 && bool::from(left.ct_eq(right))
}

fn validate_code(code: &str) -> Result<(), FlowError> {
    if code.strip_prefix(CODE_PREFIX).is_some_and(is_hex_secret) {
        Ok(())
    } else {
        Err(FlowError::InvalidChallenge)
    }
}

pub(crate) fn display_name(actor: &ResolvedActor) -> String {
    let fallback = actor
        .handle
        .trim_start_matches('@')
        .split('@')
        .next()
        .unwrap_or("회원");
    let candidate = actor
        .name
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(fallback);
    let value: String = candidate
        .trim()
        .chars()
        .filter(|c| !c.is_control())
        .take(64)
        .collect();
    if value.is_empty() {
        fallback.chars().take(64).collect()
    } else {
        value
    }
}

fn validate_actor(actor: &ResolvedActor) -> Result<(), FlowError> {
    let id = safe_url(&actor.id).map_err(|_| FlowError::InvalidChallenge)?;
    let outbox = safe_url(&actor.outbox).map_err(|_| FlowError::InvalidChallenge)?;
    let handle = AccountHandle::parse(&actor.handle).map_err(|_| FlowError::InvalidChallenge)?;
    if id.as_str() != actor.id
        || outbox.as_str() != actor.outbox
        || id.origin() != outbox.origin()
        || handle.display() != actor.handle
        || actor.id.len() > 2048
        || actor.outbox.len() > 2048
    {
        return Err(FlowError::InvalidChallenge);
    }
    if let Some(profile) = &actor.profile_url {
        safe_url(profile).map_err(|_| FlowError::InvalidChallenge)?;
        if profile.len() > 2048 {
            return Err(FlowError::InvalidChallenge);
        }
    }
    Ok(())
}

pub(crate) fn validate_binding(
    row: &ProofChallenge,
    nonce: &str,
    code: &str,
    session: Option<&AuthenticatedSession>,
    now: DateTime<Utc>,
) -> Result<(), FlowError> {
    if !is_hex_secret(nonce)
        || validate_code(code).is_err()
        || row.consumed_at.is_some()
        || !same_hash(&row.browser_binding_hash, &digest(nonce))
        || !same_hash(&row.code_hash, &digest(code))
    {
        return Err(FlowError::InvalidChallenge);
    }
    if row.created_at > now
        || row.expires_at <= now
        || now - row.created_at >= Duration::minutes(MAX_CHALLENGE_AGE_MINUTES)
    {
        return Err(FlowError::Expired);
    }
    match (row.purpose.as_str(), session) {
        ("login", None) if row.member_id.is_none() && row.session_id.is_none() => Ok(()),
        ("link", Some(session))
            if row.member_id == Some(session.member.id)
                && row.session_id == Some(session.id)
                && session.expires_at > now =>
        {
            Ok(())
        }
        _ => Err(FlowError::InvalidChallenge),
    }
}

pub async fn begin_challenge(
    pool: &Database,
    actor: &ResolvedActor,
    browser_nonce: &str,
    session: Option<&AuthenticatedSession>,
) -> Result<IssuedChallenge, FlowError> {
    if !is_hex_secret(browser_nonce) {
        return Err(FlowError::InvalidChallenge);
    }
    validate_actor(actor)?;
    // The HTTP boundary reserves challenge_begin before remote actor resolution.
    // Do not charge the same begin twice here, after the network request.
    if let Some(session) = session {
        if !pool.session_active(session).await? {
            return Err(FlowError::Unauthenticated);
        }
        if Utc::now() - session.authenticated_at >= Duration::minutes(15)
            && !pool
                .linked_accounts(session.member.id)
                .await?
                .iter()
                .any(|a| a.actor_url == actor.id)
        {
            // Fail before asking for a public post. The commit transaction
            // still repeats freshness/ownership checks after remote I/O.
            return Err(AuthError::FreshAuthenticationRequired.into());
        }
    }
    let id = Uuid::new_v4();
    let code = format!("{CODE_PREFIX}{}", hex_secret()?);
    let expires_at = pool
        .create_challenge(id, actor, &digest(browser_nonce), &digest(&code), session)
        .await?;
    Ok(IssuedChallenge {
        id,
        code,
        handle: actor.handle.clone(),
        expires_at,
    })
}

pub async fn get_pending(
    pool: &Database,
    id: Uuid,
    nonce: &str,
    code: &str,
    session: Option<&AuthenticatedSession>,
) -> Result<PendingChallenge, FlowError> {
    if !is_hex_secret(nonce) {
        return Err(FlowError::InvalidChallenge);
    }
    validate_code(code)?;
    auth::consume_attempt(pool, "challenge_verify", nonce.as_bytes(), 30).await?;
    let row = pool
        .challenge(id)
        .await?
        .ok_or(FlowError::InvalidChallenge)?;
    validate_binding(&row, nonce, code, session, Utc::now())?;
    if let Some(session) = session {
        if !pool.session_active(session).await? {
            return Err(FlowError::Unauthenticated);
        }
    }
    Ok(row.pending())
}

pub(crate) fn validate_proof(
    row: &ProofChallenge,
    proof: &VerifiedIdentity,
    now: DateTime<Utc>,
) -> Result<(), FlowError> {
    if proof.actor().id != row.actor_url
        || proof.actor().handle != row.handle
        || !same_hash(&proof.challenge_code_hash(), &row.code_hash)
        || proof.challenge_issued_at() != row.created_at
        || proof.verified_at() < row.created_at
        || proof.verified_at() >= row.expires_at
        || proof.verified_at() > now
    {
        return Err(FlowError::InvalidChallenge);
    }
    validate_actor(proof.actor())
}

pub async fn complete_challenge(
    pool: &Database,
    id: Uuid,
    nonce: &str,
    code: &str,
    session_token: Option<&str>,
    proof: &VerifiedIdentity,
) -> Result<SessionGrant, FlowError> {
    let session = match session_token {
        Some(token) => Some(
            auth::get_session_details(pool, token)
                .await?
                .ok_or(FlowError::Unauthenticated)?,
        ),
        None => None,
    };
    let initial = pool
        .challenge(id)
        .await?
        .ok_or(FlowError::InvalidChallenge)?;
    validate_binding(&initial, nonce, code, session.as_ref(), Utc::now())?;
    validate_proof(&initial, proof, Utc::now())?;
    pool.complete_proof(id, nonce, code, session.as_ref(), proof, &initial)
        .await
}

pub async fn list_linked_accounts(
    db: &Database,
    member_id: Uuid,
) -> Result<Vec<LinkedAccount>, FlowError> {
    Ok(db.linked_accounts(member_id).await?)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::backend::db::fixtures;
    use crate::backend::federation::{
        transport::{FederationTransport, JsonFuture},
        FederationClient, FederationError,
    };
    use serde_json::{json, Value};
    use std::collections::HashMap;
    use url::Url;

    struct FakeTransport {
        docs: HashMap<String, Value>,
    }
    impl FederationTransport for FakeTransport {
        fn get_json<'a>(&'a self, url: &'a Url, _: bool) -> JsonFuture<'a> {
            Box::pin(async move {
                self.docs
                    .get(url.as_str())
                    .cloned()
                    .ok_or(FederationError::HttpStatus(404))
            })
        }
    }
    pub(crate) fn actor(name: &str) -> ResolvedActor {
        ResolvedActor {
            id: format!("https://social.example.com/users/{name}"),
            handle: format!("@{name}@social.example.com"),
            name: Some(name.to_owned()),
            published: None,
            outbox: format!("https://social.example.com/users/{name}/outbox"),
            profile_url: None,
            media: Default::default(),
        }
    }
    pub(crate) async fn proof(
        actor: &ResolvedActor,
        code: &str,
        issued_at: DateTime<Utc>,
    ) -> VerifiedIdentity {
        let account = AccountHandle::parse(&actor.handle).unwrap();
        let now = Utc::now();
        let transport = FakeTransport {
            docs: HashMap::from([
                (
                    account.lookup_url().to_string(),
                    json!({"subject":account.resource(),"links":[{"rel":"self","type":"application/activity+json","href":actor.id}]}),
                ),
                (
                    actor.id.clone(),
                    json!({"id":actor.id,"type":"Person","preferredUsername":account.username,"name":actor.name,"outbox":actor.outbox,"published":actor.published}),
                ),
                (
                    actor.outbox.clone(),
                    json!({"type":"OrderedCollection","orderedItems":[{"type":"Note","id":format!("https://social.example.com/notes/{}",Uuid::new_v4()),"attributedTo":actor.id,"to":["https://www.w3.org/ns/activitystreams#Public"],"published":now.to_rfc3339(),"content":format!("<p>{code}</p>")}]}),
                ),
            ]),
        };
        FederationClient::new(transport)
            .verify_public_post_at(actor, code, issued_at, now)
            .await
            .unwrap()
    }

    #[test]
    fn challenge_code_is_256_random_bits_and_binding_is_exact() {
        let nonce = hex_secret().unwrap();
        let other = hex_secret().unwrap();
        assert_ne!(nonce, other);
        assert!(is_hex_secret(&nonce));
        assert!(validate_code(&format!("{CODE_PREFIX}{nonce}")).is_ok());
        assert!(validate_code(&nonce).is_err());
        assert!(!same_hash(&digest(&nonce), &digest(&other)));
        assert!(!same_hash(&[], &[]));
        assert!(same_hash(&digest(&nonce), &digest(&nonce)));
        let expected = digest(&nonce);
        for index in 0..32 {
            let mut changed = expected.clone();
            changed[index] ^= 1;
            assert!(!same_hash(&expected, &changed));
        }
        assert!(!same_hash(&expected[..31], &expected[..31]));
    }

    #[tokio::test]
    async fn verified_public_post_is_bound_to_exact_code_actor_and_issue_time() {
        let actor = actor("flow_unit");
        let nonce = hex_secret().unwrap();
        let code = format!("{CODE_PREFIX}{}", hex_secret().unwrap());
        let now = Utc::now();
        let issued = now - Duration::seconds(2);
        let mut row = ProofChallenge {
            purpose: "login".into(),
            member_id: None,
            session_id: None,
            browser_binding_hash: digest(&nonce),
            code_hash: digest(&code),
            actor_url: actor.id.clone(),
            handle: actor.handle.clone(),
            display_name: "flow_unit".into(),
            profile_url: actor.id.clone(),
            outbox_url: actor.outbox.clone(),
            actor_published_at: None,
            created_at: issued,
            expires_at: issued + Duration::minutes(30),
            consumed_at: None,
        };
        let verified = proof(&actor, &code, issued).await;
        assert!(validate_binding(&row, &nonce, &code, None, Utc::now()).is_ok());
        assert!(validate_proof(&row, &verified, Utc::now()).is_ok());
        row.code_hash = digest(&format!("{CODE_PREFIX}{}", hex_secret().unwrap()));
        assert_eq!(
            validate_proof(&row, &verified, Utc::now()),
            Err(FlowError::InvalidChallenge)
        );
        row.code_hash = digest(&code);
        row.created_at += Duration::microseconds(1);
        assert_eq!(
            validate_proof(&row, &verified, Utc::now()),
            Err(FlowError::InvalidChallenge)
        );
        row.created_at = issued;
        row.actor_url = self::actor("someone_else").id;
        assert_eq!(
            validate_proof(&row, &verified, Utc::now()),
            Err(FlowError::InvalidChallenge)
        );
    }

    #[tokio::test]
    #[ignore = "requires migrated disposable FEDKR_TEST_DATABASE_URL"]
    async fn postgres_public_proof_login_link_and_atomic_consume() {
        let pool = fixtures::database().await;
        let unique = format!("flow_{}", Uuid::new_v4().simple());
        let alice = actor(&format!("{unique}_alice"));
        let bob = actor(&format!("{unique}_bob"));
        let carol = actor(&format!("{unique}_carol"));
        let nonce = hex_secret().unwrap();
        let other_nonce = hex_secret().unwrap();
        let issued = begin_challenge(&pool, &alice, &nonce, None).await.unwrap();
        assert!(matches!(
            get_pending(&pool, issued.id, &other_nonce, &issued.code, None).await,
            Err(FlowError::InvalidChallenge)
        ));
        let wrong_code = format!("{CODE_PREFIX}{}", hex_secret().unwrap());
        assert!(matches!(
            get_pending(&pool, issued.id, &nonce, &wrong_code, None).await,
            Err(FlowError::InvalidChallenge)
        ));
        let pending = get_pending(&pool, issued.id, &nonce, &issued.code, None)
            .await
            .unwrap();
        let verified = proof(&pending.actor, &issued.code, pending.created_at).await;
        let unrelated = begin_challenge(&pool, &alice, &other_nonce, None)
            .await
            .unwrap();
        assert!(matches!(
            complete_challenge(
                &pool,
                unrelated.id,
                &other_nonce,
                &unrelated.code,
                None,
                &verified
            )
            .await,
            Err(FlowError::InvalidChallenge)
        ));
        let first = complete_challenge(&pool, issued.id, &nonce, &issued.code, None, &verified)
            .await
            .unwrap();
        let member_id = first.member.id;
        assert!(first.member.login_id.is_none());
        assert!(matches!(
            complete_challenge(&pool, issued.id, &nonce, &issued.code, None, &verified).await,
            Err(FlowError::InvalidChallenge)
        ));
        let rows = list_linked_accounts(&pool, member_id).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert!(!rows[0].is_public);
        assert!(rows[0].account_created_at.is_none());
        let session = auth::get_session_details(&pool, &first.token)
            .await
            .unwrap()
            .unwrap();
        let link = begin_challenge(&pool, &bob, &nonce, Some(&session))
            .await
            .unwrap();
        assert!(matches!(
            get_pending(&pool, link.id, &nonce, &link.code, None).await,
            Err(FlowError::InvalidChallenge)
        ));
        let link_pending = get_pending(&pool, link.id, &nonce, &link.code, Some(&session))
            .await
            .unwrap();
        let link_proof = proof(&link_pending.actor, &link.code, link_pending.created_at).await;
        let linked = complete_challenge(
            &pool,
            link.id,
            &nonce,
            &link.code,
            Some(&first.token),
            &link_proof,
        )
        .await
        .unwrap();
        assert_eq!(linked.member.id, member_id);
        assert_eq!(
            list_linked_accounts(&pool, member_id).await.unwrap().len(),
            2
        );
        assert!(auth::get_session(&pool, &first.token)
            .await
            .unwrap()
            .is_none());
        assert!(auth::get_session(&pool, &linked.token)
            .await
            .unwrap()
            .is_some());
        // A new anonymous proof of either identity returns the same membership.
        let returning = begin_challenge(&pool, &bob, &other_nonce, None)
            .await
            .unwrap();
        let pending = get_pending(&pool, returning.id, &other_nonce, &returning.code, None)
            .await
            .unwrap();
        let verified = proof(&pending.actor, &returning.code, pending.created_at).await;
        let returned = complete_challenge(
            &pool,
            returning.id,
            &other_nonce,
            &returning.code,
            None,
            &verified,
        )
        .await
        .unwrap();
        assert_eq!(returned.member.id, member_id);
        // Two browsers completing one code cannot issue two sessions.
        let race_nonce = hex_secret().unwrap();
        let race = begin_challenge(&pool, &carol, &race_nonce, None)
            .await
            .unwrap();
        let pending = get_pending(&pool, race.id, &race_nonce, &race.code, None)
            .await
            .unwrap();
        let verified = proof(&pending.actor, &race.code, pending.created_at).await;
        let (a, b) = tokio::join!(
            complete_challenge(&pool, race.id, &race_nonce, &race.code, None, &verified),
            complete_challenge(&pool, race.id, &race_nonce, &race.code, None, &verified)
        );
        assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
        let carol_member = a.or(b).unwrap();
        // A proven identity already owned elsewhere is never silently merged.
        let session = auth::get_session_details(&pool, &carol_member.token)
            .await
            .unwrap()
            .unwrap();
        let collision = begin_challenge(&pool, &alice, &race_nonce, Some(&session))
            .await
            .unwrap();
        let pending = get_pending(
            &pool,
            collision.id,
            &race_nonce,
            &collision.code,
            Some(&session),
        )
        .await
        .unwrap();
        let verified = proof(&pending.actor, &collision.code, pending.created_at).await;
        assert!(matches!(
            complete_challenge(
                &pool,
                collision.id,
                &race_nonce,
                &collision.code,
                Some(&carol_member.token),
                &verified
            )
            .await,
            Err(FlowError::AlreadyLinked)
        ));
        let wrong_session = auth::get_session_details(&pool, &linked.token)
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(
            get_pending(
                &pool,
                collision.id,
                &race_nonce,
                &collision.code,
                Some(&wrong_session)
            )
            .await,
            Err(FlowError::InvalidChallenge)
        ));
        // Expiry is checked again, not just when fetching the remote proof.
        fixtures::expire_challenge(&pool, collision.id).await;
        assert!(matches!(
            get_pending(
                &pool,
                collision.id,
                &race_nonce,
                &collision.code,
                Some(&session)
            )
            .await,
            Err(FlowError::Expired)
        ));
        assert!(matches!(
            complete_challenge(
                &pool,
                collision.id,
                &race_nonce,
                &collision.code,
                Some(&carol_member.token),
                &verified
            )
            .await,
            Err(FlowError::Expired)
        ));
        assert_eq!(
            list_linked_accounts(&pool, member_id).await.unwrap().len(),
            2
        );
        assert_eq!(
            list_linked_accounts(&pool, carol_member.member.id)
                .await
                .unwrap()
                .len(),
            1
        );
        // Exact fixture rows only; no TRUNCATE or broad production cleanup.
        fixtures::delete_challenges(&pool, &[&alice.handle, &bob.handle, &carol.handle]).await;
        fixtures::delete_members(&pool, &[member_id, carol_member.member.id]).await;
        for nonce in [&nonce, &other_nonce, &race_nonce] {
            for scope in ["challenge_begin", "challenge_verify"] {
                fixtures::delete_rate(&pool, scope, &digest(nonce)).await;
            }
        }
    }
}
