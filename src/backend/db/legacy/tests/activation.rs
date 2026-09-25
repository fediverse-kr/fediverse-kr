use super::super::activation::activate_legacy;
use super::*;
use crate::backend::{
    activation::{Error as ActivationError, Request},
    legacy_assets::tests::Files,
    storage::ObjectStore,
};
const ORIGIN: &str = "https://cutover.example.org";

impl Fixture {
    async fn rename_for_activation(&mut self, live: bool) {
        let name = if live {
            self.target_name.replace("fedkr_rehearsal_", "fedkr_live_")
        } else {
            self.target_name.replace("fedkr_live_", "fedkr_rehearsal_")
        };
        assert!(name.starts_with(if live {
            "fedkr_live_"
        } else {
            "fedkr_rehearsal_"
        }));
        self.admin
            .batch_execute(&format!(
                "ALTER DATABASE {} RENAME TO {name}",
                self.target_name
            ))
            .await
            .unwrap();
        self.target_name = name.clone();
        let mut url = url::Url::parse(&self.target).unwrap();
        url.set_path(&name);
        self.target = url.to_string();
    }
    fn activation(&self, files: &Files, apply: bool) -> Request {
        Request {
            source: self.source.clone(),
            target: self.target.clone(),
            origin: ORIGIN.into(),
            paths: files.paths(),
            apply,
        }
    }
    async fn ready_for_activation(&mut self, files: &Files) {
        files.fixture();
        self.run(ImportMode::Apply).await.unwrap();
        super::super::assets::preserve_legacy_assets(&self.target, &files.paths(), true)
            .await
            .unwrap();
        self.rename_for_activation(true).await;
    }
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn reviewed_copy_requires_apply_bound_origin_and_actual_store_before_runtime() {
    let mut f = Fixture::new().await;
    let files = Files::new();
    f.ready_for_activation(&files).await;
    assert!(ensure_runtime_allowed(&f.target, Some(ORIGIN))
        .await
        .is_err());
    let dry = activate_legacy(&f.activation(&files, false)).await.unwrap();
    assert!(dry.validated_now && !dry.runtime_enabled);
    assert_eq!(dry.asset_objects, 4);
    assert_eq!(f.target_count("legacy_runtime_activation").await, 0);
    let applied = activate_legacy(&f.activation(&files, true)).await.unwrap();
    assert_eq!(applied.outcome, "activated");
    assert!(applied.runtime_enabled && applied.validated_now);
    assert!(ensure_runtime_allowed(&f.target, Some(ORIGIN))
        .await
        .is_ok());
    assert!(
        ensure_runtime_allowed(&f.target, Some("https://wrong.example.org"))
            .await
            .is_err()
    );
    assert!(ensure_runtime_allowed(&f.target, None).await.is_err());
    assert!(ensure_runtime_allowed(&f.source, Some(ORIGIN))
        .await
        .is_err());
    assert_eq!(f.target_count("legacy_import_state").await, 1);
    let db = Database::connect(&f.target).await.unwrap();
    assert!(db.verify_activated_store(None).await.is_err());
    let store = ObjectStore::open(&files.bundle).unwrap();
    db.verify_activated_store(Some(&store)).await.unwrap();
    assert!(db
        .verify_activated_store(Some(&ObjectStore::open(&files.source).unwrap()))
        .await
        .is_err());
    f.target_sql("DELETE FROM stored_files; INSERT INTO media_deletions(sha256,byte_count) SELECT sha256,byte_count FROM legacy_asset_files LIMIT 1").await;
    assert!(
        db.verify_activated_store(None).await.is_err(),
        "pending deletion needs its store even with no live mappings"
    );
    db.verify_activated_store(Some(&store)).await.unwrap();
    f.target_sql("DELETE FROM media_deletions; INSERT INTO stored_files SELECT object_key,sha256,byte_count FROM legacy_asset_files").await;
    let object = files.paths().read("avatars/fixture.png").await.unwrap();
    std::fs::write(files.bundle.join("objects").join(object.hash), b"changed").unwrap();
    assert!(db.verify_activated_store(Some(&store)).await.is_err());
    drop(db);
    f.dispose().await;
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn concurrent_apply_is_idempotent_and_never_replays_over_live_writes() {
    let mut f = Fixture::new().await;
    let files = Files::new();
    f.ready_for_activation(&files).await;
    let request = f.activation(&files, true);
    let (a, b) = tokio::join!(activate_legacy(&request), activate_legacy(&request));
    let results = [a.unwrap(), b.unwrap()];
    assert_eq!(
        results.iter().filter(|r| r.outcome == "activated").count(),
        1
    );
    assert_eq!(
        results
            .iter()
            .filter(|r| r.outcome == "already_activated" && !r.validated_now)
            .count(),
        1
    );
    f.target_sql("UPDATE member_users SET display_name='live edit'")
        .await;
    let retry = activate_legacy(&f.activation(&files, true)).await.unwrap();
    assert_eq!(retry.outcome, "already_activated");
    assert!(!retry.validated_now);
    let mut wrong = f.activation(&files, true);
    wrong.origin = "https://wrong.example.org".into();
    assert!(matches!(
        activate_legacy(&wrong).await,
        Err(ActivationError::Conflict)
    ));
    assert_eq!(f.target_count("legacy_runtime_activation").await, 1);
    f.rename_for_activation(false).await;
    assert!(ensure_runtime_allowed(&f.target, Some(ORIGIN))
        .await
        .is_err());
    assert!(matches!(
        f.run(ImportMode::Apply).await,
        Err(Error::TargetNotEmpty)
    ));
    assert!(
        super::super::assets::preserve_legacy_assets(&f.target, &files.paths(), true)
            .await
            .is_err()
    );
    f.dispose().await;
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn changed_source_projection_or_file_after_preflight_never_activates() {
    let mut f = Fixture::new().await;
    let files = Files::new();
    f.ready_for_activation(&files).await;
    activate_legacy(&f.activation(&files, false)).await.unwrap();
    f.source_sql(
        "UPDATE servers SET user_count=124 WHERE id='10000000-0000-0000-0000-000000000001'",
    )
    .await;
    assert!(matches!(
        activate_legacy(&f.activation(&files, true)).await,
        Err(ActivationError::Inventory)
    ));
    f.source_sql(
        "UPDATE servers SET user_count=123 WHERE id='10000000-0000-0000-0000-000000000001'",
    )
    .await;
    f.target_sql("UPDATE directory_sites SET is_hidden=false WHERE id='10000000-0000-0000-0000-000000000001'").await;
    assert!(activate_legacy(&f.activation(&files, true)).await.is_err());
    f.target_sql(
        "UPDATE directory_sites SET is_hidden=true WHERE id='10000000-0000-0000-0000-000000000001'",
    )
    .await;
    files.write("avatars/fixture.png", b"changed source bytes");
    assert!(matches!(
        activate_legacy(&f.activation(&files, true)).await,
        Err(ActivationError::Files)
    ));
    assert_eq!(f.target_count("legacy_runtime_activation").await, 0);
    assert!(ensure_runtime_allowed(&f.target, Some(ORIGIN))
        .await
        .is_err());
    f.dispose().await;
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn missing_manifest_is_not_treated_as_an_empty_inventory() {
    let mut f = Fixture::new().await;
    let files = Files::new();
    files.fixture();
    f.run(ImportMode::Apply).await.unwrap();
    f.rename_for_activation(true).await;
    assert!(matches!(
        activate_legacy(&f.activation(&files, true)).await,
        Err(ActivationError::Files)
    ));
    assert_eq!(f.target_count("legacy_runtime_activation").await, 0);
    f.target_sql("DELETE FROM legacy_import_state").await;
    assert!(ensure_runtime_allowed(&f.target, Some(ORIGIN))
        .await
        .is_err());
    f.dispose().await;
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn empty_live_named_database_cannot_accidentally_start_as_a_fresh_site() {
    let mut f = Fixture::new().await;
    let files = Files::new();
    f.rename_for_activation(true).await;
    assert!(ensure_runtime_allowed(&f.target, Some(ORIGIN))
        .await
        .is_err());
    assert!(matches!(
        activate_legacy(&f.activation(&files, true)).await,
        Err(ActivationError::Inventory)
    ));
    assert_eq!(
        tables(&mut connect(&f.target).await.unwrap())
            .await
            .unwrap()
            .len(),
        0
    );
    f.dispose().await;
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn ordinary_writes_are_locked_out_during_final_verification() {
    let mut f = Fixture::new().await;
    let files = Files::new();
    f.ready_for_activation(&files).await;
    let mut blocker = connect(&f.target).await.unwrap();
    blocker
        .batch_execute("BEGIN; LOCK TABLE legacy_runtime_activation IN SHARE MODE;")
        .await
        .unwrap();
    let request = f.activation(&files, true);
    let task = tokio::spawn(async move { activate_legacy(&request).await });
    let mut probe = connect(&f.target).await.unwrap();
    // Wait for the actual target lock, not an assumed duration. The last table
    // is held by blocker, so approval cannot commit until this check finishes.
    tokio::time::timeout(std::time::Duration::from_secs(10),async {
        loop {
            if flag(&mut probe,"SELECT EXISTS(SELECT 1 FROM pg_locks WHERE relation='public.member_users'::regclass AND mode='ShareRowExclusiveLock' AND granted AND pid<>pg_backend_pid()) AS value").await.unwrap() {break;}
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }).await.unwrap();
    assert!(ensure_runtime_allowed(&f.target, Some(ORIGIN))
        .await
        .is_err());
    probe
        .batch_execute("SET lock_timeout='50ms'")
        .await
        .unwrap();
    assert!(probe
        .batch_execute("UPDATE member_users SET display_name='racing test edit'")
        .await
        .is_err());
    blocker.batch_execute("ROLLBACK").await.unwrap();
    drop(blocker);
    assert_eq!(task.await.unwrap().unwrap().outcome, "activated");
    assert!(flag(
        &mut probe,
        "SELECT bool_and(display_name<>'racing test edit') AS value FROM member_users"
    )
    .await
    .unwrap());
    drop(probe);
    f.dispose().await;
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn explicitly_verified_empty_inventory_can_start_without_store() {
    let mut f = Fixture::new().await;
    let files = Files::new();
    f.source_sql("UPDATE users SET avatar_key=NULL,emojis=NULL; UPDATE servers SET favicon_key=NULL; UPDATE softwares SET logo_key=NULL;").await;
    f.ready_for_activation(&files).await;
    let report = activate_legacy(&f.activation(&files, true)).await.unwrap();
    assert_eq!(report.asset_objects, 0);
    let db = Database::connect(&f.target).await.unwrap();
    db.verify_activated_store(None).await.unwrap();
    drop(db);
    f.dispose().await;
}
