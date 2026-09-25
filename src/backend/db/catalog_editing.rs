//! Member -> session -> catalog row -> revision locks. No remote IO in a write.
use super::{members::authorize_action, Database};
use crate::{
    backend::{
        auth::AuthenticatedSession,
        catalog_editing::{self as domain, Error, ValidatedEdit},
    },
    directory::catalog_editing::{EditableSoftware, History, Revision, SoftwareEdit},
};
use diesel::{
    prelude::*,
    sql_types::{Array, BigInt, Bool, Text, Uuid as SqlUuid},
};
use diesel_async::{AsyncConnection, AsyncPgConnection, RunQueryDsl};
mod moderation;
impl From<diesel::result::Error> for Error {
    fn from(_: diesel::result::Error) -> Self {
        Self::Unavailable
    }
}

const EDIT: &str="jsonb_build_object('display_name',s.display_name,'family',coalesce(s.family,''),'description',coalesce(s.description,''),'tech_stack',coalesce(s.tech_stack,''),'website_url',coalesce(s.website_url,''),'categories',ARRAY(SELECT v FROM unnest(CASE WHEN cardinality(s.categories)>0 THEN s.categories ELSE ARRAY[s.category_tag] END) v WHERE v IS NOT NULL),'features',ARRAY(SELECT v FROM unnest(s.features) v WHERE v IS NOT NULL))::text";
#[derive(QueryableByName)]
struct Row {
    #[diesel(sql_type=BigInt)]
    id: i64,
    #[diesel(sql_type=Text)]
    name: String,
    #[diesel(sql_type=BigInt)]
    revision: i64,
    #[diesel(sql_type=Bool)]
    locked: bool,
    #[diesel(sql_type=Text)]
    edit: String,
    #[diesel(sql_type=Text)]
    snapshot: String,
}
impl Row {
    fn dto(&self) -> Result<EditableSoftware, Error> {
        Ok(EditableSoftware {
            name: self.name.clone(),
            revision: self.revision,
            locked: self.locked,
            edit: serde_json::from_str(&self.edit).map_err(|_| Error::Unavailable)?,
        })
    }
}
#[derive(QueryableByName)]
struct Count {
    #[diesel(sql_type=BigInt)]
    value: i64,
}
#[derive(QueryableByName)]
struct Flag {
    #[diesel(sql_type=Bool)]
    value: bool,
}
async fn row(conn: &mut AsyncPgConnection, name: &str, lock: bool) -> Result<Row, Error> {
    // Lock the editorial row first; state never locks in the opposite direction.
    if lock {
        diesel::sql_query("SELECT id FROM catalog_software WHERE name=$1 FOR UPDATE")
            .bind::<Text, _>(name)
            .execute(conn)
            .await?;
    }
    diesel::sql_query(format!("SELECT s.id,s.name,coalesce(t.revision,0) AS revision,coalesce(t.locked,false) AS locked,{EDIT} AS edit,to_jsonb(s)::text AS snapshot FROM catalog_software s LEFT JOIN catalog_software_state t ON t.software_id=s.id WHERE s.name=$1 AND octet_length(to_jsonb(s)::text)<=1048576"))
        .bind::<Text,_>(name).get_result(conn).await.optional()?.ok_or(Error::Missing)
}
async fn writable(conn: &mut AsyncPgConnection, s: &AuthenticatedSession) -> Result<(), Error> {
    authorize_action(conn, s, false).await?;
    let n=diesel::sql_query("SELECT count(*) AS value FROM catalog_software_edits WHERE actor_id=$1 AND created_at>now()-interval '1 minute'")
        .bind::<SqlUuid,_>(s.member.id).get_result::<Count>(conn).await?.value;
    if n >= 30 {
        return Err(Error::RateLimited);
    }
    Ok(())
}
async fn categories(
    conn: &mut AsyncPgConnection,
    edit: &SoftwareEdit,
    previous: Option<&SoftwareEdit>,
) -> Result<(), Error> {
    let need: Vec<_> = edit
        .categories
        .iter()
        .filter(|c| !previous.is_some_and(|p| p.categories.contains(c)))
        .cloned()
        .collect();
    let n =
        diesel::sql_query("SELECT count(*) AS value FROM catalog_categories WHERE name=ANY($1)")
            .bind::<Array<Text>, _>(&need)
            .get_result::<Count>(conn)
            .await?
            .value;
    if n != need.len() as i64 {
        return Err(Error::Invalid);
    }
    Ok(())
}
async fn save_fields(conn: &mut AsyncPgConnection, id: i64, e: &SoftwareEdit) -> Result<(), Error> {
    // Never accept logo storage keys, display order, or featured status from members.
    diesel::sql_query("UPDATE catalog_software SET display_name=$2,family=nullif($3,''),description=nullif($4,''),categories=$5,category_tag=($5::text[])[1],features=$6,website_url=nullif($7,''),tech_stack=nullif($8,''),updated_at=now() AT TIME ZONE 'UTC' WHERE id=$1")
        .bind::<BigInt,_>(id).bind::<Text,_>(&e.display_name).bind::<Text,_>(&e.family).bind::<Text,_>(&e.description).bind::<Array<Text>,_>(&e.categories).bind::<Array<Text>,_>(&e.features).bind::<Text,_>(&e.website_url).bind::<Text,_>(&e.tech_stack).execute(conn).await?;
    Ok(())
}
async fn append(
    conn: &mut AsyncPgConnection,
    row: &Row,
    s: &AuthenticatedSession,
    action: &str,
    summary: &str,
) -> Result<(), Error> {
    diesel::sql_query("INSERT INTO catalog_software_edits(software_id,revision,actor_id,action,summary,snapshot) VALUES($1,$2,$3,$4,$5,$6::jsonb)")
        .bind::<BigInt,_>(row.id).bind::<BigInt,_>(row.revision).bind::<SqlUuid,_>(s.member.id).bind::<Text,_>(action).bind::<Text,_>(summary).bind::<Text,_>(&row.snapshot).execute(conn).await?;
    Ok(())
}
async fn edit_locked(
    conn: &mut AsyncPgConnection,
    s: &AuthenticatedSession,
    name: &str,
    revision: i64,
    value: &ValidatedEdit,
    action: &str,
    administrator: bool,
) -> Result<EditableSoftware, Error> {
    let before = row(conn, name, true).await?;
    diesel::sql_query(
        "INSERT INTO catalog_software_state(software_id) VALUES($1) ON CONFLICT DO NOTHING",
    )
    .bind::<BigInt, _>(before.id)
    .execute(conn)
    .await?;
    // Recheck state under its own lock, including operator intervention during IO.
    let state = diesel::sql_query(
        "SELECT locked AS value FROM catalog_software_state WHERE software_id=$1 FOR UPDATE",
    )
    .bind::<BigInt, _>(before.id)
    .get_result::<Flag>(conn)
    .await?;
    if state.value && !administrator {
        return Err(Error::Locked);
    }
    if before.revision != revision {
        return Err(Error::Conflict);
    }
    let old = before.dto()?;
    categories(conn, &value.edit, Some(&old.edit)).await?;
    if old.edit == value.edit {
        return Ok(old);
    }
    if before.revision == 0 {
        diesel::sql_query("INSERT INTO catalog_software_edits(software_id,revision,action,summary,snapshot) VALUES($1,0,'baseline','기존 카탈로그',$2::jsonb)")
            .bind::<BigInt,_>(before.id).bind::<Text,_>(&before.snapshot).execute(conn).await?;
    }
    save_fields(conn, before.id, &value.edit).await?;
    diesel::sql_query("UPDATE catalog_software_state SET revision=revision+1 WHERE software_id=$1")
        .bind::<BigInt, _>(before.id)
        .execute(conn)
        .await?;
    let after = row(conn, name, false).await?;
    append(conn, &after, s, action, &value.summary).await?;
    after.dto()
}
impl Database {
    pub async fn catalog_kinds(&self) -> Result<crate::directory::catalog_editing::Kinds, Error> {
        #[derive(QueryableByName)]
        struct Kind {
            #[diesel(sql_type=Text)]
            name: String,
            #[diesel(sql_type=Text)]
            label: String,
            #[diesel(sql_type=Text)]
            emoji: String,
        }
        let mut conn = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        let mut rows=diesel::sql_query("SELECT name,left(label,200) AS label,left(coalesce(emoji,''),32) AS emoji FROM catalog_categories ORDER BY display_order,name LIMIT 201").load::<Kind>(&mut conn).await?;
        let truncated = rows.len() > 200;
        rows.truncate(200);
        Ok(crate::directory::catalog_editing::Kinds {
            truncated,
            items: rows
                .into_iter()
                .map(|r| crate::directory::Category {
                    name: r.name,
                    label: r.label,
                    emoji: r.emoji,
                })
                .collect(),
        })
    }
    pub async fn editable_software(&self, name: &str) -> Result<EditableSoftware, Error> {
        domain::existing_name(name)?;
        let mut conn = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        row(&mut conn, name, false).await?.dto()
    }
    pub async fn create_software(
        &self,
        s: &AuthenticatedSession,
        name: &str,
        value: &ValidatedEdit,
    ) -> Result<EditableSoftware, Error> {
        let name = domain::new_name(name)?;
        let mut conn = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        (&mut *conn).transaction(async move|conn|{
            writable(conn,s).await?;
            categories(conn,&value.edit,None).await?;
            // The imported bigint IDs have no sequence. Serialize only allocation,
            // preserving all original IDs and the offline importer schema contract.
            diesel::sql_query("SELECT pg_advisory_xact_lock(6810476213309)").execute(conn).await?;
            let exists=diesel::sql_query("SELECT EXISTS(SELECT 1 FROM catalog_software WHERE lower(name)=lower($1)) AS value").bind::<Text,_>(&name).get_result::<Flag>(conn).await?.value;
            if exists {return Err(Error::Duplicate)}
            let id=diesel::sql_query("INSERT INTO catalog_software(id,name,display_name,categories,features,is_featured,display_order,inserted_at,updated_at) SELECT greatest(coalesce(max(id),0),0)+1,$1,$2,ARRAY[]::text[],ARRAY[]::text[],false,0,now() AT TIME ZONE 'UTC',now() AT TIME ZONE 'UTC' FROM catalog_software RETURNING id AS value")
                .bind::<Text,_>(&name).bind::<Text,_>(&value.edit.display_name).get_result::<Count>(conn).await?.value;
            save_fields(conn,id,&value.edit).await?;
            diesel::sql_query("INSERT INTO catalog_software_state(software_id,revision) VALUES($1,1)").bind::<BigInt,_>(id).execute(conn).await?;
            let after=row(conn,&name,false).await?;append(conn,&after,s,"create",&value.summary).await?;after.dto()
        }).await
    }
    pub async fn edit_software(
        &self,
        s: &AuthenticatedSession,
        name: &str,
        revision: i64,
        value: &ValidatedEdit,
    ) -> Result<EditableSoftware, Error> {
        domain::existing_name(name)?;
        if revision < 0 {
            return Err(Error::Invalid);
        }
        let mut conn = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        (&mut *conn)
            .transaction(async move |conn| {
                writable(conn, s).await?;
                edit_locked(conn, s, name, revision, value, "edit", false).await
            })
            .await
    }
    pub async fn software_revision(
        &self,
        name: &str,
        revision: i64,
    ) -> Result<SoftwareEdit, Error> {
        domain::existing_name(name)?;
        if revision < 0 {
            return Err(Error::Invalid);
        }
        let mut conn = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        revision_edit(&mut conn, name, revision).await
    }
    pub async fn restore_software(
        &self,
        s: &AuthenticatedSession,
        name: &str,
        current: i64,
        restore: i64,
        summary: String,
    ) -> Result<EditableSoftware, Error> {
        domain::existing_name(name)?;
        if current < 0 || restore < 0 || current == restore {
            return Err(Error::Invalid);
        }
        let mut conn = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        (&mut *conn)
            .transaction(async move |conn| {
                writable(conn, s).await?;
                let old = revision_edit(conn, name, restore).await?;
                let value = domain::validate(old, summary)?;
                edit_locked(conn, s, name, current, &value, "restore", false).await
            })
            .await
    }
    pub async fn software_history(&self, name: &str, page: u32) -> Result<History, Error> {
        domain::existing_name(name)?;
        if page > 10_000 {
            return Err(Error::Invalid);
        }
        let mut conn = self.pool.get().await.map_err(|_| Error::Unavailable)?;
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
        (&mut *conn).transaction(async move|conn|{
            diesel::sql_query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY").execute(conn).await?;
            let current=row(conn,name,false).await?;
            let mut entries=diesel::sql_query("SELECT revision,action,summary,to_char(created_at AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at FROM catalog_software_edits WHERE software_id=$1 ORDER BY revision DESC LIMIT 21 OFFSET $2")
                .bind::<BigInt,_>(current.id).bind::<BigInt,_>(i64::from(page)*20).load::<Item>(conn).await?;
            let has_next=entries.len()>20;entries.truncate(20);
            Ok(History{name:name.into(),current_revision:current.revision,locked:current.locked,page,has_next,entries:entries.into_iter().map(|r|Revision{revision:r.revision,action:r.action,summary:r.summary,created_at:r.created_at}).collect()})
        }).await
    }
}
async fn revision_edit(
    conn: &mut AsyncPgConnection,
    name: &str,
    revision: i64,
) -> Result<SoftwareEdit, Error> {
    #[derive(QueryableByName)]
    struct Value {
        #[diesel(sql_type=Text)]
        edit: String,
    }
    let value=diesel::sql_query(format!("SELECT {EDIT} AS edit FROM catalog_software_edits e JOIN catalog_software live ON live.id=e.software_id CROSS JOIN LATERAL jsonb_populate_record(NULL::catalog_software,e.snapshot) s WHERE live.name=$1 AND e.revision=$2"))
        .bind::<Text,_>(name).bind::<BigInt,_>(revision).get_result::<Value>(conn).await.optional()?.ok_or(Error::Missing)?;
    serde_json::from_str(&value.edit).map_err(|_| Error::Unavailable)
}
#[cfg(test)]
mod tests;
