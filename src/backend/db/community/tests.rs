use super::*;
use crate::backend::{auth, db::fixtures};

struct Fixture {
    db: Database,
    site: Uuid,
    domain: String,
    s: AuthenticatedSession,
    other: AuthenticatedSession,
}
impl Fixture {
    async fn new() -> Self {
        let db = fixtures::database().await;
        let a = fixtures::member(&db).await;
        let b = fixtures::member(&db).await;
        let s = auth::get_session_details(&db, &a.token)
            .await
            .unwrap()
            .unwrap();
        let other = auth::get_session_details(&db, &b.token)
            .await
            .unwrap()
            .unwrap();
        let domain = format!("community-{}.example.org", Uuid::new_v4().simple());
        let site = db.add_site(&domain).await.unwrap();
        Self {
            db,
            site,
            domain,
            s,
            other,
        }
    }
    async fn root(&self) -> Comment {
        self.db
            .create_comment(
                &self.s,
                &self.domain,
                None,
                "첫 댓글 <script> &\n둘째 줄".into(),
            )
            .await
            .unwrap()
    }
    async fn sql(&self, sql: &str) {
        diesel::sql_query(sql)
            .bind::<SqlUuid, _>(self.site)
            .execute(&mut self.db.pool.get().await.unwrap())
            .await
            .unwrap();
    }
    async fn flag(&self, sql: &str) -> bool {
        diesel::sql_query(sql)
            .bind::<SqlUuid, _>(self.site)
            .get_result::<Flag>(&mut self.db.pool.get().await.unwrap())
            .await
            .unwrap()
            .value
    }
    async fn age(&self) {
        self.sql("UPDATE community_comments SET inserted_at=inserted_at-interval '61 seconds' WHERE server_id=$1").await;
    }
    async fn clean(self) {
        let mut conn = self.db.pool.get().await.unwrap();
        (&mut *conn).transaction::<_,diesel::result::Error,_>(async move |conn|{
            diesel::sql_query("DELETE FROM community_reports WHERE comment_id IN (SELECT id FROM community_comments WHERE server_id=$1)").bind::<SqlUuid,_>(self.site).execute(conn).await?;
            diesel::sql_query("DELETE FROM community_comments WHERE server_id=$1").bind::<SqlUuid,_>(self.site).execute(conn).await?;
            diesel::sql_query("DELETE FROM directory_sites WHERE id=$1").bind::<SqlUuid,_>(self.site).execute(conn).await?;
            Ok(())
        }).await.unwrap();
        drop(conn);
        fixtures::delete_members(&self.db, &[self.s.member.id, self.other.member.id]).await;
    }
}
fn id(c: &Comment) -> Uuid {
    Uuid::parse_str(&c.id).unwrap()
}
fn rev(c: &Comment) -> NaiveDateTime {
    domain::revision(&c.revision).unwrap()
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn own_comments_are_private_paginated_and_preserve_hidden_site_authors_history() {
    let f = Fixture::new().await;
    assert!(f
        .db
        .own_comments(&f.s, 0)
        .await
        .unwrap()
        .comments
        .is_empty());
    let root = f.root().await;
    f.db.create_comment(
        &f.other,
        &f.domain,
        Some(id(&root)),
        "other author secret".into(),
    )
    .await
    .unwrap();
    let mut conn = f.db.pool.get().await.unwrap();
    diesel::sql_query("INSERT INTO community_comments(id,user_id,server_id,parent_id,body,is_deleted,inserted_at,updated_at) SELECT gen_random_uuid(),$1,$2,CASE WHEN n=1 THEN $3::uuid END,'Own history '||n,false,(now() AT TIME ZONE 'UTC')-interval '1 minute'*n,now() AT TIME ZONE 'UTC' FROM generate_series(1,24) n")
        .bind::<SqlUuid,_>(f.s.member.id).bind::<SqlUuid,_>(f.site).bind::<SqlUuid,_>(id(&root)).execute(&mut conn).await.unwrap();
    diesel::sql_query("INSERT INTO community_comments(id,user_id,server_id,body,is_deleted,inserted_at,updated_at) VALUES(gen_random_uuid(),$1,$2,'Deleted private body',true,now() AT TIME ZONE 'UTC',now() AT TIME ZONE 'UTC')")
        .bind::<SqlUuid,_>(f.s.member.id).bind::<SqlUuid,_>(f.site).execute(&mut conn).await.unwrap();
    diesel::sql_query("UPDATE community_comments SET body=repeat('가',2001) WHERE id=$1")
        .bind::<SqlUuid, _>(id(&root))
        .execute(&mut conn)
        .await
        .unwrap();
    drop(conn);
    let first = f.db.own_comments(&f.s, 0).await.unwrap();
    let second = f.db.own_comments(&f.s, 1).await.unwrap();
    assert_eq!(first.comments.len(), 20);
    assert!(first.has_next);
    assert_eq!(second.comments.len(), 5);
    assert!(!second.has_next);
    assert!(second
        .comments
        .iter()
        .all(|c| !first.comments.iter().any(|a| a.id == c.id)));
    assert!(first.comments.iter().any(|c| c.reply));
    assert_eq!(first.comments[0].id, root.id);
    assert!(first.comments[0].truncated);
    assert_eq!(first.comments[0].body.chars().count(), 2000);
    let json = serde_json::to_string(&first).unwrap();
    for private in [
        "other author secret",
        "Deleted private body",
        "user_id",
        "handle",
        "actor_url",
        "login_id",
    ] {
        assert!(!json.contains(private));
    }
    assert!(first
        .comments
        .iter()
        .all(|c| c.server_available && c.domain.as_deref() == Some(f.domain.as_str())));
    for flags in ["is_hidden=true", "is_hidden=false,is_force_hidden=true"] {
        f.sql(&format!("UPDATE directory_sites SET {flags} WHERE id=$1"))
            .await;
        let hidden = f.db.own_comments(&f.s, 0).await.unwrap();
        assert_eq!(hidden.comments.len(), 20);
        assert!(hidden
            .comments
            .iter()
            .all(|c| !c.server_available && c.domain.as_deref() == Some(f.domain.as_str())));
    }
    let mut forged = f.s.clone();
    forged.id = f.other.id;
    assert_eq!(
        f.db.own_comments(&forged, 0).await,
        Err(Error::Auth(auth::AuthError::Unauthenticated))
    );
    fixtures::ban(&f.db, f.s.member.id, true).await;
    assert_eq!(
        f.db.own_comments(&f.s, 0).await,
        Err(Error::Auth(auth::AuthError::Unauthenticated))
    );
    assert!(f.db.own_comments(&f.other, 10_001).await.is_err());
    f.clean().await;
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn public_comments_private_permissions_and_soft_delete_preserve_replies() {
    let f = Fixture::new().await;
    let root = f.root().await;
    assert_eq!(root.created_at, root.updated_at);
    let reply =
        f.db.create_comment(&f.other, &f.domain, Some(id(&root)), "답글".into())
            .await
            .unwrap();
    let public = f.db.comment_threads(&f.domain, 0).await.unwrap();
    assert_eq!(public.threads.len(), 1);
    assert_eq!(public.threads[0].replies, vec![reply.clone()]);
    let json = serde_json::to_string(&public).unwrap();
    for secret in [
        f.s.member.id.to_string(),
        f.other.member.id.to_string(),
        "actor_url".into(),
        "handle".into(),
        "user_id".into(),
        "login_id".into(),
    ] {
        assert!(!json.contains(&secret));
    }
    assert!(json.contains("<script>")); // Plain text DTO; the component must escape it.
    let own =
        f.db.comment_access(&f.s, &f.domain, &[id(&root), id(&reply)])
            .await
            .unwrap();
    assert!(own
        .permissions
        .iter()
        .any(|p| p.id == root.id && p.own && p.can_edit && !p.can_report));
    assert!(own
        .permissions
        .iter()
        .any(|p| p.id == reply.id && !p.own && !p.can_edit && p.can_report));
    assert_eq!(
        f.db.change_comment(&f.other, &f.domain, id(&root), rev(&root), None)
            .await,
        Err(Error::Missing)
    );
    let deleted =
        f.db.change_comment(&f.s, &f.domain, id(&root), rev(&root), None)
            .await
            .unwrap();
    assert!(deleted.deleted && deleted.body.is_empty() && deleted.author_name.is_empty());
    let after = f.db.comment_threads(&f.domain, 0).await.unwrap();
    assert_eq!(after.threads[0].comment, deleted);
    assert_eq!(after.threads[0].replies, vec![reply]);
    assert!(f.flag("SELECT EXISTS(SELECT 1 FROM community_comments WHERE server_id=$1 AND parent_id IS NULL AND is_deleted AND length(body)>0) AS value").await);
    assert_eq!(
        f.db.create_comment(&f.s, &f.domain, None, "다시 작성".into())
            .await,
        Err(Error::TooSoon)
    );
    f.age().await;
    assert_eq!(
        f.db.create_comment(&f.s, &f.domain, Some(id(&root)), "삭제된 글에 답글".into())
            .await,
        Err(Error::Missing)
    );
    f.clean().await;
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn comment_rate_serializes_and_replies_are_one_level_same_visible_site() {
    let f = Fixture::new().await;
    let (a, b) = tokio::join!(
        f.db.create_comment(&f.s, &f.domain, None, "a".into()),
        f.db.create_comment(&f.s, &f.domain, None, "b".into())
    );
    assert!(matches!(
        (&a, &b),
        (Ok(_), Err(Error::TooSoon)) | (Err(Error::TooSoon), Ok(_))
    ));
    let root = a.or(b).unwrap();
    let reply =
        f.db.create_comment(&f.other, &f.domain, Some(id(&root)), "reply".into())
            .await
            .unwrap();
    f.age().await;
    assert_eq!(
        f.db.create_comment(&f.s, &f.domain, Some(id(&reply)), "depth two".into())
            .await,
        Err(Error::Missing)
    );
    let second = Fixture::new().await;
    assert_eq!(
        second
            .db
            .create_comment(&f.s, &second.domain, Some(id(&root)), "wrong site".into())
            .await,
        Err(Error::Missing)
    );
    // A recent comment in the first site does not throttle a different site.
    second
        .db
        .create_comment(&f.s, &second.domain, None, "other site".into())
        .await
        .unwrap();
    second.clean().await;
    f.sql("UPDATE directory_sites SET is_hidden=true WHERE id=$1")
        .await;
    assert_eq!(
        f.db.comment_threads(&f.domain, 0).await,
        Err(Error::Missing)
    );
    assert_eq!(
        f.db.comment_replies(&f.domain, id(&root), 0).await,
        Err(Error::Missing)
    );
    assert_eq!(
        f.db.comment_access(&f.s, &f.domain, &[id(&root)]).await,
        Err(Error::Missing)
    );
    assert_eq!(f.db.comment_site(&f.domain).await, Err(Error::Missing));
    assert_eq!(
        f.db.create_comment(&f.s, &f.domain, None, "hidden".into())
            .await,
        Err(Error::Missing)
    );
    f.sql("UPDATE directory_sites SET is_hidden=false,is_force_hidden=true WHERE id=$1")
        .await;
    assert_eq!(f.db.comment_site(&f.domain).await, Err(Error::Missing));
    f.clean().await;
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn edit_compare_and_swap_deadline_and_old_session_revocation() {
    let f = Fixture::new().await;
    let root = f.root().await;
    let (a, b) = tokio::join!(
        f.db.change_comment(
            &f.s,
            &f.domain,
            id(&root),
            rev(&root),
            Some("edit a".into())
        ),
        f.db.change_comment(
            &f.s,
            &f.domain,
            id(&root),
            rev(&root),
            Some("edit b".into())
        )
    );
    assert!(matches!(
        (&a, &b),
        (Ok(_), Err(Error::Conflict)) | (Err(Error::Conflict), Ok(_))
    ));
    let current = a.or(b).unwrap();
    assert_ne!(current.revision, root.revision);
    assert_eq!(
        f.db.change_comment(&f.s, &f.domain, id(&root), rev(&root), None)
            .await,
        Err(Error::Conflict)
    );
    f.sql("UPDATE community_comments SET inserted_at=inserted_at-interval '31 minutes' WHERE server_id=$1").await;
    assert_eq!(
        f.db.change_comment(
            &f.s,
            &f.domain,
            id(&current),
            rev(&current),
            Some("too late".into())
        )
        .await,
        Err(Error::TooLate)
    );
    let rights =
        f.db.comment_access(&f.s, &f.domain, &[id(&root)])
            .await
            .unwrap();
    assert!(rights.permissions[0].own && !rights.permissions[0].can_edit);
    fixtures::ban(&f.db, f.s.member.id, true).await;
    assert!(matches!(
        f.db.change_comment(&f.s, &f.domain, id(&current), rev(&current), None)
            .await,
        Err(Error::Auth(_))
    ));
    fixtures::ban(&f.db, f.s.member.id, false).await;
    f.db.change_comment(&f.s, &f.domain, id(&current), rev(&current), None)
        .await
        .unwrap();
    diesel::sql_query("DELETE FROM member_sessions WHERE id=$1")
        .bind::<SqlUuid, _>(f.s.id)
        .execute(&mut f.db.pool.get().await.unwrap())
        .await
        .unwrap();
    assert!(matches!(
        f.db.create_comment(&f.s, &f.domain, None, "revoked".into())
            .await,
        Err(Error::Auth(_))
    ));
    f.clean().await;
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn private_report_evidence_duplicate_protection_and_retained_original() {
    let f = Fixture::new().await;
    let root = f.root().await;
    assert_eq!(
        f.db.report_comment(&f.s, &f.domain, id(&root), Reason::Spam, "".into())
            .await,
        Err(Error::Missing)
    );
    assert_eq!(
        f.db.report_comment(
            &f.other,
            &f.domain,
            id(&root),
            Reason::Other,
            "가".repeat(501)
        )
        .await,
        Err(Error::Invalid)
    );
    let (a, b) = tokio::join!(
        f.db.report_comment(
            &f.other,
            &f.domain,
            id(&root),
            Reason::Abuse,
            "private reason".into()
        ),
        f.db.report_comment(
            &f.other,
            &f.domain,
            id(&root),
            Reason::Abuse,
            "private reason".into()
        )
    );
    assert!(matches!(
        (&a, &b),
        (Ok(()), Err(Error::DuplicateReport)) | (Err(Error::DuplicateReport), Ok(()))
    ));
    assert!(f.flag("SELECT EXISTS(SELECT 1 FROM community_reports r JOIN community_report_evidence e ON e.report_id=r.id JOIN community_comments c ON c.id=r.comment_id WHERE c.server_id=$1 AND r.status='pending' AND r.detail='private reason' AND e.body=c.body AND e.comment_updated_at=c.updated_at) AS value").await);
    let rights =
        f.db.comment_access(&f.other, &f.domain, &[id(&root)])
            .await
            .unwrap();
    assert!(!rights.permissions[0].can_report);
    let updated =
        f.db.change_comment(
            &f.s,
            &f.domain,
            id(&root),
            rev(&root),
            Some("edited".into()),
        )
        .await
        .unwrap();
    f.db.change_comment(&f.s, &f.domain, id(&root), rev(&updated), None)
        .await
        .unwrap();
    assert!(f.flag("SELECT EXISTS(SELECT 1 FROM community_report_evidence e JOIN community_reports r ON r.id=e.report_id JOIN community_comments c ON c.id=r.comment_id WHERE c.server_id=$1 AND c.is_deleted AND e.body<>c.body AND e.comment_updated_at<>c.updated_at) AS value").await);
    let json = serde_json::to_string(&f.db.comment_threads(&f.domain, 0).await.unwrap()).unwrap();
    for private in [
        "private reason",
        "<script>",
        "edited",
        "reporter_id",
        "admin_note",
    ] {
        assert!(!json.contains(private));
    }
    assert_eq!(
        f.db.report_comment(&f.other, &f.domain, id(&root), Reason::Other, "".into())
            .await,
        Err(Error::Missing)
    );
    f.clean().await;
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn bounded_thread_reply_pages_legacy_tombstones_and_text_limits() {
    let f = Fixture::new().await;
    let root = f.root().await;
    // Exact fixture site only. IDs generated by PostgreSQL, not production data.
    f.sql("INSERT INTO community_comments(id,user_id,server_id,body,is_deleted,inserted_at,updated_at) SELECT gen_random_uuid(),c.user_id,c.server_id,'root '||n,false,c.inserted_at-interval '1 hour'*n,c.updated_at FROM community_comments c CROSS JOIN generate_series(1,20) n WHERE c.server_id=$1 AND c.parent_id IS NULL").await;
    let mut conn = f.db.pool.get().await.unwrap();
    diesel::sql_query("INSERT INTO community_comments(id,user_id,server_id,parent_id,body,is_deleted,inserted_at,updated_at) SELECT gen_random_uuid(),$2,$1,$3,'reply '||n,false,now() AT TIME ZONE 'UTC'+interval '1 microsecond'*n,now() AT TIME ZONE 'UTC' FROM generate_series(1,21) n").bind::<SqlUuid,_>(f.site).bind::<SqlUuid,_>(f.other.member.id).bind::<SqlUuid,_>(id(&root)).execute(&mut conn).await.unwrap();
    drop(conn);
    let first = f.db.comment_threads(&f.domain, 0).await.unwrap();
    let second = f.db.comment_threads(&f.domain, 1).await.unwrap();
    assert_eq!(first.threads.len(), 20);
    assert!(first.has_next);
    assert_eq!(second.threads.len(), 1);
    assert!(!second.has_next);
    assert_eq!(first.threads[0].comment.id, root.id);
    assert_eq!(first.threads[0].replies.len(), 3);
    assert_eq!(first.threads[0].reply_count, 21);
    let replies = f.db.comment_replies(&f.domain, id(&root), 0).await.unwrap();
    assert_eq!(replies.replies.len(), 20);
    assert!(replies.has_next);
    assert_eq!(replies.replies[0].body, "reply 1");
    assert_eq!(
        f.db.comment_replies(&f.domain, id(&root), 1)
            .await
            .unwrap()
            .replies[0]
            .body,
        "reply 21"
    );
    assert_eq!(
        f.db.comment_threads(&f.domain, 10_001).await,
        Err(Error::Invalid)
    );
    assert_eq!(
        f.db.comment_access(&f.s, &f.domain, &vec![id(&root); 101])
            .await,
        Err(Error::Invalid)
    );
    f.sql("UPDATE community_comments SET body=repeat('가',2001) WHERE server_id=$1 AND parent_id IS NULL").await;
    let long = f.db.comment_threads(&f.domain, 0).await.unwrap();
    assert!(long.threads[0].comment.truncated);
    assert_eq!(long.threads[0].comment.body.chars().count(), 2000);
    f.sql("UPDATE community_comments SET user_id=NULL WHERE server_id=$1 AND parent_id IS NULL")
        .await;
    let orphan = f.db.comment_threads(&f.domain, 0).await.unwrap();
    assert!(orphan.threads[0].comment.deleted);
    assert!(orphan.threads[0].comment.body.is_empty());
    assert!(!orphan.threads[0].comment.truncated);
    f.clean().await;
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn committed_changes_wake_readers_without_content_and_report_flood_is_bounded() {
    let f = Fixture::new().await;
    let mut changes = domain::changes::subscribe();
    let root = f.root().await;
    let changed = tokio::time::timeout(std::time::Duration::from_secs(1), async {
        loop {
            if changes.recv().await.unwrap() == f.site {
                break;
            }
        }
    })
    .await;
    assert!(changed.is_ok());
    assert_eq!(
        f.db.comment_threads(&f.domain, 0).await.unwrap().threads[0].comment,
        root
    );
    f.sql("INSERT INTO community_comments(id,user_id,server_id,body,is_deleted,inserted_at,updated_at) SELECT gen_random_uuid(),c.user_id,c.server_id,'rate fixture '||n,false,c.inserted_at-interval '1 hour'*n,c.updated_at FROM community_comments c CROSS JOIN generate_series(1,30) n WHERE c.server_id=$1 AND c.parent_id IS NULL").await;
    let mut conn = f.db.pool.get().await.unwrap();
    let ids = diesel::sql_query("SELECT id FROM community_comments WHERE server_id=$1")
        .bind::<SqlUuid, _>(f.site)
        .load::<Id>(&mut conn)
        .await
        .unwrap();
    drop(conn);
    for c in ids.iter().take(30) {
        f.db.report_comment(&f.other, &f.domain, c.id, Reason::Spam, String::new())
            .await
            .unwrap();
    }
    assert_eq!(
        f.db.report_comment(&f.other, &f.domain, ids[30].id, Reason::Spam, String::new())
            .await,
        Err(Error::ReportRate)
    );
    f.clean().await;
}
