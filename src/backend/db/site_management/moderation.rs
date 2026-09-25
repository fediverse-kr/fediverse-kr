//! Administrator -> site -> detail. Share owner revisions and manual queue;
//! never overwrite editorial fields while changing unrelated moderation flags.
use super::{locked_site, queue_refresh, Database, DomainRow};
use crate::{
    backend::{
        auth::AuthenticatedSession,
        db::{
            media_cleanup,
            moderation::{authorize, gate},
        },
        moderation::{self as policy, Error},
        site_management,
        site_moderation::{self as domain, Command, DeleteCommand},
    },
    directory::management::SiteEdit,
    moderation::sites::{
        Action, Change, DeleteImpact, DeleteRequest, Event, History, Request, Site, SitePage,
    },
};
use diesel::{
    prelude::*,
    sql_types::{Array, BigInt, Text, Uuid as SqlUuid},
};
use diesel_async::{AsyncConnection, AsyncPgConnection, RunQueryDsl};
use uuid::Uuid;

fn owner_error(e: site_management::Error) -> Error {
    match e {
        site_management::Error::Auth(e) => Error::Auth(e),
        site_management::Error::NotOwned => Error::Missing,
        site_management::Error::Conflict => Error::Conflict,
        site_management::Error::RefreshTooSoon => Error::RefreshTooSoon,
        _ => Error::Unavailable,
    }
}
#[derive(QueryableByName)]
struct JsonRow {
    #[diesel(sql_type=Text)]
    value: String,
}
#[derive(QueryableByName)]
struct Count {
    #[diesel(sql_type=BigInt)]
    value: i64,
}
#[derive(QueryableByName)]
struct Key {
    #[diesel(sql_type=Text)]
    key: String,
}
const SELECT: &str = "SELECT jsonb_build_object('id',s.id,'domain',s.domain,'name',left(coalesce(s.name,o.observed_name,s.domain),200),'revision',coalesce(d.revision,0),'hidden',s.is_hidden,'force_hidden',s.is_force_hidden,'closed',s.is_closed,'invite_only',d.invite_only,'tags',left(coalesce(array_to_string(d.tags,', '),''),600),'tags_truncated',char_length(coalesce(array_to_string(d.tags,', '),''))>600,'software',left(o.software,100),'users',o.user_count,'owner_name',left(u.display_name,80),'refresh',(SELECT jsonb_build_object('state',CASE WHEN j.state='dead' OR (j.state='complete' AND j.last_error IS NOT NULL) THEN 'failed' ELSE j.state END,'requested_at',d.refresh_requested_at) FROM directory_jobs j WHERE j.id=d.refresh_job_id))::text AS value FROM directory_sites s LEFT JOIN directory_site_details d ON d.site_id=s.id LEFT JOIN member_users u ON u.id=d.owner_id LEFT JOIN directory_observations o ON o.site_id=s.id";
fn dto<T: serde::de::DeserializeOwned>(row: JsonRow) -> Result<T, Error> {
    serde_json::from_str(&row.value).map_err(|_| Error::Unavailable)
}
async fn detail(conn: &mut AsyncPgConnection, id: Uuid) -> Result<Site, Error> {
    let row = diesel::sql_query(format!("{SELECT} WHERE s.id=$1"))
        .bind::<SqlUuid, _>(id)
        .get_result::<JsonRow>(conn)
        .await
        .optional()?
        .ok_or(Error::Missing)?;
    dto(row)
}
fn label(value: Option<bool>) -> String {
    match value {
        Some(true) => "켜짐",
        Some(false) => "꺼짐",
        None => "미확인",
    }
    .into()
}
fn change(field: &str, before: String, after: String) -> Change {
    let truncated = before.chars().count() > 600 || after.chars().count() > 600;
    Change {
        field: field.into(),
        before: before.chars().take(600).collect(),
        after: after.chars().take(600).collect(),
        truncated,
    }
}

async fn delete_impact(conn: &mut AsyncPgConnection, id: Uuid) -> Result<DeleteImpact, Error> {
    let site = detail(conn, id).await?;
    let comments =
        diesel::sql_query("SELECT count(*) AS value FROM community_comments WHERE server_id=$1")
            .bind::<SqlUuid, _>(id)
            .get_result::<Count>(conn)
            .await?
            .value;
    let reports = diesel::sql_query("SELECT count(*) AS value FROM community_reports r WHERE EXISTS(SELECT 1 FROM community_comments c WHERE c.id=r.comment_id AND c.server_id=$1)")
        .bind::<SqlUuid, _>(id)
        .get_result::<Count>(conn)
        .await?
        .value;
    // The runtime health projection already includes imported health samples.
    // Counting legacy_health_checks as well would double-count imported rows.
    let health_checks =
        diesel::sql_query("SELECT count(*) AS value FROM directory_health_checks WHERE site_id=$1")
            .bind::<SqlUuid, _>(id)
            .get_result::<Count>(conn)
            .await?
            .value;
    Ok(DeleteImpact {
        site,
        comments,
        reports,
        health_checks,
    })
}

