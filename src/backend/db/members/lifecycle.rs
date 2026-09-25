//! Every membership write takes member -> session locks. Foreign account IDs
//! are treated exactly like nonexistent IDs; no public disclosure endpoint.
use super::*;

#[cfg(test)]
mod tests;

pub(in crate::backend::db) async fn authorize(
    conn: &mut AsyncPgConnection,
    session: &AuthenticatedSession,
    fresh: bool,
) -> Result<MemberCredentials, AuthError> {
    let member = locked_member(conn, session.member.id).await?;
    if member.is_banned {
        return Err(AuthError::Unauthenticated);
    }
    let at = sessions::table
        .find(session.id)
        .filter(sessions::member_id.eq(member.id))
        .filter(sessions::expires_at.gt(now))
        .for_update()
        .select(sessions::authenticated_at)
        .first::<DateTime<Utc>>(conn)
        .await
        .optional()?
        .ok_or(AuthError::Unauthenticated)?;
    let current = database_time(conn).await?;
    if fresh && (at > current || current - at >= Duration::minutes(15)) {
        return Err(AuthError::FreshAuthenticationRequired);
    }
    Ok(member)
}

async fn cancel_identity_challenges(
    conn: &mut AsyncPgConnection,
    member: Uuid,
) -> Result<(), AuthError> {
    // Include outstanding anonymous login proofs for the identities being
    // detached, not only session-bound link challenges (which cascade).
    diesel::delete(
        challenges::table.filter(
            challenges::actor_url.eq_any(
                accounts::table
                    .filter(accounts::member_id.eq(member))
                    .select(accounts::actor_url),
            ),
        ),
    )
    .execute(conn)
    .await?;
    Ok(())
}

impl Database {
    pub(crate) async fn update_member_name(
        &self,
        session: &AuthenticatedSession,
        name: &str,
    ) -> Result<AuthenticatedMember, AuthError> {
        let mut conn = self.pool.get().await.map_err(|_| AuthError::Unavailable)?;
        (&mut *conn)
            .transaction(async move |conn| {
                let current = authorize(conn, session, false).await?;
                let row = diesel::update(users::table.find(current.id))
                    .set((users::display_name.eq(name), users::updated_at.eq(now)))
                    .returning(MemberRecord::as_returning())
                    .get_result::<MemberRecord>(conn)
                    .await?;
                Ok(MemberCredentials::from(row).member())
            })
            .await
    }

    pub(crate) async fn set_link_visibility(
        &self,
        session: &AuthenticatedSession,
        account: Uuid,
        public: bool,
    ) -> Result<(), AuthError> {
        let mut conn = self.pool.get().await.map_err(|_| AuthError::Unavailable)?;
        (&mut *conn)
            .transaction(async move |conn| {
                let current = authorize(conn, session, false).await?;
                let changed = diesel::update(
                    accounts::table
                        .find(account)
                        .filter(accounts::member_id.eq(current.id)),
                )
                .set((accounts::is_public.eq(public), accounts::updated_at.eq(now)))
                .execute(conn)
                .await?;
                if changed != 1 {
                    return Err(AuthError::AccountNotFound);
                }
                Ok(())
            })
            .await
    }

    pub(crate) async fn unlink_account(
        &self,
        session: &AuthenticatedSession,
        account: Uuid,
    ) -> Result<(), AuthError> {
        let mut conn = self.pool.get().await.map_err(|_| AuthError::Unavailable)?;
        (&mut *conn)
            .transaction(async move |conn| {
                let current = authorize(conn, session, true).await?;
                let actor = accounts::table
                    .find(account)
                    .filter(accounts::member_id.eq(current.id))
                    .select(accounts::actor_url)
                    .first::<String>(conn)
                    .await
                    .optional()?
                    .ok_or(AuthError::AccountNotFound)?;
                let count = accounts::table
                    .filter(accounts::member_id.eq(current.id))
                    .count()
                    .get_result::<i64>(conn)
                    .await?;
                if count <= 1 && current.password_hash.is_none() {
                    return Err(AuthError::LastLoginMethod);
                }
                diesel::delete(challenges::table.filter(challenges::actor_url.eq(actor)))
                    .execute(conn)
                    .await?;
                super::super::profile_refresh::detach(conn, current.id, account).await?;
                diesel::delete(accounts::table.find(account))
                    .execute(conn)
                    .await?;
                diesel::delete(sessions::table.filter(sessions::member_id.eq(current.id)))
                    .execute(conn)
                    .await?;
                Ok(())
            })
            .await
    }

