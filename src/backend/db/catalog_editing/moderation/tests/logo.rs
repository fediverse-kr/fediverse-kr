use super::*;
use crate::backend::{
    catalog_logos,
    legacy_assets::tests::Files,
    media,
    storage::{ObjectRef, ObjectStore},
};
use base64::{engine::general_purpose::STANDARD, Engine};
use diesel_async::SimpleAsyncConnection;

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn software_media_import_supports_header_metadata_and_revision_cas() {
    let f = Fixture::new().await;
    let files = Files::new();
    let store = ObjectStore::open(&files.bundle).unwrap();
    let logo = payload(&format!("{}-logo", f.name));
    let header = payload(&format!("{}-header", f.name));
    let initial = f.item().await;
    let metadata = crate::directory::SoftwareMediaMetadata {
        source_url: Some("https://example.test/source".into()),
        attribution: "Example Project".into(),
        license: "CC BY 4.0".into(),
        license_url: Some("https://creativecommons.org/licenses/by/4.0/".into()),
        caption: "공식 소개 화면".into(),
        fit: crate::directory::MediaFit::Cover,
        background: Some("#112233".into()),
    };
    let saved = catalog_logos::change_media(
        &f.db,
        Some(&store),
        &f.admin,
        dto::MediaRequest {
            name: f.name.clone(),
            revision: initial.revision,
            role: dto::MediaRole::Header,
            note: "헤더 등록".into(),
            data: Some(STANDARD.encode(&header)),
            metadata: Some(metadata.clone()),
        },
    )
    .await
    .unwrap();
    assert_eq!(saved.revision, initial.revision + 1);
    assert_eq!(
        saved.header.as_ref().unwrap().metadata.caption,
        "공식 소개 화면"
    );
    assert!(matches!(
        catalog_logos::change_media(
            &f.db,
            Some(&store),
            &f.admin,
            dto::MediaRequest {
                name: f.name.clone(),
                revision: initial.revision,
                role: dto::MediaRole::Logo,
                note: "오래된 버전".into(),
                data: Some(STANDARD.encode(&logo)),
                metadata: Some(metadata.clone()),
            },
        )
        .await,
        Err(Error::Conflict)
    ));
    let saved = catalog_logos::change_media(
        &f.db,
        Some(&store),
        &f.admin,
        dto::MediaRequest {
            name: f.name.clone(),
            revision: saved.revision,
            role: dto::MediaRole::Logo,
            note: "로고 등록".into(),
            data: Some(STANDARD.encode(&logo)),
            metadata: None,
        },
    )
    .await
    .unwrap();
    assert!(saved.logo_available);
    assert_eq!(saved.revision, initial.revision + 2);
    assert!(media::read(
        &f.db,
        Some(&store),
        &media::Target::SoftwareHeader(f.name.clone()),
        None
    )
    .await
    .unwrap()
    .is_some());
    let header_replacement = payload(&format!("{}-header-replacement", f.name));
    let replaced = catalog_logos::change_media(
        &f.db,
        Some(&store),
        &f.admin,
        dto::MediaRequest {
            name: f.name.clone(),
            revision: saved.revision,
            role: dto::MediaRole::Header,
            note: "헤더 교체".into(),
            data: Some(STANDARD.encode(&header_replacement)),
            metadata: Some(crate::directory::SoftwareMediaMetadata {
                caption: "교체된 화면".into(),
                ..metadata
            }),
        },
    )
    .await
    .unwrap();
    assert_eq!(replaced.revision, saved.revision + 1);
    assert_eq!(
        media::read(
            &f.db,
            Some(&store),
            &media::Target::SoftwareHeader(f.name.clone()),
            None
        )
        .await
        .unwrap()
        .unwrap()
        .bytes,
        header_replacement
    );
    let metadata_update = crate::directory::SoftwareMediaMetadata {
        caption: "재업로드 없이 표시 변경".into(),
        fit: crate::directory::MediaFit::Contain,
        ..replaced.header.as_ref().unwrap().metadata.clone()
    };
    let updated = catalog_logos::change_media(
        &f.db,
        Some(&store),
        &f.admin,
        dto::MediaRequest {
            name: f.name.clone(),
            revision: replaced.revision,
            role: dto::MediaRole::Header,
            note: "표시 변경".into(),
            data: None,
            metadata: Some(metadata_update.clone()),
        },
    )
    .await
    .unwrap();
    assert_eq!(updated.revision, replaced.revision + 1);
    assert_eq!(updated.header.as_ref().unwrap().metadata, metadata_update);
    assert_eq!(
        media::read(
            &f.db,
            Some(&store),
            &media::Target::SoftwareHeader(f.name.clone()),
            None
        )
        .await
        .unwrap()
        .unwrap()
        .bytes,
        header_replacement
    );
    let replaced = updated;
    let removed = catalog_logos::change_media(
        &f.db,
        Some(&store),
        &f.admin,
        dto::MediaRequest {
            name: f.name.clone(),
            revision: replaced.revision,
            role: dto::MediaRole::Header,
            note: "헤더 제거".into(),
            data: None,
            metadata: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(removed.revision, replaced.revision + 1);
    assert!(media::read(
        &f.db,
        Some(&store),
        &media::Target::SoftwareHeader(f.name.clone()),
        None
    )
    .await
    .unwrap()
    .is_none());
    while f.db.collect_retired_media(&store).await.unwrap() {}
    assert!(store
        .read(&ObjectRef::from_bytes(&header).unwrap())
        .await
        .is_err());
    assert!(store
        .read(&ObjectRef::from_bytes(&header_replacement).unwrap())
        .await
        .is_err());
    assert!(media::read(
        &f.db,
        Some(&store),
        &media::Target::Software(f.name.clone()),
        None
    )
    .await
    .unwrap()
    .is_some());
    clean(
        f,
        &[
            ObjectRef::from_bytes(&logo).unwrap(),
            ObjectRef::from_bytes(&header).unwrap(),
            ObjectRef::from_bytes(&header_replacement).unwrap(),
        ],
    )
    .await;
}

fn payload(id: &str) -> Vec<u8> {
    format!("<svg xmlns='http://www.w3.org/2000/svg' width='32' height='32'><title>{id}</title><path d='M0 0h32v32z'/></svg>").into_bytes()
}
fn request(f: &Fixture, revision: i64, bytes: Option<&[u8]>) -> dto::LogoRequest {
    dto::LogoRequest {
        name: f.name.clone(),
        revision,
        note: "비공개 로고 교체 사유".into(),
        data: bytes.map(|b| STANDARD.encode(b)),
        metadata: None,
    }
}
async fn clean(f: Fixture, objects: &[ObjectRef]) {
    let mut conn = f.db.pool.get().await.unwrap();
    let hashes = objects.iter().map(|o| o.hash.clone()).collect::<Vec<_>>();
    diesel::sql_query("DELETE FROM stored_files WHERE sha256=ANY($1)")
        .bind::<Array<Text>, _>(&hashes)
        .execute(&mut conn)
        .await
        .unwrap();
    diesel::sql_query("DELETE FROM media_deletions WHERE sha256=ANY($1)")
        .bind::<Array<Text>, _>(&hashes)
        .execute(&mut conn)
        .await
        .unwrap();
    drop(conn);
    f.clean().await;
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn upload_replace_same_bytes_remove_and_shared_hash_survive() {
    let f = Fixture::new().await;
    let files = Files::new();
    let store = ObjectStore::open(&files.bundle).unwrap();
    let bytes = payload(&f.name);
    let a = ObjectRef::from_bytes(&bytes).unwrap();
    let target = media::Target::Software(f.name.clone());
    let initial = f.item().await;
    assert!(!initial.logo_available);
    let saved = catalog_logos::change(
        &f.db,
        Some(&store),
        &f.admin,
        request(&f, initial.revision, Some(&bytes)),
    )
    .await
    .unwrap();
    assert_eq!(saved.revision, initial.revision + 1);
    assert!(saved.logo_available);
    assert_eq!(
        media::read(&f.db, Some(&store), &target, None)
            .await
            .unwrap()
            .unwrap()
            .bytes,
        bytes
    );
    let same = catalog_logos::change(
        &f.db,
        Some(&store),
        &f.admin,
        request(&f, saved.revision, Some(&bytes)),
    )
    .await
    .unwrap();
    assert_eq!(same.revision, saved.revision);
    let no_retirement = diesel::sql_query(
        "SELECT NOT EXISTS(SELECT 1 FROM media_deletions WHERE sha256=$1) AS value",
    )
    .bind::<Text, _>(&a.hash)
    .get_result::<Flag>(&mut f.db.pool.get().await.unwrap())
    .await
    .unwrap()
    .value;
    assert!(no_retirement);
    let alias = format!("software-logos/{}-shared", f.name);
    diesel::sql_query("INSERT INTO stored_files VALUES($1,$2,$3)")
        .bind::<Text, _>(&alias)
        .bind::<Text, _>(&a.hash)
        .bind::<BigInt, _>(a.bytes)
        .execute(&mut f.db.pool.get().await.unwrap())
        .await
        .unwrap();
    let next = payload(&format!("{}-new", f.name));
    let b = ObjectRef::from_bytes(&next).unwrap();
    let saved = catalog_logos::change(
        &f.db,
        Some(&store),
        &f.admin,
        request(&f, same.revision, Some(&next)),
    )
    .await
    .unwrap();
    assert_eq!(
        media::read(&f.db, Some(&store), &target, None)
            .await
            .unwrap()
            .unwrap()
            .bytes,
        next
    );
    while f.db.collect_retired_media(&store).await.unwrap() {}
    assert_eq!(store.read(&a).await.unwrap(), bytes); // another key still uses it
    let removed = catalog_logos::change(
        &f.db,
        Some(&store),
        &f.admin,
        request(&f, saved.revision, None),
    )
    .await
    .unwrap();
    assert!(!removed.logo_available);
    assert!(media::read(&f.db, Some(&store), &target, None)
        .await
        .unwrap()
        .is_none());
    while f.db.collect_retired_media(&store).await.unwrap() {}
    assert!(matches!(
        store.read(&b).await,
        Err(crate::backend::storage::Error::Missing)
    ));
    assert_eq!(store.read(&a).await.unwrap(), bytes);
    let noop = catalog_logos::change(
        &f.db,
        Some(&store),
        &f.admin,
        request(&f, removed.revision, None),
    )
    .await
    .unwrap();
    assert_eq!(noop.revision, removed.revision);
    let private =
        f.db.moderation_catalog_history(&f.admin, &f.name, false, 0)
            .await
            .unwrap();
    assert_eq!(private.items.len(), 3);
    assert!(private.items.iter().all(|e| e.action == "logo"));
    let public = serde_json::to_string(&f.db.software_history(&f.name, 0).await.unwrap()).unwrap();
    for secret in [&a.hash, &b.hash, "비공개 로고", "software-logos/"] {
        assert!(!public.contains(secret));
    }
    clean(f, &[a, b]).await;
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn unauthorized_stale_and_invalid_requests_never_publish() {
    let f = Fixture::new().await;
    let files = Files::new();
    let store = ObjectStore::open(&files.bundle).unwrap();
    let bytes = payload(&f.name);
    let initial = f.item().await;
    assert!(matches!(
        catalog_logos::change(
            &f.db,
            Some(&store),
            &f.member,
            request(&f, initial.revision, Some(&bytes))
        )
        .await,
        Err(Error::Forbidden)
    ));
    assert!(matches!(
        catalog_logos::change(
            &f.db,
            Some(&store),
            &f.admin,
            request(&f, initial.revision - 1, Some(&bytes))
        )
        .await,
        Err(Error::Conflict)
    ));
    assert!(matches!(
        catalog_logos::change(
            &f.db,
            Some(&store),
            &f.admin,
            request(&f, initial.revision, Some(b"fake PNG"))
        )
        .await,
        Err(Error::Invalid)
    ));
    assert!(matches!(
        catalog_logos::change(
            &f.db,
            None,
            &f.admin,
            request(&f, initial.revision, Some(&bytes))
        )
        .await,
        Err(Error::Unavailable)
    ));
    assert_eq!(files.objects(), 0);
    assert_eq!(f.item().await.revision, initial.revision);
    f.clean().await;
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn rollback_after_payload_keeps_old_logo_and_durable_orphan_intent() {
    let f = Fixture::new().await;
    let files = Files::new();
    let store = ObjectStore::open(&files.bundle).unwrap();
    let bytes = payload(&f.name);
    let a = ObjectRef::from_bytes(&bytes).unwrap();
    let initial = f.item().await;
    let saved = catalog_logos::change(
        &f.db,
        Some(&store),
        &f.admin,
        request(&f, initial.revision, Some(&bytes)),
    )
    .await
    .unwrap();
    let mut fault = f.db.pool.get().await.unwrap();
    let trigger = format!("logo_failure_{}", Uuid::new_v4().simple());
    // Exact synthetic row only. Hold the owning connection until trigger cleanup.
    fault.batch_execute(&format!("CREATE FUNCTION pg_temp.{trigger}() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'synthetic logo transaction failure'; END $$;
        CREATE TRIGGER {trigger} BEFORE UPDATE OF logo_key ON catalog_software FOR EACH ROW WHEN (NEW.name='{}') EXECUTE FUNCTION pg_temp.{trigger}()", f.name)).await.unwrap();
    let next = payload(&format!("{}-rollback", f.name));
    let b = ObjectRef::from_bytes(&next).unwrap();
    assert!(matches!(
        catalog_logos::change(
            &f.db,
            Some(&store),
            &f.admin,
            request(&f, saved.revision, Some(&next))
        )
        .await,
        Err(Error::Unavailable)
    ));
    fault
        .batch_execute(&format!(
            "DROP TRIGGER {trigger} ON catalog_software; DROP FUNCTION pg_temp.{trigger}()"
        ))
        .await
        .unwrap();
    drop(fault);
    assert_eq!(f.item().await.revision, saved.revision);
    assert_eq!(
        media::read(
            &f.db,
            Some(&store),
            &media::Target::Software(f.name.clone()),
            None
        )
        .await
        .unwrap()
        .unwrap()
        .bytes,
        bytes
    );
    assert_eq!(store.read(&b).await.unwrap(), next);
    let queued =
        diesel::sql_query("SELECT EXISTS(SELECT 1 FROM media_deletions WHERE sha256=$1) AS value")
            .bind::<Text, _>(&b.hash)
            .get_result::<Flag>(&mut f.db.pool.get().await.unwrap())
            .await
            .unwrap()
            .value;
    assert!(queued);
    while f.db.collect_retired_media(&store).await.unwrap() {}
    assert!(store.read(&a).await.is_ok());
    assert!(matches!(
        store.read(&b).await,
        Err(crate::backend::storage::Error::Missing)
    ));
    clean(f, &[a, b]).await;
}
