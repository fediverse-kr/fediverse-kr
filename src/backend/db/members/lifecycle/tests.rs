use super::*;
use crate::backend::{
    db::fixtures,
    flow::tests::{actor, proof},
};

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn member_guard_allows_deferred_owner_fk_but_serializes_member_changes() {
    use diesel_async::SimpleAsyncConnection;
    let db = fixtures::database().await;
    let member = fixtures::member(&db).await;
    let session = auth::get_session_details(&db, &member.token)
        .await
        .unwrap()
        .unwrap();
    let site = db
        .add_site(&format!(
            "member-lock-{}.example.org",
            Uuid::new_v4().simple()
        ))
        .await
        .unwrap();
    let mut guard = db.pool.get().await.unwrap();
    let mut other = db.pool.get().await.unwrap();
    guard.batch_execute("BEGIN").await.unwrap();
    authorize(&mut guard, &session, true).await.unwrap();
    other
        .batch_execute("BEGIN; SET LOCAL lock_timeout='100ms'")
        .await
        .unwrap();
    diesel::sql_query(
        "INSERT INTO directory_site_details(site_id,owner_id,owner_method) VALUES($1,$2,'dns')",
    )
    .bind::<diesel::sql_types::Uuid, _>(site)
    .bind::<diesel::sql_types::Uuid, _>(member.member.id)
    .execute(&mut other)
    .await
    .unwrap();
    // The deferred owner FK is checked at COMMIT, while authorization still
    // holds the member guard. This must not wait for that guard to disappear.
    let reference_commit = other.batch_execute("COMMIT").await;
    other
        .batch_execute("ROLLBACK; BEGIN; SET LOCAL lock_timeout='100ms'")
        .await
        .unwrap();
    let concurrent_change = diesel::sql_query("UPDATE member_users SET is_banned=true WHERE id=$1")
        .bind::<diesel::sql_types::Uuid, _>(member.member.id)
        .execute(&mut other)
        .await;
    other.batch_execute("ROLLBACK").await.unwrap();
    guard.batch_execute("ROLLBACK").await.unwrap();
    diesel::sql_query("DELETE FROM directory_sites WHERE id=$1")
        .bind::<diesel::sql_types::Uuid, _>(site)
        .execute(&mut other)
        .await
        .unwrap();
    drop(other);
    drop(guard);
    fixtures::delete_members(&db, &[member.member.id]).await;
    assert!(
        reference_commit.is_ok(),
        "unchanged UUID references must not deadlock"
    );
    assert!(
        concurrent_change.is_err(),
        "member changes must remain serialized"
    );
}

async fn verified(db: &Database, name: &str, token: Option<&str>) -> SessionGrant {
    try_verified(db, name, token).await.unwrap()
}

