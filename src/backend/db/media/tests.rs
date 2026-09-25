use super::*;
use crate::backend::{db::fixtures, legacy_assets::tests::Files, media, storage::ObjectStore};
use diesel::sql_types::{Binary, Uuid as SqlUuid};
use uuid::Uuid;
mod races;

struct Fixture {
    db: Database,
    member: auth::SessionGrant,
    other: auth::SessionGrant,
    files: Files,
    software: String,
    site: String,
    site_id: Uuid,
    keys: Vec<String>,
}
impl Fixture {
    async fn new() -> Self {
        let db = fixtures::database().await;
        let member = fixtures::member(&db).await;
        let other = fixtures::member(&db).await;
        let id = member.member.id;
        let files = Files::new();
        let software = format!("media-{id}");
        let site = format!("media-{id}.example.org");
        let site_id = db.add_site(&site).await.unwrap();
        let keys = vec![
            format!("avatars/{id}.svg"),
            format!("emojis/{id}/wave.png"),
            format!("emojis/{id}/blob-cat.@____.png"),
            format!("software-logos/{id}.svg"),
            format!("favicons/{id}.png"),
        ];
        let mut conn = db.pool.get().await.unwrap();
        for (n, key) in keys.iter().enumerate() {
            let bytes: &[u8] = if matches!(n, 0 | 3) {
                b"<svg xmlns='http://www.w3.org/2000/svg'><circle r='8'/></svg>"
            } else {
                b"\x89PNG\r\n\x1a\n"
            };
            files.write(key, bytes);
            let object = files.paths().read(key).await.unwrap();
            files.paths().put(&object).await.unwrap();
            diesel::sql_query(
                "INSERT INTO stored_files(object_key,sha256,byte_count) VALUES($1,$2,$3)",
            )
            .bind::<Text, _>(key)
            .bind::<Text, _>(&object.hash)
            .bind::<BigInt, _>(bytes.len() as i64)
            .execute(&mut conn)
            .await
            .unwrap();
        }
        let emoji = serde_json::json!({"wave":keys[1],"blob-cat.@/한국어":keys[2]}).to_string();
        diesel::sql_query("INSERT INTO legacy_members(id,fediverse_handle,fediverse_domain,display_name,avatar_key,emojis,inserted_at,updated_at) VALUES($1,$2,'media-fixture.example.org','Private legacy :wave:',$3,$4::jsonb,now() AT TIME ZONE 'UTC',now() AT TIME ZONE 'UTC')")
    .bind::<SqlUuid,_>(id).bind::<Text,_>(format!("@{id}@media-fixture.example.org")).bind::<Text,_>(&keys[0]).bind::<Text,_>(&emoji).execute(&mut conn).await.unwrap();
        let software_id = -((id.as_u128() & 0x0fff_ffff_ffff_ffff) as i64) - 1;
        diesel::sql_query("INSERT INTO catalog_software(id,name,display_name,logo_key,is_featured,display_order,inserted_at,updated_at) VALUES($1,$2,'Media fixture',$3,false,0,now() AT TIME ZONE 'UTC',now() AT TIME ZONE 'UTC')")
    .bind::<BigInt,_>(software_id).bind::<Text,_>(&software).bind::<Text,_>(&keys[3]).execute(&mut conn).await.unwrap();
        diesel::sql_query("INSERT INTO legacy_sites(id,domain,favicon_key,is_force_hidden,inserted_at,updated_at) VALUES($1,$2,$3,false,now() AT TIME ZONE 'UTC',now() AT TIME ZONE 'UTC')")
    .bind::<SqlUuid,_>(site_id).bind::<Text,_>(&site).bind::<Text,_>(&keys[4]).execute(&mut conn).await.unwrap();
        drop(conn);
        Self {
            db,
            member,
            other,
            files,
            software,
            site,
            site_id,
            keys,
        }
    }
    fn store(&self) -> ObjectStore {
        ObjectStore::open(&self.files.bundle).unwrap()
    }
    async fn sql(&self, sql: &str) {
        use diesel_async::SimpleAsyncConnection;
        self.db
            .pool
            .get()
            .await
            .unwrap()
            .batch_execute(sql)
            .await
            .unwrap();
    }
    async fn dispose(self) {
        let mut conn = self.db.pool.get().await.unwrap();
        diesel::sql_query("DELETE FROM legacy_members WHERE id=$1")
            .bind::<SqlUuid, _>(self.member.member.id)
            .execute(&mut conn)
            .await
            .unwrap();
        diesel::sql_query("DELETE FROM legacy_sites WHERE id=$1")
            .bind::<SqlUuid, _>(self.site_id)
            .execute(&mut conn)
            .await
            .unwrap();
        diesel::sql_query("DELETE FROM directory_sites WHERE id=$1")
            .bind::<SqlUuid, _>(self.site_id)
            .execute(&mut conn)
            .await
            .unwrap();
        diesel::sql_query("DELETE FROM catalog_software WHERE name=$1")
            .bind::<Text, _>(&self.software)
            .execute(&mut conn)
            .await
            .unwrap();
        diesel::sql_query("DELETE FROM stored_files WHERE object_key=ANY($1)")
            .bind::<Array<Text>, _>(&self.keys)
            .execute(&mut conn)
            .await
            .unwrap();
        drop(conn);
        fixtures::delete_members(&self.db, &[self.member.member.id, self.other.member.id]).await;
    }
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn public_logo_and_legacy_icon_use_current_entity_visibility_not_hash_urls() {
    let f = Fixture::new().await;
    let store = f.store();
    let logo = Target::Software(f.software.clone());
    let site = Target::Site(f.site.clone());
    assert_eq!(
        media::read(&f.db, Some(&store), &logo, None)
            .await
            .unwrap()
            .unwrap()
            .mime,
        "image/svg+xml"
    );
    assert_eq!(
        media::read(&f.db, Some(&store), &site, None)
            .await
            .unwrap()
            .unwrap()
            .mime,
        "image/png"
    );
    let detail = f.db.public_software(&f.software).await.unwrap();
    assert!(detail.software.as_ref().unwrap().logo_available);
    let serialized = serde_json::to_string(&detail).unwrap();
    assert!(!serialized.contains("software-logos/") && !serialized.contains("sha256"));
    let public = f.db.public_site(&f.site).await.unwrap().unwrap();
    assert!(public.icon_available);
    f.sql(&format!(
        "UPDATE directory_sites SET is_hidden=true WHERE id='{}'",
        f.site_id
    ))
    .await;
    assert!(media::read(&f.db, Some(&store), &site, None)
        .await
        .unwrap()
        .is_none());
    f.sql(&format!(
        "UPDATE directory_sites SET is_hidden=false,is_force_hidden=true WHERE id='{}'",
        f.site_id
    ))
    .await;
    assert!(media::read(&f.db, Some(&store), &site, None)
        .await
        .unwrap()
        .is_none());
    let hash = f.files.paths().read(&f.keys[0]).await.unwrap().hash;
    assert!(
        media::read(&f.db, Some(&store), &Target::Software(hash), None)
            .await
            .unwrap()
            .is_none()
    );
    f.dispose().await;
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn own_avatar_and_emoji_require_current_session_and_never_other_member_id() {
    let f = Fixture::new().await;
    let store = f.store();
    let own = Some(f.member.token.as_str());
    assert!(matches!(
        media::read(&f.db, Some(&store), &Target::OwnAvatar, None).await,
        Err(Error::Unauthorized)
    ));
    assert!(media::read(
        &f.db,
        Some(&store),
        &Target::OwnAvatar,
        Some(&f.other.token)
    )
    .await
    .unwrap()
    .is_none());
    let profile = f.db.own_profile_media(&f.member.token).await.unwrap();
    assert!(profile.avatar_available);
    assert_eq!(profile.emojis, vec!["blob-cat.@/한국어", "wave"]);
    assert_eq!(
        media::read(&f.db, Some(&store), &Target::OwnAvatar, own)
            .await
            .unwrap()
            .unwrap()
            .mime,
        "image/svg+xml"
    );
    assert_eq!(
        media::read(&f.db, Some(&store), &Target::OwnEmoji("wave".into()), own)
            .await
            .unwrap()
            .unwrap()
            .mime,
        "image/png"
    );
    assert_eq!(
        media::read(
            &f.db,
            Some(&store),
            &Target::OwnEmoji("blob-cat.@/한국어".into()),
            own,
        )
        .await
        .unwrap()
        .unwrap()
        .mime,
        "image/png"
    );
    assert!(media::read(
        &f.db,
        Some(&store),
        &Target::OwnEmoji("unknown".into()),
        own
    )
    .await
    .unwrap()
    .is_none());
    fixtures::ban(&f.db, f.member.member.id, true).await;
    assert!(matches!(
        media::read(&f.db, Some(&store), &Target::OwnAvatar, own).await,
        Err(Error::Unauthorized)
    ));
    assert!(f.db.own_profile_media(&f.member.token).await.is_err());
    fixtures::ban(&f.db, f.member.member.id, false).await;
    let mut conn = f.db.pool.get().await.unwrap();
    diesel::sql_query("DELETE FROM member_sessions WHERE token_hash=$1")
        .bind::<Binary, _>(auth::token_hash(&f.member.token).unwrap())
        .execute(&mut conn)
        .await
        .unwrap();
    drop(conn);
    assert!(matches!(
        media::read(&f.db, Some(&store), &Target::OwnEmoji("wave".into()), own).await,
        Err(Error::Unauthorized)
    ));
    f.dispose().await;
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn configured_reference_missing_store_file_or_changed_bytes_is_not_preview_success() {
    let f = Fixture::new().await;
    let target = Target::Software(f.software.clone());
    let store = f.store();
    assert!(matches!(
        media::read(&f.db, None, &target, None).await,
        Err(Error::Unavailable)
    ));
    assert!(
        media::read(&f.db, None, &Target::Software("missing".into()), None)
            .await
            .unwrap()
            .is_none()
    );
    let object = f.files.paths().read(&f.keys[3]).await.unwrap();
    let path = f.files.bundle.join("objects").join(object.hash);
    std::fs::write(&path, b"changed").unwrap();
    assert!(matches!(
        media::read(&f.db, Some(&store), &target, None).await,
        Err(Error::Unavailable)
    ));
    std::fs::remove_file(&path).unwrap();
    assert!(matches!(
        media::read(&f.db, Some(&store), &target, None).await,
        Err(Error::Unavailable)
    ));
    f.dispose().await;
}
