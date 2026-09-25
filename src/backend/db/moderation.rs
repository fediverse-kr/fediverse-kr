//! Private moderation repository. Mutation lock order: moderation gate ->
//! acting member/session -> optional non-admin target member -> site -> comment
//! -> report. Other community/lifecycle writers never acquire the gate.
use super::{members::authorize_action, Database};
use crate::{
    backend::{
        auth::AuthenticatedSession,
        moderation::{self as domain, Command, Error},
    },
    moderation::*,
};
use chrono::{DateTime, NaiveDateTime, Utc};
use diesel::{
    prelude::*,
    sql_types::{BigInt, Bool, Nullable, Text, Timestamp, Timestamptz, Uuid as SqlUuid},
};
use diesel_async::{AsyncConnection, AsyncPgConnection, RunQueryDsl};
use uuid::Uuid;
impl From<diesel::result::Error> for Error {
    fn from(_: diesel::result::Error) -> Self {
        Self::Unavailable
    }
}
#[derive(QueryableByName)]
struct Flag {
    #[diesel(sql_type=Bool)]
    value: bool,
}
#[derive(QueryableByName)]
struct Id {
    #[diesel(sql_type=SqlUuid)]
    id: Uuid,
}
pub(super) async fn gate(conn: &mut AsyncPgConnection) -> Result<(), Error> {
    diesel::sql_query("SELECT pg_advisory_xact_lock(6810476213314)")
        .execute(conn)
        .await?;
    Ok(())
}
async fn is_admin(conn: &mut AsyncPgConnection, id: Uuid) -> Result<bool, Error> {
    Ok(diesel::sql_query(
        "SELECT EXISTS(SELECT 1 FROM member_admin_roles WHERE member_id=$1) AS value",
    )
    .bind::<SqlUuid, _>(id)
    .get_result::<Flag>(conn)
    .await?
    .value)
}
pub(super) async fn authorize(
    conn: &mut AsyncPgConnection,
    s: &AuthenticatedSession,
    fresh: bool,
) -> Result<(), Error> {
    authorize_action(conn, s, fresh).await?;
    if !is_admin(conn, s.member.id).await? {
        return Err(Error::Forbidden);
    }
    Ok(())
}
fn revision(at: NaiveDateTime) -> String {
    at.format("%Y-%m-%dT%H:%M:%S%.6f").to_string()
}
#[derive(QueryableByName)]
struct ReportRow {
    #[diesel(sql_type=SqlUuid)]
    id: Uuid,
    #[diesel(sql_type=Nullable<SqlUuid>)]
    comment_id: Option<Uuid>,
    #[diesel(sql_type=Nullable<SqlUuid>)]
    site_id: Option<Uuid>,
    #[diesel(sql_type=Nullable<Text>)]
    domain: Option<String>,
    #[diesel(sql_type=Text)]
    status: String,
    #[diesel(sql_type=Text)]
    reason: String,
    #[diesel(sql_type=Nullable<Text>)]
    reporter_name: Option<String>,
    #[diesel(sql_type=Text)]
    excerpt: String,
    #[diesel(sql_type=Text)]
    detail: String,
    #[diesel(sql_type=Bool)]
    detail_truncated: bool,
    #[diesel(sql_type=Text)]
    admin_note: String,
    #[diesel(sql_type=Bool)]
    note_truncated: bool,
    #[diesel(sql_type=Nullable<Text>)]
    resolver_name: Option<String>,
    #[diesel(sql_type=Nullable<Timestamp>)]
    resolved_at: Option<NaiveDateTime>,
    #[diesel(sql_type=Timestamp)]
    inserted_at: NaiveDateTime,
    #[diesel(sql_type=Timestamp)]
    updated_at: NaiveDateTime,
}
const REPORT:&str="SELECT r.id,r.comment_id,c.server_id AS site_id,s.domain,r.status,r.reason,left(u.display_name,80) AS reporter_name,left(coalesce(c.body,''),180) AS excerpt,left(coalesce(r.detail,''),1000) AS detail,coalesce(char_length(r.detail)>1000,false) AS detail_truncated,left(coalesce(r.admin_note,''),1000) AS admin_note,coalesce(char_length(r.admin_note)>1000,false) AS note_truncated,left(a.display_name,80) AS resolver_name,r.resolved_at,r.inserted_at,r.updated_at FROM community_reports r LEFT JOIN community_comments c ON c.id=r.comment_id LEFT JOIN directory_sites s ON s.id=c.server_id LEFT JOIN member_users u ON u.id=r.reporter_id LEFT JOIN member_users a ON a.id=r.resolved_by_id";
impl ReportRow {
    fn summary(&self) -> ReportSummary {
        ReportSummary {
            id: self.id.to_string(),
            status: self.status.clone(),
            reason: self.reason.clone(),
            domain: self.domain.clone(),
            reporter_name: self.reporter_name.clone(),
            excerpt: self.excerpt.clone(),
            created_at: self.inserted_at.and_utc().to_rfc3339(),
        }
    }
}
async fn report(conn: &mut AsyncPgConnection, id: Uuid, lock: bool) -> Result<ReportRow, Error> {
    let lock = if lock { " FOR UPDATE OF r" } else { "" };
    diesel::sql_query(format!("{REPORT} WHERE r.id=$1{lock}"))
        .bind::<SqlUuid, _>(id)
        .get_result(conn)
        .await
        .optional()?
        .ok_or(Error::Missing)
}
#[derive(QueryableByName)]
struct CommentRow {
    #[diesel(sql_type=SqlUuid)]
    id: Uuid,
    #[diesel(sql_type=Nullable<SqlUuid>)]
    user_id: Option<Uuid>,
    #[diesel(sql_type=Nullable<Text>)]
    author_name: Option<String>,
    #[diesel(sql_type=Nullable<Bool>)]
    banned: Option<bool>,
    #[diesel(sql_type=Nullable<Timestamp>)]
    author_updated_at: Option<NaiveDateTime>,
    #[diesel(sql_type=Bool)]
    admin: bool,
    #[diesel(sql_type=Text)]
    body: String,
    #[diesel(sql_type=Bool)]
    deleted: bool,
    #[diesel(sql_type=Timestamp)]
    updated_at: NaiveDateTime,
}
async fn comment(conn: &mut AsyncPgConnection, id: Uuid, lock: bool) -> Result<CommentRow, Error> {
    let lock = if lock { " FOR UPDATE OF c" } else { "" };
    diesel::sql_query(format!("SELECT c.id,c.user_id,left(u.display_name,80) AS author_name,u.is_banned AS banned,u.updated_at AT TIME ZONE 'UTC' AS author_updated_at,EXISTS(SELECT 1 FROM member_admin_roles WHERE member_id=c.user_id) AS admin,c.body,(c.is_deleted IS TRUE OR c.user_id IS NULL) AS deleted,c.updated_at FROM community_comments c LEFT JOIN member_users u ON u.id=c.user_id WHERE c.id=$1 AND octet_length(c.body)<=1048576{lock}"))
        .bind::<SqlUuid,_>(id).get_result(conn).await.optional()?.ok_or(Error::Missing)
}
impl CommentRow {
    fn dto(self) -> ReportComment {
        ReportComment {
            id: self.id.to_string(),
            revision: revision(self.updated_at),
            body: self.body,
            author_id: self.user_id.map(|v| v.to_string()),
            author_name: self.author_name,
            author_banned: self.banned,
            author_revision: self.author_updated_at.map(revision),
            author_admin: self.admin,
            deleted: self.deleted,
        }
    }
}
#[derive(QueryableByName)]
struct EvidenceRow {
    #[diesel(sql_type=Text)]
    body: String,
    #[diesel(sql_type=Text)]
    author_name: String,
    #[diesel(sql_type=Timestamp)]
    comment_updated_at: NaiveDateTime,
}
async fn detail(conn: &mut AsyncPgConnection, id: Uuid) -> Result<ReportDetail, Error> {
    let r = report(conn, id, false).await?;
    let c = if let Some(id) = r.comment_id {
        Some(comment(conn, id, false).await?.dto())
    } else {
        None
    };
    let evidence=diesel::sql_query("SELECT body,left(author_name,80) AS author_name,comment_updated_at FROM community_report_evidence WHERE report_id=$1").bind::<SqlUuid,_>(id).get_result::<EvidenceRow>(conn).await.optional()?;
    Ok(ReportDetail {
        summary: r.summary(),
        revision: revision(r.updated_at),
        detail: r.detail,
        detail_truncated: r.detail_truncated,
        admin_note: r.admin_note,
        note_truncated: r.note_truncated,
        resolver_name: r.resolver_name,
        resolved_at: r.resolved_at.map(|v| v.and_utc().to_rfc3339()),
        comment: c,
        evidence: evidence.map(|e| Evidence {
            body: e.body,
            author_name: e.author_name,
            saved_revision: revision(e.comment_updated_at),
        }),
    })
}
struct Audit<'a> {
    actor: Option<Uuid>,
    kind: &'a str,
    target: Uuid,
    report: Option<Uuid>,
    action: &'a str,
    before: &'a str,
    after: &'a str,
    note: &'a str,
}
async fn audit(conn: &mut AsyncPgConnection, e: Audit<'_>) -> Result<(), Error> {
    diesel::sql_query("INSERT INTO moderation_events(id,actor_id,target_kind,target_id,report_id,action,before_state,after_state,note) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9)")
        .bind::<SqlUuid,_>(Uuid::new_v4()).bind::<Nullable<SqlUuid>,_>(e.actor).bind::<Text,_>(e.kind).bind::<SqlUuid,_>(e.target).bind::<Nullable<SqlUuid>,_>(e.report).bind::<Text,_>(e.action).bind::<Text,_>(e.before).bind::<Text,_>(e.after).bind::<Text,_>(e.note).execute(conn).await?;
    Ok(())
}
impl Database {
    pub async fn moderation_access(&self, s: &AuthenticatedSession) -> Result<bool, Error> {
        let mut conn = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        (&mut *conn)
            .transaction(async move |conn| {
                authorize_action(conn, s, false).await?;
                is_admin(conn, s.member.id).await
            })
            .await
    }
    /// Called only by the explicit CLI and test fixtures; not an HTTP method.
    pub async fn set_admin_role(&self, member: Uuid, grant: bool) -> Result<bool, Error> {
        let mut conn = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        (&mut *conn)
            .transaction(async move |conn| {
                gate(conn).await?;
                let banned = diesel::sql_query(
                    "SELECT is_banned AS value FROM member_users WHERE id=$1 FOR UPDATE",
                )
                .bind::<SqlUuid, _>(member)
                .get_result::<Flag>(conn)
                .await
                .optional()?
                .ok_or(Error::Missing)?
                .value;
                if grant && banned {
                    return Err(Error::Protected);
                }
                if is_admin(conn, member).await? == grant {
                    return Ok(false);
                }
                let sql = if grant {
                    "INSERT INTO member_admin_roles(member_id) VALUES($1)"
                } else {
                    "DELETE FROM member_admin_roles WHERE member_id=$1"
                };
                diesel::sql_query(sql)
                    .bind::<SqlUuid, _>(member)
                    .execute(conn)
                    .await?;
                audit(
                    conn,
                    Audit {
                        actor: None,
                        kind: "admin_role",
                        target: member,
                        report: None,
                        action: if grant { "grant" } else { "revoke" },
                        before: if grant { "member" } else { "admin" },
                        after: if grant { "admin" } else { "member" },
                        note: "Explicit host administrator command",
                    },
                )
                .await?;
                Ok(true)
            })
            .await
    }
    pub async fn moderation_reports(
        &self,
        s: &AuthenticatedSession,
        status: &str,
        page: u32,
    ) -> Result<ReportPage, Error> {
        let status = domain::filter(status)?.to_owned();
        domain::page(page)?;
        let mut conn = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        (&mut *conn).transaction(async move|conn|{
            authorize(conn,s,false).await?;
            let mut rows=diesel::sql_query(format!("{REPORT} WHERE ($1='all' OR r.status=$1) ORDER BY r.inserted_at DESC,r.id DESC LIMIT 21 OFFSET $2")).bind::<Text,_>(status).bind::<BigInt,_>(i64::from(page)*20).load::<ReportRow>(conn).await?;
            let has_next=rows.len()>20;rows.truncate(20);Ok(ReportPage{reports:rows.iter().map(ReportRow::summary).collect(),page,has_next})
        }).await
    }
    pub async fn moderation_report(
        &self,
        s: &AuthenticatedSession,
        id: Uuid,
    ) -> Result<ReportDetail, Error> {
        let mut conn = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        (&mut *conn)
            .transaction(async move |conn| {
                // A consistent read of current comment, original evidence and report.
                diesel::sql_query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ")
                    .execute(conn)
                    .await?;
                authorize(conn, s, false).await?;
                detail(conn, id).await
            })
            .await
    }
    pub async fn moderation_events(
        &self,
        s: &AuthenticatedSession,
        id: Uuid,
        page: u32,
    ) -> Result<EventPage, Error> {
        domain::page(page)?;
        #[derive(QueryableByName)]
        struct Row {
            #[diesel(sql_type=SqlUuid)]
            id: Uuid,
            #[diesel(sql_type=Text)]
            action: String,
            #[diesel(sql_type=Nullable<Text>)]
            actor_name: Option<String>,
            #[diesel(sql_type=Text)]
            before_state: String,
            #[diesel(sql_type=Text)]
            after_state: String,
            #[diesel(sql_type=Text)]
            note: String,
            #[diesel(sql_type=Timestamptz)]
            created_at: DateTime<Utc>,
        }
        let mut conn = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        (&mut *conn).transaction(async move|conn|{
            authorize(conn,s,false).await?;report(conn,id,false).await?;
            let mut rows=diesel::sql_query("SELECT e.id,e.action,left(u.display_name,80) AS actor_name,e.before_state,e.after_state,e.note,e.created_at FROM moderation_events e LEFT JOIN member_users u ON u.id=e.actor_id WHERE e.report_id=$1 ORDER BY e.created_at DESC,e.id DESC LIMIT 21 OFFSET $2").bind::<SqlUuid,_>(id).bind::<BigInt,_>(i64::from(page)*20).load::<Row>(conn).await?;
            let has_next=rows.len()>20;rows.truncate(20);
            Ok(EventPage{page,has_next,events:rows.into_iter().map(|r|Event{id:r.id.to_string(),action:r.action,actor_name:r.actor_name,before:r.before_state,after:r.after_state,note:r.note,created_at:r.created_at.to_rfc3339()}).collect()})
        }).await
    }
    pub async fn moderate_report(
        &self,
        s: &AuthenticatedSession,
        request: ActionRequest,
    ) -> Result<ReportDetail, Error> {
        let cmd = Command::try_from(request)?;
        let mut conn = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        let (result,notify)=(&mut *conn).transaction(async move|conn|{
            gate(conn).await?;authorize(conn,s,true).await?;
            let initial=report(conn,cmd.report,false).await?;
            let member_action=matches!(cmd.action,Action::Ban|Action::Unban);
            if member_action {
                let target=cmd.author.ok_or(Error::Conflict)?;
                if target==s.member.id||is_admin(conn,target).await?{return Err(Error::Protected)}
                #[derive(QueryableByName)] struct Target {#[diesel(sql_type=Bool)] banned:bool,#[diesel(sql_type=Timestamp)] updated_at:NaiveDateTime}
                let current=diesel::sql_query("SELECT is_banned AS banned,updated_at AT TIME ZONE 'UTC' AS updated_at FROM member_users WHERE id=$1 FOR UPDATE").bind::<SqlUuid,_>(target).get_result::<Target>(conn).await.optional()?.ok_or(Error::Conflict)?;
                if Some(current.banned)!=cmd.banned || Some(current.updated_at)!=cmd.author_revision{return Err(Error::Conflict)}
            }
            if let Some(site)=initial.site_id {
                let locked=diesel::sql_query("SELECT id FROM directory_sites WHERE id=$1 FOR SHARE").bind::<SqlUuid,_>(site).get_result::<Id>(conn).await?;
                if locked.id!=site{return Err(Error::Missing)}
            }
            let c=if let Some(id)=initial.comment_id {Some(comment(conn,id,true).await?)}else{None};
            let r=report(conn,cmd.report,true).await?;
            if r.updated_at!=cmd.revision || r.comment_id!=initial.comment_id || r.site_id!=initial.site_id || c.as_ref().map(|c|c.updated_at)!=cmd.comment_revision {return Err(Error::Conflict)}
            let mut notify=None;
            let (kind,target,before,after)=match cmd.action {
                Action::Resolve|Action::Dismiss|Action::Reopen=>{
                    let next=match cmd.action {Action::Resolve=>"resolved",Action::Dismiss=>"dismissed",_=>"pending"};
                    if r.status==next{return Err(Error::Conflict)}
                    diesel::sql_query("UPDATE community_reports SET status=$2,admin_note=$3,resolved_by_id=CASE WHEN $2='pending' THEN NULL ELSE $4 END,resolved_at=CASE WHEN $2='pending' THEN NULL ELSE clock_timestamp() AT TIME ZONE 'UTC' END,updated_at=greatest(clock_timestamp() AT TIME ZONE 'UTC',updated_at+interval '1 microsecond') WHERE id=$1")
                        .bind::<SqlUuid,_>(cmd.report).bind::<Text,_>(next).bind::<Text,_>(&cmd.note).bind::<SqlUuid,_>(s.member.id).execute(conn).await?;
                    ("report",cmd.report,r.status,next.to_owned())
                },
                Action::DeleteComment=>{
                    let c=c.as_ref().ok_or(Error::Missing)?;if c.deleted{return Err(Error::Conflict)}
                    diesel::sql_query("UPDATE community_comments SET is_deleted=true,updated_at=greatest(clock_timestamp() AT TIME ZONE 'UTC',updated_at+interval '1 microsecond') WHERE id=$1").bind::<SqlUuid,_>(c.id).execute(conn).await?;
                    notify=r.site_id;("comment",c.id,"visible".into(),"deleted".into())
                },
                Action::Ban|Action::Unban=>{
                    let c=c.as_ref().ok_or(Error::Missing)?;
                    let next=cmd.action==Action::Ban;
                    if c.user_id!=cmd.author||c.banned!=cmd.banned||c.banned==Some(next){return Err(Error::Conflict)}
                    let target=c.user_id.ok_or(Error::Conflict)?;
                    diesel::sql_query("UPDATE member_users SET is_banned=$2,updated_at=greatest(clock_timestamp(),updated_at+interval '1 microsecond') WHERE id=$1").bind::<SqlUuid,_>(target).bind::<Bool,_>(next).execute(conn).await?;
                    if next {
                        diesel::sql_query("DELETE FROM member_link_challenges WHERE member_id=$1 OR actor_url IN (SELECT actor_url FROM member_linked_accounts WHERE member_id=$1) OR actor_url IN (SELECT actor_url FROM member_legacy_claims WHERE member_id=$1) OR lower(handle) IN (SELECT handle FROM member_legacy_claims WHERE member_id=$1)").bind::<SqlUuid,_>(target).execute(conn).await?;
                        diesel::sql_query("DELETE FROM member_sessions WHERE member_id=$1").bind::<SqlUuid,_>(target).execute(conn).await?;
                    }
                    ("member",target,if next{"active"}else{"banned"}.into(),if next{"banned"}else{"active"}.into())
                }
            };
            audit(conn,Audit{actor:Some(s.member.id),kind,target,report:Some(cmd.report),action:cmd.action.key(),before:&before,after:&after,note:&cmd.note}).await?;
            Ok::<_,Error>((detail(conn,cmd.report).await?,notify))
        }).await?;
        if let Some(site) = notify {
            crate::backend::community::changes::notify(site)
        }
        Ok(result)
    }
}
#[cfg(test)]
mod tests;
