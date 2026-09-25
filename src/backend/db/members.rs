use super::{
    schema::{
        federation_instance_keys as keys, member_auth_rate_limits as rates,
        member_link_challenges as challenges, member_linked_accounts as accounts,
        member_sessions as sessions, member_users as users,
    },
    Database, StoreError,
};
use crate::backend::{
    auth::{
        self, AuthError, AuthenticatedMember, AuthenticatedSession, MemberCredentials, SessionGrant,
    },
    federation::transport::safe_url,
    flow::{self, FlowError, LinkedAccount, ProofChallenge},
    identity::{ResolvedActor, VerifiedIdentity},
};
use chrono::{DateTime, Duration, Utc};
use diesel::{
    dsl::{exists, now},
    prelude::*,
    upsert::excluded,
};
use diesel_async::{AsyncConnection, AsyncPgConnection, RunQueryDsl};
use sha2::{Digest, Sha256};
use uuid::Uuid;

mod legacy;
mod lifecycle;
pub(super) use lifecycle::authorize as authorize_action;

enum SessionAuthentication {
    Password,
    Federated,
    Preserve(AuthenticatedSession),
}

#[derive(Queryable, Selectable)]
#[diesel(table_name = users)]
struct MemberRecord {
    id: Uuid,
    login_id: Option<String>,
    display_name: String,
    password_hash: Option<String>,
    is_banned: bool,
    created_at: DateTime<Utc>,
}
impl From<MemberRecord> for MemberCredentials {
    fn from(r: MemberRecord) -> Self {
        Self {
            id: r.id,
            login_id: r.login_id,
            display_name: r.display_name,
            password_hash: r.password_hash,
            is_banned: r.is_banned,
            created_at: r.created_at,
        }
    }
}
#[derive(Queryable, Selectable)]
#[diesel(table_name = challenges)]
struct ChallengeRecord {
    purpose: String,
    member_id: Option<Uuid>,
    session_id: Option<Uuid>,
    browser_binding_hash: Vec<u8>,
    code_hash: Vec<u8>,
    actor_url: String,
    handle: String,
    display_name: String,
    profile_url: String,
    outbox_url: String,
    actor_published_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
    consumed_at: Option<DateTime<Utc>>,
}
impl From<ChallengeRecord> for ProofChallenge {
    fn from(r: ChallengeRecord) -> Self {
        Self {
            purpose: r.purpose,
            member_id: r.member_id,
            session_id: r.session_id,
            browser_binding_hash: r.browser_binding_hash,
            code_hash: r.code_hash,
            actor_url: r.actor_url,
            handle: r.handle,
            display_name: r.display_name,
            profile_url: r.profile_url,
            outbox_url: r.outbox_url,
            actor_published_at: r.actor_published_at,
            created_at: r.created_at,
            expires_at: r.expires_at,
            consumed_at: r.consumed_at,
        }
    }
}
#[derive(Queryable, Selectable)]
#[diesel(table_name = accounts)]
struct AccountRecord {
    id: Uuid,
    actor_url: String,
    handle: String,
    display_name: String,
    profile_url: String,
    is_public: bool,
    verified_at: DateTime<Utc>,
    account_created_at: Option<DateTime<Utc>>,
}

pub(crate) struct CreationDateTarget {
    pub(crate) id: Uuid,
    pub(crate) actor_url: String,
    pub(crate) handle: String,
}

impl From<AccountRecord> for LinkedAccount {
    fn from(r: AccountRecord) -> Self {
        Self {
            id: r.id,
            actor_url: r.actor_url,
            handle: r.handle,
            display_name: r.display_name,
            profile_url: r.profile_url,
            is_public: r.is_public,
            verified_at: r.verified_at,
            account_created_at: r.account_created_at,
        }
    }
}
impl From<diesel::result::Error> for AuthError {
    fn from(_: diesel::result::Error) -> Self {
        Self::Unavailable
    }
}
impl From<diesel::result::Error> for FlowError {
    fn from(_: diesel::result::Error) -> Self {
        Self::Unavailable
    }
}