async fn try_verified(
    db: &Database,
    name: &str,
    token: Option<&str>,
) -> Result<SessionGrant, FlowError> {
    let actor = actor(name);
    let nonce = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    let session = if let Some(token) = token {
        auth::get_session_details(db, token).await.unwrap()
    } else {
        None
    };
    let issued = flow::begin_challenge(db, &actor, &nonce, session.as_ref()).await?;
    let pending = flow::get_pending(db, issued.id, &nonce, &issued.code, session.as_ref()).await?;
    let proof = proof(&actor, &issued.code, pending.created_at).await;
    flow::complete_challenge(db, issued.id, &nonce, &issued.code, token, &proof).await
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn postgres_member_settings_ownership_and_last_login_guard() {
    let db = fixtures::database().await;
    let name = format!("life_{}", Uuid::new_v4().simple());
    let a = verified(&db, &name, None).await;
    let b = fixtures::member(&db).await;
    let links = db.linked_accounts(a.member.id).await.unwrap();
    let id = links[0].id;
    assert!(!links[0].is_public);
    assert_eq!(
        auth::update_display_name(&db, &a.token, "  검토 이름  ")
            .await
            .unwrap()
            .display_name,
        "검토 이름"
    );
    assert_eq!(
        auth::update_display_name(&db, &a.token, "\n")
            .await
            .unwrap_err(),
        AuthError::InvalidDisplayName
    );
    assert_eq!(
        auth::set_link_visibility(&db, &b.token, id, true).await,
        Err(AuthError::AccountNotFound)
    );
    auth::set_link_visibility(&db, &a.token, id, true)
        .await
        .unwrap();
    assert!(db.linked_accounts(a.member.id).await.unwrap()[0].is_public);
    auth::set_link_visibility(&db, &a.token, id, false)
        .await
        .unwrap();
    assert_eq!(
        auth::unlink_account(&db, &a.token, id).await,
        Err(AuthError::LastLoginMethod)
    );
    assert_eq!(
        auth::unlink_account(&db, &b.token, id).await,
        Err(AuthError::AccountNotFound)
    );
    fixtures::ban(&db, a.member.id, true).await;
    assert_eq!(
        auth::set_link_visibility(&db, &a.token, id, true).await,
        Err(AuthError::Unauthenticated)
    );
    fixtures::ban(&db, a.member.id, false).await;

    let linked = verified(&db, &format!("{name}_two"), Some(&a.token)).await;
    let links = db.linked_accounts(a.member.id).await.unwrap();
    let (first, second) = tokio::join!(
        auth::unlink_account(&db, &linked.token, links[0].id),
        auth::unlink_account(&db, &linked.token, links[1].id)
    );
    assert_eq!(usize::from(first.is_ok()) + usize::from(second.is_ok()), 1);
    assert_eq!(db.linked_accounts(a.member.id).await.unwrap().len(), 1);
    assert!(auth::get_session(&db, &linked.token)
        .await
        .unwrap()
        .is_none());
    let survivor = db.linked_accounts(a.member.id).await.unwrap().remove(0);
    let remaining_name = survivor
        .handle
        .trim_start_matches('@')
        .split('@')
        .next()
        .unwrap();
    let logged = verified(&db, remaining_name, None).await;
    let local_id = format!("l_{}", &Uuid::new_v4().simple().to_string()[..20]);
    let local =
        auth::set_local_credentials(&db, &logged.token, &local_id, "a strong fixture password")
            .await
            .unwrap();
    auth::unlink_account(&db, &local.token, survivor.id)
        .await
        .unwrap();
    assert!(db.linked_accounts(a.member.id).await.unwrap().is_empty());
    assert!(auth::get_session(&db, &local.token)
        .await
        .unwrap()
        .is_none());
    let back = auth::login(&db, &local_id, "a strong fixture password")
        .await
        .unwrap();
    assert_eq!(back.member.id, a.member.id);
    fixtures::delete_members(&db, &[a.member.id, b.member.id]).await;
    fixtures::delete_rate(&db, "login", &Sha256::digest(local_id.as_bytes())).await;
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn postgres_new_link_never_reauthenticates_and_existing_proof_recovers_password() {
    let db = fixtures::database().await;
    let name = format!("recover_{}", Uuid::new_v4().simple());
    let start = verified(&db, &name, None).await;
    let old = auth::get_session_details(&db, &start.token)
        .await
        .unwrap()
        .unwrap();
    let candidate = actor(&format!("{name}_new"));
    let nonce = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    let begun = flow::begin_challenge(&db, &candidate, &nonce, Some(&old))
        .await
        .unwrap();
    let pending = flow::get_pending(&db, begun.id, &nonce, &begun.code, Some(&old))
        .await
        .unwrap();
    let candidate_proof = proof(&candidate, &begun.code, pending.created_at).await;
    fixtures::age_session(&db, old.id).await;
    assert_eq!(
        flow::complete_challenge(
            &db,
            begun.id,
            &nonce,
            &begun.code,
            Some(&start.token),
            &candidate_proof
        )
        .await
        .unwrap_err(),
        FlowError::Auth(AuthError::FreshAuthenticationRequired)
    );
    assert_eq!(
        try_verified(&db, &format!("{name}_new"), Some(&start.token))
            .await
            .unwrap_err(),
        FlowError::Auth(AuthError::FreshAuthenticationRequired)
    );
    assert_eq!(
        auth::set_local_credentials(&db, &start.token, "unreachable", "fixture password long")
            .await
            .unwrap_err(),
        AuthError::FreshAuthenticationRequired
    );
    assert_eq!(
        auth::withdraw(&db, &start.token, "탈퇴").await,
        Err(AuthError::FreshAuthenticationRequired)
    );
    let fresh = verified(&db, &name, Some(&start.token)).await;
    let local_id = format!("r_{}", &Uuid::new_v4().simple().to_string()[..20]);
    let local =
        auth::set_local_credentials(&db, &fresh.token, &local_id, "initial fixture password")
            .await
            .unwrap();
    let password_login = auth::login(&db, &local_id, "initial fixture password")
        .await
        .unwrap();
    assert_eq!(
        auth::recover_password(&db, &password_login.token, "replacement fixture password")
            .await
            .unwrap_err(),
        AuthError::FederatedAuthenticationRequired
    );
    let added_again = verified(&db, &format!("{name}_another"), Some(&password_login.token)).await;
    let rotated = auth::get_session_details(&db, &added_again.token)
        .await
        .unwrap()
        .unwrap();
    assert!(rotated.federated_authenticated_at.is_none());
    assert_eq!(
        auth::recover_password(&db, &added_again.token, "replacement fixture password")
            .await
            .unwrap_err(),
        AuthError::FederatedAuthenticationRequired
    );
    let proof_login = verified(&db, &name, Some(&added_again.token)).await;
    let recovered = auth::recover_password(&db, &proof_login.token, "replacement fixture password")
        .await
        .unwrap();
    assert!(auth::get_session(&db, &local.token)
        .await
        .unwrap()
        .is_none());
    assert!(auth::get_session(&db, &proof_login.token)
        .await
        .unwrap()
        .is_none());
    assert_eq!(
        auth::login(&db, &local_id, "initial fixture password")
            .await
            .unwrap_err(),
        AuthError::InvalidCredentials
    );
    assert_eq!(
        auth::login(&db, &local_id, "replacement fixture password")
            .await
            .unwrap()
            .member
            .id,
        start.member.id
    );
    assert_eq!(
        auth::recover_password(&db, &recovered.token, "replacement fixture password")
            .await
            .unwrap_err(),
        AuthError::FederatedAuthenticationRequired
    );
    fixtures::delete_members(&db, &[start.member.id]).await;
    fixtures::delete_challenges(&db, &[&actor(&format!("{name}_new")).handle]).await;
    fixtures::delete_rate(&db, "login", &Sha256::digest(local_id.as_bytes())).await;
    fixtures::delete_rate(
        &db,
        "password_change",
        &Sha256::digest(start.member.id.as_bytes()),
    )
    .await;
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn postgres_creation_date_retry_is_owner_scoped_conditional_and_bounded() {
    let db = fixtures::database().await;
    let name = format!("age_retry_{}", Uuid::new_v4().simple());
    let owner = verified(&db, &name, None).await;
    let owner_session = auth::get_session_details(&db, &owner.token)
        .await
        .unwrap()
        .unwrap();
    let target = db
        .creation_date_targets(&owner_session, 4)
        .await
        .unwrap()
        .pop()
        .expect("fixture actor has no AP published date");
    let date = Utc::now() - chrono::Duration::days(365);
    assert!(db
        .persist_creation_date_if_missing(
            &owner_session,
            target.id,
            &target.actor_url,
            &target.handle,
            date,
        )
        .await
        .unwrap());
    let linked = db.linked_accounts(owner.member.id).await.unwrap();
    assert_eq!(
        linked[0]
            .account_created_at
            .map(|stored| stored.timestamp_micros()),
        Some(date.timestamp_micros())
    );
    let proof_again = verified(&db, &name, Some(&owner.token)).await;
    let linked_after_proof = db.linked_accounts(owner.member.id).await.unwrap();
    assert_eq!(
        linked_after_proof[0]
            .account_created_at
            .map(|stored| stored.timestamp_micros()),
        Some(date.timestamp_micros())
    );
    assert_eq!(proof_again.member.id, owner.member.id);
    assert!(matches!(
        db.creation_date_targets(&owner_session, 4).await,
        Err(AuthError::Unauthenticated)
    ));
    let owner_session = auth::get_session_details(&db, &proof_again.token)
        .await
        .unwrap()
        .unwrap();
    assert!(db
        .creation_date_targets(&owner_session, 4)
        .await
        .unwrap()
        .is_empty());
    assert!(!db
        .persist_creation_date_if_missing(
            &owner_session,
            target.id,
            &target.actor_url,
            &target.handle,
            date,
        )
        .await
        .unwrap());

    let foreign = fixtures::member(&db).await;
    let foreign_session = auth::get_session_details(&db, &foreign.token)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        db.persist_creation_date_if_missing(
            &foreign_session,
            target.id,
            &target.actor_url,
            &target.handle,
            date,
        )
        .await
        .unwrap_err(),
        AuthError::AccountNotFound
    );

    let second = verified(&db, &format!("{name}_second"), Some(&proof_again.token)).await;
    let second_session = auth::get_session_details(&db, &second.token)
        .await
        .unwrap()
        .unwrap();
    let stale = db
        .creation_date_targets(&second_session, 4)
        .await
        .unwrap()
        .pop()
        .unwrap();
    assert_eq!(
        db.persist_creation_date_if_missing(
            &second_session,
            stale.id,
            "https://changed.example/users/other",
            &stale.handle,
            date,
        )
        .await
        .unwrap_err(),
        AuthError::AccountNotFound
    );
    fixtures::delete_members(&db, &[owner.member.id, foreign.member.id]).await;
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn postgres_creation_date_retry_scope_is_migrated_and_limit_survives_reconnect() {
    let db = fixtures::database().await;
    let key = Uuid::new_v4();
    for _ in 0..4 {
        auth::consume_attempt(&db, "account_age_retry", key.as_bytes(), 4)
            .await
            .unwrap();
    }
    let reopened = fixtures::database().await;
    assert_eq!(
        auth::consume_attempt(&reopened, "account_age_retry", key.as_bytes(), 4).await,
        Err(AuthError::RateLimited)
    );
    fixtures::delete_rate(&db, "account_age_retry", &Sha256::digest(key.as_bytes())).await;
}

#[derive(QueryableByName)]
struct Check {
    #[diesel(sql_type = diesel::sql_types::Bool)]
    valid: bool,
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn postgres_withdraw_preserves_threads_and_detaches_legacy_relationships() {
    let db = fixtures::database().await;
    let name = format!("withdraw_{}", Uuid::new_v4().simple());
    let member = verified(&db, &name, None).await;
    let other = fixtures::member(&db).await;
    let site = Uuid::new_v4();
    let comment = Uuid::new_v4();
    let reply = Uuid::new_v4();
    let report = Uuid::new_v4();
    let mut conn = db.pool.get().await.unwrap();
    use diesel::sql_types::{Text, Uuid as SqlUuid};
    diesel::sql_query("INSERT INTO directory_sites(id,domain) VALUES($1,$2)")
        .bind::<SqlUuid, _>(site)
        .bind::<Text, _>(format!("{site}.example"))
        .execute(&mut conn)
        .await
        .unwrap();
    diesel::sql_query("INSERT INTO legacy_sites(id,domain,is_force_hidden,admin_user_id,admin_verified_via,inserted_at,updated_at) VALUES($1,$2,false,$3,'dns',now(),now())").bind::<SqlUuid,_>(site).bind::<Text,_>(format!("{site}.example")).bind::<SqlUuid,_>(member.member.id).execute(&mut conn).await.unwrap();
    diesel::sql_query("INSERT INTO legacy_members(id,fediverse_handle,fediverse_domain,inserted_at,updated_at) VALUES($1,$2,'social.example.com',now(),now())").bind::<SqlUuid,_>(member.member.id).bind::<Text,_>(&name).execute(&mut conn).await.unwrap();
    diesel::sql_query("INSERT INTO member_legacy_claims(member_id,handle) VALUES($1,$2)")
        .bind::<SqlUuid, _>(member.member.id)
        .bind::<Text, _>(&name)
        .execute(&mut conn)
        .await
        .unwrap();
    diesel::sql_query("INSERT INTO community_comments(id,user_id,server_id,body,is_deleted,inserted_at,updated_at) VALUES($1,$2,$3,'retained fixture comment',false,now(),now())").bind::<SqlUuid,_>(comment).bind::<SqlUuid,_>(member.member.id).bind::<SqlUuid,_>(site).execute(&mut conn).await.unwrap();
    diesel::sql_query("INSERT INTO community_comments(id,user_id,server_id,parent_id,body,is_deleted,inserted_at,updated_at) VALUES($1,$2,$3,$4,'other member reply',false,now(),now())").bind::<SqlUuid,_>(reply).bind::<SqlUuid,_>(other.member.id).bind::<SqlUuid,_>(site).bind::<SqlUuid,_>(comment).execute(&mut conn).await.unwrap();
    diesel::sql_query("INSERT INTO community_reports(id,reporter_id,resolved_by_id,comment_id,reason,status,inserted_at,updated_at) VALUES($1,$2,$2,$3,'other','resolved',now(),now())").bind::<SqlUuid,_>(report).bind::<SqlUuid,_>(member.member.id).bind::<SqlUuid,_>(comment).execute(&mut conn).await.unwrap();
    assert_eq!(
        auth::withdraw(&db, &member.token, "yes").await,
        Err(AuthError::ConfirmationRequired)
    );
    auth::withdraw(&db, &member.token, "탈퇴").await.unwrap();
    assert!(auth::get_session(&db, &member.token)
        .await
        .unwrap()
        .is_none());
    assert!(db
        .linked_accounts(member.member.id)
        .await
        .unwrap()
        .is_empty());
    let checked = diesel::sql_query("SELECT (SELECT is_deleted AND user_id IS NULL AND body='retained fixture comment' FROM community_comments WHERE id=$1) AND (SELECT NOT is_deleted AND user_id=$2 AND parent_id=$1 FROM community_comments WHERE id=$3) AND (SELECT reporter_id IS NULL AND resolved_by_id IS NULL FROM community_reports WHERE id=$4) AND (SELECT admin_user_id IS NULL AND admin_verified_via IS NULL FROM legacy_sites WHERE id=$5) AND NOT EXISTS(SELECT 1 FROM legacy_members WHERE id=$6) AND NOT EXISTS(SELECT 1 FROM member_legacy_claims WHERE member_id=$6) AS valid")
        .bind::<SqlUuid,_>(comment).bind::<SqlUuid,_>(other.member.id).bind::<SqlUuid,_>(reply).bind::<SqlUuid,_>(report).bind::<SqlUuid,_>(site).bind::<SqlUuid,_>(member.member.id).get_result::<Check>(&mut conn).await.unwrap();
    assert!(checked.valid);
    assert!(auth::get_session(&db, &other.token)
        .await
        .unwrap()
        .is_some());
    for (sql, id) in [
        ("DELETE FROM community_reports WHERE id=$1", report),
        ("DELETE FROM community_comments WHERE server_id=$1", site),
        ("DELETE FROM legacy_sites WHERE id=$1", site),
        ("DELETE FROM directory_sites WHERE id=$1", site),
    ] {
        diesel::sql_query(sql)
            .bind::<SqlUuid, _>(id)
            .execute(&mut conn)
            .await
            .unwrap();
    }
    drop(conn);
    fixtures::delete_members(&db, &[other.member.id]).await;
}
