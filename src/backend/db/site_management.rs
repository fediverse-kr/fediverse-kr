//! Owner writes: member -> session -> site -> detail -> challenge lock order.
//! Remote verification never holds a transaction open; all authority is rechecked.
use super::{members::authorize_action, schema::directory_sites as sites, Database};
use crate::{
    backend::{
        auth::AuthenticatedSession,
        site_management::{Error, ValidatedEdit, VerifiedDns},
    },
    directory::management::{OwnedPage, OwnedSite, SiteEdit},
};
use diesel::{
    prelude::*,
    sql_types::{BigInt, Binary, Bool, Nullable, Text, Uuid as SqlUuid},
};
use diesel_async::{AsyncConnection, AsyncPgConnection, RunQueryDsl};
use uuid::Uuid;
mod api;
mod moderation;

impl From<diesel::result::Error> for Error {
    fn from(_: diesel::result::Error) -> Self {
        Self::Unavailable
    }
}

#[derive(QueryableByName)]
struct OwnerRow {
    #[diesel(sql_type=SqlUuid)]
    site_id: Uuid,
    #[diesel(sql_type=Nullable<SqlUuid>)]
    owner_id: Option<Uuid>,
    #[diesel(sql_type=Nullable<Text>)]
    owner_method: Option<String>,
    #[diesel(sql_type=BigInt)]
    revision: i64,
    #[diesel(sql_type=Text)]
    domain: String,
    #[diesel(sql_type=Bool)]
    force_hidden: bool,
    #[diesel(sql_type=Bool)]
    closed: bool,
    #[diesel(sql_type=Text)]
    edit: String,
    #[diesel(sql_type=Text)]
    snapshot: String,
    #[diesel(sql_type=Nullable<Text>)]
    refresh: Option<String>,
}
const SELECT: &str="SELECT s.id AS site_id,s.domain,d.owner_id,d.owner_method,d.revision,s.is_force_hidden AS force_hidden,s.is_closed AS closed,
    jsonb_build_object('name',coalesce(s.name,''),'description',coalesce(s.description,''),'rules',coalesce(d.rules,''),'language',coalesce(d.language,''),'tags',coalesce(array_to_string(d.tags,', '),''),'owner_comment',coalesce(d.owner_comment,''),'invite_only',d.invite_only,'approval_required',d.approval_required,'hidden',s.is_hidden)::text AS edit,
    jsonb_build_object('site',to_jsonb(s),'details',to_jsonb(d))::text AS snapshot,
    (SELECT jsonb_build_object('state',CASE WHEN j.state='complete' AND j.last_error IS NOT NULL THEN 'failed' WHEN j.state='dead' THEN 'failed' ELSE j.state END,'requested_at',d.refresh_requested_at)::text FROM directory_jobs j WHERE j.id=d.refresh_job_id) AS refresh
    FROM directory_sites s JOIN directory_site_details d ON d.site_id=s.id";
impl OwnerRow {
    fn dto(self) -> Result<OwnedSite, Error> {
        Ok(OwnedSite {
            domain: self.domain,
            revision: self.revision,
            owner_method: self.owner_method,
            force_hidden: self.force_hidden,
            closed: self.closed,
            edit: serde_json::from_str::<SiteEdit>(&self.edit).map_err(|_| Error::Unavailable)?,
            refresh: self
                .refresh
                .map(|s| serde_json::from_str(&s))
                .transpose()
                .map_err(|_| Error::Unavailable)?,
        })
    }
}
#[derive(QueryableByName)]
struct DomainRow {
    #[diesel(sql_type=Text)]
    domain: String,
}

