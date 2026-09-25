use super::*;
use crate::backend::moderation::Error as AdminError;

struct BatchFixture {
    f: Fixture,
    admin: auth::SessionGrant,
    session: AuthenticatedSession,
    batches: Vec<Uuid>,
}
impl BatchFixture {
    async fn new() -> Self {
        let f = Fixture::new().await;
        let admin = fixtures::member(&f.db).await;
        // Fixture-only role; actual revocation tests use the audited host command.
        diesel::sql_query("INSERT INTO member_admin_roles(member_id) VALUES($1)")
            .bind::<SqlUuid, _>(admin.member.id)
            .execute(&mut f.db.pool.get().await.unwrap())
            .await
            .unwrap();
        let session = auth::get_session_details(&f.db, &admin.token)
            .await
            .unwrap()
            .unwrap();
        Self {
            f,
            admin,
            session,
            batches: vec![],
        }
    }
    async fn start(&mut self) -> Uuid {
        let batch = self.f.db.start_profile_batch(&self.session).await.unwrap();
        let id = Uuid::parse_str(&batch.id).unwrap();
        self.batches.push(id);
        id
    }
    async fn job(&self) -> BatchJob {
        // Skip only the other synthetic targets; the real worker follows the same queue.
        while let Some(job) = self.f.db.claim_profile_job().await.unwrap() {
            if job.member == self.f.grant.member.id {
                return job;
            }
            self.f
                .db
                .complete_profile_job(&job, "skipped")
                .await
                .unwrap();
        }
        panic!("fixture target was not queued")
    }
    async fn chosen_source(&self) {
        let ticket = self.f.ticket().await;
        self.f
            .db
            .finish_profile_refresh(&ticket, &self.f.store, self.f.downloads("before"))
            .await
            .unwrap();
        self.f.reset_clock().await;
    }
    async fn expire(&self, job: &BatchJob) {
        diesel::sql_query("UPDATE profile_refresh_jobs SET lease_until=now()-interval '1 second' WHERE batch_id=$1 AND member_id=$2")
            .bind::<SqlUuid,_>(job.batch).bind::<SqlUuid,_>(job.member)
            .execute(&mut self.f.db.pool.get().await.unwrap()).await.unwrap();
    }
    async fn clean(self) {
        let mut conn = self.f.db.pool.get().await.unwrap();
        for id in self.batches {
            diesel::sql_query("DELETE FROM profile_refresh_batches WHERE id=$1")
                .bind::<SqlUuid, _>(id)
                .execute(&mut conn)
                .await
                .unwrap();
        }
        diesel::sql_query("DELETE FROM moderation_events WHERE target_id=$1")
            .bind::<SqlUuid, _>(self.admin.member.id)
            .execute(&mut conn)
            .await
            .unwrap();
        diesel::sql_query("DELETE FROM member_legacy_claims WHERE member_id=$1")
            .bind::<SqlUuid, _>(self.f.grant.member.id)
            .execute(&mut conn)
            .await
            .unwrap();
        drop(conn);
        fixtures::delete_members(&self.f.db, &[self.admin.member.id]).await;
        self.f.clean().await;
    }
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn profile_batch_requires_admin_and_fresh_login_and_snapshots_once() {
    let mut b = BatchFixture::new().await;
    assert!(matches!(
        b.f.db.start_profile_batch(&b.f.session).await,
        Err(AdminError::Forbidden)
    ));
    assert!(matches!(
        b.f.db.profile_batch_status(&b.f.session).await,
        Err(AdminError::Forbidden)
    ));
    fixtures::age_session(&b.f.db, b.session.id).await;
    assert!(matches!(
        b.f.db.start_profile_batch(&b.session).await,
        Err(AdminError::Auth(AuthError::FreshAuthenticationRequired))
    ));
    diesel::sql_query(
        "UPDATE member_sessions SET created_at=now(),authenticated_at=now() WHERE id=$1",
    )
    .bind::<SqlUuid, _>(b.session.id)
    .execute(&mut b.f.db.pool.get().await.unwrap())
    .await
    .unwrap();
    let id = b.start().await;
    let status =
        b.f.db
            .profile_batch_status(&b.session)
            .await
            .unwrap()
            .unwrap();
    assert_eq!(status.total, 2);
    assert_eq!(status.pending, 2);
    let later = fixtures::member(&b.f.db).await;
    let same = b.f.db.start_profile_batch(&b.session).await.unwrap();
    assert_eq!(same.id, id.to_string());
    assert_eq!(same.total, 2);
    assert!(matches!(
        b.f.db.cancel_profile_batch(&b.f.session, id).await,
        Err(AdminError::Forbidden)
    ));
    let json = serde_json::to_string(&same).unwrap();
    assert!(!json.contains(&b.f.grant.member.id.to_string()));
    assert!(!json.contains(&b.admin.member.id.to_string()));
    assert!(!json.contains("handle"));
    let cancelled = b.f.db.cancel_profile_batch(&b.session, id).await.unwrap();
    assert_eq!(cancelled.state, "cancelled");
    assert_eq!(cancelled.cancelled, 2);
    assert!(b.f.db.claim_profile_job().await.unwrap().is_none());
    fixtures::delete_members(&b.f.db, &[later.member.id]).await;
    b.clean().await;
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn profile_batch_preserves_chosen_identity_name_privacy_and_survives_logout() {
    let mut b = BatchFixture::new().await;
    b.chosen_source().await;
    b.start().await;
    let job = b.job().await;
    let ticket = b.f.db.begin_batch_profile(&job).await.unwrap().unwrap();
    assert_eq!(ticket.account, Some(b.f.account));
    auth::revoke_session(&b.f.db, &b.admin.token).await.unwrap();
    // The explicit durable batch, not the browser session, authorizes this work.
    b.f.db
        .finish_profile_refresh(&ticket, &b.f.store, b.f.downloads("batch"))
        .await
        .unwrap();
    b.f.db
        .complete_profile_job(&job, "succeeded")
        .await
        .unwrap();
    assert_eq!(b.f.avatar().await, Some(b.f.bytes("batch")));
    let user = auth::get_session(&b.f.db, &b.f.grant.token)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(user.display_name, "core test fixture");
    assert!(b
        .f
        .db
        .linked_accounts(user.id)
        .await
        .unwrap()
        .iter()
        .all(|a| !a.is_public));
    assert_eq!(
        b.f.db
            .own_profile_media(&b.f.grant.token)
            .await
            .unwrap()
            .source_account_id,
        Some(b.f.account.to_string())
    );
    assert!(matches!(
        b.f.db.profile_batch_status(&b.session).await,
        Err(AdminError::Auth(AuthError::Unauthenticated))
    ));
    b.clean().await;
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn profile_batch_cancellation_and_role_revocation_fence_file_publication() {
    let mut b = BatchFixture::new().await;
    b.chosen_source().await;
    let id = b.start().await;
    let job = b.job().await;
    let ticket = b.f.db.begin_batch_profile(&job).await.unwrap().unwrap();
    b.f.db.cancel_profile_batch(&b.session, id).await.unwrap();
    assert!(matches!(
        b.f.db
            .finish_profile_refresh(&ticket, &b.f.store, b.f.downloads("cancelled"))
            .await,
        Err(Error::Superseded)
    ));
    b.f.db
        .complete_profile_job(&job, "succeeded")
        .await
        .unwrap();
    assert!(
        b.f.db
            .profile_batch_status(&b.session)
            .await
            .unwrap()
            .unwrap()
            .cancelled
            > 0
    );
    b.f.reset_clock().await;
    b.start().await;
    let job = b.job().await;
    let ticket = b.f.db.begin_batch_profile(&job).await.unwrap().unwrap();
    b.f.db
        .set_admin_role(b.admin.member.id, false)
        .await
        .unwrap();
    assert!(matches!(
        b.f.db
            .finish_profile_refresh(&ticket, &b.f.store, b.f.downloads("revoked"))
            .await,
        Err(Error::Superseded)
    ));
    assert!(b.f.db.claim_profile_job().await.unwrap().is_none());
    assert_eq!(b.f.avatar().await, Some(b.f.bytes("before")));
    for key in ["cancelled", "revoked"] {
        assert!(b
            .f
            .store
            .read(&storage::ObjectRef::from_bytes(&b.f.bytes(key)).unwrap())
            .await
            .is_err());
    }
    b.clean().await;
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn profile_batch_expired_lease_is_reclaimed_and_stale_result_cannot_win() {
    let mut b = BatchFixture::new().await;
    b.chosen_source().await;
    b.start().await;
    let first = b.job().await;
    let ticket = b.f.db.begin_batch_profile(&first).await.unwrap().unwrap();
    b.expire(&first).await;
    // A fresh DB handle models the restarted worker: no in-memory queue required.
    let restarted = Database::connect(&std::env::var("FEDKR_TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    let next = restarted.claim_profile_job().await.unwrap().unwrap();
    assert_eq!(first.member, next.member);
    assert_ne!(first.lease, next.lease);
    b.f.db
        .complete_profile_job(&first, "succeeded")
        .await
        .unwrap();
    assert!(matches!(
        b.f.db
            .finish_profile_refresh(&ticket, &b.f.store, b.f.downloads("stale"))
            .await,
        Err(Error::Superseded)
    ));
    b.expire(&next).await;
    let last = restarted.claim_profile_job().await.unwrap().unwrap();
    assert_eq!(last.member, first.member);
    b.expire(&last).await;
    while let Some(job) = restarted.claim_profile_job().await.unwrap() {
        restarted
            .complete_profile_job(&job, "skipped")
            .await
            .unwrap();
    }
    let status =
        b.f.db
            .profile_batch_status(&b.session)
            .await
            .unwrap()
            .unwrap();
    assert_eq!(status.state, "complete");
    assert_eq!(status.failed, 1);
    assert_eq!(status.succeeded, 0);
    assert_eq!(status.skipped, 1);
    assert_eq!(b.f.avatar().await, Some(b.f.bytes("before")));
    b.clean().await;
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn profile_batch_selects_only_unambiguous_source_and_never_resurrects_unlinked_one() {
    let mut b = BatchFixture::new().await;
    let id = b.start().await;
    let job = b.job().await;
    assert!(b.f.db.begin_batch_profile(&job).await.unwrap().is_none()); // two identities, no chosen source
    diesel::sql_query("DELETE FROM member_linked_accounts WHERE id=$1")
        .bind::<SqlUuid, _>(b.f.other)
        .execute(&mut b.f.db.pool.get().await.unwrap())
        .await
        .unwrap();
    let ticket = b.f.db.begin_batch_profile(&job).await.unwrap().unwrap();
    assert_eq!(ticket.account, Some(b.f.account));
    b.f.db
        .finish_profile_refresh(&ticket, &b.f.store, b.f.downloads("sole"))
        .await
        .unwrap();
    // Simulate the detach helper in the same transaction as account deletion.
    let mut conn = b.f.db.pool.get().await.unwrap();
    (&mut *conn)
        .transaction::<_, AuthError, _>(async |conn| {
            detach(conn, b.f.grant.member.id, b.f.account).await?;
            diesel::sql_query("DELETE FROM member_linked_accounts WHERE id=$1")
                .bind::<SqlUuid, _>(b.f.account)
                .execute(conn)
                .await?;
            Ok(())
        })
        .await
        .unwrap();
    drop(conn);
    assert!(b.f.db.begin_batch_profile(&job).await.unwrap().is_none());
    assert!(b.f.avatar().await.is_none());
    b.f.db.cancel_profile_batch(&b.session, id).await.unwrap();
    b.clean().await;
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn profile_batch_legacy_photo_is_not_an_account_claim() {
    let mut b = BatchFixture::new().await;
    let mut conn = b.f.db.pool.get().await.unwrap();
    diesel::sql_query("DELETE FROM member_linked_accounts WHERE member_id=$1")
        .bind::<SqlUuid, _>(b.f.grant.member.id)
        .execute(&mut conn)
        .await
        .unwrap();
    diesel::sql_query("INSERT INTO member_legacy_claims(member_id,handle) VALUES($1,'@legacy@social.example.org')")
        .bind::<SqlUuid,_>(b.f.grant.member.id).execute(&mut conn).await.unwrap();
    drop(conn);
    b.start().await;
    let job = b.job().await;
    let ticket = b.f.db.begin_batch_profile(&job).await.unwrap().unwrap();
    assert!(ticket.account.is_none() && ticket.actor_id.is_none());
    b.f.db
        .finish_profile_refresh(&ticket, &b.f.store, b.f.downloads("legacy-batch"))
        .await
        .unwrap();
    let mut conn = b.f.db.pool.get().await.unwrap();
    assert!(
        super::super::batches::source(&mut conn, b.f.grant.member.id)
            .await
            .unwrap()
            .unwrap()
            .actor_id
            .is_none()
    );
    drop(conn);
    assert!(b
        .f
        .db
        .linked_accounts(b.f.grant.member.id)
        .await
        .unwrap()
        .is_empty());
    assert_eq!(b.f.avatar().await, Some(b.f.bytes("legacy-batch")));
    b.f.reset_clock().await;
    let ticket = b.f.db.begin_batch_profile(&job).await.unwrap().unwrap();
    // Claiming that legacy identity while a fetch runs invalidates the media-only authority.
    diesel::sql_query("UPDATE member_legacy_claims SET actor_url='https://social.example.org/users/legacy',claimed_at=now() WHERE member_id=$1")
        .bind::<SqlUuid,_>(b.f.grant.member.id).execute(&mut b.f.db.pool.get().await.unwrap()).await.unwrap();
    assert!(matches!(
        b.f.db
            .finish_profile_refresh(&ticket, &b.f.store, b.f.downloads("late"))
            .await,
        Err(Error::Superseded)
    ));
    assert_eq!(b.f.avatar().await, Some(b.f.bytes("legacy-batch")));
    b.clean().await;
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn profile_batch_unlink_and_initiator_withdrawal_invalidate_inflight_results() {
    let mut b = BatchFixture::new().await;
    b.chosen_source().await;
    b.start().await;
    let job = b.job().await;
    let ticket = b.f.db.begin_batch_profile(&job).await.unwrap().unwrap();
    auth::unlink_account(&b.f.db, &b.f.grant.token, b.f.account)
        .await
        .unwrap();
    assert!(matches!(
        b.f.db
            .finish_profile_refresh(&ticket, &b.f.store, b.f.downloads("unlinked"))
            .await,
        Err(Error::Superseded)
    ));
    auth::withdraw(&b.f.db, &b.admin.token, "탈퇴")
        .await
        .unwrap();
    assert!(b.f.db.claim_profile_job().await.unwrap().is_none());
    assert!(matches!(
        b.f.db.begin_batch_profile(&job).await,
        Err(Error::Superseded)
    ));
    b.clean().await;
}
