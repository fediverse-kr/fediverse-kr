//! Administrator-only, read-only worker snapshots. Queue lease failures and
//! remote observation failures are intentionally reported as separate sets.

use super::{moderation, Database};
use crate::{
    backend::{auth::AuthenticatedSession, moderation::Error},
    moderation::workers::{WorkerOverview, ISSUE_LIMIT},
};
use diesel::sql_types::{BigInt, Text};
use diesel_async::{AsyncConnection, RunQueryDsl};

#[derive(diesel::QueryableByName)]
struct JsonRow {
    #[diesel(sql_type = Text)]
    value: String,
}

const JOB_ISSUE_WHERE: &str =
    "(j.state='dead' OR (j.state='running' AND (j.lease_until IS NULL OR j.lease_until<=now())))";

// Authorization deliberately happens before this statement. In READ COMMITTED
// that makes a completed role revocation visible to authorization; this one
// statement then supplies the coherent dashboard snapshot without extending a
// stale authorization snapshot across several reads.
const OVERVIEW: &str = "WITH captured AS MATERIALIZED (SELECT clock_timestamp() AS value),\
queue AS (SELECT \
count(*) FILTER (WHERE j.state='pending' AND j.scheduled_at<=now() AND (NOT s.is_closed OR j.owner_requested))::bigint AS ready,\
count(*) FILTER (WHERE j.state='pending' AND j.scheduled_at>now() AND (NOT s.is_closed OR j.owner_requested))::bigint AS scheduled,\
count(*) FILTER (WHERE j.state='pending' AND s.is_closed AND NOT j.owner_requested)::bigint AS paused,\
count(*) FILTER (WHERE j.state='running' AND j.lease_until>now())::bigint AS running,\
count(*) FILTER (WHERE j.state='running' AND (j.lease_until IS NULL OR j.lease_until<=now()))::bigint AS expired,\
count(*) FILTER (WHERE j.state='dead')::bigint AS dead,\
count(*) FILTER (WHERE j.state='complete' AND j.completed_at>=now()-interval '24 hours')::bigint AS completed_24h,\
max(j.completed_at) FILTER (WHERE j.state='complete') AS latest_completion \
FROM directory_jobs j JOIN directory_sites s ON s.id=j.site_id),\
job_rows AS (SELECT j.id,j.site_id,s.domain,j.scheduled_at AS scheduled_order,\
to_char(j.scheduled_at AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS scheduled_at,j.attempts,\
CASE WHEN j.state='dead' THEN 'dead' ELSE 'lease_expired' END AS kind \
FROM directory_jobs j JOIN directory_sites s ON s.id=j.site_id WHERE ";

const OVERVIEW_TAIL: &str = "),\
job_issues AS (SELECT count(*)::bigint AS total,coalesce(jsonb_agg(jsonb_build_object(\
'site_id',site_id::text,'domain',domain,'scheduled_at',scheduled_at,'attempts',attempts,'kind',kind\
) ORDER BY scheduled_order DESC,id DESC) FILTER (WHERE ordinal<=$1), '[]'::jsonb) AS rows \
FROM (SELECT *,row_number() OVER (ORDER BY scheduled_order DESC,id DESC) AS ordinal FROM job_rows) r),\
site_rows AS (SELECT o.site_id,s.domain,o.checked_at AS checked_order,\
to_char(o.checked_at AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS checked_at,o.is_alive AS alive,o.status_code,\
(o.nodeinfo_error IS NOT NULL) AS nodeinfo_failed \
FROM directory_observations o JOIN directory_sites s ON s.id=o.site_id \
WHERE NOT s.is_closed AND (NOT o.is_alive OR o.nodeinfo_error IS NOT NULL)),\
site_issues AS (SELECT count(*)::bigint AS total,coalesce(jsonb_agg(jsonb_build_object(\
'site_id',site_id::text,'domain',domain,'checked_at',checked_at,'alive',alive,'status_code',status_code,'nodeinfo_failed',nodeinfo_failed\
) ORDER BY checked_order DESC,site_id DESC) FILTER (WHERE ordinal<=$1), '[]'::jsonb) AS rows \
FROM (SELECT *,row_number() OVER (ORDER BY checked_order DESC,site_id DESC) AS ordinal FROM site_rows) r) \
SELECT jsonb_build_object(\
'captured_at',to_char(captured.value AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"'),\
'queue',jsonb_build_object('ready',queue.ready,'scheduled',queue.scheduled,'paused',queue.paused,'running',queue.running,'expired',queue.expired,'dead',queue.dead,'completed_24h',queue.completed_24h,'latest_completion',to_char(queue.latest_completion AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"')),\
'job_issues_total',job_issues.total,'job_issues',job_issues.rows,\
'site_issues_total',site_issues.total,'site_issues',site_issues.rows\
)::text AS value FROM captured CROSS JOIN queue CROSS JOIN job_issues CROSS JOIN site_issues";

impl Database {
    pub async fn worker_overview(
        &self,
        session: &AuthenticatedSession,
    ) -> Result<WorkerOverview, Error> {
        let mut conn = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        (&mut *conn)
            .transaction(async move |conn| {
                // Require READ COMMITTED even if the DB default differs. The role
                // check must see a completed revocation after it waits on the
                // member lock; the following single statement is the snapshot.
                diesel::sql_query("SET TRANSACTION ISOLATION LEVEL READ COMMITTED")
                    .execute(conn)
                    .await?;
                moderation::authorize(conn, session, false).await?;
                let row = diesel::sql_query(format!("{OVERVIEW}{JOB_ISSUE_WHERE}{OVERVIEW_TAIL}"))
                    .bind::<BigInt, _>(ISSUE_LIMIT as i64)
                    .get_result::<JsonRow>(conn)
                    .await?;
                serde_json::from_str(&row.value).map_err(|_| Error::Unavailable)
            })
            .await
    }
}

#[cfg(test)]
mod tests;
