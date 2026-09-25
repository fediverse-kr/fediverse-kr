use super::*;
use crate::backend::{auth, db::fixtures, legacy_assets::tests::Files};
use diesel_async::SimpleAsyncConnection;

struct Fixture {
    db: Database,
    files: Files,
    members: Vec<auth::SessionGrant>,
    keys: Vec<String>,
    objects: Vec<ObjectRef>,
}
impl Fixture {
    async fn new() -> Self {
        let db = fixtures::database().await;
        let files = Files::new();
        let a = fixtures::member(&db).await;
        let b = fixtures::member(&db).await;
        let id = a.member.id;
        let keys = vec![
            format!("avatars/{id}.png"),
            format!("emojis/{id}/shared.png"),
            format!("software/{id}.png"),
        ];
        let mut objects = vec![];
        let mut conn = db.pool.get().await.unwrap();
        for (i, key) in keys.iter().enumerate() {
            // Emoji and software share bytes but not a logical key. UUID bytes
            // prevent unrelated tests in this database sharing the same hash.
            files.write(
                key,
                format!(
                    "synthetic-{id}-{}",
                    if i == 0 { "avatar" } else { "shared" }
                )
                .as_bytes(),
            );
            let object = files.paths().read(key).await.unwrap();
            files.paths().put(&object).await.unwrap();
            diesel::sql_query("INSERT INTO stored_files VALUES($1,$2,$3)")
                .bind::<Text, _>(key)
                .bind::<Text, _>(&object.hash)
                .bind::<BigInt, _>(object.bytes.len() as i64)
                .execute(&mut conn)
                .await
                .unwrap();
            objects.push(ObjectRef {
                hash: object.hash,
                bytes: object.bytes.len() as i64,
            });
        }
        for member in [&a, &b] {
            diesel::sql_query("INSERT INTO legacy_members(id,fediverse_handle,fediverse_domain,avatar_key,emojis,inserted_at,updated_at)
                VALUES($1,$2,'cleanup.example.org',$3,$4::jsonb,now(),now())")
                .bind::<SqlUuid,_>(member.member.id).bind::<Text,_>(format!("{}@cleanup.example.org", member.member.id))
                .bind::<diesel::sql_types::Nullable<Text>,_>(if member.member.id==id {Some(&keys[0])} else {None})
                .bind::<Text,_>(serde_json::json!({"shared":keys[1]}).to_string())
                .execute(&mut conn).await.unwrap();
        }
        drop(conn);
        Self {
            db,
            files,
            members: vec![a, b],
            keys,
            objects,
        }
    }
    fn store(&self) -> ObjectStore {
        ObjectStore::open(&self.files.bundle).unwrap()
    }
    async fn flag(&self, query: &str, value: &str) -> bool {
        diesel::sql_query(query)
            .bind::<Text, _>(value)
            .get_result::<Flag>(&mut self.db.pool.get().await.unwrap())
            .await
            .unwrap()
            .value
    }
    async fn mapped(&self, key: &str) -> bool {
        self.flag(
            "SELECT EXISTS(SELECT 1 FROM stored_files WHERE object_key=$1) AS value",
            key,
        )
        .await
    }
    async fn queued(&self, object: &ObjectRef) -> bool {
        self.flag(
            "SELECT EXISTS(SELECT 1 FROM media_deletions WHERE sha256=$1) AS value",
            &object.hash,
        )
        .await
    }
    async fn sql(&self, query: &str, value: &str) {
        diesel::sql_query(query)
            .bind::<Text, _>(value)
            .execute(&mut self.db.pool.get().await.unwrap())
            .await
            .unwrap();
    }
    async fn retire(&self, keys: &[String]) {
        (&mut *self.db.pool.get().await.unwrap())
            .transaction::<_, StoreError, _>(async |conn| retire_keys(conn, keys).await)
            .await
            .unwrap();
    }
    async fn dispose(self) {
        let mut conn = self.db.pool.get().await.unwrap();
        for member in &self.members {
            diesel::sql_query("DELETE FROM legacy_members WHERE id=$1")
                .bind::<SqlUuid, _>(member.member.id)
                .execute(&mut conn)
                .await
                .unwrap();
        }
        diesel::sql_query("DELETE FROM stored_files WHERE object_key=ANY($1)")
            .bind::<Array<Text>, _>(&self.keys)
            .execute(&mut conn)
            .await
            .unwrap();
        for object in &self.objects {
            diesel::sql_query("DELETE FROM media_deletions WHERE sha256=$1")
                .bind::<Text, _>(&object.hash)
                .execute(&mut conn)
                .await
                .unwrap();
        }
        drop(conn);
        fixtures::delete_members(
            &self.db,
            &self.members.iter().map(|m| m.member.id).collect::<Vec<_>>(),
        )
        .await;
    }
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn withdrawal_retains_shared_keys_and_shared_content_until_last_reference() {
    let f = Fixture::new().await;
    let store = f.store();
    auth::withdraw(&f.db, &f.members[0].token, "탈퇴")
        .await
        .unwrap();
    assert!(!f.mapped(&f.keys[0]).await);
    assert!(f.mapped(&f.keys[1]).await);
    assert!(f.queued(&f.objects[0]).await);
    assert!(!f.queued(&f.objects[1]).await);
    assert!(
        store.read(&f.objects[0]).await.is_ok(),
        "HTTP transaction never deletes payloads"
    );
    f.db.collect_retired_media(&store).await.unwrap();
    assert!(matches!(
        store.read(&f.objects[0]).await,
        Err(storage::Error::Missing)
    ));
    auth::withdraw(&f.db, &f.members[1].token, "탈퇴")
        .await
        .unwrap();
    assert!(!f.mapped(&f.keys[1]).await);
    assert!(f.queued(&f.objects[1]).await);
    f.db.collect_retired_media(&store).await.unwrap();
    assert!(
        store.read(&f.objects[1]).await.is_ok(),
        "software uses these bytes"
    );
    f.retire(&[f.keys[2].clone()]).await;
    f.db.collect_retired_media(&store).await.unwrap();
    assert!(matches!(
        store.read(&f.objects[1]).await,
        Err(storage::Error::Missing)
    ));
    assert_eq!(f.files.objects(), 0);
    for key in &f.keys {
        assert!(f.files.source.join(key).is_file());
    }
    f.dispose().await;
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn withdrawal_rollback_keeps_members_mappings_and_files_together() {
    let f = Fixture::new().await;
    let mut conn = f.db.pool.get().await.unwrap();
    let result = (&mut *conn)
        .transaction::<(), StoreError, _>(async |conn| {
            let keys = member_keys(conn, f.members[0].member.id).await?;
            diesel::sql_query("DELETE FROM legacy_members WHERE id=$1")
                .bind::<SqlUuid, _>(f.members[0].member.id)
                .execute(conn)
                .await?;
            retire_keys(conn, &keys).await?;
            Err(StoreError)
        })
        .await;
    assert!(result.is_err());
    assert!(f.mapped(&f.keys[0]).await);
    assert!(!f.queued(&f.objects[0]).await);
    assert!(f.store().read(&f.objects[0]).await.is_ok());
    drop(conn);
    f.dispose().await;
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn deletion_failure_is_durable_backed_off_and_recovers_on_new_connection() {
    let f = Fixture::new().await;
    auth::withdraw(&f.db, &f.members[0].token, "탈퇴")
        .await
        .unwrap();
    f.db.collect_retired_media_with(|_| async { Err(storage::Error::Io) })
        .await
        .unwrap();
    assert!(f.flag("SELECT attempts=1 AND available_at>now()+interval '50 seconds' AS value FROM media_deletions WHERE sha256=$1",&f.objects[0].hash).await);
    assert!(!f
        .db
        .collect_retired_media_with(|_| async { panic!("backoff must not call storage") })
        .await
        .unwrap());
    f.sql(
        "UPDATE media_deletions SET available_at=now()-interval '1 second' WHERE sha256=$1",
        &f.objects[0].hash,
    )
    .await;
    let reopened = Database::connect(&std::env::var("FEDKR_TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    reopened.collect_retired_media(&f.store()).await.unwrap();
    assert!(!f.queued(&f.objects[0]).await);
    assert!(matches!(
        f.store().read(&f.objects[0]).await,
        Err(storage::Error::Missing)
    ));
    drop(reopened);
    f.dispose().await;
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn restored_mapping_prevents_deletion_and_mapping_writes_wait_for_cleanup() {
    let f = Fixture::new().await;
    auth::withdraw(&f.db, &f.members[0].token, "탈퇴")
        .await
        .unwrap();
    let object = &f.objects[0];
    let mut conn = f.db.pool.get().await.unwrap();
    diesel::sql_query("INSERT INTO stored_files VALUES($1,$2,$3)")
        .bind::<Text, _>(&f.keys[0])
        .bind::<Text, _>(&object.hash)
        .bind::<BigInt, _>(object.bytes)
        .execute(&mut conn)
        .await
        .unwrap();
    f.db.collect_retired_media_with(|_| async { panic!("referenced content must not be deleted") })
        .await
        .unwrap();
    assert!(!f.queued(object).await);
    f.retire(&[f.keys[0].clone()]).await;
    f.db.collect_retired_media_with(|_| async {
        conn.batch_execute("BEGIN; SET LOCAL lock_timeout='100ms'")
            .await
            .unwrap();
        let write = diesel::sql_query("INSERT INTO stored_files VALUES($1,$2,$3)")
            .bind::<Text, _>(&f.keys[0])
            .bind::<Text, _>(&object.hash)
            .bind::<BigInt, _>(object.bytes)
            .execute(&mut conn)
            .await;
        conn.batch_execute("ROLLBACK").await.unwrap();
        assert!(
            write.is_err(),
            "metadata publication cannot race physical deletion"
        );
        Err(storage::Error::Io)
    })
    .await
    .unwrap();
    drop(conn);
    f.dispose().await;
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn interrupted_acknowledgement_retries_missing_file_without_losing_work() {
    let f = Fixture::new().await;
    auth::withdraw(&f.db, &f.members[0].token, "탈퇴")
        .await
        .unwrap();
    let db = f.db.clone();
    let store = f.store();
    let (done, ready) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(async move {
        db.collect_retired_media_with(|object| async move {
            store.delete_unreferenced(&object).await.unwrap();
            done.send(()).unwrap();
            std::future::pending::<Result<(), storage::Error>>().await
        })
        .await
    });
    tokio::time::timeout(std::time::Duration::from_secs(5), ready)
        .await
        .unwrap()
        .unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert!(f.queued(&f.objects[0]).await);
    assert!(matches!(
        f.store().read(&f.objects[0]).await,
        Err(storage::Error::Missing)
    ));
    f.db.collect_retired_media(&f.store()).await.unwrap();
    assert!(!f.queued(&f.objects[0]).await);
    f.dispose().await;
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn concurrent_withdrawal_retires_last_shared_key_once() {
    let f = Fixture::new().await;
    let (a, b) = tokio::join!(
        auth::withdraw(&f.db, &f.members[0].token, "탈퇴"),
        auth::withdraw(&f.db, &f.members[1].token, "탈퇴")
    );
    assert!(a.is_ok() && b.is_ok());
    assert!(!f.mapped(&f.keys[0]).await && !f.mapped(&f.keys[1]).await);
    assert!(f.queued(&f.objects[0]).await && f.queued(&f.objects[1]).await);
    f.dispose().await;
}
