//! member -> session -> site -> comment locks; public reads have no identities.
use super::{members::authorize_action, Database};
use crate::{
    backend::{
        auth::AuthenticatedSession,
        community::{self as domain, Error},
    },
    community::{Access, Comment, Permission, Reason, ReplyPage, Thread, ThreadPage},
};
use chrono::NaiveDateTime;
use diesel::{
    prelude::*,
    sql_types::{Array, BigInt, Bool, Nullable, Text, Timestamp, Uuid as SqlUuid},
};
use diesel_async::{AsyncConnection, AsyncPgConnection, RunQueryDsl};
use uuid::Uuid;
mod personal;
impl From<diesel::result::Error> for Error {
    fn from(_: diesel::result::Error) -> Self {
        Self::Unavailable
    }
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
#[derive(QueryableByName)]
struct Count {
    #[diesel(sql_type=BigInt)]
    value: i64,
}
#[derive(QueryableByName)]
struct PublicRow {
    #[diesel(sql_type=SqlUuid)]
    id: Uuid,
    #[diesel(sql_type=Nullable<SqlUuid>)]
    parent_id: Option<Uuid>,
    #[diesel(sql_type=Text)]
    author_name: String,
    #[diesel(sql_type=Text)]
    body: String,
    #[diesel(sql_type=Bool)]
    deleted: bool,
    #[diesel(sql_type=Bool)]
    truncated: bool,
    #[diesel(sql_type=Timestamp)]
    inserted_at: NaiveDateTime,
    #[diesel(sql_type=Timestamp)]
    updated_at: NaiveDateTime,
    #[diesel(sql_type=BigInt)]
    reply_count: i64,
}
impl PublicRow {
    fn dto(&self) -> Comment {
        Comment {
            id: self.id.to_string(),
            parent_id: self.parent_id.map(|id| id.to_string()),
            author_name: self.author_name.clone(),
            body: self.body.clone(),
            deleted: self.deleted,
            truncated: self.truncated,
            created_at: self.inserted_at.and_utc().to_rfc3339(),
            updated_at: self.updated_at.and_utc().to_rfc3339(),
            revision: self.updated_at.format("%Y-%m-%dT%H:%M:%S%.6f").to_string(),
        }
    }
}
const PUBLIC:&str="SELECT c.id,c.parent_id,CASE WHEN c.is_deleted IS TRUE OR c.user_id IS NULL THEN '' ELSE left(coalesce(u.display_name,'회원'),80) END AS author_name,CASE WHEN c.is_deleted IS TRUE OR c.user_id IS NULL THEN '' ELSE left(c.body,2000) END AS body,(c.is_deleted IS TRUE OR c.user_id IS NULL) AS deleted,(c.is_deleted IS NOT TRUE AND c.user_id IS NOT NULL AND char_length(c.body)>2000) AS truncated,c.inserted_at,c.updated_at,(SELECT count(*) FROM community_comments r WHERE r.parent_id=c.id AND r.server_id=c.server_id) AS reply_count FROM community_comments c LEFT JOIN member_users u ON u.id=c.user_id";

async fn site(conn: &mut AsyncPgConnection, domain: &str, lock: bool) -> Result<Uuid, Error> {
    let suffix = if lock { " FOR SHARE" } else { "" };
    diesel::sql_query(format!("SELECT id FROM directory_sites WHERE domain=$1 AND NOT is_hidden AND NOT is_force_hidden{suffix}"))
        .bind::<Text,_>(domain).get_result::<Id>(conn).await.optional()?.map(|r|r.id).ok_or(Error::Missing)
}
async fn comment(conn: &mut AsyncPgConnection, site: Uuid, id: Uuid) -> Result<Comment, Error> {
    diesel::sql_query(format!("{PUBLIC} WHERE c.server_id=$1 AND c.id=$2"))
        .bind::<SqlUuid, _>(site)
        .bind::<SqlUuid, _>(id)
        .get_result::<PublicRow>(conn)
        .await
        .optional()?
        .map(|r| r.dto())
        .ok_or(Error::Missing)
}
#[derive(QueryableByName)]
struct Locked {
    #[diesel(sql_type=Nullable<SqlUuid>)]
    user_id: Option<Uuid>,
    #[diesel(sql_type=Nullable<SqlUuid>)]
    parent_id: Option<Uuid>,
    #[diesel(sql_type=Text)]
    body: String,
    #[diesel(sql_type=Text)]
    author_name: String,
    #[diesel(sql_type=Bool)]
    deleted: bool,
    #[diesel(sql_type=Bool)]
    editable: bool,
    #[diesel(sql_type=Timestamp)]
    updated_at: NaiveDateTime,
}
async fn locked(conn: &mut AsyncPgConnection, site: Uuid, id: Uuid) -> Result<Locked, Error> {
    diesel::sql_query("SELECT c.user_id,c.parent_id,c.body,coalesce(u.display_name,'회원') AS author_name,(c.is_deleted IS TRUE OR c.user_id IS NULL) AS deleted,(clock_timestamp() AT TIME ZONE 'UTC'<=c.inserted_at+interval '30 minutes') AS editable,c.updated_at FROM community_comments c LEFT JOIN member_users u ON u.id=c.user_id WHERE c.server_id=$1 AND c.id=$2 AND octet_length(c.body)<=1048576 FOR UPDATE OF c")
        .bind::<SqlUuid,_>(site).bind::<SqlUuid,_>(id).get_result(conn).await.optional()?.ok_or(Error::Missing)
}
impl Database {
    pub async fn comment_site(&self, domain: &str) -> Result<Uuid, Error> {
        let domain = domain::domain(domain)?;
        let mut conn = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        site(&mut conn, &domain, false).await
    }
    pub async fn comment_threads(&self, domain: &str, page: u32) -> Result<ThreadPage, Error> {
        let domain = domain::domain(domain)?;
        domain::page(page)?;
        let mut conn = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        (&mut *conn).transaction(async move|conn|{
            diesel::sql_query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY").execute(conn).await?;
            let id=site(conn,&domain,false).await?;
            let mut roots=diesel::sql_query(format!("{PUBLIC} WHERE c.server_id=$1 AND c.parent_id IS NULL ORDER BY c.inserted_at DESC,c.id DESC LIMIT 21 OFFSET $2")).bind::<SqlUuid,_>(id).bind::<BigInt,_>(i64::from(page)*20).load::<PublicRow>(conn).await?;
            let has_next=roots.len()>20;roots.truncate(20);let ids:Vec<_>=roots.iter().map(|r|r.id).collect();
            // One bounded query for the first three replies of every root.
            let replies=diesel::sql_query(format!("{PUBLIC} WHERE c.id IN (SELECT id FROM (SELECT id,row_number() OVER(PARTITION BY parent_id ORDER BY inserted_at,id) AS n FROM community_comments WHERE server_id=$1 AND parent_id=ANY($2)) ranked WHERE n<=3) ORDER BY c.inserted_at,c.id"))
                .bind::<SqlUuid,_>(id).bind::<Array<SqlUuid>,_>(&ids).load::<PublicRow>(conn).await?;
            Ok(ThreadPage{domain,preview:false,page,has_next,threads:roots.into_iter().map(|r|Thread{comment:r.dto(),reply_count:r.reply_count,replies:replies.iter().filter(|c|c.parent_id==Some(r.id)).map(PublicRow::dto).collect()}).collect()})
        }).await
    }
    pub async fn comment_replies(
        &self,
        domain: &str,
        parent: Uuid,
        page: u32,
    ) -> Result<ReplyPage, Error> {
        let domain = domain::domain(domain)?;
        domain::page(page)?;
        let mut conn = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        (&mut *conn).transaction(async move|conn|{
            diesel::sql_query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY").execute(conn).await?;
            let id=site(conn,&domain,false).await?;let root=comment(conn,id,parent).await?;
            if root.parent_id.is_some(){return Err(Error::Missing)}
            let mut replies=diesel::sql_query(format!("{PUBLIC} WHERE c.server_id=$1 AND c.parent_id=$2 ORDER BY c.inserted_at,c.id LIMIT 21 OFFSET $3")).bind::<SqlUuid,_>(id).bind::<SqlUuid,_>(parent).bind::<BigInt,_>(i64::from(page)*20).load::<PublicRow>(conn).await?;
            let has_next=replies.len()>20;replies.truncate(20);
            Ok(ReplyPage{domain,parent:parent.to_string(),page,has_next,replies:replies.iter().map(PublicRow::dto).collect()})
        }).await
    }
    pub async fn comment_access(
        &self,
        s: &AuthenticatedSession,
        domain: &str,
        ids: &[Uuid],
    ) -> Result<Access, Error> {
        let domain = domain::domain(domain)?;
        if ids.len() > 100 {
            return Err(Error::Invalid);
        }
        let mut conn = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        #[derive(QueryableByName)]
        struct Rights {
            #[diesel(sql_type=SqlUuid)]
            id: Uuid,
            #[diesel(sql_type=Bool)]
            own: bool,
            #[diesel(sql_type=Bool)]
            can_edit: bool,
            #[diesel(sql_type=Bool)]
            can_report: bool,
        }
        (&mut *conn).transaction(async move|conn|{
            let member=authorize_action(conn,s,false).await?;let site=site(conn,&domain,true).await?;
            let rows=diesel::sql_query("SELECT c.id,c.user_id=$2 AS own,(c.user_id=$2 AND clock_timestamp() AT TIME ZONE 'UTC'<=c.inserted_at+interval '30 minutes') AS can_edit,(c.user_id<>$2 AND NOT EXISTS(SELECT 1 FROM community_reports r WHERE r.comment_id=c.id AND r.reporter_id=$2)) AS can_report FROM community_comments c WHERE c.server_id=$1 AND c.id=ANY($3) AND c.is_deleted IS NOT TRUE AND c.user_id IS NOT NULL")
                .bind::<SqlUuid,_>(site).bind::<SqlUuid,_>(s.member.id).bind::<Array<SqlUuid>,_>(ids).load::<Rights>(conn).await?;
            Ok(Access{display_name:Some(member.display_name),available:true,permissions:rows.into_iter().map(|r|Permission{id:r.id.to_string(),own:r.own,can_edit:r.can_edit,can_report:r.can_report}).collect()})
        }).await
    }
    pub async fn create_comment(
        &self,
        s: &AuthenticatedSession,
        domain: &str,
        parent: Option<Uuid>,
        body: String,
    ) -> Result<Comment, Error> {
        let domain = domain::domain(domain)?;
        let body = domain::text(body, 2000, true)?;
        let mut conn = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        let (result,id)=(&mut *conn).transaction(async move|conn|{
            authorize_action(conn,s,false).await?;let site=site(conn,&domain,true).await?;
            let recent=diesel::sql_query("SELECT EXISTS(SELECT 1 FROM community_comments WHERE user_id=$1 AND server_id=$2 AND inserted_at>clock_timestamp() AT TIME ZONE 'UTC'-interval '1 minute') AS value")
                .bind::<SqlUuid,_>(s.member.id).bind::<SqlUuid,_>(site).get_result::<Flag>(conn).await?.value;
            if recent{return Err(Error::TooSoon)}
            if let Some(parent)=parent {let p=locked(conn,site,parent).await?;if p.parent_id.is_some()||p.deleted{return Err(Error::Missing)}}
            let id=Uuid::new_v4();
            diesel::sql_query("INSERT INTO community_comments(id,user_id,server_id,parent_id,body,is_deleted,inserted_at,updated_at) VALUES($1,$2,$3,$4,$5,false,statement_timestamp() AT TIME ZONE 'UTC',statement_timestamp() AT TIME ZONE 'UTC')")
                .bind::<SqlUuid,_>(id).bind::<SqlUuid,_>(s.member.id).bind::<SqlUuid,_>(site).bind::<Nullable<SqlUuid>,_>(parent).bind::<Text,_>(body).execute(conn).await?;
            Ok::<_,Error>((comment(conn,site,id).await?,site))
        }).await?;
        domain::changes::notify(id);
        Ok(result)
    }
    pub async fn change_comment(
        &self,
        s: &AuthenticatedSession,
        domain: &str,
        id: Uuid,
        revision: NaiveDateTime,
        body: Option<String>,
    ) -> Result<Comment, Error> {
        let domain = domain::domain(domain)?;
        let body = body.map(|v| domain::text(v, 2000, true)).transpose()?;
        let mut conn = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        let (result,site)=(&mut *conn).transaction(async move|conn|{
            authorize_action(conn,s,false).await?;let site=site(conn,&domain,true).await?;
            let before=locked(conn,site,id).await?;
            if before.user_id!=Some(s.member.id) || before.deleted{return Err(Error::Missing)}
            if before.updated_at!=revision {return Err(Error::Conflict)}
            if let Some(body)=body {
                if !before.editable{return Err(Error::TooLate)}
                if body!=before.body {diesel::sql_query("UPDATE community_comments SET body=$2,updated_at=greatest(clock_timestamp() AT TIME ZONE 'UTC',updated_at+interval '1 microsecond') WHERE id=$1").bind::<SqlUuid,_>(id).bind::<Text,_>(body).execute(conn).await?;}
            }else{diesel::sql_query("UPDATE community_comments SET is_deleted=true,updated_at=greatest(clock_timestamp() AT TIME ZONE 'UTC',updated_at+interval '1 microsecond') WHERE id=$1").bind::<SqlUuid,_>(id).execute(conn).await?;}
            Ok::<_,Error>((comment(conn,site,id).await?,site))
        }).await?;
        domain::changes::notify(site);
        Ok(result)
    }
    pub async fn report_comment(
        &self,
        s: &AuthenticatedSession,
        domain: &str,
        id: Uuid,
        reason: Reason,
        detail: String,
    ) -> Result<(), Error> {
        let domain = domain::domain(domain)?;
        let detail = domain::text(detail, 500, false)?;
        let mut conn = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        (&mut *conn).transaction(async move|conn|{
            authorize_action(conn,s,false).await?;let site=site(conn,&domain,true).await?;let c=locked(conn,site,id).await?;
            if c.deleted || c.user_id==Some(s.member.id){return Err(Error::Missing)}
            let duplicate=diesel::sql_query("SELECT EXISTS(SELECT 1 FROM community_reports WHERE reporter_id=$1 AND comment_id=$2) AS value").bind::<SqlUuid,_>(s.member.id).bind::<SqlUuid,_>(id).get_result::<Flag>(conn).await?.value;
            if duplicate{return Err(Error::DuplicateReport)}
            let count=diesel::sql_query("SELECT count(*) AS value FROM community_reports WHERE reporter_id=$1 AND inserted_at>clock_timestamp() AT TIME ZONE 'UTC'-interval '1 minute'").bind::<SqlUuid,_>(s.member.id).get_result::<Count>(conn).await?.value;
            if count>=30{return Err(Error::ReportRate)}
            let report=Uuid::new_v4();
            diesel::sql_query("INSERT INTO community_reports(id,reporter_id,comment_id,reason,detail,status,inserted_at,updated_at) VALUES($1,$2,$3,$4,nullif($5,''),'pending',now() AT TIME ZONE 'UTC',now() AT TIME ZONE 'UTC')")
                .bind::<SqlUuid,_>(report).bind::<SqlUuid,_>(s.member.id).bind::<SqlUuid,_>(id).bind::<Text,_>(reason.key()).bind::<Text,_>(detail).execute(conn).await?;
            diesel::sql_query("INSERT INTO community_report_evidence(report_id,body,author_name,comment_updated_at) VALUES($1,$2,$3,$4)").bind::<SqlUuid,_>(report).bind::<Text,_>(c.body).bind::<Text,_>(c.author_name).bind::<Timestamp,_>(c.updated_at).execute(conn).await?;
            Ok(())
        }).await
    }
}
#[cfg(test)]
mod tests;