async fn locked_member(
    conn: &mut AsyncPgConnection,
    id: Uuid,
) -> Result<MemberCredentials, AuthError> {
    users::table
        .find(id)
        // Serialize changes to this member without blocking foreign-key checks
        // of its immutable UUID. An owner waiting for a site lock must not also
        // block an administrator's deferred owner FK check at commit.
        .for_no_key_update()
        .select(MemberRecord::as_select())
        .first::<MemberRecord>(conn)
        .await
        .optional()?
        .map(Into::into)
        .ok_or(AuthError::Unauthenticated)
}
async fn persist_session(
    conn: &mut AsyncPgConnection,
    member: AuthenticatedMember,
    authentication: SessionAuthentication,
) -> Result<SessionGrant, AuthError> {
    let at = database_time(conn).await?;
    let (authenticated_at, federated_authenticated_at) = match authentication {
        SessionAuthentication::Password => (at, None),
        SessionAuthentication::Federated => (at, Some(at)),
        SessionAuthentication::Preserve(s) => (s.authenticated_at, s.federated_authenticated_at),
    };
    let grant = auth::new_session(member);
    diesel::insert_into(sessions::table)
        .values((
            sessions::id.eq(Uuid::new_v4()),
            sessions::member_id.eq(grant.member.id),
            sessions::token_hash.eq(auth::token_hash(&grant.token).expect("generated token")),
            sessions::expires_at.eq(grant.expires_at),
            sessions::authenticated_at.eq(authenticated_at),
            sessions::federated_authenticated_at.eq(federated_authenticated_at),
        ))
        .execute(conn)
        .await?;
    Ok(grant)
}

async fn database_time(conn: &mut AsyncPgConnection) -> Result<DateTime<Utc>, AuthError> {
    Ok(
        diesel::select(diesel::dsl::sql::<diesel::sql_types::Timestamptz>(
            "CURRENT_TIMESTAMP",
        ))
        .get_result(conn)
        .await?,
    )
}

