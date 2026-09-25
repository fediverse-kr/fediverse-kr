use super::*;
use crate::backend::site_management as service;
use crate::backend::{
    auth,
    db::fixtures,
    site_management::api_verification::{
        tests::{actor, v2},
        verify,
    },
};

async fn setup(db: &Database, session: &AuthenticatedSession) -> Candidate {
    let domain = format!("operator-{}.example.org", Uuid::new_v4().simple());
    let site_id = db.add_site(&domain).await.unwrap();
    let account_id = Uuid::new_v4();
    let actor_url = format!("https://{domain}/users/admin");
    let handle = format!("@admin@{domain}");
    let mut conn = db.pool.get().await.unwrap();
    diesel::sql_query("INSERT INTO member_linked_accounts(id,member_id,actor_url,handle,display_name,profile_url,provider,provider_origin,provider_subject_id,verified_at) VALUES($1,$2,$3,$4,'fixture',$3,'activitypub_post',$5,$3,now())")
        .bind::<SqlUuid,_>(account_id).bind::<SqlUuid,_>(session.member.id).bind::<Text,_>(&actor_url)
        .bind::<Text,_>(&handle).bind::<Text,_>(format!("https://{domain}")).execute(&mut conn).await.unwrap();
    diesel::sql_query("INSERT INTO directory_observations(site_id,is_alive,checked_at,software) VALUES($1,true,now(),'mastodon')")
        .bind::<SqlUuid,_>(site_id).execute(&mut conn).await.unwrap();
    drop(conn);
    db.api_owner_candidate(session, account_id, &domain)
        .await
        .unwrap()
}
async fn session(db: &Database, token: &str) -> AuthenticatedSession {
    auth::get_session_details(db, token).await.unwrap().unwrap()
}
async fn clean(db: &Database, c: &Candidate, members: &[Uuid]) {
    diesel::sql_query("DELETE FROM directory_sites WHERE id=$1")
        .bind::<SqlUuid, _>(c.site_id)
        .execute(&mut db.pool.get().await.unwrap())
        .await
        .unwrap();
    fixtures::delete_members(db, members).await;
}
#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn api_owner_claim_rechecks_member_session_account_revision_and_dns_priority() {
    let db = fixtures::database().await;
    let a = fixtures::member(&db).await;
    let b = fixtures::member(&db).await;
    let sa = session(&db, &a.token).await;
    let sb = session(&db, &b.token).await;
    let c = setup(&db, &sa).await;
    // Legacy API ownership can be replaced by a currently verified operator.
    diesel::sql_query(
        "UPDATE directory_site_details SET owner_id=$2,owner_method='api' WHERE site_id=$1",
    )
    .bind::<SqlUuid, _>(c.site_id)
    .bind::<SqlUuid, _>(b.member.id)
    .execute(&mut db.pool.get().await.unwrap())
    .await
    .unwrap();
    assert!(matches!(
        db.api_owner_candidate(&sb, c.account_id, &c.domain).await,
        Err(Error::ApiNotOwner)
    ));
    let proof = verify(c.clone(), &actor(&c), &v2(&c)).await.unwrap();
    assert_eq!(
        db.claim_site_api(&sb, &proof).await,
        Err(Error::ApiNotOwner)
    );
    service::api_verification::claim(&db, &sa, c.account_id, &c.domain, &actor(&c), &v2(&c))
        .await
        .unwrap();
    let site = db.owned_sites(&sa, 0).await.unwrap().sites.remove(0);
    assert_eq!(site.owner_method.as_deref(), Some("api"));
    assert_eq!(site.revision, 1);
    assert_eq!(db.claim_site_api(&sa, &proof).await, Err(Error::Conflict));
    let current = db
        .api_owner_candidate(&sa, c.account_id, &c.domain)
        .await
        .unwrap();
    let pending = verify(current.clone(), &actor(&current), &v2(&current))
        .await
        .unwrap();
    // Simulate a concurrent DNS claim through the actual DNS service.
    struct Dns(String);
    impl service::TxtResolver for Dns {
        async fn txt(&self, _: &str) -> Result<Vec<Vec<Vec<u8>>>, Error> {
            Ok(vec![vec![self.0.as_bytes().to_vec()]])
        }
    }
    let code = service::begin(&db, &sb, &c.domain).await.unwrap();
    service::finish(
        &db,
        &sb,
        Uuid::parse_str(&code.id).unwrap(),
        &code.value,
        &Dns(code.value.clone()),
    )
    .await
    .unwrap();
    assert_eq!(
        db.claim_site_api(&sa, &pending).await,
        Err(Error::DnsPriority)
    );
    assert_eq!(
        db.owned_sites(&sb, 0).await.unwrap().sites[0]
            .owner_method
            .as_deref(),
        Some("dns")
    );
    clean(&db, &c, &[a.member.id, b.member.id]).await;
}
#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn api_owner_claim_rejects_unlink_reverification_ban_and_revocation_during_io() {
    let db = fixtures::database().await;
    let a = fixtures::member(&db).await;
    let sa = session(&db, &a.token).await;
    let c = setup(&db, &sa).await;
    let proof = verify(c.clone(), &actor(&c), &v2(&c)).await.unwrap();
    diesel::sql_query(
        "UPDATE member_linked_accounts SET verified_at=verified_at+interval '1 second' WHERE id=$1",
    )
    .bind::<SqlUuid, _>(c.account_id)
    .execute(&mut db.pool.get().await.unwrap())
    .await
    .unwrap();
    assert_eq!(db.claim_site_api(&sa, &proof).await, Err(Error::Conflict));
    let c2 = db
        .api_owner_candidate(&sa, c.account_id, &c.domain)
        .await
        .unwrap();
    let proof = verify(c2.clone(), &actor(&c2), &v2(&c2)).await.unwrap();
    fixtures::ban(&db, a.member.id, true).await;
    assert_eq!(
        db.claim_site_api(&sa, &proof).await,
        Err(Error::Auth(auth::AuthError::Unauthenticated))
    );
    fixtures::ban(&db, a.member.id, false).await;
    diesel::sql_query("DELETE FROM member_linked_accounts WHERE id=$1")
        .bind::<SqlUuid, _>(c.account_id)
        .execute(&mut db.pool.get().await.unwrap())
        .await
        .unwrap();
    assert_eq!(
        db.claim_site_api(&sa, &proof).await,
        Err(Error::ApiNotOwner)
    );
    auth::revoke_session(&db, &a.token).await.unwrap();
    assert_eq!(
        db.claim_site_api(&sa, &proof).await,
        Err(Error::Auth(auth::AuthError::Unauthenticated))
    );
    clean(&db, &c, &[a.member.id]).await;
}
