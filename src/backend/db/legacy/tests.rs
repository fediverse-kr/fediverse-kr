use super::*;
use rand::{rngs::StdRng, SeedableRng};
use rsa::{
    pkcs1::EncodeRsaPrivateKey,
    pkcs8::{EncodePublicKey, LineEnding},
    RsaPrivateKey,
};
use std::sync::OnceLock;
mod activation;
mod assets;

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn profile_batch_in_quarantine_is_detected_not_erased() {
    let f = Fixture::new().await;
    f.run(ImportMode::Apply).await.unwrap();
    f.target_sql("INSERT INTO profile_refresh_batches(id,actor_id,state) SELECT gen_random_uuid(),id,'running' FROM member_users LIMIT 1; INSERT INTO profile_refresh_jobs(batch_id,member_id) SELECT b.id,u.id FROM profile_refresh_batches b CROSS JOIN member_users u").await;
    let result = f.run(ImportMode::Apply).await;
    assert_eq!(f.target_count("profile_refresh_batches").await, 1);
    assert!(f.target_count("profile_refresh_jobs").await > 0);
    f.dispose().await;
    assert!(matches!(result, Err(Error::Mismatch)));
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn profile_media_in_quarantine_is_detected_not_erased() {
    let f = Fixture::new().await;
    f.run(ImportMode::Apply).await.unwrap();
    f.target_sql("INSERT INTO member_profile_media(member_id) SELECT id FROM member_users LIMIT 1")
        .await;
    let result = f.run(ImportMode::Apply).await;
    assert_eq!(f.target_count("member_profile_media").await, 1);
    f.dispose().await;
    assert!(matches!(result, Err(Error::Mismatch)));
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn pending_media_cleanup_in_quarantine_is_detected_not_erased() {
    let f = Fixture::new().await;
    f.run(ImportMode::Apply).await.unwrap();
    f.target_sql("INSERT INTO media_deletions(sha256,byte_count) VALUES(repeat('a',64),1)")
        .await;
    let result = f.run(ImportMode::Apply).await;
    assert_eq!(f.target_count("media_deletions").await, 1);
    f.dispose().await;
    assert!(matches!(result, Err(Error::Mismatch)));
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn registration_attribution_in_quarantine_is_detected_not_erased() {
    let f = Fixture::new().await;
    f.run(ImportMode::Apply).await.unwrap();
    f.target_sql("INSERT INTO directory_site_registrations(site_id,member_id) SELECT s.id,m.id FROM directory_sites s CROSS JOIN member_users m LIMIT 1").await;
    let result = f.run(ImportMode::Apply).await;
    assert_eq!(f.target_count("directory_site_registrations").await, 1);
    f.dispose().await;
    assert!(matches!(result, Err(Error::Mismatch)));
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn administrator_state_in_quarantine_cannot_be_reset_by_reimport() {
    let f = Fixture::new().await;
    f.run(ImportMode::Apply).await.unwrap();
    f.target_sql("INSERT INTO member_admin_roles(member_id) SELECT id FROM member_users LIMIT 1")
        .await;
    assert!(matches!(
        f.run(ImportMode::Apply).await,
        Err(Error::Mismatch)
    ));
    assert_eq!(f.target_count("member_admin_roles").await, 1);
    f.target_sql("DELETE FROM member_admin_roles").await;
    f.target_sql("INSERT INTO moderation_events(id,target_kind,target_id,action,before_state,after_state,note) SELECT gen_random_uuid(),'admin_role',id,'grant','member','admin','fixture command' FROM member_users LIMIT 1").await;
    assert!(matches!(
        f.run(ImportMode::Apply).await,
        Err(Error::Mismatch)
    ));
    assert_eq!(f.target_count("moderation_events").await, 1);
    assert!(ensure_runtime_allowed(&f.target, None).await.is_err());
    f.dispose().await;
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn site_admin_history_in_quarantine_cannot_be_erased_by_reimport() {
    let f = Fixture::new().await;
    f.run(ImportMode::Apply).await.unwrap();
    f.target_sql("INSERT INTO directory_site_edits(id,site_id,action,revision,previous,admin_note,admin_change) SELECT gen_random_uuid(),id,'admin_edit',0,'{}'::jsonb,'fixture audit','{\"field\":\"hidden\",\"before\":\"off\",\"after\":\"on\",\"truncated\":false}'::jsonb FROM directory_sites LIMIT 1").await;
    let result = f.run(ImportMode::Apply).await;
    assert_eq!(f.target_count("directory_site_edits").await, 1);
    assert!(ensure_runtime_allowed(&f.target, None).await.is_err());
    f.dispose().await;
    assert!(matches!(result, Err(Error::Mismatch)));
}

struct Fixture {
    source: String,
    target: String,
    source_name: String,
    target_name: String,
    admin: AsyncPgConnection,
}
impl Fixture {
    async fn new() -> Self {
        let base =
            std::env::var("FEDKR_TEST_DATABASE_URL").expect("isolated test database required");
        assert!(crate::backend::settings::local_database(&base) && base.ends_with("/fedkr_test"));
        let mut admin = connect(&base).await.unwrap();
        let suffix = uuid::Uuid::new_v4().simple().to_string();
        let source_name = format!("fedkr_snapshot_{suffix}");
        let target_name = format!("fedkr_rehearsal_{suffix}");
        // Only these two freshly generated exact names are ever dropped.
        admin
            .batch_execute(&format!("CREATE DATABASE {source_name}"))
            .await
            .unwrap();
        admin
            .batch_execute(&format!("CREATE DATABASE {target_name}"))
            .await
            .unwrap();
        let mut u = url::Url::parse(&base).unwrap();
        u.set_path(&source_name);
        let source = u.to_string();
        u.set_path(&target_name);
        let target = u.to_string();
        let mut conn = connect(&source).await.unwrap();
        conn.batch_execute(include_str!("fixtures.sql"))
            .await
            .unwrap();
        static KEYS: OnceLock<(String, String)> = OnceLock::new();
        let (private, public) = KEYS.get_or_init(|| {
            let mut rng = StdRng::seed_from_u64(20260913);
            let key = RsaPrivateKey::new(&mut rng, 2048).unwrap();
            (
                format!("{}\n", key.to_pkcs1_pem(LineEnding::LF).unwrap().as_str()),
                format!(
                    "{}\n",
                    key.to_public_key()
                        .to_public_key_pem(LineEnding::LF)
                        .unwrap()
                ),
            )
        });
        diesel::sql_query("INSERT INTO instance_keys VALUES('60000000-0000-0000-0000-000000000001',$1,$2,'2025-01-01','2025-01-01')")
            .bind::<Text,_>(public).bind::<Text,_>(private).execute(&mut conn).await.unwrap();
        Self {
            source,
            target,
            source_name,
            target_name,
            admin,
        }
    }
    async fn run(&self, mode: ImportMode) -> Result<ImportReport, Error> {
        import_snapshot(&self.source, &self.target, mode).await
    }
    async fn source_sql(&self, sql: &str) {
        connect(&self.source)
            .await
            .unwrap()
            .batch_execute(sql)
            .await
            .unwrap();
    }
    async fn target_sql(&self, sql: &str) {
        connect(&self.target)
            .await
            .unwrap()
            .batch_execute(sql)
            .await
            .unwrap();
    }
    async fn target_count(&self, table: &str) -> i64 {
        count(&mut connect(&self.target).await.unwrap(), table)
            .await
            .unwrap()
    }
    async fn dispose(mut self) {
        self.admin
            .batch_execute(&format!("DROP DATABASE {} WITH (FORCE)", self.source_name))
            .await
            .unwrap();
        self.admin
            .batch_execute(&format!("DROP DATABASE {} WITH (FORCE)", self.target_name))
            .await
            .unwrap();
    }
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL; creates disposable fixture databases"]
async fn dry_run_apply_repeat_preserve_data_and_never_start_runtime() {
    let f = Fixture::new().await;
    let result = async {
        let report=f.run(ImportMode::DryRun).await?;
        assert_eq!(report.outcome,"dry_run_rolled_back");
        assert_eq!(report.reserved_members,2);
        assert_eq!(report.display_names_adapted,2);
        assert_eq!(report.retained_signing_keys,1);
        assert_eq!(report.external_asset_references,4);
        assert!(!report.runtime_enabled);
        assert!(report.tables.iter().any(|r| r.table=="health_checks" && r.rows==280));
        for m in TABLES { assert_eq!(f.target_count(m.target).await,0); }
        assert_eq!(f.target_count("member_users").await,0);
        assert_eq!(f.target_count("legacy_import_state").await,0);
        assert_eq!(f.run(ImportMode::Apply).await?.outcome,"applied_quarantined");
        assert_eq!(f.target_count("member_users").await,2);
        assert_eq!(f.target_count("directory_sites").await,3);
        assert_eq!(f.target_count("directory_site_details").await,3);
        assert_eq!(f.target_count("directory_observations").await,2);
        assert_eq!(f.target_count("directory_health_checks").await,280);
        for name in ["member_sessions","member_linked_accounts","directory_jobs"] { assert_eq!(f.target_count(name).await,0); }
        assert_eq!(f.run(ImportMode::Apply).await?.outcome,"already_imported_verified");
        assert_eq!(f.run(ImportMode::DryRun).await?.outcome,"already_imported_verified");
        assert!(ensure_runtime_allowed(&f.source, None).await.is_err());
        assert!(ensure_runtime_allowed(&f.target, None).await.is_err());
        // Exercise the database marker, not just the spelling of the URL path.
        assert!(ensure_runtime_allowed(&f.target.replace("fedkr_rehearsal_","fedkr_%72ehearsal_"), None).await.is_err());
        assert!(ensure_runtime_allowed(&f.source.replace("fedkr_snapshot_","fedkr_%73napshot_"), None).await.is_err());
        let mut conn=connect(&f.target).await.unwrap();
        assert!(flag(&mut conn,"SELECT NOT EXISTS(SELECT 1 FROM legacy_sites s JOIN directory_site_details d ON d.site_id=s.id WHERE d.owner_id IS DISTINCT FROM s.admin_user_id OR d.owner_method IS DISTINCT FROM s.admin_verified_via OR d.rules IS DISTINCT FROM s.rules OR d.tags IS DISTINCT FROM s.tags OR d.invite_only IS DISTINCT FROM s.invite_only OR d.approval_required IS DISTINCT FROM s.approval_required OR d.owner_comment IS DISTINCT FROM s.admin_comment) AS value").await?);
        assert!(handle_reserved(&mut conn,"@ALICE@SOCIAL.EXAMPLE.COM").await.unwrap());
        assert!(!handle_reserved(&mut conn,"@unknown@social.example.com").await.unwrap());
        assert!(flag(&mut conn,"SELECT (SELECT is_banned FROM member_users WHERE id='00000000-0000-0000-0000-000000000002') AND (SELECT char_length(display_name)=70 FROM legacy_members WHERE id='00000000-0000-0000-0000-000000000001') AND (SELECT user_id IS NULL AND is_deleted FROM community_comments WHERE id='20000000-0000-0000-0000-000000000001') AND (SELECT admin_user_id='00000000-0000-0000-0000-000000000001'::uuid AND is_hidden AND invite_only FROM legacy_sites WHERE id='10000000-0000-0000-0000-000000000001') AS value").await?);
        let json=serde_json::to_string(&report).unwrap();
        for secret in ["alice","social.example.com","PRIVATE KEY","비공개","DO-NOT-IMPORT"] { assert!(!json.contains(secret)); }
        let mut source=connect(&f.source).await.unwrap();
        assert_eq!(count(&mut source,"verifications").await?,1);
        assert_eq!(count(&mut source,"oban_jobs").await?,1);
        assert!(!flag(&mut source,"SELECT to_regclass('member_users') IS NOT NULL AS value").await?);
        Ok::<_,Error>(())
    }.await;
    f.dispose().await;
    assert!(result.is_ok(), "{result:?}");
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL; creates disposable fixture databases"]
async fn health_history_reads_preserved_phoenix_checks_without_recollecting() {
    let f = Fixture::new().await;
    f.run(ImportMode::Apply).await.unwrap();
    let db = Database::connect(&f.target).await.unwrap();
    assert!(db
        .public_health_history("social.example.com")
        .await
        .unwrap()
        .is_none());
    // Only this disposable projection is made public. The source and startup
    // quarantine remain intact; no HTTP worker or remote request is started.
    f.target_sql("UPDATE directory_sites SET is_hidden=false WHERE id='10000000-0000-0000-0000-000000000001'").await;
    let history = db
        .public_health_history("social.example.com")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(history.checks.len(), 48);
    assert_eq!(
        history.checks.as_slice().first().unwrap().response_ms,
        Some(273)
    );
    assert_eq!(
        history.checks.as_slice().last().unwrap().response_ms,
        Some(320)
    );
    assert_eq!(
        history.checks.as_slice().last().unwrap().checked_at_kst,
        "2026-09-12 13:40:00"
    );
    assert_eq!(f.target_count("directory_health_checks").await, 280);
    assert_eq!(f.target_count("directory_jobs").await, 0);
    assert!(ensure_runtime_allowed(&f.target, None).await.is_err());
    drop(db);
    f.dispose().await;
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn imported_member_public_proof_retains_uuid_and_quarantine_still_blocks_runtime() {
    use crate::backend::{
        auth, flow,
        flow::tests::{actor, proof},
    };
    let f = Fixture::new().await;
    let result=async {
        f.run(ImportMode::Apply).await?;
        // This is a repository-level fixture test, not a runtime bypass exposed
        // by HTTP. The quarantined database must still fail the startup guard.
        let db=Database::connect(&f.target).await.unwrap();
        let old_id=uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();
        let a=actor("alice");
        let nonce=format!("{}{}",uuid::Uuid::new_v4().simple(),uuid::Uuid::new_v4().simple());
        let c=flow::begin_challenge(&db,&a,&nonce,None).await.unwrap();
        let pending=flow::get_pending(&db,c.id,&nonce,&c.code,None).await.unwrap();
        let p=proof(&a,&c.code,pending.created_at).await;
        let grant=flow::complete_challenge(&db,c.id,&nonce,&c.code,None,&p).await.unwrap();
        assert_eq!(grant.member.id,old_id);
        assert_eq!(grant.member.display_name,"가".repeat(64));
        assert_eq!(f.target_count("member_users").await,2);
        let session=auth::get_session_details(&db,&grant.token).await.unwrap().unwrap();
        let comments=db.own_comments(&session,0).await.unwrap();
        assert_eq!(comments.comments.len(),1);
        assert_eq!(comments.comments[0].id,"20000000-0000-0000-0000-000000000002");
        assert_eq!(comments.comments[0].body,"부모 댓글");
        assert_eq!(comments.comments[0].domain.as_deref(),Some("social.example.com"));
        assert!(!comments.comments[0].server_available); // Private author history, not a hidden public page.
        let sites=db.owned_sites(&session,0).await.unwrap();
        assert_eq!(sites.sites.len(),1);assert_eq!(sites.sites[0].domain,"social.example.com");
        let mut target=connect(&f.target).await.unwrap();
        assert!(flag(&mut target,"SELECT (SELECT user_id='00000000-0000-0000-0000-000000000001'::uuid FROM community_comments WHERE id='20000000-0000-0000-0000-000000000002') AND (SELECT char_length(display_name)=70 FROM legacy_members WHERE id='00000000-0000-0000-0000-000000000001') AND (SELECT actor_url='https://social.example.com/users/alice' AND claimed_at IS NOT NULL FROM member_legacy_claims WHERE member_id='00000000-0000-0000-0000-000000000001') AND (SELECT detail='비공개 신고 내용' FROM community_reports LIMIT 1) AS value").await?);
        // The new binding is live state, not a changed copy of the old user.
        // Re-import must not reset it, delete new sessions, or undo the login.
        assert!(matches!(f.run(ImportMode::Apply).await,Err(Error::Mismatch)));
        assert_eq!(f.target_count("member_linked_accounts").await,1);
        assert!(ensure_runtime_allowed(&f.target, None).await.is_err());
        let mut source=connect(&f.source).await.unwrap();
        assert!(flag(&mut source,"SELECT (SELECT char_length(display_name)=70 FROM users WHERE id='00000000-0000-0000-0000-000000000001') AND to_regclass('member_legacy_claims') IS NULL AS value").await?);
        Ok::<_,Error>(())
    }.await;
    f.dispose().await;
    assert!(result.is_ok(), "{result:?}");
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn a_pinned_identity_alone_makes_quarantined_reimport_refuse_without_reset() {
    let f = Fixture::new().await;
    f.run(ImportMode::Apply).await.unwrap();
    f.target_sql("UPDATE member_legacy_claims SET actor_url='https://social.example.com/users/alice',claimed_at=now() WHERE handle='@alice@social.example.com'").await;
    let result = f.run(ImportMode::Apply).await;
    let mut conn = connect(&f.target).await.unwrap();
    let retained=flag(&mut conn,"SELECT EXISTS(SELECT 1 FROM member_legacy_claims WHERE actor_url IS NOT NULL AND claimed_at IS NOT NULL) AS value").await.unwrap();
    drop(conn);
    f.dispose().await;
    assert!(matches!(result, Err(Error::Mismatch)));
    assert!(retained);
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn schema_drift_or_unknown_table_is_rejected_without_rows() {
    let f = Fixture::new().await;
    f.source_sql("ALTER TABLE users ADD COLUMN unexpected_profile_field text")
        .await;
    let r = f.run(ImportMode::Apply).await;
    assert_eq!(f.target_count("legacy_members").await, 0);
    assert_eq!(f.target_count("legacy_import_state").await, 0);
    f.source_sql("ALTER TABLE users DROP COLUMN unexpected_profile_field; CREATE TABLE unexpected_business_table(id bigint PRIMARY KEY)").await;
    let unknown = f.run(ImportMode::Apply).await;
    f.dispose().await;
    assert!(matches!(r, Err(Error::Schema)));
    assert!(matches!(unknown, Err(Error::Schema)));
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn catalog_edit_state_in_quarantine_is_detected_not_cleared() {
    let f = Fixture::new().await;
    f.run(ImportMode::Apply).await.unwrap();
    f.target_sql("INSERT INTO catalog_software_state(software_id,locked) SELECT id,true FROM catalog_software LIMIT 1").await;
    let result = f.run(ImportMode::Apply).await;
    let retained = f.target_count("catalog_software_state").await;
    f.dispose().await;
    assert!(matches!(result, Err(Error::Mismatch)));
    assert_eq!(retained, 1);
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn catalog_administration_in_quarantine_is_detected_not_cleared() {
    let f = Fixture::new().await;
    f.run(ImportMode::Apply).await.unwrap();
    f.target_sql("INSERT INTO catalog_category_state(category_id,revision) SELECT id,1 FROM catalog_categories LIMIT 1").await;
    let state = f.run(ImportMode::Apply).await;
    assert_eq!(f.target_count("catalog_category_state").await, 1);
    f.target_sql("DELETE FROM catalog_category_state; INSERT INTO catalog_admin_events(category_id,revision,action,note,before_value,after_value) SELECT id,1,'category_edit','private fixture','null','{}' FROM catalog_categories LIMIT 1").await;
    let events = f.run(ImportMode::Apply).await;
    assert_eq!(f.target_count("catalog_admin_events").await, 1);
    f.dispose().await;
    assert!(matches!(state, Err(Error::Mismatch)));
    assert!(matches!(events, Err(Error::Mismatch)));
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn community_evidence_in_quarantine_is_detected_not_cleared() {
    let f = Fixture::new().await;
    f.run(ImportMode::Apply).await.unwrap();
    f.target_sql("INSERT INTO community_report_evidence(report_id,body,author_name,comment_updated_at) SELECT r.id,c.body,'private fixture',c.updated_at FROM community_reports r JOIN community_comments c ON c.id=r.comment_id LIMIT 1").await;
    let result = f.run(ImportMode::Apply).await;
    let retained = f.target_count("community_report_evidence").await;
    f.dispose().await;
    assert!(matches!(result, Err(Error::Mismatch)));
    assert_eq!(retained, 1);
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn foreign_key_identity_and_signing_key_errors_rollback_all_rows() {
    let f = Fixture::new().await;
    f.source_sql("UPDATE servers SET admin_user_id='99999999-0000-0000-0000-000000000000' WHERE domain='social.example.com'").await;
    let fk = f.run(ImportMode::Apply).await;
    assert_eq!(f.target_count("member_users").await, 0);
    f.source_sql("UPDATE servers SET admin_user_id='00000000-0000-0000-0000-000000000001' WHERE domain='social.example.com'; UPDATE users SET fediverse_handle='ALICE@social.example.com' WHERE id='00000000-0000-0000-0000-000000000002'").await;
    let collision = f.run(ImportMode::Apply).await;
    assert_eq!(f.target_count("member_legacy_claims").await, 0);
    f.source_sql("UPDATE users SET fediverse_handle='banned@social.example.com' WHERE id='00000000-0000-0000-0000-000000000002'").await;
    let mut rng = StdRng::seed_from_u64(20260915);
    let mismatched_public = format!(
        "{}\n",
        RsaPrivateKey::new(&mut rng, 2048)
            .unwrap()
            .to_public_key()
            .to_public_key_pem(LineEnding::LF)
            .unwrap()
    );
    let mut source = connect(&f.source).await.unwrap();
    diesel::sql_query("UPDATE instance_keys SET public_key_pem=$1")
        .bind::<Text, _>(&mismatched_public)
        .execute(&mut source)
        .await
        .unwrap();
    drop(source);
    let key = f.run(ImportMode::Apply).await;
    assert_eq!(f.target_count("legacy_members").await, 0);
    assert_eq!(f.target_count("legacy_instance_keys").await, 0);
    f.dispose().await;
    assert!(matches!(fk, Err(Error::Data)));
    assert!(matches!(collision, Err(Error::Identity)));
    assert!(matches!(key, Err(Error::SigningKey)));
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn changed_snapshot_or_changed_destination_is_never_overwritten() {
    let f = Fixture::new().await;
    let first = f.run(ImportMode::Apply).await;
    assert!(first.is_ok(), "{first:?}");
    f.source_sql("UPDATE reports SET admin_note='CHANGED-PRIVATE-NOTE'")
        .await;
    let source_changed = f.run(ImportMode::Apply).await;
    f.source_sql("UPDATE reports SET admin_note='비공개 관리자 메모'")
        .await;
    f.target_sql(
        "UPDATE directory_sites SET name='changed after import' WHERE domain='social.example.com'",
    )
    .await;
    let target_changed = f.run(ImportMode::Apply).await;
    let mut target = connect(&f.target).await.unwrap();
    assert!(flag(&mut target,"SELECT name='changed after import' AS value FROM directory_sites WHERE domain='social.example.com'").await.unwrap());
    drop(target);
    f.dispose().await;
    assert!(matches!(source_changed, Err(Error::Mismatch)));
    assert!(matches!(target_changed, Err(Error::Mismatch)));
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn nonempty_destination_is_refused_and_retained() {
    let f = Fixture::new().await;
    Database::migrate(&f.target).await.unwrap();
    f.target_sql("INSERT INTO member_users(id,display_name) VALUES('99999999-0000-0000-0000-000000000000','existing fixture')").await;
    let result = f.run(ImportMode::Apply).await;
    assert_eq!(f.target_count("member_users").await, 1);
    assert_eq!(f.target_count("legacy_import_state").await, 0);
    f.dispose().await;
    assert!(matches!(result, Err(Error::TargetNotEmpty)));
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn concurrent_imports_serialize_and_changed_retained_rows_are_rejected() {
    let f = Fixture::new().await;
    let (a, b) = tokio::join!(f.run(ImportMode::Apply), f.run(ImportMode::Apply));
    let valid = matches!((&a,&b),(Ok(a),Ok(b)) if
        [a.outcome,b.outcome].contains(&"applied_quarantined") &&
        [a.outcome,b.outcome].contains(&"already_imported_verified"));
    assert!(valid, "concurrent fixture import: {a:?} / {b:?}");
    f.target_sql("UPDATE community_reports SET admin_note='changed retained fixture'")
        .await;
    let changed = f.run(ImportMode::Apply).await;
    assert_eq!(f.target_count("legacy_import_state").await, 1);
    f.dispose().await;
    assert!(matches!(changed, Err(Error::Mismatch)));
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn oversized_rows_are_rejected_not_skipped_or_logged() {
    let f = Fixture::new().await;
    f.source_sql(
        "UPDATE servers SET description=repeat('x',1048577) WHERE domain='social.example.com'",
    )
    .await;
    let result = f.run(ImportMode::Apply).await;
    assert_eq!(f.target_count("legacy_members").await, 0);
    assert_eq!(f.target_count("legacy_sites").await, 0);
    assert_eq!(f.target_count("legacy_import_state").await, 0);
    let mut conn = private_import_connection(&f.source).await.unwrap();
    assert!(flag(&mut conn,"SELECT current_setting('log_statement')='none' AND current_setting('log_min_error_statement')='panic' AND current_setting('log_parameter_max_length')='0' AND current_setting('log_parameter_max_length_on_error')='0' AS value").await.unwrap());
    drop(conn);
    f.dispose().await;
    assert!(matches!(result, Err(Error::Data)));
}
