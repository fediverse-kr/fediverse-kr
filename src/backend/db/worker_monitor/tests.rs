use super::*;
use crate::{
    backend::{auth, db::fixtures, moderation::Error},
    moderation::workers::{QueueSummary, ISSUE_LIMIT},
};
use diesel::sql_types::{BigInt, Bool, Int4, Nullable, Text, Uuid as SqlUuid};
use diesel_async::{AsyncConnection, RunQueryDsl};
use uuid::Uuid;

async fn session(db: &Database, token: &str) -> auth::AuthenticatedSession {
    auth::get_session_details(db, token).await.unwrap().unwrap()
}

struct Fixture {
    db: Database,
    admin: auth::AuthenticatedSession,
    member: auth::AuthenticatedSession,
    admin_token: String,
    sites: Vec<Uuid>,
    members: Vec<Uuid>,
}

impl Fixture {
    async fn new() -> Self {
        let db = fixtures::database().await;
        let admin_grant = fixtures::member(&db).await;
        let member_grant = fixtures::member(&db).await;
        let admin = session(&db, &admin_grant.token).await;
        let member = session(&db, &member_grant.token).await;
        db.set_admin_role(admin.member.id, true).await.unwrap();
        Self {
            db,
            admin,
            member,
            admin_token: admin_grant.token,
            sites: Vec::new(),
            members: vec![admin_grant.member.id, member_grant.member.id],
        }
    }

    async fn site(&mut self, label: &str, closed: bool, hidden: bool) -> Uuid {
        let domain = format!("worker-{label}-{}.example.org", Uuid::new_v4().simple());
        let id = self.db.add_site(&domain).await.unwrap();
        diesel::sql_query("UPDATE directory_sites SET is_closed=$2,is_hidden=$3 WHERE id=$1")
            .bind::<SqlUuid, _>(id)
            .bind::<Bool, _>(closed)
            .bind::<Bool, _>(hidden)
            .execute(&mut self.db.pool.get().await.unwrap())
            .await
            .unwrap();
        self.sites.push(id);
        id
    }

    async fn job(
        &self,
        site: Uuid,
        state: &str,
        scheduled: &str,
        lease: &str,
        owner_requested: bool,
        completed: &str,
        attempts: i32,
        error: &str,
    ) {
        // All time fragments are fixed test constants, never test input.
        let running = state == "running";
        assert_eq!(
            running,
            lease != "NULL",
            "fixture must satisfy job lease CHECK"
        );
        let token = if running { "gen_random_uuid()" } else { "NULL" };
        let sql = format!(
            "INSERT INTO directory_jobs(id,site_id,scheduled_at,state,attempts,lease_token,lease_until,owner_requested,completed_at,last_error) VALUES(gen_random_uuid(),$1,{scheduled},$2,$3,{token},{lease},$4,{completed},$5)"
        );
        diesel::sql_query(sql)
            .bind::<SqlUuid, _>(site)
            .bind::<Text, _>(state)
            .bind::<Int4, _>(attempts)
            .bind::<Bool, _>(owner_requested)
            .bind::<Text, _>(error)
            .execute(&mut self.db.pool.get().await.unwrap())
            .await
            .unwrap();
    }

    async fn observation(
        &self,
        site: Uuid,
        alive: bool,
        status: Option<i32>,
        nodeinfo_error: Option<&str>,
        offset: &str,
    ) {
        let sql = format!(
            "INSERT INTO directory_observations(site_id,is_alive,status_code,nodeinfo_error,checked_at) VALUES($1,$2,$3,$4,{offset}) ON CONFLICT(site_id) DO UPDATE SET is_alive=EXCLUDED.is_alive,status_code=EXCLUDED.status_code,nodeinfo_error=EXCLUDED.nodeinfo_error,checked_at=EXCLUDED.checked_at"
        );
        diesel::sql_query(sql)
            .bind::<SqlUuid, _>(site)
            .bind::<Bool, _>(alive)
            .bind::<Nullable<Int4>, _>(status)
            .bind::<Nullable<Text>, _>(nodeinfo_error)
            .execute(&mut self.db.pool.get().await.unwrap())
            .await
            .unwrap();
    }

