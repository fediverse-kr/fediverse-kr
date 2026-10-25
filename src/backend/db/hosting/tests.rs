use super::*;
use crate::backend::{auth, db::fixtures};
use uuid::Uuid;

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn managed_hosting_member_edits_conflict_history_and_restore() {
    let db = fixtures::database().await;
    let a = fixtures::member(&db).await;
    let b = fixtures::member(&db).await;
    let a = auth::get_session_details(&db, &a.token)
        .await
        .unwrap()
        .unwrap();
    let b = auth::get_session_details(&db, &b.token)
        .await
        .unwrap()
        .unwrap();
    let slug = format!("hosting-test-{}", Uuid::new_v4().simple());
    let edit = HostingEdit {
        name: "시험".into(),
        website_url: "https://example.org".into(),
        ..Default::default()
    };
    let first = db
        .create_hosting(&a, &slug, edit.clone(), "등록".into())
        .await
        .unwrap();
    assert_eq!(first.revision, 1);
    assert_eq!(db.hosting(&slug).await.unwrap().edit, edit);
    let mut changed = edit.clone();
    changed.scope = "개인 서버".into();
    changed.operator_region = crate::hosting::OperatorRegion::Korea;
    changed.korean_support = Some(true);
    changed.classification_source_url = "https://example.org/about".into();
    let second = db
        .save_hosting(&b, &slug, 1, changed.clone(), "수정".into())
        .await
        .unwrap();
    assert_eq!(second.revision, 2);
    assert_eq!(db.hosting(&slug).await.unwrap().edit, changed);
    assert_eq!(db.hosting_revision(&slug, 2).await.unwrap(), changed);
    assert!(matches!(
        db.save_hosting(&a, &slug, 1, edit.clone(), "오래된 수정".into())
            .await,
        Err(Error::Conflict)
    ));
    assert_eq!(db.hosting(&slug).await.unwrap().edit.scope, "개인 서버");
    assert_eq!(db.hosting_history(&slug, 0).await.unwrap().entries.len(), 2);
    assert_eq!(db.hosting_revision(&slug, 1).await.unwrap(), edit);
    let restored = db
        .restore_hosting(&a, &slug, 2, 1, "복원".into())
        .await
        .unwrap();
    assert_eq!(restored.revision, 3);
    assert_eq!(db.hosting(&slug).await.unwrap().edit, edit);
    assert_eq!(
        db.hosting_history(&slug, 0).await.unwrap().entries[0].action,
        "restore"
    );
    assert!(
        !serde_json::to_string(&db.hosting_history(&slug, 0).await.unwrap())
            .unwrap()
            .contains(&a.member.id.to_string())
    );
    fixtures::ban(&db, a.member.id, true).await;
    assert!(matches!(
        db.save_hosting(&a, &slug, 3, changed, "차단".into()).await,
        Err(Error::Auth(_))
    ));
    fixtures::ban(&db, a.member.id, false).await;
    fixtures::age_session(&db, a.id).await;
    assert!(matches!(
        db.save_hosting(&a, &slug, 3, edit.clone(), "오래된 인증".into())
            .await,
        Err(Error::Auth(_))
    ));
    let mut conn = db.pool.get().await.unwrap();
    diesel::delete(services::table.find(&slug))
        .execute(&mut conn)
        .await
        .unwrap();
    drop(conn);
    fixtures::delete_members(&db, &[a.member.id, b.member.id]).await;
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn managed_hosting_regional_rotation_is_fair_stable_and_paginated() {
    use crate::hosting::OperatorRegion::{Korea, Overseas, Unknown};
    let db = fixtures::database().await;
    let member = fixtures::member(&db).await;
    let session = auth::get_session_details(&db, &member.token)
        .await
        .unwrap()
        .unwrap();
    let prefix = format!("rotation-{}", Uuid::new_v4().simple());
    let regions = [Korea, Korea]
        .into_iter()
        .chain(std::iter::repeat_n(Overseas, 21))
        .chain([Unknown]);
    let mut slugs = Vec::new();
    for (i, region) in regions.enumerate() {
        let slug = format!("{prefix}-{i}");
        db.create_hosting(
            &session,
            &slug,
            HostingEdit {
                name: match region {
                    Korea => "Z 한국",
                    Overseas => "A 해외",
                    Unknown => "0 미확인",
                }
                .into(),
                website_url: format!("https://example.org/{slug}"),
                operator_region: region,
                classification_source_url: "https://example.org/about".into(),
                ..Default::default()
            },
            "시험 등록".into(),
        )
        .await
        .unwrap();
        slugs.push(slug);
    }
    let first = db.hosting_list(0, 0).await.unwrap();
    assert_eq!(first, db.hosting_list(0, 0).await.unwrap());
    assert_eq!(first.rotation_day, 0);
    assert!(first.has_next);
    let second = db.hosting_list(1, first.rotation_day).await.unwrap();
    assert!(!second.has_next);
    let all: Vec<_> = first.items.iter().chain(&second.items).collect();
    assert_eq!(all.len(), 24);
    assert_eq!(
        all.iter()
            .map(|r| &r.slug)
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        24
    );
    assert!(all[..2].iter().all(|r| r.edit.operator_region == Korea));
    assert!(all[2..23]
        .iter()
        .all(|r| r.edit.operator_region == Overseas));
    assert_eq!(all[23].edit.operator_region, Unknown);
    let mut leading = std::collections::BTreeSet::new();
    for day in 0..21 {
        let page = db.hosting_list(0, day).await.unwrap();
        assert_eq!(page.items[0].slug, all[day as usize % 2].slug);
        leading.insert(page.items[2].slug.clone());
    }
    assert_eq!(
        leading.len(),
        21,
        "each overseas provider gets one leading day per cycle"
    );
    assert_eq!(db.hosting_list(0, 42).await.unwrap().items, first.items);
    // Names are editable, but cannot be used to buy a better position.
    let target = &all[5];
    let mut renamed = target.edit.clone();
    renamed.name = "000 a renamed service".into();
    db.save_hosting(
        &session,
        &target.slug,
        target.revision,
        renamed,
        "이름만 변경".into(),
    )
    .await
    .unwrap();
    assert_eq!(
        db.hosting_list(0, 0)
            .await
            .unwrap()
            .items
            .iter()
            .map(|x| &x.slug)
            .collect::<Vec<_>>(),
        first.items.iter().map(|x| &x.slug).collect::<Vec<_>>()
    );
    let mut conn = db.pool.get().await.unwrap();
    diesel::delete(services::table.filter(services::slug.eq_any(&slugs)))
        .execute(&mut conn)
        .await
        .unwrap();
    drop(conn);
    fixtures::delete_members(&db, &[session.member.id]).await;
}
