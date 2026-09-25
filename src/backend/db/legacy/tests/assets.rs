use super::*;
use crate::backend::{
    db::legacy::assets::preserve_legacy_assets,
    legacy_assets::{tests::Files, Error as AssetError},
};

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn previous_bundle_projection_upgrade_keeps_quarantine_and_revalidates() {
    let f = Fixture::new().await;
    let files = Files::new();
    files.fixture();
    f.run(ImportMode::Apply).await.unwrap();
    preserve_legacy_assets(&f.target, &files.paths(), true)
        .await
        .unwrap();
    // Reconstruct only the missing v18 projection in this generated rehearsal DB.
    // The v17 ledger and quarantined business rows remain unchanged.
    f.target_sql("DROP TABLE stored_files; DELETE FROM __diesel_schema_migrations WHERE version='202609140018'").await;
    crate::backend::db::migrations::run(&f.target)
        .await
        .unwrap();
    assert_eq!(f.target_count("stored_files").await, 4);
    assert_eq!(
        preserve_legacy_assets(&f.target, &files.paths(), false)
            .await
            .unwrap()
            .outcome,
        "already_preserved_verified"
    );
    assert!(ensure_runtime_allowed(&f.target, None).await.is_err());
    assert_eq!(
        f.run(ImportMode::Apply).await.unwrap().outcome,
        "already_imported_verified"
    );
    f.dispose().await;
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn bundle_dry_run_apply_concurrent_repeat_and_import_revalidation() {
    let f = Fixture::new().await;
    let files = Files::new();
    files.fixture();
    let paths = files.paths();
    let result = async {
        assert!(matches!(
            preserve_legacy_assets(&f.source, &paths, true).await,
            Err(AssetError::Configuration)
        ));
        assert!(matches!(
            preserve_legacy_assets(&f.target, &paths, true).await,
            Err(AssetError::Inventory)
        ));
        f.run(ImportMode::DryRun).await?;
        assert!(matches!(
            preserve_legacy_assets(&f.target, &paths, true).await,
            Err(AssetError::Inventory)
        ));
        f.run(ImportMode::Apply).await?;
        let dry = preserve_legacy_assets(&f.target, &paths, false)
            .await
            .unwrap();
        assert_eq!((dry.references, dry.objects, dry.copied_objects), (4, 4, 0));
        assert_eq!(files.objects(), 0);
        assert_eq!(f.target_count("legacy_asset_files").await, 0);
        assert_eq!(f.target_count("stored_files").await, 0);
        let (a, b) = tokio::join!(
            preserve_legacy_assets(&f.target, &paths, true),
            preserve_legacy_assets(&f.target, &paths, true)
        );
        let a = a.unwrap();
        let b = b.unwrap();
        assert_eq!(a.copied_objects + b.copied_objects, 3);
        assert_eq!(a.bytes, b.bytes);
        assert_eq!(
            a.bytes,
            2 * b"same synthetic bytes".len() as i64
                + b"emoji bytes".len() as i64
                + b"<svg>raw preserved bytes, not public</svg>".len() as i64
        );
        assert_eq!(files.objects(), 3);
        assert_eq!(f.target_count("legacy_asset_files").await, 4);
        assert_eq!(f.target_count("stored_files").await, 4);
        assert_eq!(f.target_count("legacy_asset_manifest").await, 1);
        assert!(!a.runtime_enabled && !b.runtime_enabled);
        assert_eq!(
            preserve_legacy_assets(&f.target, &paths, false)
                .await
                .unwrap()
                .outcome,
            "already_preserved_verified"
        );
        assert_eq!(
            f.run(ImportMode::Apply).await?.outcome,
            "already_imported_verified"
        );
        assert_eq!(
            f.run(ImportMode::DryRun).await?.external_asset_references,
            4
        );
        for name in [
            "member_sessions",
            "member_linked_accounts",
            "directory_jobs",
        ] {
            assert_eq!(f.target_count(name).await, 0);
        }
        assert!(ensure_runtime_allowed(&f.target, None).await.is_err());
        let serialized = serde_json::to_string(&a).unwrap();
        for secret in ["fixture", "social.example.com", "avatars/", "<svg>"] {
            assert!(!serialized.contains(secret));
        }
        Ok::<_, Error>(())
    }
    .await;
    f.dispose().await;
    assert!(result.is_ok(), "{result:?}");
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn missing_file_rolls_back_index_but_retry_reuses_exact_orphan_bytes() {
    let f = Fixture::new().await;
    let files = Files::new();
    let paths = files.paths();
    f.run(ImportMode::Apply).await.unwrap();
    files.write("avatars/fixture.png", b"same synthetic bytes");
    let first = preserve_legacy_assets(&f.target, &paths, true).await;
    assert!(matches!(first, Err(AssetError::Missing)));
    assert_eq!(files.objects(), 1);
    assert_eq!(f.target_count("legacy_asset_files").await, 0);
    assert_eq!(f.target_count("stored_files").await, 0);
    assert_eq!(f.target_count("legacy_asset_manifest").await, 0);
    files.fixture();
    let retry = preserve_legacy_assets(&f.target, &paths, true)
        .await
        .unwrap();
    assert_eq!(retry.copied_objects, 2);
    assert_eq!(files.objects(), 3);
    assert_eq!(f.target_count("legacy_asset_files").await, 4);
    assert!(ensure_runtime_allowed(&f.target, None).await.is_err());
    f.dispose().await;
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn altered_source_bundle_manifest_and_retained_rows_are_not_overwritten() {
    let f = Fixture::new().await;
    let files = Files::new();
    files.fixture();
    let paths = files.paths();
    f.run(ImportMode::Apply).await.unwrap();
    preserve_legacy_assets(&f.target, &paths, true)
        .await
        .unwrap();
    files.write("avatars/fixture.png", b"changed");
    assert!(matches!(
        preserve_legacy_assets(&f.target, &paths, true).await,
        Err(AssetError::Changed)
    ));
    files.fixture();
    let object = paths.read("avatars/fixture.png").await.unwrap();
    let path = files.bundle.join("objects").join(&object.hash);
    std::fs::write(&path, b"corrupt").unwrap();
    assert!(matches!(
        preserve_legacy_assets(&f.target, &paths, false).await,
        Err(AssetError::Changed)
    ));
    assert!(matches!(
        preserve_legacy_assets(&f.target, &paths, true).await,
        Err(AssetError::Changed)
    ));
    assert_eq!(std::fs::read(&path).unwrap(), b"corrupt");
    std::fs::write(&path, &object.bytes).unwrap();
    std::fs::remove_file(&path).unwrap();
    assert!(matches!(
        preserve_legacy_assets(&f.target, &paths, true).await,
        Err(AssetError::Missing)
    ));
    assert!(!path.exists());
    std::fs::write(&path, &object.bytes).unwrap();
    f.target_sql("UPDATE legacy_asset_files SET sha256=repeat('0',64) WHERE object_key='avatars/fixture.png'").await;
    assert!(matches!(
        preserve_legacy_assets(&f.target, &paths, true).await,
        Err(AssetError::Inventory)
    ));
    let mut conn = connect(&f.target).await.unwrap();
    diesel::sql_query(
        "UPDATE legacy_asset_files SET sha256=$1 WHERE object_key='avatars/fixture.png'",
    )
    .bind::<Text, _>(&object.hash)
    .execute(&mut conn)
    .await
    .unwrap();
    drop(conn);
    f.target_sql(
        "UPDATE stored_files SET byte_count=byte_count+1 WHERE object_key='avatars/fixture.png'",
    )
    .await;
    assert!(matches!(
        preserve_legacy_assets(&f.target, &paths, true).await,
        Err(AssetError::Inventory)
    ));
    assert!(matches!(
        f.run(ImportMode::Apply).await,
        Err(Error::Mismatch)
    ));
    f.target_sql(
        "UPDATE stored_files SET byte_count=byte_count-1 WHERE object_key='avatars/fixture.png'",
    )
    .await;
    assert_eq!(
        preserve_legacy_assets(&f.target, &paths, false)
            .await
            .unwrap()
            .outcome,
        "already_preserved_verified"
    );
    f.target_sql("UPDATE legacy_asset_manifest SET byte_count=byte_count+1")
        .await;
    assert!(matches!(
        preserve_legacy_assets(&f.target, &paths, true).await,
        Err(AssetError::Inventory)
    ));
    assert!(matches!(
        f.run(ImportMode::DryRun).await,
        Err(Error::Mismatch)
    ));
    f.target_sql("UPDATE legacy_asset_manifest SET byte_count=byte_count-1; UPDATE legacy_members SET avatar_key='avatars/other.png' WHERE avatar_key IS NOT NULL").await;
    assert!(matches!(
        preserve_legacy_assets(&f.target, &paths, true).await,
        Err(AssetError::Inventory)
    ));
    assert_eq!(f.target_count("legacy_asset_files").await, 4);
    f.dispose().await;
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn malformed_emoji_storage_shapes_are_rejected_without_partial_import() {
    let f = Fixture::new().await;
    for sql in [
        "UPDATE users SET emojis='[]'::jsonb",
        "UPDATE users SET emojis='{\"smile\":{\"url\":\"https://example.com/a\"}}'::jsonb",
        "UPDATE users SET emojis='{\"smile\":null}'::jsonb",
    ] {
        f.source_sql(sql).await;
        assert!(matches!(f.run(ImportMode::Apply).await, Err(Error::Data)));
        assert_eq!(f.target_count("legacy_import_state").await, 0);
        assert_eq!(f.target_count("legacy_members").await, 0);
    }
    f.dispose().await;
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn unsafe_empty_and_oversized_keys_never_create_a_manifest() {
    for (query,expected) in [
        ("UPDATE users SET avatar_key='avatars/../secret' WHERE avatar_key IS NOT NULL",AssetError::UnsafePath),
        ("UPDATE users SET emojis=jsonb_build_object('large','emojis/'||repeat('x',1024)) WHERE avatar_key IS NOT NULL",AssetError::UnsafePath),
        ("UPDATE users SET avatar_key='' WHERE avatar_key IS NOT NULL",AssetError::UnsafePath),
    ] {
        let f=Fixture::new().await;let files=Files::new();files.fixture();
        f.source_sql(query).await;f.run(ImportMode::Apply).await.unwrap();
        let result=preserve_legacy_assets(&f.target,&files.paths(),true).await;
        assert!(matches!(result,Err(e) if e==expected));
        assert_eq!(f.target_count("legacy_asset_manifest").await,0);f.dispose().await;
    }
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn all_reference_pages_are_preserved_and_empty_inventory_is_explicit() {
    let f = Fixture::new().await;
    let files = Files::new();
    files.fixture();
    f.source_sql("UPDATE users SET emojis=(SELECT jsonb_object_agg('e'||n,'emojis/social.example.com/'||lpad(n::text,3,'0')||'.png') FROM generate_series(1,280) n) WHERE avatar_key IS NOT NULL").await;
    for n in 1..=280 {
        files.write(
            &format!("emojis/social.example.com/{n:03}.png"),
            b"shared emoji bytes",
        );
    }
    f.run(ImportMode::Apply).await.unwrap();
    let report = preserve_legacy_assets(&f.target, &files.paths(), true)
        .await
        .unwrap();
    assert_eq!((report.references, report.objects), (283, 283));
    assert_eq!(report.copied_objects, 3);
    assert_eq!(f.target_count("legacy_asset_files").await, 283);
    assert_eq!(f.target_count("stored_files").await, 283);
    f.run(ImportMode::Apply).await.unwrap();
    f.dispose().await;
    let f = Fixture::new().await;
    let empty = Files::new();
    f.source_sql("UPDATE users SET avatar_key=NULL,emojis='null'::jsonb; UPDATE servers SET favicon_key=NULL; UPDATE softwares SET logo_key=NULL").await;
    f.run(ImportMode::Apply).await.unwrap();
    let report = preserve_legacy_assets(&f.target, &empty.paths(), true)
        .await
        .unwrap();
    assert_eq!(
        (
            report.references,
            report.objects,
            report.bytes,
            report.copied_objects
        ),
        (0, 0, 0, 0)
    );
    assert_eq!(f.target_count("legacy_asset_manifest").await, 1);
    f.run(ImportMode::DryRun).await.unwrap();
    f.dispose().await;
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn case_sensitive_storage_keys_are_never_silently_merged() {
    let f = Fixture::new().await;
    let files = Files::new();
    files.fixture();
    files.write("avatars/FIXTURE.png", b"distinct original object");
    f.source_sql("UPDATE users SET emojis='{\"a\":\"avatars/fixture.png\",\"b\":\"avatars/FIXTURE.png\"}'::jsonb WHERE avatar_key IS NOT NULL").await;
    f.run(ImportMode::Apply).await.unwrap();
    let result = preserve_legacy_assets(&f.target, &files.paths(), true).await;
    if cfg!(windows) {
        assert!(matches!(result, Err(AssetError::Inventory)));
        assert_eq!(f.target_count("legacy_asset_manifest").await, 0);
    } else {
        assert_eq!(result.unwrap().objects, 4);
        assert_eq!(f.target_count("legacy_asset_files").await, 4);
    }
    f.dispose().await;
}
