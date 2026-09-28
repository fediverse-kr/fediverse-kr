use super::{members::authorize_action, schema::hosting_services as services, Database};
use crate::{
    backend::{
        auth::AuthenticatedSession,
        hosting::{self as domain, Error},
    },
    hosting::{HostingEdit, HostingHistory, HostingPage, HostingRevision, HostingService},
};
use diesel::{
    prelude::*,
    sql_types::{BigInt, Bool, Text, Uuid as SqlUuid},
};
use diesel_async::{AsyncConnection, AsyncPgConnection, RunQueryDsl};

#[cfg(test)]
mod tests;
impl From<diesel::result::Error> for Error {
    fn from(_: diesel::result::Error) -> Self {
        Self::Unavailable
    }
}

#[derive(QueryableByName)]
struct Row {
    #[diesel(sql_type=Text)]
    slug: String,
    #[diesel(sql_type=BigInt)]
    revision: i64,
    #[diesel(sql_type=Text)]
    snapshot: String,
}
impl Row {
    fn dto(self) -> Result<HostingService, Error> {
        Ok(HostingService {
            slug: self.slug,
            revision: self.revision,
            edit: serde_json::from_str(&self.snapshot).map_err(|_| Error::Unavailable)?,
        })
    }
}
const PROJECTION: &str = "SELECT slug,revision,jsonb_set((to_jsonb(h)-'slug'-'revision'-'updated_at'),'{checked_on}',to_jsonb(coalesce(h.checked_on::text,'')))::text AS snapshot FROM hosting_services h";
async fn load(conn: &mut AsyncPgConnection, slug: &str) -> Result<HostingService, Error> {
    diesel::sql_query(format!("{PROJECTION} WHERE slug=$1"))
        .bind::<Text, _>(slug)
        .get_result::<Row>(conn)
        .await
        .optional()?
        .ok_or(Error::Missing)?
        .dto()
}
async fn writable(
    conn: &mut AsyncPgConnection,
    session: &AuthenticatedSession,
) -> Result<(), Error> {
    authorize_action(conn, session, true).await?;
    #[derive(QueryableByName)]
    struct Count {
        #[diesel(sql_type=BigInt)]
        value: i64,
    }
    let n = diesel::sql_query("SELECT count(*) AS value FROM hosting_service_edits WHERE actor_id=$1 AND created_at>now()-interval '1 minute'")
        .bind::<SqlUuid,_>(session.member.id).get_result::<Count>(conn).await?.value;
    if n >= 30 {
        return Err(Error::RateLimited);
    }
    Ok(())
}
async fn record(
    conn: &mut AsyncPgConnection,
    session: &AuthenticatedSession,
    service: &HostingService,
    action: &str,
    summary: &str,
) -> Result<(), Error> {
    let snapshot = serde_json::to_string(&service.edit).map_err(|_| Error::Unavailable)?;
    diesel::sql_query("INSERT INTO hosting_service_edits(slug,revision,actor_id,action,summary,snapshot) VALUES($1,$2,$3,$4,$5,$6::jsonb)")
        .bind::<Text,_>(&service.slug).bind::<BigInt,_>(service.revision).bind::<SqlUuid,_>(session.member.id)
        .bind::<Text,_>(action).bind::<Text,_>(summary).bind::<Text,_>(&snapshot).execute(conn).await?;
    Ok(())
}
async fn save_locked(
    conn: &mut AsyncPgConnection,
    session: &AuthenticatedSession,
    slug: &str,
    revision: i64,
    edit: HostingEdit,
    summary: &str,
    action: &str,
) -> Result<HostingService, Error> {
    // Member/session locks precede this row lock. Serialize concurrent writers before CAS.
    let current_rev = services::table
        .find(slug)
        .for_update()
        .select(services::revision)
        .first::<i64>(conn)
        .await
        .optional()?
        .ok_or(Error::Missing)?;
    if current_rev != revision {
        return Err(Error::Conflict);
    }
    let before = load(conn, slug).await?;
    if before.edit == edit {
        return Ok(before);
    }
    diesel::sql_query("UPDATE hosting_services SET name=$2,website_url=$3,scope=$4,software=$5,provider_responsibilities=$6,customer_responsibilities=$7,source_url=$8,checked_on=NULLIF($9,'')::date,revision=revision+1,updated_at=now() WHERE slug=$1")
        .bind::<Text,_>(slug).bind::<Text,_>(&edit.name).bind::<Text,_>(&edit.website_url).bind::<Text,_>(&edit.scope)
        .bind::<Text,_>(&edit.software).bind::<Text,_>(&edit.provider_responsibilities)
        .bind::<Text,_>(&edit.customer_responsibilities).bind::<Text,_>(&edit.source_url)
        .bind::<Text,_>(&edit.checked_on).execute(conn).await?;
    let after = load(conn, slug).await?;
    record(conn, session, &after, action, summary).await?;
    Ok(after)
}
impl Database {
    pub async fn hosting_list(&self, page: u32) -> Result<HostingPage, Error> {
        if page > 10000 {
            return Err(Error::Invalid);
        }
        let mut conn = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        let mut rows = diesel::sql_query(format!(
            "{PROJECTION} ORDER BY lower(name),slug LIMIT 21 OFFSET $1"
        ))
        .bind::<BigInt, _>(i64::from(page) * 20)
        .load::<Row>(&mut conn)
        .await?;
        let has_next = rows.len() > 20;
        rows.truncate(20);
        Ok(HostingPage {
            items: rows.into_iter().map(Row::dto).collect::<Result<_, _>>()?,
            page,
            has_next,
        })
    }
    pub async fn hosting(&self, slug: &str) -> Result<HostingService, Error> {
        domain::slug(slug)?;
        let mut conn = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        load(&mut conn, slug).await
    }
    pub async fn create_hosting(
        &self,
        session: &AuthenticatedSession,
        slug: &str,
        edit: HostingEdit,
        summary: String,
    ) -> Result<HostingService, Error> {
        let slug = domain::slug(slug)?;
        let (edit, summary) = domain::validate(edit, summary)?;
        let mut conn = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        (&mut *conn).transaction(async move |conn| {
            writable(conn,session).await?;
            // Serialize registration across slug and website checks.
            diesel::sql_query("SELECT pg_advisory_xact_lock(6810476213310)").execute(conn).await?;
            #[derive(QueryableByName)] struct Flag { #[diesel(sql_type=Bool)] value: bool }
            let exists=diesel::sql_query("SELECT EXISTS(SELECT 1 FROM hosting_services WHERE slug=$1 OR lower(website_url)=lower($2)) AS value")
                .bind::<Text,_>(&slug).bind::<Text,_>(&edit.website_url).get_result::<Flag>(conn).await?.value;
            if exists { return Err(Error::Duplicate); }
            diesel::sql_query("INSERT INTO hosting_services(slug,name,website_url,scope,software,provider_responsibilities,customer_responsibilities,source_url,checked_on) VALUES($1,$2,$3,$4,$5,$6,$7,$8,NULLIF($9,'')::date)")
                .bind::<Text,_>(&slug).bind::<Text,_>(&edit.name).bind::<Text,_>(&edit.website_url)
                .bind::<Text,_>(&edit.scope).bind::<Text,_>(&edit.software).bind::<Text,_>(&edit.provider_responsibilities)
                .bind::<Text,_>(&edit.customer_responsibilities).bind::<Text,_>(&edit.source_url).bind::<Text,_>(&edit.checked_on)
                .execute(conn).await?;
            let saved=load(conn,&slug).await?;
            record(conn,session,&saved,"create",&summary).await?;
            Ok(saved)
        }).await
    }
    pub async fn save_hosting(
        &self,
        session: &AuthenticatedSession,
        slug: &str,
        revision: i64,
        edit: HostingEdit,
        summary: String,
    ) -> Result<HostingService, Error> {
        domain::slug(slug)?;
        if revision < 1 {
            return Err(Error::Invalid);
        }
        let (edit, summary) = domain::validate(edit, summary)?;
        let mut conn = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        (&mut *conn)
            .transaction(async move |conn| {
                writable(conn, session).await?;
                save_locked(conn, session, slug, revision, edit, &summary, "edit").await
            })
            .await
    }
    pub async fn hosting_revision(&self, slug: &str, revision: i64) -> Result<HostingEdit, Error> {
        domain::slug(slug)?;
        if revision < 1 {
            return Err(Error::Invalid);
        }
        #[derive(QueryableByName)]
        struct Snapshot {
            #[diesel(sql_type=Text)]
            value: String,
        }
        let mut conn = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        let row=diesel::sql_query("SELECT snapshot::text AS value FROM hosting_service_edits WHERE slug=$1 AND revision=$2")
            .bind::<Text,_>(slug).bind::<BigInt,_>(revision).get_result::<Snapshot>(&mut conn).await.optional()?.ok_or(Error::Missing)?;
        serde_json::from_str(&row.value).map_err(|_| Error::Unavailable)
    }
    pub async fn restore_hosting(
        &self,
        session: &AuthenticatedSession,
        slug: &str,
        current: i64,
        previous: i64,
        summary: String,
    ) -> Result<HostingService, Error> {
        domain::slug(slug)?;
        if current < 1 || previous < 1 || current == previous {
            return Err(Error::Invalid);
        }
        let edit = self.hosting_revision(slug, previous).await?;
        let (edit, summary) = domain::validate(edit, summary)?;
        let mut conn = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        (&mut *conn)
            .transaction(async move |conn| {
                writable(conn, session).await?;
                save_locked(conn, session, slug, current, edit, &summary, "restore").await
            })
            .await
    }
    pub async fn hosting_history(&self, slug: &str, page: u32) -> Result<HostingHistory, Error> {
        domain::slug(slug)?;
        if page > 10000 {
            return Err(Error::Invalid);
        }
        #[derive(QueryableByName)]
        struct Item {
            #[diesel(sql_type=BigInt)]
            revision: i64,
            #[diesel(sql_type=Text)]
            action: String,
            #[diesel(sql_type=Text)]
            summary: String,
            #[diesel(sql_type=Text)]
            created_at: String,
        }
        let mut conn = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        load(&mut conn, slug).await?;
        let mut rows=diesel::sql_query("SELECT revision,action,summary,to_char(created_at AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at FROM hosting_service_edits WHERE slug=$1 ORDER BY revision DESC LIMIT 21 OFFSET $2")
            .bind::<Text,_>(slug).bind::<BigInt,_>(i64::from(page)*20).load::<Item>(&mut conn).await?;
        let has_next = rows.len() > 20;
        rows.truncate(20);
        Ok(HostingHistory {
            entries: rows
                .into_iter()
                .map(|r| HostingRevision {
                    revision: r.revision,
                    action: r.action,
                    summary: r.summary,
                    created_at: r.created_at,
                })
                .collect(),
            page,
            has_next,
        })
    }
}