impl Database {
    pub(crate) async fn credentials_by_login(
        &self,
        login: &str,
    ) -> Result<Option<MemberCredentials>, StoreError> {
        let mut conn = self.pool.get().await.map_err(|_| StoreError)?;
        Ok(users::table
            .filter(users::login_id.eq(login))
            .select(MemberRecord::as_select())
            .first::<MemberRecord>(&mut conn)
            .await
            .optional()?
            .map(Into::into))
    }
    pub(crate) async fn credentials_by_session(
        &self,
        hash: &[u8],
    ) -> Result<Option<MemberCredentials>, StoreError> {
        let mut conn = self.pool.get().await.map_err(|_| StoreError)?;
        Ok(users::table
            .inner_join(sessions::table)
            .filter(sessions::token_hash.eq(hash))
            .filter(sessions::expires_at.gt(now))
            .filter(users::is_banned.eq(false))
            .select(MemberRecord::as_select())
            .first::<MemberRecord>(&mut conn)
            .await
            .optional()?
            .map(Into::into))
    }
    pub(crate) async fn session(
        &self,
        hash: &[u8],
    ) -> Result<Option<AuthenticatedSession>, StoreError> {
        let mut conn = self.pool.get().await.map_err(|_| StoreError)?;
        let row = users::table
            .inner_join(sessions::table)
            .filter(sessions::token_hash.eq(hash))
            .filter(sessions::expires_at.gt(now))
            .filter(users::is_banned.eq(false))
            .select((
                sessions::id,
                sessions::created_at,
                sessions::expires_at,
                sessions::authenticated_at,
                sessions::federated_authenticated_at,
                MemberRecord::as_select(),
            ))
            .first::<(
                Uuid,
                DateTime<Utc>,
                DateTime<Utc>,
                DateTime<Utc>,
                Option<DateTime<Utc>>,
                MemberRecord,
            )>(&mut conn)
            .await
            .optional()?;
        Ok(row.map(
            |(id, created_at, expires_at, authenticated_at, federated_authenticated_at, r)| {
                AuthenticatedSession {
                    id,
                    created_at,
                    expires_at,
                    authenticated_at,
                    federated_authenticated_at,
                    member: MemberCredentials::from(r).member(),
                }
            },
        ))
    }
    pub(crate) async fn revoke(&self, hash: &[u8]) -> Result<(), StoreError> {
        let mut conn = self.pool.get().await.map_err(|_| StoreError)?;
        diesel::delete(sessions::table.filter(sessions::token_hash.eq(hash)))
            .execute(&mut conn)
            .await?;
        Ok(())
    }
    pub(crate) async fn session_active(
        &self,
        session: &AuthenticatedSession,
    ) -> Result<bool, StoreError> {
        let mut conn = self.pool.get().await.map_err(|_| StoreError)?;
        Ok(diesel::select(exists(
            sessions::table
                .inner_join(users::table)
                .filter(sessions::id.eq(session.id))
                .filter(users::id.eq(session.member.id))
                .filter(sessions::expires_at.gt(now))
                .filter(users::is_banned.eq(false)),
        ))
        .get_result(&mut conn)
        .await?)
    }
    pub(crate) async fn set_credentials(
        &self,
        session: &AuthenticatedSession,
        login: &str,
        hash: &str,
    ) -> Result<SessionGrant, AuthError> {
        let mut conn = self.pool.get().await.map_err(|_| AuthError::Unavailable)?;
        (&mut *conn)
            .transaction(async move |conn| {
                let current = locked_member(conn, session.member.id).await?;
                if current.is_banned {
                    return Err(AuthError::Unauthenticated);
                }
                if current.login_id.is_some() || current.password_hash.is_some() {
                    return Err(AuthError::CredentialAlreadySet);
                }
                let active = sessions::table
                    .find(session.id)
                    .filter(sessions::member_id.eq(current.id))
                    .filter(sessions::expires_at.gt(now))
                    .filter(sessions::authenticated_at.gt(Utc::now() - Duration::minutes(15)))
                    .for_update()
                    .select(sessions::id)
                    .first::<Uuid>(conn)
                    .await
                    .optional()?;
                if active.is_none() {
                    return Err(AuthError::Unauthenticated);
                }
                let updated = diesel::update(users::table.find(current.id))
                    .set((
                        users::login_id.eq(Some(login)),
                        users::password_hash.eq(Some(hash)),
                        users::updated_at.eq(now),
                    ))
                    .returning(MemberRecord::as_returning())
                    .get_result::<MemberRecord>(conn)
                    .await;
                let member = match updated {
                    Ok(row) => MemberCredentials::from(row).member(),
                    Err(diesel::result::Error::DatabaseError(
                        diesel::result::DatabaseErrorKind::UniqueViolation,
                        info,
                    )) if info.constraint_name() == Some("member_users_login_id_key") => {
                        return Err(AuthError::LoginIdTaken)
                    }
                    Err(_) => return Err(AuthError::Unavailable),
                };
                diesel::delete(sessions::table.filter(sessions::member_id.eq(member.id)))
                    .execute(conn)
                    .await?;
                persist_session(
                    conn,
                    member,
                    SessionAuthentication::Preserve(session.clone()),
                )
                .await
            })
            .await
    }
    pub(crate) async fn login_session(
        &self,
        verified: &MemberCredentials,
    ) -> Result<SessionGrant, AuthError> {
        let mut conn = self.pool.get().await.map_err(|_| AuthError::Unavailable)?;
        (&mut *conn)
            .transaction(async move |conn| {
                let current = locked_member(conn, verified.id).await.map_err(|e| {
                    if e == AuthError::Unauthenticated {
                        AuthError::InvalidCredentials
                    } else {
                        e
                    }
                })?;
                if current.is_banned || current.password_hash != verified.password_hash {
                    return Err(AuthError::InvalidCredentials);
                }
                persist_session(conn, current.member(), SessionAuthentication::Password).await
            })
            .await
    }
    pub(crate) async fn change_credentials(
        &self,
        verified: &MemberCredentials,
        session_hash: &[u8],
        next: &str,
    ) -> Result<SessionGrant, AuthError> {
        let mut conn = self.pool.get().await.map_err(|_| AuthError::Unavailable)?;
        (&mut *conn)
            .transaction(async move |conn| {
                let current = locked_member(conn, verified.id).await?;
                if current.is_banned || current.password_hash != verified.password_hash {
                    return Err(AuthError::Unauthenticated);
                }
                let active = sessions::table
                    .filter(sessions::token_hash.eq(session_hash))
                    .filter(sessions::member_id.eq(current.id))
                    .filter(sessions::expires_at.gt(now))
                    .for_update()
                    .select(sessions::id)
                    .first::<Uuid>(conn)
                    .await
                    .optional()?;
                if active.is_none() {
                    return Err(AuthError::Unauthenticated);
                }
                diesel::update(users::table.find(current.id))
                    .set((
                        users::password_hash.eq(Some(next)),
                        users::updated_at.eq(now),
                    ))
                    .execute(conn)
                    .await?;
                diesel::delete(sessions::table.filter(sessions::member_id.eq(current.id)))
                    .execute(conn)
                    .await?;
                persist_session(conn, current.member(), SessionAuthentication::Password).await
            })
            .await
    }
    pub(crate) async fn consume_attempt(
        &self,
        scope: &str,
        hash: &[u8],
        limit: i32,
    ) -> Result<(), AuthError> {
        let mut conn = self.pool.get().await.map_err(|_| AuthError::Unavailable)?;
        // Database time and one UPSERT retain atomic rate limits under racing requests.
        // PostgreSQL-specific expressions stay here, never in domain services.
        use diesel::dsl::sql;
        use diesel::sql_types::{Integer, Timestamptz};
        diesel::sql_query("DELETE FROM member_auth_rate_limits WHERE (scope,key_hash) IN (SELECT scope,key_hash FROM member_auth_rate_limits WHERE resets_at<=now() ORDER BY resets_at LIMIT 128)").execute(&mut conn).await?;
        let count = diesel::insert_into(rates::table).values((rates::scope.eq(scope), rates::key_hash.eq(hash), rates::attempts.eq(1),
            rates::resets_at.eq(sql::<Timestamptz>("now() + interval '15 minutes'"))))
            .on_conflict((rates::scope, rates::key_hash)).do_update().set((
                rates::attempts.eq(sql::<Integer>("CASE WHEN member_auth_rate_limits.resets_at <= now() THEN 1 ELSE least(member_auth_rate_limits.attempts + 1, 1000000) END")),
                rates::resets_at.eq(sql::<Timestamptz>("CASE WHEN member_auth_rate_limits.resets_at <= now() THEN now() + interval '15 minutes' ELSE member_auth_rate_limits.resets_at END"))))
            .returning(rates::attempts).get_result::<i32>(&mut conn).await?;
        if count > limit {
            Err(AuthError::RateLimited)
        } else {
            Ok(())
        }
    }
    pub(crate) async fn cleanup_members(&self) -> Result<u64, StoreError> {
        let mut conn = self.pool.get().await.map_err(|_| StoreError)?;
        let r = diesel::sql_query("DELETE FROM member_auth_rate_limits WHERE (scope,key_hash) IN (SELECT scope,key_hash FROM member_auth_rate_limits WHERE resets_at<=now() ORDER BY resets_at LIMIT 1000)").execute(&mut conn).await?;
        let c = diesel::sql_query("DELETE FROM member_link_challenges WHERE id IN (SELECT id FROM member_link_challenges WHERE expires_at<=now() ORDER BY expires_at LIMIT 1000)").execute(&mut conn).await?;
        let s = diesel::sql_query("DELETE FROM member_sessions WHERE id IN (SELECT id FROM member_sessions WHERE expires_at<=now() ORDER BY expires_at LIMIT 1000)").execute(&mut conn).await?;
        Ok((r + c + s) as u64)
    }
    pub(crate) async fn challenge(&self, id: Uuid) -> Result<Option<ProofChallenge>, StoreError> {
        let mut conn = self.pool.get().await.map_err(|_| StoreError)?;
        Ok(challenges::table
            .find(id)
            .select(ChallengeRecord::as_select())
            .first::<ChallengeRecord>(&mut conn)
            .await
            .optional()?
            .map(Into::into))
    }
    pub(crate) async fn create_challenge(
        &self,
        id: Uuid,
        actor: &ResolvedActor,
        browser_hash: &[u8],
        code_hash: &[u8],
        session: Option<&AuthenticatedSession>,
    ) -> Result<DateTime<Utc>, StoreError> {
        let mut conn = self.pool.get().await.map_err(|_| StoreError)?;
        diesel::sql_query("DELETE FROM member_link_challenges WHERE id IN (SELECT id FROM member_link_challenges WHERE expires_at<=now() ORDER BY expires_at LIMIT 128)").execute(&mut conn).await?;
        Ok(diesel::insert_into(challenges::table)
            .values((
                challenges::id.eq(id),
                challenges::purpose.eq(if session.is_some() { "link" } else { "login" }),
                challenges::member_id.eq(session.map(|s| s.member.id)),
                challenges::session_id.eq(session.map(|s| s.id)),
                challenges::browser_binding_hash.eq(browser_hash),
                challenges::code_hash.eq(code_hash),
                challenges::actor_url.eq(&actor.id),
                challenges::handle.eq(&actor.handle),
                challenges::display_name.eq(flow::display_name(actor)),
                challenges::profile_url.eq(actor.profile_url.as_deref().unwrap_or(&actor.id)),
                challenges::outbox_url.eq(&actor.outbox),
                challenges::actor_published_at.eq(actor.published),
                challenges::expires_at.eq(diesel::dsl::sql::<diesel::sql_types::Timestamptz>(
                    "now() + interval '30 minutes'",
                )),
            ))
            .returning(challenges::expires_at)
            .get_result(&mut conn)
            .await?)
    }
    pub(crate) async fn complete_proof(
        &self,
        id: Uuid,
        nonce: &str,
        code: &str,
        session: Option<&AuthenticatedSession>,
        proof: &VerifiedIdentity,
        initial: &ProofChallenge,
    ) -> Result<SessionGrant, FlowError> {
        let mut conn = self.pool.get().await.map_err(|_| FlowError::Unavailable)?;
        (&mut *conn)
            .transaction(async move |conn| {
                // Serialize actor creation, then user -> session -> challenge. This
                // lock order matches credential rotation and its cascading deletes.
                let actor_digest = Sha256::digest(initial.actor_url.as_bytes());
                let key = i64::from_be_bytes(actor_digest[..8].try_into().expect("SHA-256"));
                diesel::sql_query("SELECT pg_advisory_xact_lock($1)")
                    .bind::<diesel::sql_types::BigInt, _>(key)
                    .execute(conn)
                    .await?;
                let legacy_claim =
                    legacy::lookup(conn, &initial.handle, &initial.actor_url).await?;
                let owners = accounts::table
                    .filter(
                        accounts::actor_url
                            .eq(&initial.actor_url)
                            .or(accounts::handle.eq(&initial.handle)),
                    )
                    .select((accounts::member_id, accounts::actor_url))
                    .load::<(Uuid, String)>(conn)
                    .await?;
                if owners.iter().any(|(_, actor)| actor != &initial.actor_url)
                    || owners.len() > 1
                    || session
                        .is_some_and(|s| owners.iter().any(|(member, _)| *member != s.member.id))
                {
                    return Err(FlowError::AlreadyLinked);
                }
                legacy::validate(legacy_claim.as_ref(), &initial.actor_url, &owners, session)?;
                let target = session
                    .map(|s| s.member.id)
                    .or_else(|| owners.as_slice().first().map(|(id, _)| *id))
                    .or_else(|| legacy_claim.as_ref().map(|c| c.member_id));
                let member = if let Some(id) = target {
                    let row = locked_member(conn, id).await?;
                    if row.is_banned {
                        return Err(FlowError::Unauthenticated);
                    }
                    row.member()
                } else {
                    let row = diesel::insert_into(users::table)
                        .values((
                            users::id.eq(Uuid::new_v4()),
                            users::display_name.eq(flow::display_name(proof.actor())),
                        ))
                        .returning(MemberRecord::as_returning())
                        .get_result::<MemberRecord>(conn)
                        .await?;
                    MemberCredentials::from(row).member()
                };
                // Unlink/withdraw serialize on the member row too. A login may
                // have read its owner before waiting for that lock; never
                // reattach an identity using that stale ownership snapshot.
                let current_owners = accounts::table
                    .filter(
                        accounts::actor_url
                            .eq(&initial.actor_url)
                            .or(accounts::handle.eq(&initial.handle)),
                    )
                    .select((accounts::member_id, accounts::actor_url))
                    .load::<(Uuid, String)>(conn)
                    .await?;
                if current_owners != owners {
                    return Err(FlowError::Unauthenticated);
                }
                if legacy::lookup(conn, &initial.handle, &initial.actor_url).await? != legacy_claim
                {
                    // A different actor may have claimed the same legacy handle
                    // while this request waited for its member lock.
                    return Err(FlowError::LegacyIdentity);
                }
                if let Some(session) = session {
                    let active = sessions::table
                        .find(session.id)
                        .filter(sessions::member_id.eq(member.id))
                        .filter(sessions::expires_at.gt(now))
                        .for_update()
                        .select(sessions::id)
                        .first::<Uuid>(conn)
                        .await
                        .optional()?;
                    if active.is_none() {
                        return Err(FlowError::Unauthenticated);
                    }
                    if owners.is_empty() {
                        // Otherwise an old stolen session could attach a new
                        // attacker-owned identity, then "reauthenticate" it.
                        lifecycle::authorize(conn, session, true).await?;
                    }
                }
                let locked: ProofChallenge = challenges::table
                    .find(id)
                    .for_update()
                    .select(ChallengeRecord::as_select())
                    .first::<ChallengeRecord>(conn)
                    .await
                    .optional()?
                    .ok_or(FlowError::InvalidChallenge)?
                    .into();
                flow::validate_binding(&locked, nonce, code, session, Utc::now())?;
                flow::validate_proof(&locked, proof, Utc::now())?;
                let actor = proof.actor();
                let origin = safe_url(&actor.id)
                    .map_err(|_| FlowError::InvalidChallenge)?
                    .origin()
                    .ascii_serialization();
                let attach = diesel::insert_into(accounts::table)
                    .values((
                        accounts::id.eq(Uuid::new_v4()),
                        accounts::member_id.eq(member.id),
                        accounts::actor_url.eq(&actor.id),
                        accounts::handle.eq(&actor.handle),
                        accounts::display_name.eq(flow::display_name(actor)),
                        accounts::profile_url.eq(actor.profile_url.as_deref().unwrap_or(&actor.id)),
                        accounts::provider.eq("activitypub_post"),
                        accounts::provider_origin.eq(origin),
                        accounts::provider_subject_id.eq(&actor.id),
                        accounts::verified_at.eq(proof.verified_at()),
                        accounts::account_created_at.eq(actor.published),
                        accounts::account_created_at_verified_at
                            .eq(actor.published.map(|_| proof.verified_at())),
                    ))
                    .on_conflict(accounts::actor_url)
                    .do_update()
                    .set((
                        accounts::handle.eq(excluded(accounts::handle)),
                        accounts::display_name.eq(excluded(accounts::display_name)),
                        accounts::profile_url.eq(excluded(accounts::profile_url)),
                        accounts::verified_at.eq(excluded(accounts::verified_at)),
                        accounts::account_created_at.eq(diesel::dsl::sql::<
                            diesel::sql_types::Nullable<diesel::sql_types::Timestamptz>,
                        >(
                            "COALESCE(EXCLUDED.account_created_at, member_linked_accounts.account_created_at)",
                        )),
                        accounts::account_created_at_verified_at.eq(diesel::dsl::sql::<
                            diesel::sql_types::Nullable<diesel::sql_types::Timestamptz>,
                        >(
                            "COALESCE(EXCLUDED.account_created_at_verified_at, member_linked_accounts.account_created_at_verified_at)",
                        )),
                        accounts::updated_at.eq(now),
                    ));
                let attach = diesel::query_dsl::methods::FilterDsl::filter(
                    attach,
                    accounts::member_id.eq(excluded(accounts::member_id)),
                )
                .execute(conn)
                .await;
                match attach {
                    Ok(1) => {}
                    Ok(_)
                    | Err(diesel::result::Error::DatabaseError(
                        diesel::result::DatabaseErrorKind::UniqueViolation,
                        _,
                    )) => return Err(FlowError::AlreadyLinked),
                    Err(_) => return Err(FlowError::Unavailable),
                }
                let consumed = diesel::update(
                    challenges::table
                        .find(id)
                        .filter(challenges::consumed_at.is_null())
                        .filter(challenges::expires_at.gt(now)),
                )
                .set((
                    challenges::consumed_at.eq(Some(Utc::now())),
                    challenges::verified_post_url.eq(Some(proof.post_id())),
                ))
                .execute(conn)
                .await?;
                if consumed != 1 {
                    return Err(FlowError::InvalidChallenge);
                }
                legacy::pin(conn, legacy_claim.as_ref(), proof).await?;
                let authentication = if session.is_none() || !owners.is_empty() {
                    SessionAuthentication::Federated
                } else {
                    // Proving ownership of a newly added identity does not
                    // prove ownership of the existing membership again.
                    SessionAuthentication::Preserve(session.expect("link session").clone())
                };
                let grant = persist_session(conn, member, authentication).await?;
                if let Some(session) = session {
                    diesel::delete(sessions::table.find(session.id))
                        .execute(conn)
                        .await?;
                }
                Ok(grant)
            })
            .await
    }
    pub(crate) async fn linked_accounts(
        &self,
        member: Uuid,
    ) -> Result<Vec<LinkedAccount>, StoreError> {
        let mut conn = self.pool.get().await.map_err(|_| StoreError)?;
        Ok(accounts::table
            .filter(accounts::member_id.eq(member))
            .order((accounts::created_at, accounts::id))
            .select(AccountRecord::as_select())
            .load::<AccountRecord>(&mut conn)
            .await?
            .into_iter()
            .map(Into::into)
            .collect())
    }

