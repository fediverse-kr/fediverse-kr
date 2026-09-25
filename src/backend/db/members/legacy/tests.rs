use super::*;
use crate::backend::{
    db::fixtures,
    flow::tests::{actor, proof},
};
use diesel::sql_types::{Text, Uuid as SqlUuid};

struct Fixture {
    db: Database,
    member: Uuid,
    actor: ResolvedActor,
}
impl Fixture {
    async fn new() -> Self {
        let db = fixtures::database().await;
        let member = fixtures::member(&db).await.member.id;
        let actor = actor(&format!("legacy_{}", Uuid::new_v4().simple()));
        let mut conn = db.pool.get().await.unwrap();
        diesel::delete(sessions::table.filter(sessions::member_id.eq(member)))
            .execute(&mut conn)
            .await
            .unwrap();
        diesel::update(users::table.find(member))
            .set((
                users::display_name.eq("옛 표시 이름"),
                users::created_at.eq("2020-01-01T00:00:00Z".parse::<DateTime<Utc>>().unwrap()),
            ))
            .execute(&mut conn)
            .await
            .unwrap();
        diesel::sql_query("INSERT INTO legacy_members(id,fediverse_handle,fediverse_domain,display_name,verified_at,is_banned,inserted_at,updated_at) VALUES($1,$2,'social.example.com','옛 표시 이름','2020-01-01',false,'2020-01-01','2020-01-01')").bind::<SqlUuid,_>(member).bind::<Text,_>(actor.handle.trim_start_matches('@')).execute(&mut conn).await.unwrap();
        diesel::insert_into(claims::table)
            .values((
                claims::member_id.eq(member),
                claims::handle.eq(actor.handle.to_lowercase()),
            ))
            .execute(&mut conn)
            .await
            .unwrap();
        drop(conn);
        Self { db, member, actor }
    }
    async fn claim(&self) -> Claim {
        claims::table
            .find(self.member)
            .select(Claim::as_select())
            .first(&mut self.db.pool.get().await.unwrap())
            .await
            .unwrap()
    }
    async fn clean(self) {
        let mut conn = self.db.pool.get().await.unwrap();
        diesel::delete(
            challenges::table.filter(
                challenges::handle
                    .eq(&self.actor.handle)
                    .or(challenges::actor_url.eq(&self.actor.id)),
            ),
        )
        .execute(&mut conn)
        .await
        .unwrap();
        diesel::delete(claims::table.find(self.member))
            .execute(&mut conn)
            .await
            .unwrap();
        diesel::sql_query("DELETE FROM legacy_members WHERE id=$1")
            .bind::<SqlUuid, _>(self.member)
            .execute(&mut conn)
            .await
            .unwrap();
        drop(conn);
        fixtures::delete_members(&self.db, &[self.member]).await;
    }
}

