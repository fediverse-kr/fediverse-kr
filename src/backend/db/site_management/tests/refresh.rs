use super::*;
use crate::{
    backend::{
        db::schema::{directory_icons as icons, directory_jobs as jobs},
        directory::{Icon, NodeInfo, Observation},
    },
    directory::management::RefreshState,
};
use chrono::{Duration, Utc};

fn png(width: u32, height: u32) -> Vec<u8> {
    use std::io::Cursor;

    let mut bytes = Vec::new();
    image::DynamicImage::new_rgba8(width, height)
        .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)
        .unwrap();
    bytes
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn owner_refresh_uses_durable_queue_limits_concurrency_and_keeps_closed_editorial_state() {
    let db = fixtures::database().await;
    let a = fixtures::member(&db).await;
    let b = fixtures::member(&db).await;
    let sa = session(&db, &a.token).await;
    let sb = session(&db, &b.token).await;
    let domain = format!("refresh-{}.example.org", Uuid::new_v4().simple());
    let id = db.add_site(&domain).await.unwrap();
    claim(&db, &sa, &domain).await;
    let mut conn = db.pool.get().await.unwrap();
    diesel::update(sites::table.find(id))
        .set((
            sites::is_hidden.eq(true),
            sites::is_closed.eq(true),
            sites::name.eq(Some("owner title")),
        ))
        .execute(&mut conn)
        .await
        .unwrap();
    diesel::insert_into(icons::table)
        .values((
            icons::site_id.eq(id),
            icons::mime.eq("image/png"),
            icons::bytes.eq(png(32, 32)),
        ))
        .execute(&mut conn)
        .await
        .unwrap();
    assert_eq!(
        db.request_site_refresh(&sb, &domain).await,
        Err(Error::NotOwned)
    );
    assert_eq!(
        db.site_refresh_status(&sb, &domain).await,
        Err(Error::NotOwned)
    );
    let (one, two) = tokio::join!(
        db.request_site_refresh(&sa, &domain),
        db.request_site_refresh(&sa, &domain)
    );
    assert_eq!([one, two].iter().filter(|r| r.is_ok()).count(), 1);
    assert!([one, two].contains(&Err(Error::RefreshTooSoon)));
    assert_eq!(
        db.site_refresh_status(&sa, &domain)
            .await
            .unwrap()
            .unwrap()
            .state,
        RefreshState::Pending
    );
    // A new DB pool/process still sees and claims the queued request.
    let restarted = fixtures::database().await;
    let first = restarted.claim_site().await.unwrap().unwrap();
    assert_eq!(first.site_id, id);
    assert!(first.needs_icon);
    assert_eq!(
        db.site_refresh_status(&sa, &domain)
            .await
            .unwrap()
            .unwrap()
            .state,
        RefreshState::Running
    );
    assert!(restarted.claim_site().await.unwrap().is_none());
    diesel::update(jobs::table.find(first.id))
        .set(jobs::lease_until.eq(Some(Utc::now() - Duration::seconds(1))))
        .execute(&mut conn)
        .await
        .unwrap();
    let next = restarted.claim_site().await.unwrap().unwrap();
    assert_eq!(first.id, next.id);
    let observation = Observation {
        alive: true,
        response_ms: Some(25),
        status: Some(200),
        health_error: None,
        checked_at: Utc::now(),
        nodeinfo: Some(NodeInfo {
            name: Some("remote title".into()),
            users: Some(91),
            ..Default::default()
        }),
        nodeinfo_error: None,
        icon: Some(Icon {
            mime: "image/png",
            bytes: png(64, 64),
        }),
        icon_collection_complete: true,
    };
    assert!(!restarted.finish_site(&first, &observation).await.unwrap());
    assert!(restarted.finish_site(&next, &observation).await.unwrap());
    assert_eq!(
        db.site_refresh_status(&sa, &domain)
            .await
            .unwrap()
            .unwrap()
            .state,
        RefreshState::Complete
    );
    let site = db.owned_sites(&sa, 0).await.unwrap().sites.remove(0);
    assert!(site.closed && site.edit.hidden);
    assert_eq!(site.edit.name, "owner title");
    assert_eq!(site.revision, 1);
    assert_eq!(
        icons::table
            .find(id)
            .select(icons::bytes)
            .first::<Vec<u8>>(&mut conn)
            .await
            .unwrap(),
        png(64, 64)
    );
    assert!(db.public_site(&domain).await.unwrap().is_none());
    // Automatic collection still skips this closed site after the manual job.
    db.enqueue_sites().await.unwrap();
    assert!(db.claim_site().await.unwrap().is_none());
    // The next hour permits another request; a failed fetch does not fake success
    // or erase the last successful nodeinfo/icon.
    diesel::sql_query("UPDATE directory_site_details SET refresh_requested_at=now()-interval '61 minutes' WHERE site_id=$1").bind::<SqlUuid,_>(id).execute(&mut conn).await.unwrap();
    db.request_site_refresh(&sa, &domain).await.unwrap();
    let failed = db.claim_site().await.unwrap().unwrap();
    db.finish_site(&failed, &Observation::failed("network"))
        .await
        .unwrap();
    assert_eq!(
        db.site_refresh_status(&sa, &domain)
            .await
            .unwrap()
            .unwrap()
            .state,
        RefreshState::Failed
    );
    assert_eq!(
        icons::table
            .find(id)
            .select(icons::bytes)
            .first::<Vec<u8>>(&mut conn)
            .await
            .unwrap(),
        png(64, 64)
    );
    fixtures::age_session(&db, sa.id).await;
    assert_eq!(
        db.request_site_refresh(&sa, &domain).await,
        Err(Error::Auth(auth::AuthError::FreshAuthenticationRequired))
    );
    remove_site(&db, id).await;
    fixtures::delete_members(&db, &[a.member.id, b.member.id]).await;
}