    /// Return only owner-scoped linked accounts whose creation date is still
    /// unknown. The limit is applied in SQL before any remote work starts.
    pub(crate) async fn creation_date_targets(
        &self,
        session: &AuthenticatedSession,
        limit: i64,
    ) -> Result<Vec<CreationDateTarget>, AuthError> {
        let mut conn = self.pool.get().await.map_err(|_| AuthError::Unavailable)?;
        (&mut *conn)
            .transaction(async move |conn| {
                let current = authorize_action(conn, session, false).await?;
                Ok(accounts::table
                    .filter(accounts::member_id.eq(current.id))
                    .filter(accounts::account_created_at.is_null())
                    .order((accounts::created_at, accounts::id))
                    .limit(limit)
                    .select((accounts::id, accounts::actor_url, accounts::handle))
                    .load::<(Uuid, String, String)>(conn)
                    .await?
                    .into_iter()
                    .map(|(id, actor_url, handle)| CreationDateTarget {
                        id,
                        actor_url,
                        handle,
                    })
                    .collect())
            })
            .await
    }

    /// Persist only a date fetched for the exact owner/account identity that
    /// was read before network I/O. Removed, foreign, or changed rows cannot
    /// be updated; an already-filled row is left untouched.
    pub(crate) async fn persist_creation_date_if_missing(
        &self,
        session: &AuthenticatedSession,
        account_id: Uuid,
        actor_url: &str,
        handle: &str,
        date: DateTime<Utc>,
    ) -> Result<bool, AuthError> {
        if date > Utc::now() {
            return Err(AuthError::Unavailable);
        }
        let mut conn = self.pool.get().await.map_err(|_| AuthError::Unavailable)?;
        (&mut *conn)
            .transaction(async move |conn| {
                let current = authorize_action(conn, session, false).await?;
                let row = accounts::table
                    .find(account_id)
                    .filter(accounts::member_id.eq(current.id))
                    .for_update()
                    .select((
                        accounts::actor_url,
                        accounts::handle,
                        accounts::account_created_at,
                    ))
                    .first::<(String, String, Option<DateTime<Utc>>)>(conn)
                    .await
                    .optional()?;
                let Some((stored_actor, stored_handle, stored_date)) = row else {
                    return Err(AuthError::AccountNotFound);
                };
                if stored_actor != actor_url || stored_handle != handle {
                    return Err(AuthError::AccountNotFound);
                }
                if stored_date.is_some() {
                    return Ok(false);
                }
                diesel::update(
                    accounts::table
                        .find(account_id)
                        .filter(accounts::member_id.eq(current.id))
                        .filter(accounts::actor_url.eq(actor_url))
                        .filter(accounts::handle.eq(handle))
                        .filter(accounts::account_created_at.is_null()),
                )
                .set((
                    accounts::account_created_at.eq(Some(date)),
                    accounts::account_created_at_verified_at.eq(Some(Utc::now())),
                    accounts::updated_at.eq(now),
                ))
                .execute(conn)
                .await?;
                Ok(true)
            })
            .await
    }
    pub async fn signing_key(&self) -> Result<String, StoreError> {
        let mut conn = self.pool.get().await.map_err(|_| StoreError)?;
        Ok(keys::table
            .find(true)
            .select(keys::private_key_pem)
            .first(&mut conn)
            .await?)
    }
    pub async fn ensure_signing_key(&self) -> Result<(), StoreError> {
        let present: bool = {
            let mut conn = self.pool.get().await.map_err(|_| StoreError)?;
            diesel::select(exists(keys::table.select(keys::singleton)))
                .get_result(&mut conn)
                .await?
        };
        if present {
            return Ok(());
        }
        let pem = tokio::task::spawn_blocking(|| {
            use rsa::pkcs8::{EncodePrivateKey, LineEnding};
            rsa::RsaPrivateKey::new(&mut rand::rngs::OsRng, 2048)
                .map_err(|_| StoreError)?
                .to_pkcs8_pem(LineEnding::LF)
                .map(|v| v.to_string())
                .map_err(|_| StoreError)
        })
        .await
        .map_err(|_| StoreError)??;
        let mut conn = self.pool.get().await.map_err(|_| StoreError)?;
        diesel::insert_into(keys::table)
            .values((keys::singleton.eq(true), keys::private_key_pem.eq(pem)))
            .on_conflict_do_nothing()
            .execute(&mut conn)
            .await?;
        Ok(())
    }
}
