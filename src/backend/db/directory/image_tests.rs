use super::*;
use crate::backend::{db::fixtures, directory::Icon};
use chrono::{Duration, Utc};

fn png(w: u32, h: u32) -> Vec<u8> {
    let mut bytes = Vec::new();
    image::DynamicImage::new_rgba8(w, h)
        .write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .unwrap();
    bytes
}

#[test]
fn identity_icon_quality_does_not_treat_a_header_as_a_better_logo() {
    let favicon = b"<svg><circle cx='8' cy='8' r='8'/></svg>";
    assert!(!icon_improves(
        "image/png",
        &png(1200, 630),
        "image/svg+xml",
        favicon
    ));
    assert!(!icon_improves(
        "image/png",
        &png(32, 32),
        "image/svg+xml",
        favicon
    ));
    assert!(icon_improves(
        "image/svg+xml",
        favicon,
        "image/png",
        &png(1200, 630)
    ));
    assert!(!icon_improves(
        "image/png",
        b"broken",
        "image/svg+xml",
        favicon
    ));
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn site_image_policy_finishes_empty_results_once_and_owner_can_retry() {
    let db = fixtures::database().await;
    let site = db
        .add_site(&format!(
            "empty-image-{}.example.org",
            Uuid::new_v4().simple()
        ))
        .await
        .unwrap();
    let mut conn = db.pool.get().await.unwrap();
    let mut completed = Observation::failed("fixture");
    completed.alive = true;
    completed.status = Some(200);
    completed.icon_collection_complete = true;
    for (iteration, owner, expected) in [(0, false, true), (1, false, false), (2, true, true)] {
        let id = Uuid::new_v4();
        diesel::insert_into(jobs::table)
            .values((
                jobs::id.eq(id),
                jobs::site_id.eq(site),
                jobs::owner_requested.eq(owner),
                jobs::scheduled_at
                    .eq(Utc::now() - Duration::days(10) + Duration::seconds(iteration)),
            ))
            .execute(&mut conn)
            .await
            .unwrap();
        let job = db.claim_site().await.unwrap().unwrap();
        assert_eq!(job.id, id);
        assert_eq!(
            job.needs_icon, expected,
            "empty completed collection must be remembered"
        );
        assert!(db.finish_site(&job, &completed).await.unwrap());
    }
    assert_eq!(
        icons::table
            .find(site)
            .count()
            .get_result::<i64>(&mut conn)
            .await
            .unwrap(),
        0
    );
    diesel::delete(sites::table.find(site))
        .execute(&mut conn)
        .await
        .unwrap();
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn header_cache_is_independent_preserves_last_good_bytes_and_respects_visibility() {
    let db = fixtures::database().await;
    let domain = format!("separate-images-{}.example.org", Uuid::new_v4().simple());
    let site = db.add_site(&domain).await.unwrap();
    let mut conn = db.pool.get().await.unwrap();
    let favicon = b"<svg><circle cx='8' cy='8' r='8'/></svg>".to_vec();
    diesel::insert_into(icons::table)
        .values((
            icons::site_id.eq(site),
            icons::mime.eq("image/svg+xml"),
            icons::bytes.eq(&favicon),
            icons::collection_version.eq(1),
        ))
        .execute(&mut conn)
        .await
        .unwrap();
    let large = png(800, 420);
    let mut good = Observation::failed("fixture");
    good.alive = true;
    good.status = Some(200);
    good.icon_collection_complete = true;
    good.header = Some(Icon {
        mime: "image/png",
        bytes: large.clone(),
    });
    for (iteration, result) in [
        (0, good.clone()),
        (1, Observation::failed("offline")),
        (
            2,
            Observation {
                header: Some(Icon {
                    mime: "image/png",
                    bytes: png(16, 16),
                }),
                ..good.clone()
            },
        ),
    ] {
        store_observation(&mut conn, Uuid::new_v4(), site, &result, true, true)
            .await
            .unwrap();
        assert_eq!(
            db.public_site_icon(&domain).await.unwrap().unwrap().bytes,
            favicon,
            "header never replaces the favicon"
        );
        assert_eq!(
            db.public_site_header(&domain)
                .await
                .unwrap()
                .map(|i| i.bytes),
            Some(large.clone()),
            "independent header survives failed or undersized collection #{iteration}"
        );
    }
    for (hidden, force) in [(true, false), (false, true)] {
        diesel::update(sites::table.find(site))
            .set((
                sites::is_hidden.eq(hidden),
                sites::is_force_hidden.eq(force),
            ))
            .execute(&mut conn)
            .await
            .unwrap();
        assert!(db.public_site_header(&domain).await.unwrap().is_none());
    }
    diesel::update(sites::table.find(site))
        .set((sites::is_hidden.eq(false), sites::is_force_hidden.eq(false)))
        .execute(&mut conn)
        .await
        .unwrap();
    diesel::delete(icons::table.find(site))
        .execute(&mut conn)
        .await
        .unwrap();
    assert_eq!(
        db.public_site_header(&domain).await.unwrap().unwrap().bytes,
        large,
        "header-only servers are supported"
    );
    diesel::insert_into(icons::table)
        .values((
            icons::site_id.eq(site),
            icons::mime.eq("image/png"),
            icons::bytes.eq(&large),
        ))
        .execute(&mut conn)
        .await
        .unwrap();
    assert!(
        db.public_site_header(&domain).await.unwrap().is_none(),
        "identical cached images are not duplicated"
    );
    diesel::delete(sites::table.find(site))
        .execute(&mut conn)
        .await
        .unwrap();
    assert!(db.public_site_header(&domain).await.unwrap().is_none());
}
