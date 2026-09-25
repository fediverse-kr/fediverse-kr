use super::*;
use crate::backend::{auth, db::fixtures};
use uuid::Uuid;

struct Fixture {
    db: Database,
    name: String,
    kind: String,
    kind_id: i64,
    s: AuthenticatedSession,
    other: AuthenticatedSession,
}
impl Fixture {
    async fn new() -> Self {
        let db = fixtures::database().await;
        let first = fixtures::member(&db).await;
        let second = fixtures::member(&db).await;
        let s = auth::get_session_details(&db, &first.token)
            .await
            .unwrap()
            .unwrap();
        let other = auth::get_session_details(&db, &second.token)
            .await
            .unwrap()
            .unwrap();
        let suffix = Uuid::new_v4().simple().to_string();
        let kind = format!("catalog-test-{suffix}");
        let kind_id = -(i64::from_str_radix(&suffix[..14], 16).unwrap() + 1);
        let mut conn = db.pool.get().await.unwrap();
        diesel::sql_query("INSERT INTO catalog_categories(id,name,label,display_order,inserted_at,updated_at) VALUES($1,$2,'테스트 종류',0,now(),now())").bind::<BigInt,_>(kind_id).bind::<Text,_>(&kind).execute(&mut conn).await.unwrap();
        drop(conn);
        Self {
            db,
            name: format!("catalog-test-{suffix}"),
            kind,
            kind_id,
            s,
            other,
        }
    }
    fn value(&self, description: &str) -> ValidatedEdit {
        domain::validate(
            SoftwareEdit {
                display_name: "도구 소개".into(),
                description: description.into(),
                categories: vec![self.kind.clone()],
                features: vec!["사진 공유".into()],
                website_url: "https://example.org".into(),
                ..Default::default()
            },
            "소개 수정".into(),
        )
        .unwrap()
    }
    async fn sql(&self, query: &str) {
        diesel::sql_query(query)
            .bind::<Text, _>(&self.name)
            .execute(&mut self.db.pool.get().await.unwrap())
            .await
            .unwrap();
    }
    async fn flag(&self, query: &str) -> bool {
        diesel::sql_query(query)
            .bind::<Text, _>(&self.name)
            .get_result::<Flag>(&mut self.db.pool.get().await.unwrap())
            .await
            .unwrap()
            .value
    }
    async fn clean(self) {
        let mut conn = self.db.pool.get().await.unwrap();
        diesel::sql_query("DELETE FROM catalog_software WHERE name=$1")
            .bind::<Text, _>(&self.name)
            .execute(&mut conn)
            .await
            .unwrap();
        diesel::sql_query("DELETE FROM catalog_categories WHERE id=$1 AND name=$2")
            .bind::<BigInt, _>(self.kind_id)
            .bind::<Text, _>(&self.kind)
            .execute(&mut conn)
            .await
            .unwrap();
        fixtures::delete_members(&self.db, &[self.s.member.id, self.other.member.id]).await;
    }
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn direct_wiki_edits_cas_history_restore_and_private_attribution() {
    let f = Fixture::new().await;
    let first =
        f.db.create_software(&f.s, &f.name.to_uppercase(), &f.value("최초 소개"))
            .await
            .unwrap();
    assert_eq!(first.revision, 1);
    assert_eq!(first.name, f.name);
    assert!(matches!(
        f.db.create_software(&f.other, &f.name, &f.value("중복"))
            .await,
        Err(Error::Duplicate)
    ));
    assert!(f
        .db
        .catalog_kinds()
        .await
        .unwrap()
        .items
        .iter()
        .any(|k| k.name == f.kind));
    f.sql("UPDATE catalog_software SET logo_key='preserve/logo',brand_color='#334455',is_featured=true,display_order=17 WHERE name=$1").await;
    let mine = f.value("첫 편집");
    let other = f.value("동시 편집");
    let (a, b) = tokio::join!(
        f.db.edit_software(&f.s, &f.name, 1, &mine),
        f.db.edit_software(&f.other, &f.name, 1, &other)
    );
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    assert!(matches!(a, Err(Error::Conflict)) || matches!(b, Err(Error::Conflict)));
    let current = f.db.editable_software(&f.name).await.unwrap();
    assert_eq!(current.revision, 2);
    assert_eq!(
        f.db.edit_software(
            &f.other,
            &f.name,
            2,
            &domain::validate(current.edit.clone(), "같은 내용".into()).unwrap()
        )
        .await
        .unwrap()
        .revision,
        2
    );
    let restored =
        f.db.restore_software(&f.other, &f.name, 2, 1, "최초 내용 복원".into())
            .await
            .unwrap();
    assert_eq!(restored.revision, 3);
    assert_eq!(restored.edit.description, "최초 소개");
    assert!(f.flag("SELECT logo_key='preserve/logo' AND brand_color='#334455' AND is_featured AND display_order=17 AS value FROM catalog_software WHERE name=$1").await);
    let history = f.db.software_history(&f.name, 0).await.unwrap();
    assert_eq!(history.entries.len(), 3);
    assert_eq!(history.entries[0].action, "restore");
    let public = serde_json::to_string(&history).unwrap();
    for private in [
        f.s.member.id.to_string(),
        f.other.member.id.to_string(),
        "actor_id".into(),
        "handle".into(),
    ] {
        assert!(!public.contains(&private));
    }
    fixtures::delete_members(&f.db, &[f.s.member.id, f.other.member.id]).await;
    assert_eq!(
        f.db.software_history(&f.name, 0)
            .await
            .unwrap()
            .entries
            .len(),
        3
    );
    assert!(f.flag("SELECT NOT EXISTS(SELECT 1 FROM catalog_software_edits e JOIN catalog_software s ON s.id=e.software_id WHERE s.name=$1 AND actor_id IS NOT NULL) AS value").await);
    f.clean().await;
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn baseline_preserves_imported_nulls_and_restricted_metadata() {
    let f = Fixture::new().await;
    // Simulate a pre-wiki imported row; the original table's schema is unchanged.
    f.sql("INSERT INTO catalog_software(id,name,display_name,logo_key,brand_color,is_featured,display_order,inserted_at,updated_at) SELECT -abs(hashtextextended($1,0)), $1,'원래 도구','old/logo','#445566',true,19,now(),now()").await;
    let changed =
        f.db.edit_software(&f.other, &f.name, 0, &f.value("새 소개"))
            .await
            .unwrap();
    assert_eq!(changed.revision, 1);
    assert!(f.flag("SELECT snapshot->'description'='null'::jsonb AND snapshot->'categories'='null'::jsonb AND snapshot->>'logo_key'='old/logo' AND actor_id IS NULL AS value FROM catalog_software_edits WHERE software_id=(SELECT id FROM catalog_software WHERE name=$1) AND revision=0").await);
    let old = f.db.software_revision(&f.name, 0).await.unwrap();
    assert!(old.description.is_empty());
    // A legacy record without a valid category cannot bypass today's validation.
    assert!(matches!(
        f.db.restore_software(&f.other, &f.name, 1, 0, "이전 복원".into())
            .await,
        Err(Error::Invalid)
    ));
    assert_eq!(
        f.db.software_history(&f.name, 0)
            .await
            .unwrap()
            .entries
            .len(),
        2
    );
    let invalid = domain::validate(
        SoftwareEdit {
            categories: vec!["not-an-existing-kind".into()],
            ..changed.edit
        },
        "분류 변경".into(),
    )
    .unwrap();
    assert!(matches!(
        f.db.edit_software(&f.s, &f.name, 1, &invalid).await,
        Err(Error::Invalid)
    ));
    f.clean().await;
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn writes_recheck_revocation_bans_and_locks_without_fresh_auth_requirement() {
    let f = Fixture::new().await;
    fixtures::age_session(&f.db, f.s.id).await;
    f.db.create_software(&f.s, &f.name, &f.value("최초"))
        .await
        .unwrap();
    f.sql("UPDATE catalog_software_state SET locked=true WHERE software_id=(SELECT id FROM catalog_software WHERE name=$1)").await;
    assert!(matches!(
        f.db.edit_software(&f.s, &f.name, 1, &f.value("잠금 우회"))
            .await,
        Err(Error::Locked)
    ));
    assert!(matches!(
        f.db.restore_software(&f.s, &f.name, 2, 1, "잠금 우회".into())
            .await,
        Err(Error::Locked)
    ));
    f.sql("UPDATE catalog_software_state SET locked=false WHERE software_id=(SELECT id FROM catalog_software WHERE name=$1)").await;
    fixtures::ban(&f.db, f.s.member.id, true).await;
    assert!(matches!(
        f.db.edit_software(&f.s, &f.name, 1, &f.value("밴 우회"))
            .await,
        Err(Error::Auth(_))
    ));
    let mut conn = f.db.pool.get().await.unwrap();
    diesel::sql_query("DELETE FROM member_sessions WHERE id=$1")
        .bind::<SqlUuid, _>(f.other.id)
        .execute(&mut conn)
        .await
        .unwrap();
    drop(conn);
    assert!(matches!(
        f.db.edit_software(&f.other, &f.name, 1, &f.value("세션 재사용"))
            .await,
        Err(Error::Auth(_))
    ));
    assert_eq!(
        f.db.software_history(&f.name, 0)
            .await
            .unwrap()
            .entries
            .len(),
        1
    );
    f.clean().await;
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn edit_flood_is_bounded_and_history_is_paginated() {
    let f = Fixture::new().await;
    f.db.create_software(&f.s, &f.name, &f.value("0"))
        .await
        .unwrap();
    for revision in 1..30 {
        f.db.edit_software(&f.s, &f.name, revision, &f.value(&revision.to_string()))
            .await
            .unwrap();
    }
    assert!(matches!(
        f.db.edit_software(&f.s, &f.name, 30, &f.value("31")).await,
        Err(Error::RateLimited)
    ));
    let first = f.db.software_history(&f.name, 0).await.unwrap();
    let second = f.db.software_history(&f.name, 1).await.unwrap();
    assert_eq!(first.entries.len(), 20);
    assert!(first.has_next);
    assert_eq!(second.entries.len(), 10);
    assert!(!second.has_next);
    assert_eq!(first.entries.last().unwrap().revision, 11);
    assert_eq!(second.entries[0].revision, 10);
    assert!(matches!(
        f.db.software_history(&f.name, 10_001).await,
        Err(Error::Invalid)
    ));
    f.clean().await;
}
