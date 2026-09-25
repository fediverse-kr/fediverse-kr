use super::*;
use crate::backend::{
    auth,
    db::fixtures,
    site_management::{self as service, TxtResolver},
};
mod refresh;

struct Records(Vec<Vec<Vec<u8>>>);
impl TxtResolver for Records {
    async fn txt(&self, fqdn: &str) -> Result<Vec<Vec<Vec<u8>>>, Error> {
        assert!(fqdn.starts_with("_fediverse-kr.") && fqdn.ends_with('.'));
        Ok(self.0.clone())
    }
}
fn records(value: &str) -> Records {
    // DNS strings concatenate within ONE record, never across separate records.
    Records(vec![vec![
        value.as_bytes()[..10].to_vec(),
        value.as_bytes()[10..].to_vec(),
    ]])
}
async fn session(db: &Database, token: &str) -> AuthenticatedSession {
    auth::get_session_details(db, token).await.unwrap().unwrap()
}
async fn claim(db: &Database, s: &AuthenticatedSession, domain: &str) {
    let c = service::begin(db, s, domain).await.unwrap();
    service::finish(
        db,
        s,
        Uuid::parse_str(&c.id).unwrap(),
        &c.value,
        &records(&c.value),
    )
    .await
    .unwrap();
}
async fn remove_site(db: &Database, id: Uuid) {
    diesel::delete(sites::table.find(id))
        .execute(&mut db.pool.get().await.unwrap())
        .await
        .unwrap();
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn dns_is_session_bound_single_use_and_only_joins_fragments_within_a_record() {
    let db = fixtures::database().await;
    let a = fixtures::member(&db).await;
    let b = fixtures::member(&db).await;
    let sa = session(&db, &a.token).await;
    let sb = session(&db, &b.token).await;
    let domain = format!("owner-{}.example.org", Uuid::new_v4().simple());
    let id = db.add_site(&domain).await.unwrap();
    let c = service::begin(&db, &sa, &domain).await.unwrap();
    let cid = Uuid::parse_str(&c.id).unwrap();
    assert_eq!(
        service::finish(&db, &sb, cid, &c.value, &records(&c.value)).await,
        Err(Error::InvalidChallenge)
    );
    let split = Records(vec![
        vec![c.value.as_bytes()[..10].to_vec()],
        vec![c.value.as_bytes()[10..].to_vec()],
    ]);
    assert_eq!(
        service::finish(&db, &sa, cid, &c.value, &split).await,
        Err(Error::DnsNotFound)
    );
    let other = format!("fk-verify={}", "a".repeat(64));
    assert_eq!(
        service::finish(&db, &sa, cid, &other, &records(&other)).await,
        Err(Error::InvalidChallenge)
    );
    service::finish(&db, &sa, cid, &c.value, &records(&c.value))
        .await
        .unwrap();
    assert_eq!(
        service::finish(&db, &sa, cid, &c.value, &records(&c.value)).await,
        Err(Error::InvalidChallenge)
    );
    let owner = db.owned_sites(&sa, 0).await.unwrap();
    assert_eq!(owner.sites.len(), 1);
    assert_eq!(owner.sites[0].owner_method.as_deref(), Some("dns"));
    assert!(db.owned_sites(&sb, 0).await.unwrap().sites.is_empty());
    remove_site(&db, id).await;
    fixtures::delete_members(&db, &[a.member.id, b.member.id]).await;
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn owner_edit_conflicts_and_hiding_apply_to_public_list_detail_and_totals() {
    let db = fixtures::database().await;
    let a = fixtures::member(&db).await;
    let b = fixtures::member(&db).await;
    let sa = session(&db, &a.token).await;
    let sb = session(&db, &b.token).await;
    let domain = format!("edit-{}.example.org", Uuid::new_v4().simple());
    let id = db.add_site(&domain).await.unwrap();
    claim(&db, &sa, &domain).await;
    let row = db.owned_sites(&sa, 0).await.unwrap().sites.remove(0);
    let mut changes = row.edit;
    changes.name = "운영자가 정한 이름".into();
    changes.rules = "첫 규칙\n둘째 규칙".into();
    changes.owner_comment = "<script>모의 문자열</script>".into();
    changes.invite_only = Some(true);
    changes.approval_required = Some(false);
    changes.tags = "사진, 개발".into();
    let value = service::validate_edit(changes.clone()).unwrap();
    assert_eq!(
        db.edit_owned_site(&sb, &domain, row.revision, &value).await,
        Err(Error::NotOwned)
    );
    let (one, two) = tokio::join!(
        db.edit_owned_site(&sa, &domain, row.revision, &value),
        db.edit_owned_site(&sa, &domain, row.revision, &value)
    );
    assert_eq!([one, two].iter().filter(|r| r.is_ok()).count(), 1);
    assert!([one, two].contains(&Err(Error::Conflict)));
    let public = db.public_site(&domain).await.unwrap().unwrap();
    assert_eq!(public.name, changes.name);
    assert_eq!(public.guidance.rules, changes.rules);
    assert_eq!(public.guidance.owner_comment, changes.owner_comment);
    assert_eq!(public.registration(), "초대 필요");
    let serialized = serde_json::to_string(&public).unwrap();
    assert!(!serialized.contains(&a.member.id.to_string()));
    assert!(!serialized.contains("owner_method"));
    let baseline = db.public_statistics().await.unwrap().sites;
    changes.hidden = true;
    db.edit_owned_site(
        &sa,
        &domain,
        row.revision + 1,
        &service::validate_edit(changes.clone()).unwrap(),
    )
    .await
    .unwrap();
    assert!(db.public_site(&domain).await.unwrap().is_none());
    assert!(db
        .public_sites(&domain, "", 0)
        .await
        .unwrap()
        .sites
        .is_empty());
    assert_eq!(db.public_statistics().await.unwrap().sites, baseline - 1);
    assert_eq!(db.owned_sites(&sa, 0).await.unwrap().sites.len(), 1);
    diesel::update(sites::table.find(id))
        .set(sites::is_force_hidden.eq(true))
        .execute(&mut db.pool.get().await.unwrap())
        .await
        .unwrap();
    changes.hidden = false;
    db.edit_owned_site(
        &sa,
        &domain,
        row.revision + 2,
        &service::validate_edit(changes).unwrap(),
    )
    .await
    .unwrap();
    assert!(db.public_site(&domain).await.unwrap().is_none());
    assert!(db.owned_sites(&sa, 0).await.unwrap().sites[0].force_hidden);
    fixtures::age_session(&db, sa.id).await;
    assert_eq!(
        db.resign_site(&sa, &domain, row.revision + 3).await,
        Err(Error::Auth(auth::AuthError::FreshAuthenticationRequired))
    );
    remove_site(&db, id).await;
    fixtures::delete_members(&db, &[a.member.id, b.member.id]).await;
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn dns_priority_rechecks_in_flight_proofs_and_revoked_sessions() {
    let db = fixtures::database().await;
    let a = fixtures::member(&db).await;
    let b = fixtures::member(&db).await;
    let sa = session(&db, &a.token).await;
    let sb = session(&db, &b.token).await;
    let domain = format!("priority-{}.example.org", Uuid::new_v4().simple());
    let id = db.add_site(&domain).await.unwrap();
    // An imported API owner may be replaced by a successful DNS proof.
    diesel::sql_query(
        "INSERT INTO directory_site_details(site_id,owner_id,owner_method) VALUES($1,$2,'api')",
    )
    .bind::<SqlUuid, _>(id)
    .bind::<SqlUuid, _>(a.member.id)
    .execute(&mut db.pool.get().await.unwrap())
    .await
    .unwrap();
    let old = service::begin(&db, &sa, &domain).await.unwrap();
    claim(&db, &sb, &domain).await;
    assert_eq!(
        service::finish(
            &db,
            &sa,
            Uuid::parse_str(&old.id).unwrap(),
            &old.value,
            &records(&old.value)
        )
        .await,
        Err(Error::InvalidChallenge)
    );
    assert!(db.owned_sites(&sa, 0).await.unwrap().sites.is_empty());
    struct Revoke {
        db: Database,
        token: String,
        value: String,
    }
    impl TxtResolver for Revoke {
        async fn txt(&self, _: &str) -> Result<Vec<Vec<Vec<u8>>>, Error> {
            auth::revoke_session(&self.db, &self.token).await?;
            Ok(records(&self.value).0)
        }
    }
    let c = service::begin(&db, &sa, &domain).await.unwrap();
    let result = service::finish(
        &db,
        &sa,
        Uuid::parse_str(&c.id).unwrap(),
        &c.value,
        &Revoke {
            db: db.clone(),
            token: a.token.clone(),
            value: c.value.clone(),
        },
    )
    .await;
    assert_eq!(result, Err(Error::Auth(auth::AuthError::Unauthenticated)));
    assert_eq!(db.owned_sites(&sb, 0).await.unwrap().sites.len(), 1);
    let row = db.owned_sites(&sb, 0).await.unwrap().sites.remove(0);
    db.resign_site(&sb, &domain, row.revision).await.unwrap();
    assert!(db.owned_sites(&sb, 0).await.unwrap().sites.is_empty());
    assert!(db.public_site(&domain).await.unwrap().is_some());
    remove_site(&db, id).await;
    fixtures::delete_members(&db, &[a.member.id, b.member.id]).await;
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn withdrawal_detaches_runtime_owner_and_retains_site_and_audit() {
    let db = fixtures::database().await;
    let a = fixtures::member(&db).await;
    let sa = session(&db, &a.token).await;
    let domain = format!("withdraw-{}.example.org", Uuid::new_v4().simple());
    let id = db.add_site(&domain).await.unwrap();
    claim(&db, &sa, &domain).await;
    auth::withdraw(&db, &a.token, "탈퇴").await.unwrap();
    assert!(db.public_site(&domain).await.unwrap().is_some());
    let row = diesel::sql_query(format!("{SELECT} WHERE s.id=$1"))
        .bind::<SqlUuid, _>(id)
        .get_result::<OwnerRow>(&mut db.pool.get().await.unwrap())
        .await
        .unwrap();
    assert!(row.owner_id.is_none());
    assert!(row.owner_method.is_none());
    assert_eq!(row.revision, 2);
    remove_site(&db, id).await;
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn expired_or_banned_owner_challenges_do_not_grant_control() {
    let db = fixtures::database().await;
    let a = fixtures::member(&db).await;
    let sa = session(&db, &a.token).await;
    let domain = format!("expired-{}.example.org", Uuid::new_v4().simple());
    let id = db.add_site(&domain).await.unwrap();
    let c = service::begin(&db, &sa, &domain).await.unwrap();
    let cid = Uuid::parse_str(&c.id).unwrap();
    diesel::sql_query("UPDATE directory_owner_challenges SET created_at=now()-interval '16 minutes',expires_at=now()-interval '1 minute' WHERE id=$1").bind::<SqlUuid,_>(cid).execute(&mut db.pool.get().await.unwrap()).await.unwrap();
    assert_eq!(
        service::finish(&db, &sa, cid, &c.value, &records(&c.value)).await,
        Err(Error::InvalidChallenge)
    );
    let c = service::begin(&db, &sa, &domain).await.unwrap();
    fixtures::ban(&db, a.member.id, true).await;
    assert_eq!(
        service::finish(
            &db,
            &sa,
            Uuid::parse_str(&c.id).unwrap(),
            &c.value,
            &records(&c.value)
        )
        .await,
        Err(Error::Auth(auth::AuthError::Unauthenticated))
    );
    assert!(db.owned_sites(&sa, 0).await.is_err());
    remove_site(&db, id).await;
    fixtures::delete_members(&db, &[a.member.id]).await;
}