async fn pending(
    conn: &mut AsyncPgConnection,
    session: &AuthenticatedSession,
    id: Uuid,
    hash: &[u8],
) -> Result<String, Error> {
    Ok(diesel::sql_query("SELECT domain FROM directory_owner_challenges WHERE id=$1 AND member_id=$2 AND session_id=$3 AND code_hash=$4 AND expires_at>now() FOR UPDATE")
        .bind::<SqlUuid,_>(id).bind::<SqlUuid,_>(session.member.id).bind::<SqlUuid,_>(session.id).bind::<Binary,_>(hash)
        .get_result::<DomainRow>(conn).await.optional()?.ok_or(Error::InvalidChallenge)?.domain)
}
async fn locked_site(conn: &mut AsyncPgConnection, domain: &str) -> Result<OwnerRow, Error> {
    let id = sites::table
        .filter(sites::domain.eq(domain))
        .select(sites::id)
        .for_update()
        .first::<Uuid>(conn)
        .await
        .optional()?
        .ok_or(Error::NotOwned)?;
    diesel::sql_query(
        "INSERT INTO directory_site_details(site_id) VALUES($1) ON CONFLICT DO NOTHING",
    )
    .bind::<SqlUuid, _>(id)
    .execute(conn)
    .await?;
    Ok(
        diesel::sql_query(format!("{SELECT} WHERE s.id=$1 FOR UPDATE OF d"))
            .bind::<SqlUuid, _>(id)
            .get_result(conn)
            .await?,
    )
}
async fn audit(
    conn: &mut AsyncPgConnection,
    row: &OwnerRow,
    actor: Uuid,
    action: &str,
) -> Result<(), Error> {
    diesel::sql_query("INSERT INTO directory_site_edits(id,site_id,actor_id,action,revision,previous) VALUES($1,$2,$3,$4,$5,$6::jsonb)")
        .bind::<SqlUuid,_>(Uuid::new_v4()).bind::<SqlUuid,_>(row.site_id).bind::<SqlUuid,_>(actor).bind::<Text,_>(action).bind::<BigInt,_>(row.revision)
        .bind::<Text,_>(&row.snapshot).execute(conn).await?;
    Ok(())
}
impl Database {
    pub async fn site_refresh_status(
        &self,
        session: &AuthenticatedSession,
        domain: &str,
    ) -> Result<Option<crate::directory::management::RefreshStatus>, Error> {
        let mut conn = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        (&mut *conn)
            .transaction(async move |conn| {
                authorize_action(conn, session, false).await?;
                let row =
                    diesel::sql_query(format!("{SELECT} WHERE s.domain=$1 AND d.owner_id=$2"))
                        .bind::<Text, _>(domain)
                        .bind::<SqlUuid, _>(session.member.id)
                        .get_result::<OwnerRow>(conn)
                        .await
                        .optional()?
                        .ok_or(Error::NotOwned)?;
                Ok(row.dto()?.refresh)
            })
            .await
    }
    pub async fn request_site_refresh(
        &self,
        session: &AuthenticatedSession,
        domain: &str,
    ) -> Result<(), Error> {
        let mut conn = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        (&mut *conn)
            .transaction(async move |conn| {
                authorize_action(conn, session, true).await?;
                let row = locked_site(conn, domain).await?;
                if row.owner_id != Some(session.member.id) {
                    return Err(Error::NotOwned);
                }
                queue_refresh(conn, row.site_id).await
            })
            .await
    }
    pub async fn owned_sites(
        &self,
        session: &AuthenticatedSession,
        page: u32,
    ) -> Result<OwnedPage, Error> {
        if page > 10_000 {
            return Err(Error::NotOwned);
        }
        let mut conn = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        (&mut *conn)
            .transaction(async move |conn| {
                authorize_action(conn, session, false).await?;
                let rows = diesel::sql_query(format!(
                    "{SELECT} WHERE d.owner_id=$1 ORDER BY s.domain LIMIT 25 OFFSET $2"
                ))
                .bind::<SqlUuid, _>(session.member.id)
                .bind::<BigInt, _>(i64::from(page) * 24)
                .load::<OwnerRow>(conn)
                .await?;
                let has_next = rows.len() > 24;
                Ok(OwnedPage {
                    sites: rows
                        .into_iter()
                        .take(24)
                        .map(OwnerRow::dto)
                        .collect::<Result<_, _>>()?,
                    has_next,
                    page,
                })
            })
            .await
    }
    pub(crate) async fn begin_owner_challenge(
        &self,
        session: &AuthenticatedSession,
        id: Uuid,
        domain: &str,
        hash: &[u8],
    ) -> Result<(), Error> {
        let mut conn = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        (&mut *conn).transaction(async move |conn| {
            authorize_action(conn,session,true).await?;
            // No unproved lookup that reveals whether this domain is hidden.
            diesel::sql_query("DELETE FROM directory_owner_challenges WHERE member_id=$1 AND domain=$2").bind::<SqlUuid,_>(session.member.id).bind::<Text,_>(domain).execute(conn).await?;
            diesel::sql_query("INSERT INTO directory_owner_challenges(id,member_id,session_id,domain,code_hash) VALUES($1,$2,$3,$4,$5)")
                .bind::<SqlUuid,_>(id).bind::<SqlUuid,_>(session.member.id).bind::<SqlUuid,_>(session.id).bind::<Text,_>(domain).bind::<Binary,_>(hash).execute(conn).await?;
            Ok(())
        }).await
    }
    pub(crate) async fn pending_owner_challenge(
        &self,
        session: &AuthenticatedSession,
        id: Uuid,
        hash: &[u8],
    ) -> Result<String, Error> {
        let mut conn = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        (&mut *conn)
            .transaction(async move |conn| {
                authorize_action(conn, session, true).await?;
                pending(conn, session, id, hash).await
            })
            .await
    }
    pub(crate) async fn claim_site_dns(
        &self,
        session: &AuthenticatedSession,
        proof: &VerifiedDns,
    ) -> Result<(), Error> {
        let mut conn = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        (&mut *conn).transaction(async move |conn| {
            authorize_action(conn,session,true).await?;
            let row=locked_site(conn,proof.domain()).await?;
            if pending(conn,session,proof.id(),proof.hash()).await?!=proof.domain() { return Err(Error::InvalidChallenge); }
            audit(conn,&row,session.member.id,"dns_claim").await?;
            // Phoenix priority: DNS control can replace an API or previous DNS owner.
            diesel::sql_query("UPDATE directory_site_details SET owner_id=$2,owner_method='dns',revision=revision+1,updated_at=now() WHERE site_id=$1")
                .bind::<SqlUuid,_>(row.site_id).bind::<SqlUuid,_>(session.member.id).execute(conn).await?;
            // Older in-flight proofs cannot immediately retake ownership.
            diesel::sql_query("DELETE FROM directory_owner_challenges WHERE domain=$1").bind::<Text,_>(proof.domain()).execute(conn).await?;
            Ok(())
        }).await
    }
    pub async fn edit_owned_site(
        &self,
        session: &AuthenticatedSession,
        domain: &str,
        revision: i64,
        value: &ValidatedEdit,
    ) -> Result<(), Error> {
        let mut conn = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        (&mut *conn).transaction(async move |conn| {
            authorize_action(conn,session,true).await?;
            let row=locked_site(conn,domain).await?;
            if row.owner_id!=Some(session.member.id) {return Err(Error::NotOwned);}
            if row.revision!=revision {return Err(Error::Conflict);}
            audit(conn,&row,session.member.id,"edit").await?;
            let e=&value.edit;
            // Owner controls only their hiding flag. Moderation/closure/observations
            // are not writable DTO fields, including when the site is force-hidden.
            diesel::update(sites::table.find(row.site_id)).set((sites::name.eq((!e.name.is_empty()).then_some(&e.name)),sites::description.eq((!e.description.is_empty()).then_some(&e.description)),sites::is_hidden.eq(e.hidden),sites::updated_at.eq(diesel::dsl::now))).execute(conn).await?;
            diesel::sql_query("UPDATE directory_site_details SET rules=nullif($2,''),language=nullif($3,''),tags=$4,owner_comment=nullif($5,''),invite_only=$6,approval_required=$7,revision=revision+1,updated_at=now() WHERE site_id=$1")
                .bind::<SqlUuid,_>(row.site_id).bind::<Text,_>(&e.rules).bind::<Text,_>(&e.language).bind::<diesel::sql_types::Array<Text>,_>(&value.tags).bind::<Text,_>(&e.owner_comment)
                .bind::<Nullable<Bool>,_>(e.invite_only).bind::<Nullable<Bool>,_>(e.approval_required).execute(conn).await?;
            Ok(())
        }).await
    }
    pub async fn resign_site(
        &self,
        session: &AuthenticatedSession,
        domain: &str,
        revision: i64,
    ) -> Result<(), Error> {
        let mut conn = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        (&mut *conn).transaction(async move |conn| {
            authorize_action(conn,session,true).await?;
            let row=locked_site(conn,domain).await?;
            if row.owner_id!=Some(session.member.id) {return Err(Error::NotOwned);}
            if row.revision!=revision {return Err(Error::Conflict);}
            audit(conn,&row,session.member.id,"resign").await?;
            diesel::sql_query("UPDATE directory_site_details SET owner_id=NULL,owner_method=NULL,revision=revision+1,updated_at=now() WHERE site_id=$1").bind::<SqlUuid,_>(row.site_id).execute(conn).await?;
            diesel::sql_query("DELETE FROM directory_owner_challenges WHERE domain=$1 AND member_id=$2").bind::<Text,_>(domain).bind::<SqlUuid,_>(session.member.id).execute(conn).await?;
            Ok(())
        }).await
    }
}

