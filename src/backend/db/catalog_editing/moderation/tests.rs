use super::*;
use crate::backend::{auth, db::fixtures};
use uuid::Uuid;
mod logo;
struct Fixture {
    db: Database,
    admin: AuthenticatedSession,
    member: AuthenticatedSession,
    name: String,
    kind: String,
    token: String,
}
impl Fixture {
    async fn new() -> Self {
        let db = fixtures::database().await;
        let a = fixtures::member(&db).await;
        let b = fixtures::member(&db).await;
        let admin = auth::get_session_details(&db, &a.token)
            .await
            .unwrap()
            .unwrap();
        let member = auth::get_session_details(&db, &b.token)
            .await
            .unwrap()
            .unwrap();
        db.set_admin_role(admin.member.id, true).await.unwrap();
        let name = format!("admin-catalog-{}", Uuid::new_v4().simple());
        let kind = format!("{name}-kind");
        db.moderate_category(
            &admin,
            dto::CategoryRequest {
                name: kind.clone(),
                revision: None,
                edit: dto::CategoryEdit {
                    label: "원래 종류".into(),
                    emoji: "📚".into(),
                    display_order: 0,
                },
                note: "비공개 분류 사유".into(),
            },
        )
        .await
        .unwrap();
        let f = Self {
            db,
            admin,
            member,
            name,
            kind,
            token: a.token,
        };
        f.db.create_software(&f.member, &f.name, &f.value("처음 소개"))
            .await
            .unwrap();
        f
    }
    fn value(&self, s: &str) -> ValidatedEdit {
        domain::validate(
            SoftwareEdit {
                display_name: "시험 제품".into(),
                description: s.into(),
                categories: vec![self.kind.clone()],
                ..Default::default()
            },
            "공개 변경 요약".into(),
        )
        .unwrap()
    }
    async fn item(&self) -> dto::Software {
        self.db
            .moderation_software(&self.admin, &self.name)
            .await
            .unwrap()
    }
    async fn act(&self, action: dto::Action) -> dto::Software {
        let revision = self.item().await.revision;
        self.db
            .moderate_software(
                &self.admin,
                dto::Request {
                    name: self.name.clone(),
                    revision,
                    action,
                    note: "관리자만 읽는 사유".into(),
                },
            )
            .await
            .unwrap()
    }
    async fn clean(self) {
        let mut conn = self.db.pool.get().await.unwrap();
        diesel::sql_query(
            "DELETE FROM catalog_software WHERE name=$1 OR strpos(name,$1||'-page-')=1",
        )
        .bind::<Text, _>(&self.name)
        .execute(&mut conn)
        .await
        .unwrap();
        diesel::sql_query("DELETE FROM catalog_categories WHERE name=$1")
            .bind::<Text, _>(&self.kind)
            .execute(&mut conn)
            .await
            .unwrap();
        diesel::sql_query("DELETE FROM moderation_events WHERE target_id=$1 OR actor_id=$1")
            .bind::<SqlUuid, _>(self.admin.member.id)
            .execute(&mut conn)
            .await
            .unwrap();
        drop(conn);
        fixtures::delete_members(&self.db, &[self.admin.member.id, self.member.member.id]).await;
    }
}
#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn locked_admin_uses_same_editor_and_preserves_metadata_private_reasons() {
    let f = Fixture::new().await;
    diesel::sql_query("UPDATE catalog_software SET logo_key='private/logo',brand_color='legacy-color',features=NULL WHERE name=$1").bind::<Text,_>(&f.name).execute(&mut f.db.pool.get().await.unwrap()).await.unwrap();
    let locked = f.act(dto::Action::Locked(true)).await;
    assert_eq!(locked.revision, 2);
    assert!(matches!(
        f.db.edit_software(&f.member, &f.name, 2, &f.value("차단"))
            .await,
        Err(Error::Locked)
    ));
    assert!(matches!(
        f.db.restore_software(&f.member, &f.name, 2, 1, "복원".into())
            .await,
        Err(Error::Locked)
    ));
    let saved =
        f.db.moderation_software_save(&f.admin, &f.name, 2, &f.value("관리자 수정"))
            .await
            .unwrap();
    assert!(saved.locked);
    assert_eq!(saved.revision, 3);
    let item = f.item().await;
    assert_eq!(item.brand_color.as_deref(), Some("legacy-color"));
    let kept = diesel::sql_query(
        "SELECT logo_key='private/logo' AS value FROM catalog_software WHERE name=$1",
    )
    .bind::<Text, _>(&f.name)
    .get_result::<Flag>(&mut f.db.pool.get().await.unwrap())
    .await
    .unwrap()
    .value;
    assert!(kept);
    let public = f.db.software_history(&f.name, 0).await.unwrap();
    assert_eq!(public.entries[0].action, "admin_edit");
    let text = serde_json::to_string(&public).unwrap();
    assert!(!text.contains("관리자만"));
    assert!(!text.contains(&f.admin.member.id.to_string()));
    assert!(!text.contains("private/logo"));
    let private =
        f.db.moderation_catalog_history(&f.admin, &f.name, false, 0)
            .await
            .unwrap();
    assert_eq!(private.items[0].note, "관리자만 읽는 사유");
    assert_eq!(private.items[0].before, "false");
    assert_eq!(private.items[0].after, "true");
    f.act(dto::Action::Locked(false)).await;
    assert!(matches!(
        f.db.edit_software(&f.member, &f.name, 3, &f.value("오래된 폼"))
            .await,
        Err(Error::Conflict)
    ));
    f.db.edit_software(&f.member, &f.name, 4, &f.value("다시 편집"))
        .await
        .unwrap();
    f.clean().await;
}
#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn settings_compare_and_swap_baseline_noop_and_typed_values() {
    let f = Fixture::new().await;
    diesel::sql_query("DELETE FROM catalog_software_edits WHERE software_id=(SELECT id FROM catalog_software WHERE name=$1)").bind::<Text,_>(&f.name).execute(&mut f.db.pool.get().await.unwrap()).await.unwrap();
    diesel::sql_query("DELETE FROM catalog_software_state WHERE software_id=(SELECT id FROM catalog_software WHERE name=$1)").bind::<Text,_>(&f.name).execute(&mut f.db.pool.get().await.unwrap()).await.unwrap();
    let a = dto::Request {
        name: f.name.clone(),
        revision: 0,
        action: dto::Action::DisplayOrder(19),
        note: "동시 설정".into(),
    };
    let mut b = a.clone();
    b.action = dto::Action::Featured(true);
    let (x, y) = tokio::join!(
        f.db.moderate_software(&f.admin, a),
        f.db.moderate_software(&f.admin, b)
    );
    assert!(matches!(
        (&x, &y),
        (Ok(_), Err(Error::Conflict)) | (Err(Error::Conflict), Ok(_))
    ));
    let first = f.item().await;
    assert_eq!(first.revision, 1);
    let unchanged = f.act(dto::Action::DisplayOrder(first.display_order)).await;
    assert_eq!(unchanged.revision, 1);
    let history = f.db.software_history(&f.name, 0).await.unwrap();
    assert_eq!(history.entries.len(), 2);
    assert_eq!(history.entries[1].action, "baseline");
    let color = f.act(dto::Action::BrandColor(Some("#ab12ef".into()))).await;
    assert_eq!(color.brand_color.as_deref(), Some("#AB12EF"));
    let clear = f.act(dto::Action::BrandColor(None)).await;
    assert_eq!(clear.brand_color, None);
    // Restore changes only editorial fields, never metadata or administrative state.
    let edit =
        f.db.moderation_software_save(&f.admin, &f.name, clear.revision, &f.value("새 소개"))
            .await
            .unwrap();
    f.db.restore_software(&f.member, &f.name, edit.revision, 0, "소개 복원".into())
        .await
        .unwrap();
    assert_eq!(f.item().await.display_order, first.display_order);
    f.clean().await;
}
#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn admin_role_freshness_ban_and_revocation_are_rechecked() {
    let f = Fixture::new().await;
    let request = dto::Request {
        name: f.name.clone(),
        revision: 1,
        action: dto::Action::Locked(true),
        note: "권한 검사".into(),
    };
    assert!(matches!(
        f.db.moderation_software_list(&f.member, "", false, 0).await,
        Err(Error::Forbidden)
    ));
    assert!(matches!(
        f.db.moderate_software(&f.member, request.clone()).await,
        Err(Error::Forbidden)
    ));
    assert!(matches!(
        f.db.moderation_software_save(&f.member, &f.name, 1, &f.value("금지"))
            .await,
        Err(Error::Forbidden)
    ));
    assert!(matches!(
        f.db.moderation_catalog_history(&f.member, &f.kind, true, 0)
            .await,
        Err(Error::Forbidden)
    ));
    diesel::sql_query("UPDATE member_sessions SET authenticated_at=now()-interval '16 minutes' WHERE member_id=$1").bind::<SqlUuid,_>(f.admin.member.id).execute(&mut f.db.pool.get().await.unwrap()).await.unwrap();
    assert!(matches!(
        f.db.moderate_software(&f.admin, request.clone()).await,
        Err(Error::Auth(_))
    ));
    assert!(f.db.moderation_software(&f.admin, &f.name).await.is_ok());
    f.db.set_admin_role(f.admin.member.id, false).await.unwrap();
    assert!(matches!(
        f.db.moderation_category(&f.admin, &f.kind).await,
        Err(Error::Forbidden)
    ));
    f.db.set_admin_role(f.admin.member.id, true).await.unwrap();
    fixtures::ban(&f.db, f.admin.member.id, true).await;
    assert!(matches!(
        f.db.moderation_software(&f.admin, &f.name).await,
        Err(Error::Auth(_))
    ));
    fixtures::ban(&f.db, f.admin.member.id, false).await;
    auth::revoke_session(&f.db, &f.token).await.unwrap();
    assert!(matches!(
        f.db.moderation_categories(&f.admin, "", 0).await,
        Err(Error::Auth(_))
    ));
    f.clean().await;
}
#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn category_edit_keeps_references_revisions_history_and_null_baseline() {
    let f = Fixture::new().await;
    let c = f.db.moderation_category(&f.admin, &f.kind).await.unwrap();
    let make = |revision| dto::CategoryRequest {
        name: f.kind.clone(),
        revision,
        edit: dto::CategoryEdit {
            label: "책과 읽기".into(),
            emoji: "📖".into(),
            display_order: -9,
        },
        note: "분류 정리".into(),
    };
    assert!(matches!(
        f.db.moderate_category(&f.member, make(Some(c.revision)))
            .await,
        Err(Error::Forbidden)
    ));
    let saved =
        f.db.moderate_category(&f.admin, make(Some(c.revision)))
            .await
            .unwrap();
    assert_eq!(saved.revision, 2);
    assert!(matches!(
        f.db.moderate_category(&f.admin, make(Some(1))).await,
        Err(Error::Conflict)
    ));
    assert_eq!(
        f.db.moderate_category(&f.admin, make(Some(2)))
            .await
            .unwrap()
            .revision,
        2
    );
    assert!(matches!(
        f.db.moderate_category(&f.admin, make(None)).await,
        Err(Error::Duplicate)
    ));
    let public = f.db.public_software(&f.name).await.unwrap();
    assert_eq!(public.categories[0].name, f.kind);
    assert_eq!(public.categories[0].label, "책과 읽기");
    assert_eq!(
        f.db.editable_software(&f.name)
            .await
            .unwrap()
            .edit
            .categories,
        vec![f.kind.clone()]
    );
    let history =
        f.db.moderation_catalog_history(&f.admin, &f.kind, true, 0)
            .await
            .unwrap();
    assert_eq!(history.items.len(), 2);
    assert_eq!(history.items[1].before, "null");
    assert!(history.items[0].before.contains("원래 종류"));
    f.clean().await;
}
#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn bounded_lists_literal_search_and_history_survive_author_removal() {
    let f = Fixture::new().await;
    diesel::sql_query("INSERT INTO catalog_software(id,name,display_name,is_featured,display_order,inserted_at,updated_at) SELECT (SELECT greatest(coalesce(max(id),0),0) FROM catalog_software)+n,$1||'-page-'||n,'100%_literal',false,n,now(),now() FROM generate_series(1,25) n").bind::<Text,_>(&f.name).execute(&mut f.db.pool.get().await.unwrap()).await.unwrap();
    let a =
        f.db.moderation_software_list(&f.admin, &format!("{}-page-", f.name), false, 0)
            .await
            .unwrap();
    let b =
        f.db.moderation_software_list(&f.admin, &format!("{}-page-", f.name), false, 1)
            .await
            .unwrap();
    assert_eq!(a.items.len(), 24);
    assert!(a.has_next);
    assert_eq!(b.items.len(), 1);
    assert!(!b.has_next);
    assert!(!a.items.iter().any(|x| x.name == b.items[0].name));
    assert_eq!(
        f.db.moderation_software_list(&f.admin, "100%_", false, 0)
            .await
            .unwrap()
            .items
            .len(),
        24
    );
    assert!(f
        .db
        .moderation_software_list(&f.admin, "100__", false, 0)
        .await
        .unwrap()
        .items
        .is_empty());
    f.act(dto::Action::Locked(true)).await;
    assert_eq!(
        f.db.moderation_software_list(&f.admin, &f.name, true, 0)
            .await
            .unwrap()
            .items
            .len(),
        1
    );
    assert!(matches!(
        f.db.moderation_catalog_history(&f.admin, &f.name, false, 10001)
            .await,
        Err(Error::Invalid)
    ));
    fixtures::delete_members(&f.db, &[f.admin.member.id]).await;
    let kept=diesel::sql_query("SELECT EXISTS(SELECT 1 FROM catalog_admin_events e JOIN catalog_software s ON s.id=e.software_id WHERE s.name=$1 AND e.actor_id IS NULL AND e.note='관리자만 읽는 사유') AS value").bind::<Text,_>(&f.name).get_result::<Flag>(&mut f.db.pool.get().await.unwrap()).await.unwrap().value;
    assert!(kept);
    f.clean().await;
}
