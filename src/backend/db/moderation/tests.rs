use super::*;
use crate::{
    backend::{auth, db::fixtures},
    community::Reason,
};
struct Fixture {
    db: Database,
    site: Uuid,
    domain: String,
    admin: AuthenticatedSession,
    author: AuthenticatedSession,
    reporter: AuthenticatedSession,
    token: String,
    id: Uuid,
}
impl Fixture {
    async fn new() -> Self {
        let db = fixtures::database().await;
        let a = fixtures::member(&db).await;
        let b = fixtures::member(&db).await;
        let c = fixtures::member(&db).await;
        let admin = auth::get_session_details(&db, &a.token)
            .await
            .unwrap()
            .unwrap();
        let author = auth::get_session_details(&db, &b.token)
            .await
            .unwrap()
            .unwrap();
        let reporter = auth::get_session_details(&db, &c.token)
            .await
            .unwrap()
            .unwrap();
        db.set_admin_role(admin.member.id, true).await.unwrap();
        let domain = format!("moderation-{}.example.org", Uuid::new_v4().simple());
        let site = db.add_site(&domain).await.unwrap();
        let root = db
            .create_comment(
                &author,
                &domain,
                None,
                "원래 댓글 <script>alert(1)</script>".into(),
            )
            .await
            .unwrap();
        db.report_comment(
            &reporter,
            &domain,
            Uuid::parse_str(&root.id).unwrap(),
            Reason::Abuse,
            "신고 내용".into(),
        )
        .await
        .unwrap();
        let id = db
            .moderation_reports(&admin, "pending", 0)
            .await
            .unwrap()
            .reports
            .into_iter()
            .find(|r| r.domain.as_deref() == Some(&domain))
            .unwrap()
            .id;
        Self {
            db,
            site,
            domain,
            admin,
            author,
            reporter,
            token: b.token,
            id: Uuid::parse_str(&id).unwrap(),
        }
    }
    async fn detail(&self) -> ReportDetail {
        self.db
            .moderation_report(&self.admin, self.id)
            .await
            .unwrap()
    }
    async fn request(&self, action: Action) -> ActionRequest {
        request(self.detail().await, action)
    }
    async fn clean(self) {
        let mut conn = self.db.pool.get().await.unwrap();
        diesel::sql_query("DELETE FROM moderation_events WHERE report_id=$1 OR target_id=ANY($2)")
            .bind::<SqlUuid, _>(self.id)
            .bind::<diesel::sql_types::Array<SqlUuid>, _>(vec![
                self.admin.member.id,
                self.author.member.id,
                self.reporter.member.id,
            ])
            .execute(&mut conn)
            .await
            .unwrap();
        diesel::sql_query("DELETE FROM community_reports WHERE id=$1")
            .bind::<SqlUuid, _>(self.id)
            .execute(&mut conn)
            .await
            .unwrap();
        diesel::sql_query("DELETE FROM community_comments WHERE server_id=$1")
            .bind::<SqlUuid, _>(self.site)
            .execute(&mut conn)
            .await
            .unwrap();
        diesel::sql_query("DELETE FROM directory_sites WHERE id=$1")
            .bind::<SqlUuid, _>(self.site)
            .execute(&mut conn)
            .await
            .unwrap();
        drop(conn);
        fixtures::delete_members(
            &self.db,
            &[
                self.admin.member.id,
                self.author.member.id,
                self.reporter.member.id,
            ],
        )
        .await;
    }
}
fn request(d: ReportDetail, action: Action) -> ActionRequest {
    let c = d.comment.as_ref();
    ActionRequest {
        report: d.summary.id,
        revision: d.revision,
        comment_revision: c.map(|c| c.revision.clone()),
        author_id: c.and_then(|c| c.author_id.clone()),
        author_banned: c.and_then(|c| c.author_banned),
        author_revision: c.and_then(|c| c.author_revision.clone()),
        action,
        note: "검토한 조치 사유".into(),
    }
}
#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn role_checks_reads_writes_revocation_ban_session_and_freshness() {
    let f = Fixture::new().await;
    assert!(!f.db.moderation_access(&f.reporter).await.unwrap());
    assert_eq!(
        f.db.moderation_reports(&f.reporter, "all", 0).await,
        Err(Error::Forbidden)
    );
    assert_eq!(
        f.db.moderation_report(&f.author, f.id).await,
        Err(Error::Forbidden)
    );
    assert_eq!(
        f.db.moderation_events(&f.author, f.id, 0).await,
        Err(Error::Forbidden)
    );
    assert_eq!(
        f.db.moderate_report(&f.reporter, f.request(Action::Resolve).await)
            .await,
        Err(Error::Forbidden)
    );
    assert!(!f.db.set_admin_role(f.admin.member.id, true).await.unwrap());
    f.db.set_admin_role(f.admin.member.id, false).await.unwrap();
    assert_eq!(
        f.db.moderation_report(&f.admin, f.id).await,
        Err(Error::Forbidden)
    );
    f.db.set_admin_role(f.admin.member.id, true).await.unwrap();
    fixtures::age_session(&f.db, f.admin.id).await;
    assert!(f.db.moderation_report(&f.admin, f.id).await.is_ok());
    assert_eq!(
        f.db.moderate_report(&f.admin, f.request(Action::Resolve).await)
            .await,
        Err(Error::Auth(auth::AuthError::FreshAuthenticationRequired))
    );
    fixtures::ban(&f.db, f.admin.member.id, true).await;
    assert_eq!(
        f.db.moderation_access(&f.admin).await,
        Err(Error::Auth(auth::AuthError::Unauthenticated))
    );
    assert_eq!(
        f.db.set_admin_role(f.admin.member.id, true).await,
        Err(Error::Protected)
    );
    fixtures::ban(&f.db, f.admin.member.id, false).await;
    diesel::sql_query("DELETE FROM member_sessions WHERE id=$1")
        .bind::<SqlUuid, _>(f.admin.id)
        .execute(&mut f.db.pool.get().await.unwrap())
        .await
        .unwrap();
    assert_eq!(
        f.db.moderation_report(&f.admin, f.id).await,
        Err(Error::Auth(auth::AuthError::Unauthenticated))
    );
    f.clean().await;
}
#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn report_statuses_are_separate_from_deletion_and_have_cas_audit() {
    let f = Fixture::new().await;
    let d = f.detail().await;
    let original = d.comment.clone().unwrap();
    assert_eq!(d.evidence.as_ref().unwrap().body, original.body);
    let req = request(d, Action::Resolve);
    let (a, b) = tokio::join!(
        f.db.moderate_report(&f.admin, req.clone()),
        f.db.moderate_report(&f.admin, req.clone())
    );
    assert!(matches!(
        (&a, &b),
        (Ok(_), Err(Error::Conflict)) | (Err(Error::Conflict), Ok(_))
    ));
    assert_eq!(f.detail().await.comment.as_ref(), Some(&original));
    assert_eq!(
        f.db.moderation_events(&f.admin, f.id, 0)
            .await
            .unwrap()
            .events
            .len(),
        1
    );
    let reopened =
        f.db.moderate_report(&f.admin, f.request(Action::Reopen).await)
            .await
            .unwrap();
    assert!(reopened.resolved_at.is_none() && reopened.resolver_name.is_none());
    assert_eq!(reopened.summary.status, "pending");
    f.db.moderate_report(&f.admin, f.request(Action::Dismiss).await)
        .await
        .unwrap();
    let mut receiver = crate::backend::community::changes::subscribe();
    let deleted =
        f.db.moderate_report(&f.admin, f.request(Action::DeleteComment).await)
            .await
            .unwrap();
    assert!(deleted.comment.as_ref().unwrap().deleted);
    assert_eq!(deleted.summary.status, "dismissed");
    assert_eq!(deleted.evidence.unwrap().body, original.body);
    assert_eq!(receiver.try_recv().unwrap(), f.site);
    assert_eq!(
        f.db.comment_threads(&f.domain, 0).await.unwrap().threads[0]
            .comment
            .body,
        ""
    );
    let events = f.db.moderation_events(&f.admin, f.id, 0).await.unwrap();
    assert_eq!(events.events.len(), 4);
    assert_eq!(events.events[0].action, "delete_comment");
    assert!(events.events.iter().all(|e| e.note == "검토한 조치 사유"));
    let json = serde_json::to_string(&f.detail().await).unwrap();
    for secret in [
        "actor_url",
        "handle",
        "password_hash",
        "login_id",
        "token_hash",
    ] {
        assert!(!json.contains(secret))
    }
    f.clean().await;
}
#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn current_comment_edit_invalidates_decision_without_changing_report_evidence() {
    let f = Fixture::new().await;
    let before = f.detail().await;
    let req = request(before.clone(), Action::DeleteComment);
    let c = before.comment.as_ref().unwrap();
    f.db.change_comment(
        &f.author,
        &f.domain,
        Uuid::parse_str(&c.id).unwrap(),
        crate::backend::community::revision(&c.revision).unwrap(),
        Some("수정된 댓글".into()),
    )
    .await
    .unwrap();
    assert_eq!(
        f.db.moderate_report(&f.admin, req).await,
        Err(Error::Conflict)
    );
    let after = f.detail().await;
    assert_eq!(after.evidence, before.evidence);
    assert_eq!(after.comment.unwrap().body, "수정된 댓글");
    assert!(f
        .db
        .moderation_events(&f.admin, f.id, 0)
        .await
        .unwrap()
        .events
        .is_empty());
    diesel::sql_query("DELETE FROM community_report_evidence WHERE report_id=$1")
        .bind::<SqlUuid, _>(f.id)
        .execute(&mut f.db.pool.get().await.unwrap())
        .await
        .unwrap();
    assert!(f.detail().await.evidence.is_none());
    f.clean().await;
}
#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn banning_revokes_all_sessions_and_cannot_target_an_admin() {
    let f = Fixture::new().await;
    f.db.set_admin_role(f.author.member.id, true).await.unwrap();
    assert_eq!(
        f.db.moderate_report(&f.admin, f.request(Action::Ban).await)
            .await,
        Err(Error::Protected)
    );
    f.db.set_admin_role(f.author.member.id, false)
        .await
        .unwrap();
    let actor = format!(
        "https://moderation.example.org/users/{}",
        f.author.member.id
    );
    let mut conn = f.db.pool.get().await.unwrap();
    diesel::sql_query("INSERT INTO member_linked_accounts(id,member_id,actor_url,handle,display_name,profile_url,provider,provider_origin,provider_subject_id,verified_at) VALUES(gen_random_uuid(),$1,$2,$2,'fixture',$2,'activitypub_post','https://moderation.example.org',$2,now())").bind::<SqlUuid,_>(f.author.member.id).bind::<Text,_>(&actor).execute(&mut conn).await.unwrap();
    // An anonymous login proof does not cascade with session deletion.
    diesel::sql_query("INSERT INTO member_link_challenges(id,purpose,browser_binding_hash,code_hash,actor_url,handle,display_name,profile_url,outbox_url,expires_at) VALUES(gen_random_uuid(),'login',decode(repeat('01',32),'hex'),decode(replace(gen_random_uuid()::text,'-','')||replace(gen_random_uuid()::text,'-',''),'hex'),$1,$1,'fixture',$1,$1||'/outbox',now()+interval '10 minutes')").bind::<Text,_>(&actor).execute(&mut conn).await.unwrap();
    let old_handle = format!(
        "@legacy_{}@moderation.example.org",
        f.author.member.id.simple()
    );
    let old_actor = format!("https://legacy.example.org/users/{}", f.author.member.id);
    diesel::sql_query("INSERT INTO member_legacy_claims(member_id,handle) VALUES($1,$2)")
        .bind::<SqlUuid, _>(f.author.member.id)
        .bind::<Text, _>(&old_handle)
        .execute(&mut conn)
        .await
        .unwrap();
    diesel::sql_query("INSERT INTO member_link_challenges(id,purpose,browser_binding_hash,code_hash,actor_url,handle,display_name,profile_url,outbox_url,expires_at) VALUES(gen_random_uuid(),'login',decode(repeat('01',32),'hex'),decode(replace(gen_random_uuid()::text,'-','')||replace(gen_random_uuid()::text,'-',''),'hex'),$1,$2,'fixture',$1,$1||'/outbox',now()+interval '10 minutes')").bind::<Text,_>(&old_actor).bind::<Text,_>(&old_handle).execute(&mut conn).await.unwrap();
    drop(conn);
    let before = f.detail().await;
    let stale_ban = request(before.clone(), Action::Ban);
    let mut wrong = request(before.clone(), Action::Ban);
    wrong.author_id = Some(f.reporter.member.id.to_string());
    assert_eq!(
        f.db.moderate_report(&f.admin, wrong).await,
        Err(Error::Conflict)
    );
    let banned =
        f.db.moderate_report(&f.admin, request(before, Action::Ban))
            .await
            .unwrap();
    assert_eq!(banned.comment.as_ref().unwrap().author_banned, Some(true));
    let left = diesel::sql_query(
        "SELECT EXISTS(SELECT 1 FROM member_link_challenges WHERE actor_url=$1) AS value",
    )
    .bind::<Text, _>(&actor)
    .get_result::<Flag>(&mut f.db.pool.get().await.unwrap())
    .await
    .unwrap()
    .value;
    assert!(!left);
    let left = diesel::sql_query(
        "SELECT EXISTS(SELECT 1 FROM member_link_challenges WHERE actor_url=$1) AS value",
    )
    .bind::<Text, _>(&old_actor)
    .get_result::<Flag>(&mut f.db.pool.get().await.unwrap())
    .await
    .unwrap()
    .value;
    assert!(!left);
    assert!(auth::get_session_details(&f.db, &f.token)
        .await
        .unwrap()
        .is_none());
    assert_eq!(
        f.db.create_comment(&f.author, &f.domain, None, "거절".into())
            .await,
        Err(crate::backend::community::Error::Auth(
            auth::AuthError::Unauthenticated
        ))
    );
    f.db.moderate_report(&f.admin, request(banned, Action::Unban))
        .await
        .unwrap();
    assert!(auth::get_session_details(&f.db, &f.token)
        .await
        .unwrap()
        .is_none());
    assert_eq!(f.detail().await.comment.unwrap().author_banned, Some(false));
    // active -> banned -> active must not make a former decision current again.
    assert_eq!(
        f.db.moderate_report(&f.admin, stale_ban).await,
        Err(Error::Conflict)
    );
    diesel::sql_query("DELETE FROM member_legacy_claims WHERE member_id=$1")
        .bind::<SqlUuid, _>(f.author.member.id)
        .execute(&mut f.db.pool.get().await.unwrap())
        .await
        .unwrap();
    f.clean().await;
}
#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn audit_survives_withdrawal_and_hidden_site_reports_remain_reviewable() {
    let f = Fixture::new().await;
    diesel::sql_query("UPDATE directory_sites SET is_hidden=true WHERE id=$1")
        .bind::<SqlUuid, _>(f.site)
        .execute(&mut f.db.pool.get().await.unwrap())
        .await
        .unwrap();
    f.db.moderate_report(&f.admin, f.request(Action::Resolve).await)
        .await
        .unwrap();
    let stale = f.request(Action::Ban).await;
    f.db.withdraw_member(&f.author).await.unwrap();
    assert_eq!(
        f.db.moderate_report(&f.admin, stale).await,
        Err(Error::Conflict)
    );
    let d = f.detail().await;
    assert!(d.comment.unwrap().author_id.is_none());
    assert!(d.evidence.is_some());
    f.db.set_admin_role(f.reporter.member.id, true)
        .await
        .unwrap();
    f.db.withdraw_member(&f.admin).await.unwrap();
    let e = f.db.moderation_events(&f.reporter, f.id, 0).await.unwrap();
    assert_eq!(e.events.len(), 1);
    assert!(e.events[0].actor_name.is_none());
    f.clean().await;
}
#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn inputs_and_pages_are_bounded_without_mutating_reports() {
    let f = Fixture::new().await;
    assert_eq!(
        f.db.moderation_reports(&f.admin, "injected", 0).await,
        Err(Error::Invalid)
    );
    assert_eq!(
        f.db.moderation_reports(&f.admin, "all", 10001).await,
        Err(Error::Invalid)
    );
    assert_eq!(
        f.db.moderation_events(&f.admin, f.id, 10001).await,
        Err(Error::Invalid)
    );
    for note in ["".into(), "한".repeat(1001), "x\0y".into()] {
        let mut req = f.request(Action::Resolve).await;
        req.note = note;
        assert_eq!(
            f.db.moderate_report(&f.admin, req).await,
            Err(Error::Invalid)
        );
    }
    let mut conn = f.db.pool.get().await.unwrap();
    diesel::sql_query("INSERT INTO moderation_events(id,actor_id,target_kind,target_id,report_id,action,before_state,after_state,note) SELECT gen_random_uuid(),$1,'report',$2,$2,'resolve','pending','resolved','fixture event' FROM generate_series(1,25)").bind::<SqlUuid,_>(f.admin.member.id).bind::<SqlUuid,_>(f.id).execute(&mut conn).await.unwrap();
    drop(conn);
    let first = f.db.moderation_events(&f.admin, f.id, 0).await.unwrap();
    let next = f.db.moderation_events(&f.admin, f.id, 1).await.unwrap();
    assert_eq!(first.events.len(), 20);
    assert!(first.has_next);
    assert_eq!(next.events.len(), 5);
    assert!(!next.has_next);
    assert!(first
        .events
        .iter()
        .all(|r| next.events.iter().all(|n| n.id != r.id)));
    f.clean().await;
}