    async fn clean(self) {
        if !self.sites.is_empty() {
            diesel::sql_query("DELETE FROM directory_sites WHERE id=ANY($1)")
                .bind::<diesel::sql_types::Array<SqlUuid>, _>(self.sites)
                .execute(&mut self.db.pool.get().await.unwrap())
                .await
                .unwrap();
        }
        let _ = self.db.set_admin_role(self.admin.member.id, false).await;
        fixtures::delete_members(&self.db, &self.members).await;
    }
}

#[derive(diesel::QueryableByName)]
struct Snapshot {
    #[diesel(sql_type = BigInt)]
    jobs: i64,
    #[diesel(sql_type = BigInt)]
    attempts: i64,
    #[diesel(sql_type = Text)]
    detail: String,
}

async fn jobs_snapshot(db: &Database) -> Snapshot {
    diesel::sql_query("SELECT count(*)::bigint AS jobs,coalesce(sum(attempts),0)::bigint AS attempts,coalesce(string_agg(concat_ws('|',id::text,site_id::text,scheduled_at::text,state,attempts::text,coalesce(lease_token::text,''),coalesce(lease_until::text,''),owner_requested::text,coalesce(completed_at::text,''),coalesce(last_error,'')),'\n' ORDER BY id),'') AS detail FROM directory_jobs")
        .get_result(&mut db.pool.get().await.unwrap())
        .await
        .unwrap()
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn worker_overview_requires_current_unbanned_administrator_but_not_fresh_authentication() {
    let fixture = Fixture::new().await;
    let empty = fixture.db.worker_overview(&fixture.admin).await.unwrap();
    assert_eq!(empty.queue, QueueSummary::default());
    assert!(empty.job_issues.is_empty() && empty.site_issues.is_empty());
    assert_eq!(
        fixture.db.worker_overview(&fixture.member).await,
        Err(Error::Forbidden)
    );
    fixtures::age_session(&fixture.db, fixture.admin.id).await;
    assert!(fixture.db.worker_overview(&fixture.admin).await.is_ok());
    fixture
        .db
        .set_admin_role(fixture.admin.member.id, false)
        .await
        .unwrap();
    assert_eq!(
        fixture.db.worker_overview(&fixture.admin).await,
        Err(Error::Forbidden)
    );
    fixture
        .db
        .set_admin_role(fixture.admin.member.id, true)
        .await
        .unwrap();
    fixtures::ban(&fixture.db, fixture.admin.member.id, true).await;
    assert_eq!(
        fixture.db.worker_overview(&fixture.admin).await,
        Err(Error::Auth(auth::AuthError::Unauthenticated))
    );
    fixtures::ban(&fixture.db, fixture.admin.member.id, false).await;
    auth::revoke_session(&fixture.db, &fixture.admin_token)
        .await
        .unwrap();
    assert_eq!(
        fixture.db.worker_overview(&fixture.admin).await,
        Err(Error::Auth(auth::AuthError::Unauthenticated))
    );
    fixture.clean().await;
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn worker_overview_separates_queue_leases_from_remote_observations_and_limits_safe_rows() {
    let mut fixture = Fixture::new().await;
    let ready = fixture.site("ready", false, false).await;
    let scheduled = fixture.site("scheduled", false, false).await;
    let paused = fixture.site("paused", true, false).await;
    let owner_closed = fixture.site("owner", true, false).await;
    let running = fixture.site("running", false, false).await;
    let expired = fixture.site("expired", false, false).await;
    let completed = fixture.site("completed", false, false).await;
    fixture
        .job(
            ready,
            "pending",
            "now()-interval '1 minute'",
            "NULL",
            false,
            "NULL",
            0,
            "SECRET-ready",
        )
        .await;
    fixture
        .job(
            scheduled,
            "pending",
            "now()+interval '1 hour'",
            "NULL",
            false,
            "NULL",
            0,
            "SECRET-scheduled",
        )
        .await;
    fixture
        .job(
            paused,
            "pending",
            "now()-interval '1 minute'",
            "NULL",
            false,
            "NULL",
            0,
            "SECRET-paused",
        )
        .await;
    fixture
        .job(
            owner_closed,
            "pending",
            "now()-interval '1 minute'",
            "NULL",
            true,
            "NULL",
            0,
            "SECRET-owner",
        )
        .await;
    fixture
        .job(
            running,
            "running",
            "now()-interval '2 minutes'",
            "now()+interval '2 minutes'",
            false,
            "NULL",
            1,
            "SECRET-running",
        )
        .await;
    fixture
        .job(
            expired,
            "running",
            "now()-interval '3 minutes'",
            "now()-interval '1 second'",
            false,
            "NULL",
            2,
            "SECRET-expired",
        )
        .await;
    fixture
        .job(
            completed,
            "complete",
            "now()-interval '5 minutes'",
            "NULL",
            false,
            "now()-interval '1 hour'",
            1,
            "SECRET-complete",
        )
        .await;
    fixture
        .job(
            completed,
            "complete",
            "now()-interval '30 hours'",
            "NULL",
            false,
            "now()-interval '30 hours'",
            1,
            "SECRET-old-complete",
        )
        .await;

    for index in 0..22 {
        let site = fixture.site(&format!("dead-{index}"), false, false).await;
        fixture
            .job(
                site,
                "dead",
                "now()-interval '10 minutes'",
                "NULL",
                false,
                "NULL",
                3,
                "SECRET-dead",
            )
            .await;
    }
    let remote_down = fixture.site("remote-down", false, false).await;
    let hidden_remote = fixture.site("hidden-remote", false, true).await;
    let closed_remote = fixture.site("closed-remote", true, false).await;
    let recovered_remote = fixture.site("recovered-remote", false, false).await;
    fixture
        .observation(
            remote_down,
            false,
            Some(503),
            Some("SECRET-nodeinfo"),
            "now()-interval '1 minute'",
        )
        .await;
    fixture
        .observation(
            hidden_remote,
            true,
            Some(200),
            Some("SECRET-hidden"),
            "now()-interval '2 minutes'",
        )
        .await;
    fixture
        .observation(
            closed_remote,
            false,
            Some(500),
            Some("SECRET-closed"),
            "now()-interval '3 minutes'",
        )
        .await;
    fixture
        .observation(
            recovered_remote,
            false,
            Some(502),
            Some("SECRET-recovered-old"),
            "now()-interval '5 minutes'",
        )
        .await;
    fixture
        .observation(
            recovered_remote,
            true,
            Some(200),
            None,
            "now()-interval '30 seconds'",
        )
        .await;
    for index in 0..21 {
        let site = fixture
            .site(&format!("remote-limit-{index}"), false, false)
            .await;
        fixture
            .observation(
                site,
                false,
                Some(503),
                Some("SECRET-remote-limit"),
                "now()-interval '4 minutes'",
            )
            .await;
    }

    let before = jobs_snapshot(&fixture.db).await;
    let overview = fixture.db.worker_overview(&fixture.admin).await.unwrap();
    let after = jobs_snapshot(&fixture.db).await;
    assert_eq!(before.jobs, after.jobs);
    assert_eq!(before.attempts, after.attempts);
    assert_eq!(before.detail, after.detail);
    assert_eq!(overview.queue.ready, 2);
    assert_eq!(overview.queue.scheduled, 1);
    assert_eq!(overview.queue.paused, 1);
    assert_eq!(overview.queue.running, 1);
    assert_eq!(overview.queue.expired, 1);
    assert_eq!(overview.queue.dead, 22);
    assert_eq!(overview.queue.completed_24h, 1);
    assert!(overview.queue.latest_completion.is_some());
    for at in std::iter::once(overview.captured_at.as_str())
        .chain(overview.queue.latest_completion.as_deref())
        .chain(
            overview
                .job_issues
                .iter()
                .map(|row| row.scheduled_at.as_str()),
        )
        .chain(
            overview
                .site_issues
                .iter()
                .map(|row| row.checked_at.as_str()),
        )
    {
        assert!(at.ends_with('Z'));
        assert!(chrono::DateTime::parse_from_rfc3339(at).is_ok());
    }
    assert_eq!(overview.job_issues_total, 23);
    assert_eq!(overview.job_issues.len(), ISSUE_LIMIT);
    assert!(overview.job_issues.iter().all(|row| row.attempts >= 2));
    assert_eq!(overview.site_issues_total, 23);
    assert_eq!(overview.site_issues.len(), ISSUE_LIMIT);
    assert!(overview
        .site_issues
        .iter()
        .any(|row| row.site_id == hidden_remote.to_string()));
    assert!(!overview
        .site_issues
        .iter()
        .any(|row| row.site_id == closed_remote.to_string()));
    assert!(!overview
        .site_issues
        .iter()
        .any(|row| row.site_id == recovered_remote.to_string()));
    assert!(overview.site_issues.iter().any(|row| !row.alive));
    assert!(overview.site_issues.iter().any(|row| row.nodeinfo_failed));
    let rendered = serde_json::to_string(&overview).unwrap();
    let admin_id = fixture.admin.member.id.to_string();
    for forbidden in ["SECRET-", "lease_token", admin_id.as_str()] {
        assert!(!rendered.contains(forbidden));
    }
    fixture.clean().await;
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn worker_overview_rechecks_role_after_waiting_for_a_concurrent_revoke() {
    let fixture = Fixture::new().await;
    let db = fixture.db.clone();
    let member = fixture.admin.member.id;
    let (locked, wait_for_commit) = tokio::sync::oneshot::channel();
    let (commit, committed) = tokio::sync::oneshot::channel();
    let revoke = tokio::spawn(async move {
        let mut conn = db.pool.get().await.unwrap();
        (&mut *conn)
            .transaction::<(), diesel::result::Error, _>(async move |conn| {
                // Match the role setter's gate and member-row lock, but hold the
                // transaction open so overview must wait before its role check.
                diesel::sql_query("SELECT pg_advisory_xact_lock(6810476213314)")
                    .execute(conn)
                    .await?;
                diesel::sql_query("SELECT id FROM member_users WHERE id=$1 FOR UPDATE")
                    .bind::<SqlUuid, _>(member)
                    .execute(conn)
                    .await?;
                diesel::sql_query("DELETE FROM member_admin_roles WHERE member_id=$1")
                    .bind::<SqlUuid, _>(member)
                    .execute(conn)
                    .await?;
                #[derive(diesel::QueryableByName)]
                struct Pid {
                    #[diesel(sql_type = Int4)]
                    value: i32,
                }
                let pid = diesel::sql_query("SELECT pg_backend_pid() AS value")
                    .get_result::<Pid>(conn)
                    .await?
                    .value;
                locked.send(pid).expect("overview test listener");
                committed.await.expect("overview test commit");
                Ok(())
            })
            .await
    });
    let revoke_pid = tokio::time::timeout(std::time::Duration::from_secs(5), wait_for_commit)
        .await
        .expect("revoke must lock member")
        .expect("revoke task must signal");

    let overview_db = fixture.db.clone();
    let overview_session = fixture.admin.clone();
    let overview =
        tokio::spawn(async move { overview_db.worker_overview(&overview_session).await });
    // Confirm an actual PostgreSQL lock wait, not merely a slow spawned task.
    #[derive(diesel::QueryableByName)]
    struct Waiting {
        #[diesel(sql_type = Bool)]
        value: bool,
    }
    let mut observer = fixture.db.pool.get().await.unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        loop {
            let waiting = diesel::sql_query("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE datname=current_database() AND $1=ANY(pg_blocking_pids(pid))) AS value")
                .bind::<Int4, _>(revoke_pid)
                .get_result::<Waiting>(&mut observer).await.unwrap();
            if waiting.value { break; }
            assert!(!overview.is_finished(), "overview bypassed member lock");
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }).await.expect("overview must wait for the role setter in PostgreSQL");
    drop(observer);

    commit.send(()).expect("revoke transaction is waiting");
    revoke.await.unwrap().unwrap();
    assert_eq!(
        tokio::time::timeout(std::time::Duration::from_secs(5), overview)
            .await
            .expect("overview must finish")
            .unwrap(),
        Err(Error::Forbidden)
    );
    fixture.clean().await;
}
