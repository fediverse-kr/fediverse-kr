use super::*;
use crate::backend::{
    auth,
    db::fixtures,
    legacy_assets::tests::Files,
    media::{self, Target},
    profile_refresh::{self, AVATAR_LIMIT, EMOJI_LIMIT},
};
use std::{collections::BTreeMap, sync::Arc};
mod batches;

struct Fixture {
    db: Database,
    grant: auth::SessionGrant,
    session: AuthenticatedSession,
    account: Uuid,
    other: Uuid,
    files: Files,
    store: ObjectStore,
}
impl Fixture {
    async fn new() -> Self {
        let db = fixtures::database().await;
        let grant = fixtures::member(&db).await;
        let session = auth::get_session_details(&db, &grant.token)
            .await
            .unwrap()
            .unwrap();
        let account = Uuid::new_v4();
        let other = Uuid::new_v4();
        let mut conn = db.pool.get().await.unwrap();
        for id in [account, other] {
            diesel::sql_query("INSERT INTO member_linked_accounts(id,member_id,actor_url,handle,display_name,profile_url,provider,provider_origin,provider_subject_id,verified_at) VALUES($1,$2,$3,$4,'Private fixture',$3,'activitypub_post','https://social.example.org',$3,now())")
                .bind::<SqlUuid,_>(id).bind::<SqlUuid,_>(grant.member.id).bind::<Text,_>(format!("https://social.example.org/users/{id}"))
                .bind::<Text,_>(format!("@{id}@social.example.org")).execute(&mut conn).await.unwrap();
        }
        drop(conn);
        let files = Files::new();
        let store = ObjectStore::open(&files.bundle).unwrap();
        Self {
            db,
            grant,
            session,
            account,
            other,
            files,
            store,
        }
    }
    fn bytes(&self, kind: &str) -> Vec<u8> {
        format!("<svg xmlns='http://www.w3.org/2000/svg' width='24' height='24'><title>{}-{}-{kind}</title><circle r='10'/></svg>",self.grant.member.id,self.account).into_bytes()
    }
    fn downloads(&self, kind: &str) -> Downloads {
        let asset = || {
            let bytes = self.bytes(kind);
            Download::Image(Asset {
                object: storage::ObjectRef::from_bytes(&bytes).unwrap(),
                bytes,
            })
        };
        Downloads {
            avatar: asset(),
            emojis: BTreeMap::from([("wave".into(), asset())]),
            failed: 0,
        }
    }
    async fn ticket(&self) -> Ticket {
        self.db
            .begin_profile_refresh(&self.session, self.account, false)
            .await
            .unwrap()
            .unwrap()
    }
    async fn reset_clock(&self) {
        diesel::sql_query("UPDATE member_profile_media SET requested_at=now()-interval '3 minutes' WHERE member_id=$1").bind::<SqlUuid,_>(self.grant.member.id).execute(&mut self.db.pool.get().await.unwrap()).await.unwrap();
    }
    async fn avatar(&self) -> Option<Vec<u8>> {
        media::read(
            &self.db,
            Some(&self.store),
            &Target::OwnAvatar,
            Some(&self.grant.token),
        )
        .await
        .unwrap()
        .map(|v| v.bytes)
    }
    async fn clean(self) {
        let mut conn = self.db.pool.get().await.unwrap();
        let keys = crate::backend::db::media_cleanup::member_keys(&mut conn, self.grant.member.id)
            .await
            .unwrap();
        diesel::sql_query("DELETE FROM legacy_members WHERE id=$1")
            .bind::<SqlUuid, _>(self.grant.member.id)
            .execute(&mut conn)
            .await
            .unwrap();
        drop(conn);
        fixtures::delete_members(&self.db, &[self.grant.member.id]).await;
        let mut conn = self.db.pool.get().await.unwrap();
        (&mut *conn)
            .transaction::<_, crate::backend::db::StoreError, _>(async |conn| {
                crate::backend::db::media_cleanup::retire_keys(conn, &keys).await
            })
            .await
            .unwrap();
        drop(conn);
        while self.db.collect_retired_media(&self.store).await.unwrap() {}
        drop(self.files);
    }
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn profile_refresh_roundtrip_replace_failure_and_unlink_remain_private() {
    let f = Fixture::new().await;
    let ticket = f.ticket().await;
    assert!(
        f.db.own_profile_media(&f.grant.token)
            .await
            .unwrap()
            .refreshing
    );
    f.db.finish_profile_refresh(&ticket, &f.store, f.downloads("first"))
        .await
        .unwrap();
    assert_eq!(f.avatar().await, Some(f.bytes("first")));
    let dto = f.db.own_profile_media(&f.grant.token).await.unwrap();
    assert_eq!(dto.source_account_id, Some(f.account.to_string()));
    assert_eq!(dto.emojis, vec!["wave"]);
    assert!(!dto.refreshing);
    assert!(matches!(
        media::read(&f.db, Some(&f.store), &Target::OwnAvatar, None).await,
        Err(media::Error::Unauthorized)
    ));
    let stranger = fixtures::member(&f.db).await;
    assert!(media::read(
        &f.db,
        Some(&f.store),
        &Target::OwnAvatar,
        Some(&stranger.token)
    )
    .await
    .unwrap()
    .is_none());
    assert_eq!(
        auth::get_session(&f.db, &f.grant.token)
            .await
            .unwrap()
            .unwrap()
            .display_name,
        "core test fixture"
    );
    assert!(f
        .db
        .linked_accounts(f.grant.member.id)
        .await
        .unwrap()
        .iter()
        .all(|a| !a.is_public));
    // Re-authentication of another private identity cannot switch the source.
    assert!(f
        .db
        .begin_profile_refresh(&f.session, f.other, true)
        .await
        .unwrap()
        .is_none());
    f.reset_clock().await;
    let ticket = f.ticket().await;
    let failed = Downloads {
        avatar: Download::Failed,
        emojis: BTreeMap::from([("wave".into(), Download::Failed)]),
        failed: 2,
    };
    assert_eq!(
        f.db.finish_profile_refresh(&ticket, &f.store, failed)
            .await
            .unwrap(),
        2
    );
    assert_eq!(f.avatar().await, Some(f.bytes("first")));
    f.reset_clock().await;
    f.db.finish_profile_refresh(&f.ticket().await, &f.store, f.downloads("next"))
        .await
        .unwrap();
    assert_eq!(f.avatar().await, Some(f.bytes("next")));
    while f.db.collect_retired_media(&f.store).await.unwrap() {}
    assert!(f
        .store
        .read(&storage::ObjectRef::from_bytes(&f.bytes("first")).unwrap())
        .await
        .is_err());
    auth::unlink_account(&f.db, &f.grant.token, f.account)
        .await
        .unwrap();
    let mut conn = f.db.pool.get().await.unwrap();
    let row = row(&mut conn, f.grant.member.id).await.unwrap();
    assert!(row.avatar.is_none());
    assert_eq!(row.emojis, "{}");
    assert!(auth::get_session(&f.db, &f.grant.token)
        .await
        .unwrap()
        .is_none());
    drop(conn);
    fixtures::delete_members(&f.db, &[stranger.member.id]).await;
    f.clean().await;
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn profile_refresh_old_projection_is_preserved_but_removed_picture_does_not_reappear() {
    let f = Fixture::new().await;
    let bytes = f.bytes("legacy");
    let object = storage::ObjectRef::from_bytes(&bytes).unwrap();
    let key = format!("avatars/{}", f.grant.member.id);
    let mutation = f.store.try_mutation().unwrap();
    mutation
        .publish(object.clone(), bytes.clone())
        .await
        .unwrap();
    let mut conn = f.db.pool.get().await.unwrap();
    diesel::sql_query("INSERT INTO stored_files VALUES($1,$2,$3)")
        .bind::<Text, _>(&key)
        .bind::<Text, _>(&object.hash)
        .bind::<BigInt, _>(object.bytes)
        .execute(&mut conn)
        .await
        .unwrap();
    diesel::sql_query("INSERT INTO legacy_members(id,fediverse_handle,fediverse_domain,display_name,avatar_key,emojis,inserted_at,updated_at) VALUES($1,$2,'social.example.org','Legacy',$3,'null',now() AT TIME ZONE 'UTC',now() AT TIME ZONE 'UTC')")
        .bind::<SqlUuid,_>(f.grant.member.id).bind::<Text,_>(format!("{}@social.example.org",f.account)).bind::<Text,_>(&key).execute(&mut conn).await.unwrap();
    drop(conn);
    drop(mutation);
    assert_eq!(f.avatar().await, Some(bytes.clone()));
    let ticket = f.ticket().await;
    f.db.fail_profile_refresh(&ticket).await.unwrap();
    assert_eq!(f.avatar().await, Some(bytes));
    f.reset_clock().await;
    f.db.finish_profile_refresh(
        &f.ticket().await,
        &f.store,
        Downloads {
            avatar: Download::Absent,
            emojis: BTreeMap::new(),
            failed: 0,
        },
    )
    .await
    .unwrap();
    assert!(f.avatar().await.is_none());
    while f.db.collect_retired_media(&f.store).await.unwrap() {}
    assert!(f.store.read(&object).await.is_ok()); // still referenced by preserved legacy row
    auth::withdraw(&f.db, &f.grant.token, "탈퇴").await.unwrap();
    while f.db.collect_retired_media(&f.store).await.unwrap() {}
    assert!(f.store.read(&object).await.is_err());
    f.clean().await;
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn profile_refresh_rechecks_ownership_session_and_generation_before_file_io() {
    let f = Fixture::new().await;
    let (a, b) = tokio::join!(
        f.db.begin_profile_refresh(&f.session, f.account, false),
        f.db.begin_profile_refresh(&f.session, f.account, false)
    );
    assert_eq!([a.is_ok(), b.is_ok()].iter().filter(|v| **v).count(), 1);
    let old = a.or(b).unwrap().unwrap();
    f.reset_clock().await;
    let next = f.ticket().await;
    assert!(matches!(
        f.db.finish_profile_refresh(&old, &f.store, f.downloads("old"))
            .await,
        Err(Error::Superseded)
    ));
    fixtures::ban(&f.db, f.grant.member.id, true).await;
    assert!(matches!(
        f.db.finish_profile_refresh(&next, &f.store, f.downloads("banned"))
            .await,
        Err(Error::Auth(AuthError::Unauthenticated))
    ));
    fixtures::ban(&f.db, f.grant.member.id, false).await;
    let stranger = fixtures::member(&f.db).await;
    let other = auth::get_session_details(&f.db, &stranger.token)
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        f.db.begin_profile_refresh(&other, f.account, false).await,
        Err(Error::Auth(AuthError::AccountNotFound))
    ));
    assert!(matches!(
        f.db.begin_profile_refresh(&other, Uuid::new_v4(), false)
            .await,
        Err(Error::Auth(AuthError::AccountNotFound))
    ));
    auth::revoke_session(&f.db, &f.grant.token).await.unwrap();
    assert!(matches!(
        f.db.finish_profile_refresh(&next, &f.store, f.downloads("revoked"))
            .await,
        Err(Error::Auth(AuthError::Unauthenticated))
    ));
    assert!(!f.files.bundle.join("objects").exists());
    fixtures::delete_members(&f.db, &[stranger.member.id]).await;
    f.clean().await;
}

#[tokio::test]
async fn profile_refresh_fetches_bounded_images_and_rejects_failed_or_active_documents() {
    use crate::backend::{
        crawler::{FetchFuture, Reply, SiteTransport},
        federation::profile::ActorMedia,
    };
    struct Transport;
    impl SiteTransport for Transport {
        fn get<'a>(&'a self, url: &'a url::Url, cap: usize) -> FetchFuture<'a> {
            Box::pin(async move {
                assert!([AVATAR_LIMIT, EMOJI_LIMIT].contains(&cap));
                let bytes = match url.path() {
                    "/ok" => {
                        b"<svg xmlns='http://www.w3.org/2000/svg' width='2' height='2'/>".to_vec()
                    }
                    "/large" => vec![0; cap + 1],
                    "/fake" => b"GIF89a".to_vec(),
                    _ => b"<html><svg/></html>".to_vec(),
                };
                Ok(Reply {
                    url: url.clone(),
                    status: 200,
                    body: Ok(bytes),
                })
            })
        }
    }
    let media = ActorMedia {
        avatar: Some("https://cdn.example.org/ok".into()),
        emojis: vec![
            ("large".into(), "https://cdn.example.org/large".into()),
            ("fake".into(), "https://cdn.example.org/fake".into()),
            ("html".into(), "https://cdn.example.org/html".into()),
            ("private".into(), "http://127.0.0.1/secret".into()),
        ],
    };
    let files = profile_refresh::fetch_assets(Arc::new(Transport), media).await;
    assert!(matches!(files.avatar, Download::Image(_)));
    assert_eq!(files.failed, 4);
    assert!(files.emojis.values().all(|d| matches!(d, Download::Failed)));
}

#[test]
fn profile_refresh_animation_first_frames_use_shared_decoder_without_changing_logo_policy() {
    let image = image::DynamicImage::ImageRgb8(image::RgbImage::new(2, 3));
    for format in [
        image::ImageFormat::Png,
        image::ImageFormat::Jpeg,
        image::ImageFormat::WebP,
        image::ImageFormat::Gif,
    ] {
        let mut encoded = std::io::Cursor::new(Vec::new());
        image.write_to(&mut encoded, format).unwrap();
        assert!(media::validation::validate(encoded.get_ref(), true).is_some());
        if format == image::ImageFormat::Gif {
            assert!(media::validation::validate(encoded.get_ref(), false).is_none());
        }
    }
}
