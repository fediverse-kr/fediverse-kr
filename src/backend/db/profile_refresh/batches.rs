//! Administrator requests are durable authority records, never fake user sessions.
use super::*;
use crate::{
    backend::{db::moderation, moderation::Error as AdminError},
    moderation::ProfileBatch,
};

#[derive(QueryableByName)]
struct BatchRow {
    #[diesel(sql_type=SqlUuid)]
    id: Uuid,
    #[diesel(sql_type=Text)]
    state: String,
    #[diesel(sql_type=BigInt)]
    total: i64,
    #[diesel(sql_type=BigInt)]
    pending: i64,
    #[diesel(sql_type=BigInt)]
    running: i64,
    #[diesel(sql_type=BigInt)]
    succeeded: i64,
    #[diesel(sql_type=BigInt)]
    partial: i64,
    #[diesel(sql_type=BigInt)]
    failed: i64,
    #[diesel(sql_type=BigInt)]
    skipped: i64,
    #[diesel(sql_type=BigInt)]
    cancelled: i64,
}
impl From<BatchRow> for ProfileBatch {
    fn from(v: BatchRow) -> Self {
        Self {
            id: v.id.to_string(),
            state: v.state,
            total: v.total,
            pending: v.pending,
            running: v.running,
            succeeded: v.succeeded,
            partial: v.partial,
            failed: v.failed,
            skipped: v.skipped,
            cancelled: v.cancelled,
        }
    }
}
const SUMMARY:&str="SELECT b.id,b.state,count(j.member_id) AS total,count(*) FILTER(WHERE j.state='pending') AS pending,count(*) FILTER(WHERE j.state='running') AS running,count(*) FILTER(WHERE j.state='succeeded') AS succeeded,count(*) FILTER(WHERE j.state='partial') AS partial,count(*) FILTER(WHERE j.state='failed') AS failed,count(*) FILTER(WHERE j.state='skipped') AS skipped,count(*) FILTER(WHERE j.state='cancelled') AS cancelled FROM profile_refresh_batches b LEFT JOIN profile_refresh_jobs j ON j.batch_id=b.id";
async fn summary(conn: &mut AsyncPgConnection, id: Uuid) -> Result<ProfileBatch, AdminError> {
    diesel::sql_query(format!("{SUMMARY} WHERE b.id=$1 GROUP BY b.id"))
        .bind::<SqlUuid, _>(id)
        .get_result::<BatchRow>(conn)
        .await
        .map(Into::into)
        .map_err(Into::into)
}
#[derive(QueryableByName)]
struct Id {
    #[diesel(sql_type=SqlUuid)]
    id: Uuid,
}
#[derive(QueryableByName)]
struct Flag {
    #[diesel(sql_type=Bool)]
    value: bool,
}
impl Database {
    pub async fn profile_batch_status(
        &self,
        s: &AuthenticatedSession,
    ) -> Result<Option<ProfileBatch>, AdminError> {
        let mut conn = self.pool.get().await.map_err(|_| AdminError::Unavailable)?;
        (&mut *conn).transaction(async |conn| {
            moderation::authorize(conn,s,false).await?;
            let last=diesel::sql_query("SELECT id FROM profile_refresh_batches ORDER BY created_at DESC,id DESC LIMIT 1").get_result::<Id>(conn).await.optional()?;
            match last {Some(row)=>summary(conn,row.id).await.map(Some),None=>Ok(None)}
        }).await
    }
    pub async fn start_profile_batch(
        &self,
        s: &AuthenticatedSession,
    ) -> Result<ProfileBatch, AdminError> {
        let mut conn = self.pool.get().await.map_err(|_| AdminError::Unavailable)?;
        (&mut *conn).transaction(async |conn| {
            moderation::gate(conn).await?;
            moderation::authorize(conn,s,true).await?;
            if let Some(current)=diesel::sql_query("SELECT id FROM profile_refresh_batches WHERE state='running'").get_result::<Id>(conn).await.optional()? { return summary(conn,current.id).await; }
            let id=Uuid::new_v4();
            diesel::sql_query("INSERT INTO profile_refresh_batches(id,actor_id,state) VALUES($1,$2,'running')").bind::<SqlUuid,_>(id).bind::<SqlUuid,_>(s.member.id).execute(conn).await?;
            // A fixed set: new signups during this batch are not silently added.
            let count=diesel::sql_query("INSERT INTO profile_refresh_jobs(batch_id,member_id) SELECT $1,id FROM member_users").bind::<SqlUuid,_>(id).execute(conn).await?;
            if count==0 { diesel::sql_query("UPDATE profile_refresh_batches SET state='complete',finished_at=now() WHERE id=$1").bind::<SqlUuid,_>(id).execute(conn).await?; }
            summary(conn,id).await
        }).await
    }
    pub async fn cancel_profile_batch(
        &self,
        s: &AuthenticatedSession,
        id: Uuid,
    ) -> Result<ProfileBatch, AdminError> {
        let mut conn = self.pool.get().await.map_err(|_| AdminError::Unavailable)?;
        (&mut *conn)
            .transaction(async |conn| {
                moderation::gate(conn).await?;
                moderation::authorize(conn, s, false).await?;
                cancel(conn, id).await?;
                summary(conn, id).await
            })
            .await
    }
    pub(crate) async fn claim_profile_job(&self) -> Result<Option<BatchJob>, Error> {
        let mut conn = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        (&mut *conn).transaction(async |conn| {
            moderation::gate(conn).await.map_err(|_|Error::Unavailable)?;
            // Revoked/deleted/banned initiators cannot keep changing other users.
            let batches=diesel::sql_query("SELECT b.id FROM profile_refresh_batches b WHERE b.state='running' AND NOT EXISTS(SELECT 1 FROM member_users u JOIN member_admin_roles r ON r.member_id=u.id WHERE u.id=b.actor_id AND NOT u.is_banned)").load::<Id>(conn).await?;
            for row in batches { cancel(conn,row.id).await.map_err(|_|Error::Unavailable)?; }
            diesel::sql_query("UPDATE profile_refresh_jobs j SET state='failed',lease_token=NULL,lease_until=NULL FROM profile_refresh_batches b WHERE b.id=j.batch_id AND b.state='running' AND j.state='running' AND j.lease_until<now() AND j.attempts>=3").execute(conn).await?;
            finish_batches(conn).await?;
            #[derive(QueryableByName)] struct Candidate { #[diesel(sql_type=SqlUuid)] batch_id:Uuid, #[diesel(sql_type=SqlUuid)] member_id:Uuid }
            let candidate=diesel::sql_query("SELECT j.batch_id,j.member_id FROM profile_refresh_jobs j JOIN profile_refresh_batches b ON b.id=j.batch_id WHERE b.state='running' AND (j.state='pending' OR (j.state='running' AND j.lease_until<now())) AND j.attempts<3 ORDER BY j.member_id LIMIT 1 FOR UPDATE OF j").get_result::<Candidate>(conn).await.optional()?;
            let Some(c)=candidate else {return Ok(None)};
            let job=BatchJob{batch:c.batch_id,member:c.member_id,lease:Uuid::new_v4()};
            diesel::sql_query("UPDATE profile_refresh_jobs SET state='running',attempts=attempts+1,lease_token=$3,lease_until=now()+interval '3 minutes' WHERE batch_id=$1 AND member_id=$2").bind::<SqlUuid,_>(job.batch).bind::<SqlUuid,_>(job.member).bind::<SqlUuid,_>(job.lease).execute(conn).await?;
            Ok(Some(job))
        }).await
    }
    pub(crate) async fn complete_profile_job(
        &self,
        job: &BatchJob,
        outcome: &str,
    ) -> Result<(), Error> {
        if !["succeeded", "partial", "failed", "skipped"].contains(&outcome) {
            return Err(Error::Unavailable);
        }
        let mut conn = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        (&mut *conn).transaction(async |conn| {
            moderation::gate(conn).await.map_err(|_|Error::Unavailable)?;
            diesel::sql_query("UPDATE profile_refresh_jobs SET state=$4,lease_token=NULL,lease_until=NULL WHERE batch_id=$1 AND member_id=$2 AND lease_token=$3 AND state='running' AND lease_until>now()").bind::<SqlUuid,_>(job.batch).bind::<SqlUuid,_>(job.member).bind::<SqlUuid,_>(job.lease).bind::<Text,_>(outcome).execute(conn).await?;
            finish_batches(conn).await
        }).await
    }
    pub(crate) async fn begin_batch_profile(
        &self,
        job: &BatchJob,
    ) -> Result<Option<Ticket>, Error> {
        let mut conn = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        (&mut *conn).transaction(async |conn| {
            authorize_job(conn,job).await?;
            let Some(source)=source(conn,job.member).await? else{return Ok(None)};
            diesel::sql_query("INSERT INTO member_profile_media(member_id,source_account_id,avatar_key,emojis) SELECT u.id,$2,l.avatar_key,CASE WHEN jsonb_typeof(l.emojis)='object' THEN l.emojis ELSE '{}'::jsonb END FROM member_users u LEFT JOIN legacy_members l ON l.id=u.id WHERE u.id=$1 ON CONFLICT DO NOTHING").bind::<SqlUuid,_>(job.member).bind::<Nullable<SqlUuid>,_>(source.account).execute(conn).await?;
            if row(conn,job.member).await?.cooling {return Err(Error::Busy)}
            let request=Uuid::new_v4();
            diesel::sql_query("UPDATE member_profile_media SET request_id=$2,requested_at=now(),refresh_failed=false WHERE member_id=$1").bind::<SqlUuid,_>(job.member).bind::<SqlUuid,_>(request).execute(conn).await?;
            Ok(Some(Ticket{authority:Authority::Batch(job.clone()),member:job.member,account:source.account,actor_id:source.actor_id,handle:source.handle,request}))
        }).await
    }
}
async fn cancel(conn: &mut AsyncPgConnection, id: Uuid) -> Result<(), AdminError> {
    diesel::sql_query("UPDATE profile_refresh_batches SET state='cancelled',finished_at=now() WHERE id=$1 AND state='running'").bind::<SqlUuid,_>(id).execute(conn).await?;
    diesel::sql_query("UPDATE profile_refresh_jobs SET state='cancelled',lease_token=NULL,lease_until=NULL WHERE batch_id=$1 AND state IN ('pending','running')").bind::<SqlUuid,_>(id).execute(conn).await?;
    Ok(())
}
async fn finish_batches(conn: &mut AsyncPgConnection) -> Result<(), Error> {
    diesel::sql_query("UPDATE profile_refresh_batches b SET state='complete',finished_at=now() WHERE state='running' AND NOT EXISTS(SELECT 1 FROM profile_refresh_jobs j WHERE j.batch_id=b.id AND j.state IN ('pending','running'))").execute(conn).await?;
    Ok(())
}
pub(super) async fn authorize_job(
    conn: &mut AsyncPgConnection,
    job: &BatchJob,
) -> Result<(), Error> {
    moderation::gate(conn)
        .await
        .map_err(|_| Error::Unavailable)?;
    // Match the batch/lease before locking the target. Role changes take the same gate.
    let allowed=diesel::sql_query("SELECT u.id FROM profile_refresh_batches b JOIN profile_refresh_jobs j ON j.batch_id=b.id JOIN member_users u ON u.id=b.actor_id JOIN member_admin_roles r ON r.member_id=u.id WHERE b.id=$1 AND b.state='running' AND j.member_id=$2 AND j.state='running' AND j.lease_token=$3 AND j.lease_until>now() AND NOT u.is_banned FOR NO KEY UPDATE OF u")
        .bind::<SqlUuid,_>(job.batch).bind::<SqlUuid,_>(job.member).bind::<SqlUuid,_>(job.lease).get_result::<Id>(conn).await.optional()?.is_some();
    if !allowed {
        return Err(Error::Superseded);
    }
    // No synthetic session is made for the target member.
    let available = diesel::sql_query(
        "SELECT NOT is_banned AS value FROM member_users WHERE id=$1 FOR NO KEY UPDATE",
    )
    .bind::<SqlUuid, _>(job.member)
    .get_result::<Flag>(conn)
    .await
    .optional()?
    .is_some_and(|r| r.value);
    if !available {
        return Err(Error::Superseded);
    }
    Ok(())
}
#[derive(QueryableByName)]
pub(super) struct Source {
    #[diesel(sql_type=Nullable<SqlUuid>)]
    pub account: Option<Uuid>,
    #[diesel(sql_type=Nullable<Text>)]
    pub actor_id: Option<String>,
    #[diesel(sql_type=Text)]
    pub handle: String,
}
pub(super) async fn source(
    conn: &mut AsyncPgConnection,
    member: Uuid,
) -> Result<Option<Source>, Error> {
    // A chosen source wins. Never choose a different identity after unlinking.
    let linked=diesel::sql_query("SELECT a.id AS account,a.actor_url AS actor_id,a.handle FROM member_linked_accounts a LEFT JOIN member_profile_media m ON m.member_id=a.member_id WHERE a.member_id=$1 AND (m.source_account_id=a.id OR (m.member_id IS NULL AND (SELECT count(*) FROM member_linked_accounts x WHERE x.member_id=a.member_id)=1)) LIMIT 1").bind::<SqlUuid,_>(member).get_result::<Source>(conn).await.optional()?;
    if linked.is_some() {
        return Ok(linked);
    }
    // Unclaimed Phoenix members still have their original address. Resolving
    // media must NOT pin the actor, attach an account, or grant login rights.
    diesel::sql_query("SELECT NULL::uuid AS account,NULL::text AS actor_id,c.handle FROM member_legacy_claims c WHERE c.member_id=$1 AND c.actor_url IS NULL AND NOT EXISTS(SELECT 1 FROM member_linked_accounts a WHERE a.member_id=c.member_id)").bind::<SqlUuid,_>(member).get_result::<Source>(conn).await.optional().map_err(Into::into)
}