    pub(crate) async fn recover_credentials(
        &self,
        session: &AuthenticatedSession,
        next: &str,
    ) -> Result<SessionGrant, AuthError> {
        let mut conn = self.pool.get().await.map_err(|_| AuthError::Unavailable)?;
        (&mut *conn)
            .transaction(async move |conn| {
                let current = authorize(conn, session, true).await?;
                if current.login_id.is_none() {
                    return Err(AuthError::InvalidCredentials);
                }
                let proof_at = sessions::table
                    .find(session.id)
                    .select(sessions::federated_authenticated_at)
                    .first::<Option<DateTime<Utc>>>(conn)
                    .await?;
                let at = database_time(conn).await?;
                if proof_at.is_none_or(|proof| proof > at || at - proof >= Duration::minutes(15)) {
                    return Err(AuthError::FederatedAuthenticationRequired);
                }
                diesel::update(users::table.find(current.id))
                    .set((users::password_hash.eq(next), users::updated_at.eq(now)))
                    .execute(conn)
                    .await?;
                cancel_identity_challenges(conn, current.id).await?;
                diesel::delete(sessions::table.filter(sessions::member_id.eq(current.id)))
                    .execute(conn)
                    .await?;
                persist_session(conn, current.member(), SessionAuthentication::Password).await
            })
            .await
    }

    pub(crate) async fn withdraw_member(
        &self,
        session: &AuthenticatedSession,
    ) -> Result<(), AuthError> {
        let mut conn = self.pool.get().await.map_err(|_| AuthError::Unavailable)?;
        (&mut *conn).transaction(async move |conn| {
            let current = authorize(conn, session, true).await?;
            cancel_identity_challenges(conn, current.id).await?;
            let media_keys = super::super::media_cleanup::member_keys(conn, current.id).await
                .map_err(|_| AuthError::Unavailable)?;
            // Phoenix Accounts.withdraw_user semantics: tombstone comments,
            // detach author/report/owner relationships, remove membership.
            // Bodies/reports remain for moderation; do not present as erasure.
            use diesel::sql_types::Uuid as SqlUuid;
            diesel::sql_query("SELECT s.id FROM directory_sites s JOIN directory_site_details d ON d.site_id=s.id WHERE d.owner_id=$1 ORDER BY s.id FOR UPDATE OF s")
                .bind::<SqlUuid,_>(current.id).execute(conn).await?;
            for sql in [
                "INSERT INTO directory_site_edits(id,site_id,actor_id,action,revision,previous) SELECT gen_random_uuid(),s.id,$1,'withdraw',d.revision,jsonb_build_object('site',to_jsonb(s),'details',to_jsonb(d)) FROM directory_site_details d JOIN directory_sites s ON s.id=d.site_id WHERE d.owner_id=$1",
                "UPDATE directory_site_details SET owner_id=NULL,owner_method=NULL,revision=revision+1,updated_at=now() WHERE owner_id=$1",
                "UPDATE community_comments SET is_deleted=true, user_id=NULL, updated_at=now() AT TIME ZONE 'UTC' WHERE user_id=$1",
                "UPDATE community_reports SET reporter_id=NULL WHERE reporter_id=$1",
                "UPDATE community_reports SET resolved_by_id=NULL WHERE resolved_by_id=$1",
                "UPDATE legacy_sites SET admin_user_id=NULL, admin_verified_via=NULL WHERE admin_user_id=$1",
                "DELETE FROM member_legacy_claims WHERE member_id=$1",
                "DELETE FROM legacy_members WHERE id=$1",
            ] {
                diesel::sql_query(sql).bind::<SqlUuid,_>(current.id).execute(conn).await?;
            }
            diesel::delete(users::table.find(current.id)).execute(conn).await?;
            super::super::media_cleanup::retire_keys(conn, &media_keys).await
                .map_err(|_| AuthError::Unavailable)?;
            Ok(())
        }).await
    }
}
