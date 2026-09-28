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
    let second = db
        .save_hosting(&b, &slug, 1, changed.clone(), "수정".into())
        .await
        .unwrap();
    assert_eq!(second.revision, 2);
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