async fn deletion_audit(
    conn: &mut AsyncPgConnection,
    actor: Uuid,
    id: Uuid,
    revision: i64,
    comments: i64,
    reports: i64,
    health_checks: i64,
    note: &str,
) -> Result<(), Error> {
    // moderation_events intentionally has no target FK. Keep only stable IDs and
    // bounded counts here; site snapshots and comment bodies must not outlive a
    // physical deletion through this audit record.
    let before = format!("site={id};rev={revision}");
    let after = format!("deleted;c={comments};r={reports};h={health_checks}");
    diesel::sql_query("INSERT INTO moderation_events(id,actor_id,target_kind,target_id,action,before_state,after_state,note) VALUES($1,$2,'site',$3,'delete_site',$4,$5,$6)")
        .bind::<SqlUuid, _>(Uuid::new_v4())
        .bind::<SqlUuid, _>(actor)
        .bind::<SqlUuid, _>(id)
        .bind::<Text, _>(before)
        .bind::<Text, _>(after)
        .bind::<Text, _>(note)
        .execute(conn)
        .await?;
    Ok(())
}
impl Database {
    pub async fn moderation_sites(
        &self,
        s: &AuthenticatedSession,
        query: &str,
        status: &str,
        sort: &str,
        page: u32,
    ) -> Result<SitePage, Error> {
        policy::page(page)?;
        let query = domain::query(query)?;
        let status = domain::filter(status)?;
        let order = domain::order(sort)?;
        let mut conn = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        (&mut *conn).transaction(async move|conn|{
            authorize(conn,s,false).await?;
            // strpos is a literal search: % and _ never become SQL wildcards.
            let rows=diesel::sql_query(format!("{SELECT} WHERE (strpos(lower(s.domain),lower($1))>0 OR strpos(lower(coalesce(s.name,o.observed_name,'')),lower($1))>0) AND ($2='all' OR ($2='visible' AND NOT s.is_hidden AND NOT s.is_force_hidden) OR ($2='hidden' AND s.is_hidden) OR ($2='force_hidden' AND s.is_force_hidden) OR ($2='closed' AND s.is_closed)) ORDER BY {order} LIMIT 25 OFFSET $3"))
                .bind::<Text,_>(&query).bind::<Text,_>(status).bind::<BigInt,_>(i64::from(page)*24).load::<JsonRow>(conn).await?;
            Ok(SitePage{has_next:rows.len()>24,page,sites:rows.into_iter().take(24).map(dto).collect::<Result<_,_>>()?})
        }).await
    }
    pub async fn moderation_site(&self, s: &AuthenticatedSession, id: Uuid) -> Result<Site, Error> {
        let mut conn = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        (&mut *conn)
            .transaction(async move |conn| {
                authorize(conn, s, false).await?;
                detail(conn, id).await
            })
            .await
    }
    pub async fn moderation_site_delete_impact(
        &self,
        s: &AuthenticatedSession,
        id: Uuid,
    ) -> Result<DeleteImpact, Error> {
        let mut conn = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        (&mut *conn)
            .transaction(async move |conn| {
                authorize(conn, s, false).await?;
                delete_impact(conn, id).await
            })
            .await
    }
    pub async fn delete_moderated_site(
        &self,
        s: &AuthenticatedSession,
        request: DeleteRequest,
    ) -> Result<(), Error> {
        let command = DeleteCommand::try_from(request)?;
        let mut conn = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        (&mut *conn)
            .transaction(async move |conn| {
                gate(conn).await?;
                authorize(conn, s, true).await?;
                // This takes the established site -> detail lock. It conflicts
                // with community writers' FOR SHARE site lock before any comment
                // rows are detached.
                let row = locked_site(conn, &command.domain).await.map_err(owner_error)?;
                if row.site_id != command.id || row.revision != command.revision {
                    return Err(Error::Conflict);
                }

                let impact = delete_impact(conn, command.id).await?;
                let favicon_keys = diesel::sql_query("SELECT favicon_key AS key FROM legacy_sites WHERE id=$1 AND favicon_key LIKE 'favicons/%'")
                    .bind::<SqlUuid, _>(command.id)
                    .load::<Key>(conn)
                    .await?
                    .into_iter()
                    .map(|row| row.key)
                    .collect::<Vec<_>>();

                // Old imported data can contain a reply on another site whose
                // parent is a comment being removed. Preserve that unrelated
                // reply as a root instead of cascading it or failing deferred FK
                // validation at commit.
                diesel::sql_query("UPDATE community_comments SET parent_id=NULL WHERE server_id<>$1 AND parent_id IN (SELECT id FROM community_comments WHERE server_id=$1)")
                    .bind::<SqlUuid, _>(command.id)
                    .execute(conn)
                    .await?;
                // Reports and their existing evidence remain. Only their link to
                // a physically removed comment is cleared.
                diesel::sql_query("UPDATE community_reports SET comment_id=NULL WHERE comment_id IN (SELECT id FROM community_comments WHERE server_id=$1)")
                    .bind::<SqlUuid, _>(command.id)
                    .execute(conn)
                    .await?;
                diesel::sql_query("DELETE FROM community_comments WHERE server_id=$1")
                    .bind::<SqlUuid, _>(command.id)
                    .execute(conn)
                    .await?;
                // These are runtime projections, not the source archive/export.
                // Delete their exact target rows before the deferred legacy FKs
                // can block the directory-site removal.
                diesel::sql_query("DELETE FROM legacy_health_checks WHERE server_id=$1")
                    .bind::<SqlUuid, _>(command.id)
                    .execute(conn)
                    .await?;
                diesel::sql_query("DELETE FROM legacy_sites WHERE id=$1")
                    .bind::<SqlUuid, _>(command.id)
                    .execute(conn)
                    .await?;
                // A challenge has no site FK; remove only this deleted domain so
                // an old proof cannot claim a later same-domain registration.
                diesel::sql_query("DELETE FROM directory_owner_challenges WHERE domain=$1")
                    .bind::<Text, _>(&command.domain)
                    .execute(conn)
                    .await?;
                media_cleanup::retire_keys(conn, &favicon_keys)
                    .await
                    .map_err(|_| Error::Unavailable)?;
                let removed = diesel::sql_query("DELETE FROM directory_sites WHERE id=$1")
                    .bind::<SqlUuid, _>(command.id)
                    .execute(conn)
                    .await?;
                if removed != 1 {
                    return Err(Error::Missing);
                }
                deletion_audit(
                    conn,
                    s.member.id,
                    command.id,
                    command.revision,
                    impact.comments,
                    impact.reports,
                    impact.health_checks,
                    &command.note,
                )
                .await?;
                Ok(())
            })
            .await
    }
    pub async fn moderate_site(&self, s: &AuthenticatedSession, r: Request) -> Result<Site, Error> {
        let c = Command::try_from(r)?;
        let mut conn = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        (&mut *conn).transaction(async move|conn|{
            gate(conn).await?;
            authorize(conn,s,true).await?;
            let domain=diesel::sql_query("SELECT domain FROM directory_sites WHERE id=$1").bind::<SqlUuid,_>(c.id)
                .get_result::<DomainRow>(conn).await.optional()?.ok_or(Error::Missing)?.domain;
            let row=locked_site(conn,&domain).await.map_err(owner_error)?;
            if row.site_id!=c.id {return Err(Error::Missing);}
            if row.revision!=c.revision {return Err(Error::Conflict);}
            let edit:SiteEdit=serde_json::from_str(&row.edit).map_err(|_|Error::Unavailable)?;
            // Joined display strings are lossy: ["a, b"] is not ["a", "b"].
            // Compare the retained array, preserving NULL for an empty no-op.
            let old_tags:Vec<String>=if matches!(c.action,Action::Tags(_)) {
                let snapshot:serde_json::Value=serde_json::from_str(&row.snapshot).map_err(|_|Error::Unavailable)?;
                serde_json::from_value::<Option<Vec<String>>>(snapshot["details"]["tags"].clone()).map_err(|_|Error::Unavailable)?.unwrap_or_default()
            }else{Vec::new()};
            let changes=match &c.action {
                Action::Hidden(value)=>change("일반 숨김",label(Some(edit.hidden)),label(Some(*value))),
                Action::ForceHidden(value)=>change("강제 숨김",label(Some(row.force_hidden)),label(Some(*value))),
                Action::Closed(value)=>change("운영 종료",label(Some(row.closed)),label(Some(*value))),
                Action::InviteOnly(value)=>change("초대제",label(edit.invite_only),label(*value)),
                Action::Tags(_)=>{let tags=c.tags.as_ref().ok_or(Error::Invalid)?;change("태그",format!("{}개: {}",old_tags.len(),edit.tags),format!("{}개: {}",tags.len(),tags.join(", ")))},
                Action::Refresh=>change("수집 요청","이전 상태 유지".into(),"작업 큐에 요청".into()),
            };
            // A no-op does not manufacture a revision or audit entry. It still
            // requires current authority, freshness and matching revision.
            let unchanged=match &c.action {
                Action::Tags(_)=>old_tags==*c.tags.as_ref().ok_or(Error::Invalid)?,
                Action::Refresh=>false,
                _=>changes.before==changes.after,
            };
            if unchanged {return detail(conn,c.id).await;}
            match c.action {
                Action::Hidden(value)=>{diesel::sql_query("UPDATE directory_sites SET is_hidden=$2,updated_at=now() WHERE id=$1").bind::<SqlUuid,_>(c.id).bind::<diesel::sql_types::Bool,_>(value).execute(conn).await?;},
                Action::ForceHidden(value)=>{diesel::sql_query("UPDATE directory_sites SET is_force_hidden=$2,updated_at=now() WHERE id=$1").bind::<SqlUuid,_>(c.id).bind::<diesel::sql_types::Bool,_>(value).execute(conn).await?;},
                Action::Closed(value)=>{diesel::sql_query("UPDATE directory_sites SET is_closed=$2,updated_at=now() WHERE id=$1").bind::<SqlUuid,_>(c.id).bind::<diesel::sql_types::Bool,_>(value).execute(conn).await?;},
                Action::InviteOnly(value)=>{diesel::sql_query("UPDATE directory_site_details SET invite_only=$2 WHERE site_id=$1").bind::<SqlUuid,_>(c.id).bind::<diesel::sql_types::Nullable<diesel::sql_types::Bool>,_>(value).execute(conn).await?;},
                Action::Tags(_)=>{diesel::sql_query("UPDATE directory_site_details SET tags=$2 WHERE site_id=$1").bind::<SqlUuid,_>(c.id).bind::<Array<Text>,_>(c.tags.as_ref().ok_or(Error::Invalid)?).execute(conn).await?;},
                Action::Refresh=>queue_refresh(conn,c.id).await.map_err(owner_error)?,
            }
            let action=if matches!(c.action,Action::Refresh){"admin_refresh"}else{"admin_edit"};
            diesel::sql_query("INSERT INTO directory_site_edits(id,site_id,actor_id,action,revision,previous,admin_note,admin_change) VALUES($1,$2,$3,$4,$5,$6::jsonb,$7,$8::jsonb)")
                .bind::<SqlUuid,_>(Uuid::new_v4()).bind::<SqlUuid,_>(c.id).bind::<SqlUuid,_>(s.member.id).bind::<Text,_>(action).bind::<BigInt,_>(row.revision)
                .bind::<Text,_>(&row.snapshot).bind::<Text,_>(&c.note).bind::<Text,_>(serde_json::to_string(&changes).map_err(|_|Error::Unavailable)?).execute(conn).await?;
            diesel::sql_query("UPDATE directory_site_details SET revision=revision+1,updated_at=now() WHERE site_id=$1").bind::<SqlUuid,_>(c.id).execute(conn).await?;
            detail(conn,c.id).await
        }).await
    }
    pub async fn moderation_site_history(
        &self,
        s: &AuthenticatedSession,
        id: Uuid,
        page: u32,
    ) -> Result<History, Error> {
        policy::page(page)?;
        let mut conn = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        (&mut *conn).transaction(async move|conn|{
            authorize(conn,s,false).await?;
            detail(conn,id).await?;
            let rows=diesel::sql_query("SELECT jsonb_build_object('id',e.id,'revision',e.revision+1,'actor_name',left(u.display_name,80),'note',e.admin_note,'change',e.admin_change,'created_at',e.created_at)::text AS value FROM directory_site_edits e LEFT JOIN member_users u ON u.id=e.actor_id WHERE e.site_id=$1 AND e.action IN ('admin_edit','admin_refresh') ORDER BY e.revision DESC LIMIT 21 OFFSET $2")
                .bind::<SqlUuid,_>(id).bind::<BigInt,_>(i64::from(page)*20).load::<JsonRow>(conn).await?;
            Ok(History{page,has_next:rows.len()>20,events:rows.into_iter().take(20).map(dto::<Event>).collect::<Result<_,_>>()?})
        }).await
    }
}
#[cfg(test)]
mod tests;