// Use the real WebFinger/AP verifier with an isolated transport. No live posts.
async fn prepare(
    db: &Database,
    actor: &ResolvedActor,
    token: Option<&str>,
) -> (flow::IssuedChallenge, String, VerifiedIdentity) {
    let nonce = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    let session = if let Some(token) = token {
        auth::get_session_details(db, token).await.unwrap()
    } else {
        None
    };
    let issued = flow::begin_challenge(db, actor, &nonce, session.as_ref())
        .await
        .unwrap();
    let at = db.challenge(issued.id).await.unwrap().unwrap().created_at;
    let verified = proof(actor, &issued.code, at).await;
    (issued, nonce, verified)
}
async fn login(
    db: &Database,
    actor: &ResolvedActor,
    token: Option<&str>,
) -> Result<SessionGrant, FlowError> {
    let (c, n, p) = prepare(db, actor, token).await;
    flow::complete_challenge(db, c.id, &n, &c.code, token, &p).await
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn first_public_proof_reuses_legacy_member_and_preserves_owned_data() {
    let f = Fixture::new().await;
    let domain = format!("legacy-{}.example.org", Uuid::new_v4().simple());
    let site = f.db.add_site(&domain).await.unwrap();
    let comment = Uuid::new_v4();
    let mut conn = f.db.pool.get().await.unwrap();
    diesel::sql_query(
        "INSERT INTO directory_site_details(site_id,owner_id,owner_method) VALUES($1,$2,'dns')",
    )
    .bind::<SqlUuid, _>(site)
    .bind::<SqlUuid, _>(f.member)
    .execute(&mut conn)
    .await
    .unwrap();
    diesel::sql_query("INSERT INTO community_comments(id,user_id,server_id,body,is_deleted,inserted_at,updated_at) VALUES($1,$2,$3,'이전 댓글',false,'2020-01-01','2020-01-01')").bind::<SqlUuid,_>(comment).bind::<SqlUuid,_>(f.member).bind::<SqlUuid,_>(site).execute(&mut conn).await.unwrap();
    drop(conn);
    let mut case = f.actor.clone();
    case.handle = case.handle.to_uppercase();
    // Domain casing must be canonical for a resolved actor; only username varies.
    case.handle = format!(
        "@{}@social.example.com",
        case.handle
            .trim_start_matches('@')
            .split('@')
            .next()
            .unwrap()
    );
    let (c, n, p) = prepare(&f.db, &case, None).await;
    let grant = flow::complete_challenge(&f.db, c.id, &n, &c.code, None, &p)
        .await
        .unwrap();
    assert_eq!(grant.member.id, f.member);
    assert_eq!(grant.member.display_name, "옛 표시 이름");
    assert_eq!(grant.member.created_at.year(), 2020);
    let link = f.db.linked_accounts(f.member).await.unwrap();
    assert_eq!(link.len(), 1);
    assert!(!link[0].is_public);
    assert_eq!(link[0].actor_url, f.actor.id);
    let pin = f.claim().await;
    assert_eq!(pin.actor_url.as_deref(), Some(f.actor.id.as_str()));
    // PostgreSQL persists microseconds, while Rust's verifier clock may carry
    // nanoseconds. Compare at the storage precision, not at a fake rounded clock.
    assert_eq!(
        pin.claimed_at.unwrap().timestamp_micros(),
        p.verified_at().timestamp_micros()
    );
    let s = auth::get_session_details(&f.db, &grant.token)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        f.db.owned_sites(&s, 0).await.unwrap().sites[0].domain,
        domain
    );
    let access = f.db.comment_access(&s, &domain, &[comment]).await.unwrap();
    assert!(access.permissions[0].own);
    assert!(s.federated_authenticated_at.is_some());
    assert!(flow::complete_challenge(&f.db, c.id, &n, &c.code, None, &p)
        .await
        .is_err());
    assert_eq!(
        login(&f.db, &f.actor, None).await.unwrap().member.id,
        f.member
    );
    let mut conn = f.db.pool.get().await.unwrap();
    diesel::sql_query("DELETE FROM community_comments WHERE id=$1")
        .bind::<SqlUuid, _>(comment)
        .execute(&mut conn)
        .await
        .unwrap();
    diesel::sql_query("DELETE FROM directory_sites WHERE id=$1")
        .bind::<SqlUuid, _>(site)
        .execute(&mut conn)
        .await
        .unwrap();
    // Remove the case-variant consumed challenge as well, without touching others.
    diesel::delete(challenges::table.filter(challenges::actor_url.eq(&f.actor.id)))
        .execute(&mut conn)
        .await
        .unwrap();
    drop(conn);
    f.clean().await;
}

