use super::*;
use crate::backend::{
    auth,
    db::fixtures,
    directory::{CrawlJob, Observation},
};
use chrono::Utc;
use diesel::sql_types::{Bool, Text, Uuid as SqlUuid};

struct Fixture {
    db: Database,
    admin: AuthenticatedSession,
    owner: AuthenticatedSession,
    site: Uuid,
    domain: String,
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
        let owner = auth::get_session_details(&db, &b.token)
            .await
            .unwrap()
            .unwrap();
        db.set_admin_role(admin.member.id, true).await.unwrap();
        let domain = format!("site-admin-{}.example.org", Uuid::new_v4().simple());
        let site = db.add_site(&domain).await.unwrap();
        diesel::sql_query("INSERT INTO directory_site_details(site_id,owner_id,owner_method,rules,language,tags,owner_comment,approval_required) VALUES($1,$2,'dns','rules preserved','ko',ARRAY['original'],'owner comment',true)")
            .bind::<SqlUuid,_>(site).bind::<SqlUuid,_>(owner.member.id).execute(&mut db.pool.get().await.unwrap()).await.unwrap();
        diesel::sql_query("UPDATE directory_sites SET name='editorial name',description='editorial description' WHERE id=$1")
            .bind::<SqlUuid,_>(site).execute(&mut db.pool.get().await.unwrap()).await.unwrap();
        Self {
            db,
            admin,
            owner,
            site,
            domain,
        }
    }
    async fn detail(&self) -> Site {
        self.db
            .moderation_site(&self.admin, self.site)
            .await
            .unwrap()
    }
    async fn request(&self, action: Action) -> Request {
        Request {
            id: self.site.to_string(),
            revision: self.detail().await.revision,
            action,
            note: "검토 후 변경 <script>실행 금지</script>".into(),
        }
    }
    async fn act(&self, action: Action) -> Site {
        self.db
            .moderate_site(&self.admin, self.request(action).await)
            .await
            .unwrap()
    }
    async fn raw(&self) -> serde_json::Value {
        let row=diesel::sql_query("SELECT jsonb_build_object('site',to_jsonb(s),'details',to_jsonb(d))::text AS value FROM directory_sites s JOIN directory_site_details d ON d.site_id=s.id WHERE s.id=$1")
            .bind::<SqlUuid,_>(self.site).get_result::<JsonRow>(&mut self.db.pool.get().await.unwrap()).await.unwrap();
        dto(row).unwrap()
    }
    async fn clean(self) {
        let mut conn = self.db.pool.get().await.unwrap();
        diesel::sql_query("DELETE FROM directory_sites WHERE id=$1")
            .bind::<SqlUuid, _>(self.site)
            .execute(&mut conn)
            .await
            .unwrap();
        diesel::sql_query("DELETE FROM moderation_events WHERE target_id=ANY($1)")
            .bind::<Array<SqlUuid>, _>(vec![self.site, self.admin.member.id, self.owner.member.id])
            .execute(&mut conn)
            .await
            .unwrap();
        drop(conn);
        fixtures::delete_members(&self.db, &[self.admin.member.id, self.owner.member.id]).await;
    }
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn site_delete_requires_admin_fresh_exact_confirmation_and_current_revision() {
    let f = Fixture::new().await;
    let current = f.detail().await;
    let request = DeleteRequest {
        id: f.site.to_string(),
        revision: current.revision,
        domain: f.domain.clone(),
        note: "physical removal reviewed".into(),
        confirmed: true,
    };
    assert_eq!(
        f.db.delete_moderated_site(&f.owner, request.clone()).await,
        Err(Error::Forbidden)
    );
    let mut malformed = request.clone();
    malformed.confirmed = false;
    assert_eq!(
        f.db.delete_moderated_site(&f.admin, malformed).await,
        Err(Error::Invalid)
    );
    let mut wrong_domain = request.clone();
    wrong_domain.domain = "other.example.org".into();
    assert_eq!(
        f.db.delete_moderated_site(&f.admin, wrong_domain).await,
        Err(Error::Missing)
    );
    fixtures::age_session(&f.db, f.admin.id).await;
    assert_eq!(
        f.db.delete_moderated_site(&f.admin, request.clone()).await,
        Err(Error::Auth(auth::AuthError::FreshAuthenticationRequired))
    );
    // The freshness fixture deliberately ages both timestamps. Restore them as
    // one coherent authentication record: the session constraint requires
    // authenticated_at <= created_at.
    diesel::sql_query("UPDATE member_sessions SET created_at=now(),authenticated_at=now(),federated_authenticated_at=NULL WHERE id=$1")
        .bind::<SqlUuid, _>(f.admin.id)
        .execute(&mut f.db.pool.get().await.unwrap())
        .await
        .unwrap();
    f.act(Action::ForceHidden(true)).await;
    assert_eq!(
        f.db.delete_moderated_site(&f.admin, request).await,
        Err(Error::Conflict)
    );
    assert!(f.db.moderation_site(&f.admin, f.site).await.is_ok());
    f.clean().await;
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn site_delete_removes_only_target_runtime_rows_retains_reports_and_fences_old_workers() {
    let f = Fixture::new().await;
    let other_domain = format!("site-delete-other-{}.example.org", Uuid::new_v4().simple());
    let other_site = f.db.add_site(&other_domain).await.unwrap();
    let target_comment = Uuid::new_v4();
    let target_reply = Uuid::new_v4();
    let foreign_reply = Uuid::new_v4();
    let reported = Uuid::new_v4();
    let unrelated_report = Uuid::new_v4();
    let health = Uuid::new_v4();
    let legacy_health = Uuid::new_v4();
    let worker_job = Uuid::new_v4();
    let lease = Uuid::new_v4();
    let favicon = format!("favicons/shared-delete-{}.ico", Uuid::new_v4().simple());
    let hash = "a".repeat(64);
    let owner_challenge = Uuid::new_v4();
    let other_challenge = Uuid::new_v4();
    let mut conn = f.db.pool.get().await.unwrap();
    diesel::sql_query("INSERT INTO legacy_sites(id,domain,is_force_hidden,favicon_key,inserted_at,updated_at) VALUES($1,$2,false,$3,now(),now()),($4,$5,false,$3,now(),now())")
        .bind::<SqlUuid,_>(f.site).bind::<Text,_>(&f.domain).bind::<Text,_>(&favicon)
        .bind::<SqlUuid,_>(other_site).bind::<Text,_>(&other_domain).execute(&mut conn).await.unwrap();
    diesel::sql_query("INSERT INTO stored_files(object_key,sha256,byte_count) VALUES($1,$2,19)")
        .bind::<Text, _>(&favicon)
        .bind::<Text, _>(&hash)
        .execute(&mut conn)
        .await
        .unwrap();
    diesel::sql_query("INSERT INTO directory_health_checks(job_id,site_id,is_alive,checked_at) VALUES($1,$2,true,now())")
        .bind::<SqlUuid,_>(health).bind::<SqlUuid,_>(f.site).execute(&mut conn).await.unwrap();
    diesel::sql_query("INSERT INTO legacy_health_checks(id,server_id,is_alive,checked_at) VALUES($1,$2,true,now())")
        .bind::<SqlUuid,_>(legacy_health).bind::<SqlUuid,_>(f.site).execute(&mut conn).await.unwrap();
    diesel::sql_query("INSERT INTO community_comments(id,user_id,server_id,parent_id,body,is_deleted,inserted_at,updated_at) VALUES($1,$2,$3,NULL,'target parent',false,now(),now()),($4,$2,$3,$1,'target reply',false,now(),now()),($5,$2,$6,$1,'cross-site reply',false,now(),now())")
        .bind::<SqlUuid,_>(target_comment).bind::<SqlUuid,_>(f.owner.member.id).bind::<SqlUuid,_>(f.site)
        .bind::<SqlUuid,_>(target_reply).bind::<SqlUuid,_>(foreign_reply).bind::<SqlUuid,_>(other_site)
        .execute(&mut conn).await.unwrap();
    diesel::sql_query("INSERT INTO community_reports(id,reporter_id,comment_id,reason,status,inserted_at,updated_at) VALUES($1,$2,$3,'target report','pending',now(),now()),($4,$2,NULL,'unrelated report','pending',now(),now())")
        .bind::<SqlUuid,_>(reported).bind::<SqlUuid,_>(f.owner.member.id).bind::<SqlUuid,_>(target_comment)
        .bind::<SqlUuid,_>(unrelated_report).execute(&mut conn).await.unwrap();
    diesel::sql_query("INSERT INTO community_report_evidence(report_id,body,author_name,comment_updated_at) VALUES($1,'saved evidence','fixture',now())")
        .bind::<SqlUuid,_>(reported).execute(&mut conn).await.unwrap();
    diesel::sql_query("INSERT INTO directory_owner_challenges(id,member_id,session_id,domain,code_hash) VALUES($1,$2,$3,$4,decode(repeat('00',32),'hex')),($5,$2,$3,$6,decode(repeat('01',32),'hex'))")
        .bind::<SqlUuid,_>(owner_challenge).bind::<SqlUuid,_>(f.owner.member.id).bind::<SqlUuid,_>(f.owner.id).bind::<Text,_>(&f.domain)
        .bind::<SqlUuid,_>(other_challenge).bind::<Text,_>(&other_domain).execute(&mut conn).await.unwrap();
    diesel::sql_query("INSERT INTO directory_jobs(id,site_id,scheduled_at,state,attempts,lease_token,lease_until) VALUES($1,$2,now(),'running',1,$3,now()+interval '2 minutes')")
        .bind::<SqlUuid,_>(worker_job).bind::<SqlUuid,_>(f.site).bind::<SqlUuid,_>(lease).execute(&mut conn).await.unwrap();
    drop(conn);

    let impact =
        f.db.moderation_site_delete_impact(&f.admin, f.site)
            .await
            .unwrap();
    assert_eq!(
        (impact.comments, impact.reports, impact.health_checks),
        (2, 1, 1)
    );
    f.db.delete_moderated_site(
        &f.admin,
        DeleteRequest {
            id: f.site.to_string(),
            revision: impact.site.revision,
            domain: f.domain.clone(),
            note: "remove imported runtime target".into(),
            confirmed: true,
        },
    )
    .await
    .unwrap();

    let old = CrawlJob {
        id: worker_job,
        site_id: f.site,
        domain: f.domain.clone(),
        lease_token: lease,
        needs_icon: false,
    };
    let result = Observation {
        alive: true,
        response_ms: None,
        status: None,
        health_error: None,
        checked_at: Utc::now(),
        nodeinfo: None,
        nodeinfo_error: None,
        icon: None,
        icon_collection_complete: false,
    };
    assert!(!f.db.finish_site(&old, &result).await.unwrap());
    let replacement = f.db.add_site(&f.domain).await.unwrap();
    assert_ne!(replacement, f.site);
    assert!(!f.db.finish_site(&old, &result).await.unwrap());

    #[derive(QueryableByName)]
    struct Check {
        #[diesel(sql_type=Bool)]
        valid: bool,
    }
    let check = diesel::sql_query("SELECT NOT EXISTS(SELECT 1 FROM directory_sites WHERE id=$1) AND NOT EXISTS(SELECT 1 FROM legacy_sites WHERE id=$1) AND NOT EXISTS(SELECT 1 FROM legacy_health_checks WHERE server_id=$1) AND NOT EXISTS(SELECT 1 FROM directory_health_checks WHERE site_id=$1) AND NOT EXISTS(SELECT 1 FROM directory_jobs WHERE site_id=$1) AND NOT EXISTS(SELECT 1 FROM community_comments WHERE server_id=$1) AND (SELECT comment_id IS NULL FROM community_reports WHERE id=$2) AND EXISTS(SELECT 1 FROM community_report_evidence WHERE report_id=$2) AND (SELECT parent_id IS NULL AND server_id=$3 FROM community_comments WHERE id=$4) AND EXISTS(SELECT 1 FROM community_reports WHERE id=$5) AND NOT EXISTS(SELECT 1 FROM directory_owner_challenges WHERE id=$6) AND EXISTS(SELECT 1 FROM directory_owner_challenges WHERE id=$7) AND EXISTS(SELECT 1 FROM stored_files WHERE object_key=$8) AND NOT EXISTS(SELECT 1 FROM directory_observations WHERE site_id=$9) AND EXISTS(SELECT 1 FROM moderation_events WHERE target_id=$1 AND target_kind='site' AND action='delete_site') AS valid")
        .bind::<SqlUuid,_>(f.site).bind::<SqlUuid,_>(reported).bind::<SqlUuid,_>(other_site).bind::<SqlUuid,_>(foreign_reply)
        .bind::<SqlUuid,_>(unrelated_report).bind::<SqlUuid,_>(owner_challenge).bind::<SqlUuid,_>(other_challenge).bind::<Text,_>(&favicon).bind::<SqlUuid,_>(replacement)
        .get_result::<Check>(&mut f.db.pool.get().await.unwrap()).await.unwrap();
    assert!(check.valid);

    let mut conn = f.db.pool.get().await.unwrap();
    diesel::sql_query("DELETE FROM directory_owner_challenges WHERE id=$1")
        .bind::<SqlUuid, _>(other_challenge)
        .execute(&mut conn)
        .await
        .unwrap();
    diesel::sql_query("DELETE FROM community_reports WHERE id=ANY($1)")
        .bind::<Array<SqlUuid>, _>(vec![reported, unrelated_report])
        .execute(&mut conn)
        .await
        .unwrap();
    diesel::sql_query("DELETE FROM community_comments WHERE server_id=$1")
        .bind::<SqlUuid, _>(other_site)
        .execute(&mut conn)
        .await
        .unwrap();
    diesel::sql_query("DELETE FROM legacy_sites WHERE id=$1")
        .bind::<SqlUuid, _>(other_site)
        .execute(&mut conn)
        .await
        .unwrap();
    diesel::sql_query("DELETE FROM directory_sites WHERE id=ANY($1)")
        .bind::<Array<SqlUuid>, _>(vec![other_site, replacement])
        .execute(&mut conn)
        .await
        .unwrap();
    diesel::sql_query("DELETE FROM stored_files WHERE object_key=$1")
        .bind::<Text, _>(&favicon)
        .execute(&mut conn)
        .await
        .unwrap();
    diesel::sql_query("DELETE FROM moderation_events WHERE target_id=ANY($1)")
        .bind::<Array<SqlUuid>, _>(vec![f.site, f.admin.member.id, f.owner.member.id])
        .execute(&mut conn)
        .await
        .unwrap();
    drop(conn);
    fixtures::delete_members(&f.db, &[f.admin.member.id, f.owner.member.id]).await;
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn site_authorization_rechecks_roles_session_ban_and_freshness() {
    let f = Fixture::new().await;
    let request = f.request(Action::ForceHidden(true)).await;
    assert_eq!(
        f.db.moderation_sites(&f.owner, "", "all", "domain", 0)
            .await,
        Err(Error::Forbidden)
    );
    assert_eq!(
        f.db.moderation_site(&f.owner, f.site).await,
        Err(Error::Forbidden)
    );
    assert_eq!(
        f.db.moderation_site_history(&f.owner, f.site, 0).await,
        Err(Error::Forbidden)
    );
    assert_eq!(
        f.db.moderate_site(&f.owner, request.clone()).await,
        Err(Error::Forbidden)
    );
    f.db.set_admin_role(f.admin.member.id, false).await.unwrap();
    assert_eq!(
        f.db.moderate_site(&f.admin, request.clone()).await,
        Err(Error::Forbidden)
    );
    f.db.set_admin_role(f.admin.member.id, true).await.unwrap();
    fixtures::age_session(&f.db, f.admin.id).await;
    assert!(f.db.moderation_site(&f.admin, f.site).await.is_ok());
    assert_eq!(
        f.db.moderate_site(&f.admin, request.clone()).await,
        Err(Error::Auth(auth::AuthError::FreshAuthenticationRequired))
    );
    fixtures::ban(&f.db, f.admin.member.id, true).await;
    assert!(matches!(
        f.db.moderation_site(&f.admin, f.site).await,
        Err(Error::Auth(_))
    ));
    assert!(matches!(
        f.db.moderate_site(&f.admin, request).await,
        Err(Error::Auth(_))
    ));
    f.clean().await;
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn independent_flags_preserve_owner_metadata_and_public_rules() {
    let f = Fixture::new().await;
    let original = f.raw().await;
    let owner_before = f.db.owned_sites(&f.owner, 0).await.unwrap().sites.remove(0);
    let edited = site_management::validate_edit(owner_before.edit.clone()).unwrap();
    let hidden = f.act(Action::ForceHidden(true)).await;
    assert!(hidden.force_hidden && !hidden.hidden && !hidden.closed);
    assert!(f.db.public_site(&f.domain).await.unwrap().is_none());
    assert_eq!(
        f.db.edit_owned_site(&f.owner, &f.domain, owner_before.revision, &edited)
            .await,
        Err(site_management::Error::Conflict)
    );
    // A current owner may edit, but cannot clear administrator force hiding.
    f.db.edit_owned_site(&f.owner, &f.domain, hidden.revision, &edited)
        .await
        .unwrap();
    assert!(f.detail().await.force_hidden);
    f.act(Action::Hidden(true)).await;
    f.act(Action::Closed(true)).await;
    f.act(Action::ForceHidden(false)).await;
    assert!(f.db.public_site(&f.domain).await.unwrap().is_none());
    f.act(Action::Hidden(false)).await;
    let current = f.detail().await;
    assert!(current.closed && !current.hidden && !current.force_hidden);
    assert!(f.db.public_site(&f.domain).await.unwrap().is_some());
    assert_eq!(f.act(Action::Closed(true)).await.revision, current.revision); // no-op
    let raw = f.raw().await;
    for key in ["name", "description", "domain", "created_at"] {
        assert_eq!(raw["site"][key], original["site"][key]);
    }
    for key in [
        "owner_id",
        "owner_method",
        "rules",
        "language",
        "tags",
        "owner_comment",
        "approval_required",
    ] {
        assert_eq!(raw["details"][key], original["details"][key]);
    }
    let history =
        f.db.moderation_site_history(&f.admin, f.site, 0)
            .await
            .unwrap();
    assert_eq!(history.events.len(), 5);
    let serialized = serde_json::to_string(&history).unwrap();
    assert!(
        !serialized.contains("owner_id")
            && !serialized.contains("previous")
            && !serialized.contains(&f.owner.member.id.to_string())
    );
    assert!(history.events.iter().all(|e| e.note.contains("실행 금지")));
    f.clean().await;
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn owner_and_administrator_race_uses_one_shared_revision() {
    let f = Fixture::new().await;
    let r = f.request(Action::ForceHidden(true)).await;
    let mut edit =
        f.db.owned_sites(&f.owner, 0)
            .await
            .unwrap()
            .sites
            .remove(0)
            .edit;
    edit.description = "concurrent owner edit".into();
    let edit = site_management::validate_edit(edit).unwrap();
    let (admin, owner) = tokio::join!(
        f.db.moderate_site(&f.admin, r.clone()),
        f.db.edit_owned_site(&f.owner, &f.domain, r.revision, &edit)
    );
    assert_eq!(usize::from(admin.is_ok()) + usize::from(owner.is_ok()), 1);
    assert!(admin == Err(Error::Conflict) || owner == Err(site_management::Error::Conflict));
    let current = f.detail().await;
    assert_eq!(current.revision, 1);
    let r = f.request(Action::Closed(true)).await;
    let (one, two) = tokio::join!(
        f.db.moderate_site(&f.admin, r.clone()),
        f.db.moderate_site(&f.admin, r)
    );
    assert_eq!(usize::from(one.is_ok()) + usize::from(two.is_ok()), 1);
    assert!(one == Err(Error::Conflict) || two == Err(Error::Conflict));
    f.clean().await;
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn legacy_long_tags_survive_unrelated_edits_and_changes_are_bounded() {
    let f = Fixture::new().await;
    let long = "옛".repeat(900);
    diesel::sql_query("UPDATE directory_site_details SET tags=ARRAY[$2] WHERE site_id=$1")
        .bind::<SqlUuid, _>(f.site)
        .bind::<Text, _>(&long)
        .execute(&mut f.db.pool.get().await.unwrap())
        .await
        .unwrap();
    assert!(f.detail().await.tags_truncated);
    f.act(Action::InviteOnly(Some(true))).await;
    assert_eq!(f.raw().await["details"]["tags"][0], long);
    f.act(Action::InviteOnly(None)).await;
    assert_eq!(f.detail().await.invite_only, None);
    let r = f.request(Action::Tags("가".repeat(257))).await;
    assert_eq!(f.db.moderate_site(&f.admin, r).await, Err(Error::Invalid));
    let r = f.request(Action::Tags("a".repeat(33))).await;
    assert_eq!(f.db.moderate_site(&f.admin, r).await, Err(Error::Invalid));
    for note in ["".to_owned(), "가".repeat(1001), "bad\u{0}".into()] {
        let mut r = f.request(Action::Hidden(true)).await;
        r.note = note;
        assert_eq!(f.db.moderate_site(&f.admin, r).await, Err(Error::Invalid));
    }
    let changed = f.act(Action::Tags("사진, 개발, 사진".into())).await;
    assert_eq!(changed.tags, "사진, 개발");
    let history =
        f.db.moderation_site_history(&f.admin, f.site, 0)
            .await
            .unwrap();
    assert_eq!(history.events[0].change.before.chars().count(), 600);
    assert!(history.events[0].change.truncated);
    // The bounded DTO is not the stored original: full before-state survives.
    let row=diesel::sql_query("SELECT (previous->'details'->'tags')->>0 AS value FROM directory_site_edits WHERE site_id=$1 ORDER BY revision DESC LIMIT 1").bind::<SqlUuid,_>(f.site).get_result::<JsonRow>(&mut f.db.pool.get().await.unwrap()).await.unwrap();
    assert_eq!(row.value, long);
    // An imported single tag containing a comma must not defeat replacement.
    diesel::sql_query("UPDATE directory_site_details SET tags=ARRAY[$2] WHERE site_id=$1")
        .bind::<SqlUuid, _>(f.site)
        .bind::<Text, _>("사진, 개발")
        .execute(&mut f.db.pool.get().await.unwrap())
        .await
        .unwrap();
    let revision = f.detail().await.revision;
    let changed = f.act(Action::Tags("사진, 개발".into())).await;
    assert_eq!(changed.revision, revision + 1);
    assert_eq!(
        f.raw().await["details"]["tags"].as_array().unwrap().len(),
        2
    );
    assert_eq!(
        f.act(Action::Tags("사진, 개발".into())).await.revision,
        changed.revision
    );
    f.clean().await;
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn admin_and_owner_share_manual_refresh_limit_and_atomic_audit() {
    let f = Fixture::new().await;
    f.act(Action::Closed(true)).await;
    let r = f.request(Action::Refresh).await;
    let (a, b) = tokio::join!(
        f.db.moderate_site(&f.admin, r),
        f.db.request_site_refresh(&f.owner, &f.domain)
    );
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    assert!(
        a == Err(Error::RefreshTooSoon) || b == Err(site_management::Error::RefreshTooSoon),
        "admin error: {:?}; owner error: {:?}",
        a.as_ref().err(),
        b.as_ref().err()
    );
    let current = f.detail().await;
    assert!(current.closed);
    assert_eq!(
        current.refresh.unwrap().state,
        crate::directory::management::RefreshState::Pending
    );
    let before =
        f.db.moderation_site_history(&f.admin, f.site, 0)
            .await
            .unwrap();
    assert_eq!(
        f.db.moderate_site(&f.admin, f.request(Action::Refresh).await)
            .await,
        Err(Error::RefreshTooSoon)
    );
    assert_eq!(
        before,
        f.db.moderation_site_history(&f.admin, f.site, 0)
            .await
            .unwrap()
    );
    let mut conn = f.db.pool.get().await.unwrap();
    diesel::sql_query("UPDATE directory_site_details SET refresh_requested_at=now()-interval '2 hours' WHERE site_id=$1").bind::<SqlUuid,_>(f.site).execute(&mut conn).await.unwrap();
    drop(conn);
    let refreshed = f.act(Action::Refresh).await;
    assert!(refreshed.closed);
    assert_eq!(
        f.db.request_site_refresh(&f.owner, &f.domain).await,
        Err(site_management::Error::RefreshTooSoon)
    );
    #[derive(QueryableByName)]
    struct Counts {
        #[diesel(sql_type=BigInt)]
        total: i64,
        #[diesel(sql_type=Bool)]
        manual: bool,
    }
    let count=diesel::sql_query("SELECT count(*) AS total,bool_and(owner_requested) AS manual FROM directory_jobs WHERE site_id=$1").bind::<SqlUuid,_>(f.site).get_result::<Counts>(&mut f.db.pool.get().await.unwrap()).await.unwrap();
    assert_eq!(count.total, 2);
    assert!(count.manual);
    assert_eq!(
        f.raw().await["details"]["owner_id"],
        f.owner.member.id.to_string()
    );
    f.clean().await;
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn admin_site_listing_handles_hidden_missing_details_and_pagination() {
    let f = Fixture::new().await;
    let prefix = format!("list-{}", Uuid::new_v4().simple());
    let mut ids = vec![];
    for i in 0..25 {
        let id =
            f.db.add_site(&format!("{prefix}-{i:02}.example.org"))
                .await
                .unwrap();
        ids.push(id);
    }
    let page =
        f.db.moderation_sites(&f.admin, &prefix, "all", "domain", 0)
            .await
            .unwrap();
    assert_eq!(page.sites.len(), 24);
    assert!(page.has_next);
    assert!(page
        .sites
        .iter()
        .all(|s| s.revision == 0 && s.owner_name.is_none()));
    let next =
        f.db.moderation_sites(&f.admin, &prefix, "all", "domain", 1)
            .await
            .unwrap();
    assert_eq!(next.sites.len(), 1);
    assert!(!next.has_next);
    assert!(!page.sites.iter().any(|s| s.id == next.sites[0].id));
    let count = diesel::sql_query(
        "SELECT count(*)::text AS value FROM directory_site_details WHERE site_id=ANY($1)",
    )
    .bind::<Array<SqlUuid>, _>(&ids)
    .get_result::<JsonRow>(&mut f.db.pool.get().await.unwrap())
    .await
    .unwrap();
    assert_eq!(count.value, "0");
    f.db.moderate_site(
        &f.admin,
        Request {
            id: ids[0].to_string(),
            revision: 0,
            action: Action::ForceHidden(true),
            note: "fixture".into(),
        },
    )
    .await
    .unwrap();
    assert_eq!(
        f.db.moderation_sites(&f.admin, &prefix, "force_hidden", "users", 0)
            .await
            .unwrap()
            .sites
            .len(),
        1
    );
    assert_eq!(
        f.db.moderation_sites(&f.admin, &prefix, "visible", "recent", 0)
            .await
            .unwrap()
            .sites
            .len(),
        24
    );
    assert!(f
        .db
        .moderation_sites(&f.admin, "%", "all", "domain", 0)
        .await
        .unwrap()
        .sites
        .is_empty());
    assert_eq!(
        f.db.moderation_sites(&f.admin, "", "anything", "domain", 0)
            .await,
        Err(Error::Invalid)
    );
    assert_eq!(
        f.db.moderation_sites(&f.admin, "", "all", "domain; DROP", 0)
            .await,
        Err(Error::Invalid)
    );
    assert_eq!(
        f.db.moderation_sites(&f.admin, &"a".repeat(129), "all", "domain", 0)
            .await,
        Err(Error::Invalid)
    );
    assert_eq!(
        f.db.moderation_sites(&f.admin, "", "all", "domain", 10001)
            .await,
        Err(Error::Invalid)
    );
    assert_eq!(
        f.db.moderation_site(&f.admin, Uuid::new_v4()).await,
        Err(Error::Missing)
    );
    diesel::sql_query("DELETE FROM directory_sites WHERE id=ANY($1)")
        .bind::<Array<SqlUuid>, _>(&ids)
        .execute(&mut f.db.pool.get().await.unwrap())
        .await
        .unwrap();
    f.clean().await;
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn administrator_history_is_paginated_and_survives_actor_removal() {
    let f = Fixture::new().await;
    for i in 0..21 {
        f.act(Action::Hidden(i % 2 == 0)).await;
    }
    let first =
        f.db.moderation_site_history(&f.admin, f.site, 0)
            .await
            .unwrap();
    let second =
        f.db.moderation_site_history(&f.admin, f.site, 1)
            .await
            .unwrap();
    assert_eq!(first.events.len(), 20);
    assert!(first.has_next);
    assert_eq!(second.events.len(), 1);
    assert!(!second.has_next);
    assert_eq!(first.events[0].revision, 21);
    assert_eq!(second.events[0].revision, 1);
    assert_eq!(
        f.db.moderation_site_history(&f.admin, f.site, 10001).await,
        Err(Error::Invalid)
    );
    f.db.set_admin_role(f.owner.member.id, true).await.unwrap();
    fixtures::delete_members(&f.db, &[f.admin.member.id]).await;
    let history =
        f.db.moderation_site_history(&f.owner, f.site, 0)
            .await
            .unwrap();
    assert!(history.events.iter().all(|e| e.actor_name.is_none()));
    assert_eq!(history.events[0].note, first.events[0].note);
    f.clean().await;
}