#[cfg(test)]
mod tests;

/// Caller holds the site/detail lock. Never lock existing jobs here: worker
/// completion uses job -> site. Owners and administrators share this limit.
async fn queue_refresh(conn: &mut AsyncPgConnection, site_id: Uuid) -> Result<(), Error> {
    let job_id = Uuid::new_v4();
    let reserved=diesel::sql_query("UPDATE directory_site_details SET refresh_requested_at=now() WHERE site_id=$1 AND (refresh_requested_at IS NULL OR refresh_requested_at<=now()-interval '1 hour')")
        .bind::<SqlUuid,_>(site_id).execute(conn).await?;
    if reserved != 1 {
        return Err(Error::RefreshTooSoon);
    }
    diesel::sql_query("INSERT INTO directory_jobs(id,site_id,scheduled_at,owner_requested) VALUES($1,$2,clock_timestamp(),true)")
        .bind::<SqlUuid,_>(job_id).bind::<SqlUuid,_>(site_id).execute(conn).await?;
    diesel::sql_query("UPDATE directory_site_details SET refresh_job_id=$2 WHERE site_id=$1")
        .bind::<SqlUuid, _>(site_id)
        .bind::<SqlUuid, _>(job_id)
        .execute(conn)
        .await?;
    Ok(())
}