use chrono::Datelike;
#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn invalid_proof_ban_and_another_member_session_cannot_claim_legacy_data() {
    let f = Fixture::new().await;
    let (c, n, p) = prepare(&f.db, &f.actor, None).await;
    assert!(
        flow::complete_challenge(&f.db, c.id, &"0".repeat(64), &c.code, None, &p)
            .await
            .is_err()
    );
    assert!(f.claim().await.actor_url.is_none());
    fixtures::ban(&f.db, f.member, true).await;
    assert!(matches!(
        flow::complete_challenge(&f.db, c.id, &n, &c.code, None, &p).await,
        Err(FlowError::Unauthenticated)
    ));
    assert!(f.claim().await.actor_url.is_none());
    assert!(f.db.linked_accounts(f.member).await.unwrap().is_empty());
    fixtures::ban(&f.db, f.member, false).await;
    let other = fixtures::member(&f.db).await;
    assert!(matches!(
        login(&f.db, &f.actor, Some(&other.token)).await,
        Err(FlowError::AlreadyLinked)
    ));
    assert!(f.claim().await.actor_url.is_none());
    assert_eq!(
        flow::complete_challenge(&f.db, c.id, &n, &c.code, None, &p)
            .await
            .unwrap()
            .member
            .id,
        f.member
    );
    fixtures::delete_members(&f.db, &[other.member.id]).await;
    f.clean().await;
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn two_actors_racing_for_one_legacy_handle_bind_exactly_one() {
    let f = Fixture::new().await;
    let mut other = f.actor.clone();
    other.id.push_str("-replacement");
    other.outbox = format!("{}/outbox", other.id);
    let (a, na, pa) = prepare(&f.db, &f.actor, None).await;
    let (b, nb, pb) = prepare(&f.db, &other, None).await;
    let (ra, rb) = tokio::join!(
        flow::complete_challenge(&f.db, a.id, &na, &a.code, None, &pa),
        flow::complete_challenge(&f.db, b.id, &nb, &b.code, None, &pb)
    );
    assert_ne!(ra.is_ok(), rb.is_ok());
    let actor = if ra.is_ok() { &f.actor.id } else { &other.id };
    assert_eq!(f.claim().await.actor_url.as_ref(), Some(actor));
    let links = f.db.linked_accounts(f.member).await.unwrap();
    assert_eq!(links.len(), 1);
    assert_eq!(&links[0].actor_url, actor);
    // A fresh proof for the losing actor still cannot replace the established pin.
    let loser = if ra.is_ok() { &other } else { &f.actor };
    assert!(matches!(
        login(&f.db, loser, None).await,
        Err(FlowError::LegacyIdentity) | Err(FlowError::AlreadyLinked)
    ));
    f.clean().await;
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn an_unlinked_legacy_actor_is_not_an_anonymous_recovery_backdoor() {
    let f = Fixture::new().await;
    let initial = login(&f.db, &f.actor, None).await.unwrap();
    let login_id = format!("legacy_{}", &f.member.simple().to_string()[..20]);
    let password = "synthetic legacy account password";
    let local = auth::set_local_credentials(&f.db, &initial.token, &login_id, password)
        .await
        .unwrap();
    let account = f.db.linked_accounts(f.member).await.unwrap()[0].id;
    let (pending, nonce, verified) = prepare(&f.db, &f.actor, None).await;
    auth::unlink_account(&f.db, &local.token, account)
        .await
        .unwrap();
    assert!(
        flow::complete_challenge(&f.db, pending.id, &nonce, &pending.code, None, &verified)
            .await
            .is_err()
    );
    assert!(matches!(
        login(&f.db, &f.actor, None).await,
        Err(FlowError::LegacyIdentity)
    ));
    let local = auth::login(&f.db, &login_id, password).await.unwrap();
    let restored = login(&f.db, &f.actor, Some(&local.token)).await.unwrap();
    assert_eq!(restored.member.id, f.member);
    assert_eq!(
        auth::recover_password(&f.db, &restored.token, "other synthetic password")
            .await
            .unwrap_err(),
        AuthError::FederatedAuthenticationRequired
    );
    assert_eq!(
        login(&f.db, &f.actor, None).await.unwrap().member.id,
        f.member
    );
    fixtures::delete_rate(&f.db, "login", &Sha256::digest(login_id.as_bytes())).await;
    f.clean().await;
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn bound_actor_can_change_handle_but_cannot_merge_another_legacy_member() {
    let f = Fixture::new().await;
    login(&f.db, &f.actor, None).await.unwrap();
    let mut alias = f.actor.clone();
    alias.handle = format!("@renamed_{}@social.example.com", Uuid::new_v4().simple());
    assert_eq!(
        login(&f.db, &alias, None).await.unwrap().member.id,
        f.member
    );
    assert_eq!(f.claim().await.handle, f.actor.handle.to_lowercase());
    let second = Fixture::new().await;
    alias.handle = second.actor.handle.clone();
    assert!(matches!(
        login(&f.db, &alias, None).await,
        Err(FlowError::AlreadyLinked)
    ));
    assert!(second.claim().await.actor_url.is_none());
    second.clean().await;
    f.clean().await;
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn withdrawal_does_not_resurrect_the_old_uuid_from_an_inflight_proof() {
    let f = Fixture::new().await;
    let initial = login(&f.db, &f.actor, None).await.unwrap();
    let (c, n, p) = prepare(&f.db, &f.actor, None).await;
    auth::withdraw(&f.db, &initial.token, "탈퇴").await.unwrap();
    assert!(flow::complete_challenge(&f.db, c.id, &n, &c.code, None, &p)
        .await
        .is_err());
    let fresh = login(&f.db, &f.actor, None).await.unwrap();
    assert_ne!(fresh.member.id, f.member);
    let mut conn = f.db.pool.get().await.unwrap();
    assert!(!diesel::select(exists(claims::table.find(f.member)))
        .get_result::<bool>(&mut conn)
        .await
        .unwrap());
    drop(conn);
    fixtures::delete_members(&f.db, &[fresh.member.id]).await;
    f.clean().await;
}
