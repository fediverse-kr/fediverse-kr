//! Gate -> member/session -> software/category -> revision. No remote IO.
use super::*;
use crate::{backend::catalog_moderation as policy, moderation::catalog as dto};
use diesel::sql_types::{Integer, Nullable};
use serde_json::{json, Value};
mod logo;

impl From<crate::backend::moderation::Error> for Error {
    fn from(e: crate::backend::moderation::Error) -> Self {
        use crate::backend::moderation::Error as M;
        match e {
            M::Auth(e) => Self::Auth(e),
            M::Forbidden => Self::Forbidden,
            _ => Self::Unavailable,
        }
    }
}
async fn admin(
    conn: &mut AsyncPgConnection,
    s: &AuthenticatedSession,
    fresh: bool,
) -> Result<(), Error> {
    super::super::moderation::gate(conn).await?;
    super::super::moderation::authorize(conn, s, fresh).await?;
    Ok(())
}
#[derive(QueryableByName)]
struct SoftwareRow {
    #[diesel(sql_type=Text)]
    name: String,
    #[diesel(sql_type=Text)]
    display_name: String,
    #[diesel(sql_type=BigInt)]
    revision: i64,
    #[diesel(sql_type=Bool)]
    locked: bool,
    #[diesel(sql_type=Nullable<Text>)]
    brand_color: Option<String>,
    #[diesel(sql_type=Bool)]
    color_truncated: bool,
    #[diesel(sql_type=Bool)]
    featured: bool,
    #[diesel(sql_type=Integer)]
    display_order: i32,
    #[diesel(sql_type=Bool)]
    logo_available: bool,
}
const SELECT:&str="SELECT s.name,left(s.display_name,200) AS display_name,coalesce(t.revision,0) AS revision,coalesce(t.locked,false) AS locked,left(s.brand_color,128) AS brand_color,coalesce(char_length(s.brand_color)>128,false) AS color_truncated,s.is_featured AS featured,s.display_order,s.logo_key IS NOT NULL AS logo_available FROM catalog_software s LEFT JOIN catalog_software_state t ON t.software_id=s.id";
impl From<SoftwareRow> for dto::Software {
    fn from(r: SoftwareRow) -> Self {
        Self {
            name: r.name,
            display_name: r.display_name,
            revision: r.revision,
            locked: r.locked,
            brand_color: r.brand_color,
            color_truncated: r.color_truncated,
            featured: r.featured,
            display_order: r.display_order,
            logo_available: r.logo_available,
        }
    }
}
async fn software(conn: &mut AsyncPgConnection, name: &str) -> Result<dto::Software, Error> {
    Ok(diesel::sql_query(format!("{SELECT} WHERE s.name=$1"))
        .bind::<Text, _>(name)
        .get_result::<SoftwareRow>(conn)
        .await
        .optional()?
        .ok_or(Error::Missing)?
        .into())
}
async fn audit(
    conn: &mut AsyncPgConnection,
    s: &AuthenticatedSession,
    ids: (Option<i64>, Option<i64>),
    revision: i64,
    action: &str,
    note: &str,
    before: &Value,
    after: &Value,
) -> Result<(), Error> {
    diesel::sql_query("INSERT INTO catalog_admin_events(software_id,category_id,revision,actor_id,action,note,before_value,after_value) VALUES($1,$2,$3,$4,$5,$6,$7::jsonb,$8::jsonb)")
        .bind::<Nullable<BigInt>,_>(ids.0).bind::<Nullable<BigInt>,_>(ids.1).bind::<BigInt,_>(revision).bind::<SqlUuid,_>(s.member.id).bind::<Text,_>(action).bind::<Text,_>(note).bind::<Text,_>(before.to_string()).bind::<Text,_>(after.to_string()).execute(conn).await?;
    Ok(())
}
#[derive(QueryableByName)]
struct CategoryRow {
    #[diesel(sql_type=BigInt)]
    id: i64,
    #[diesel(sql_type=Text)]
    name: String,
    #[diesel(sql_type=BigInt)]
    revision: i64,
    #[diesel(sql_type=Text)]
    snapshot: String,
}
const CATEGORY:&str="SELECT c.id,c.name,coalesce(t.revision,0) AS revision,to_jsonb(c)::text AS snapshot FROM catalog_categories c LEFT JOIN catalog_category_state t ON t.category_id=c.id";
impl CategoryRow {
    fn dto(&self) -> Result<dto::Category, Error> {
        let v: Value = serde_json::from_str(&self.snapshot).map_err(|_| Error::Unavailable)?;
        Ok(dto::Category {
            name: self.name.clone(),
            revision: self.revision,
            edit: dto::CategoryEdit {
                label: v["label"].as_str().ok_or(Error::Unavailable)?.into(),
                emoji: v["emoji"].as_str().unwrap_or("").into(),
                display_order: v["display_order"]
                    .as_i64()
                    .ok_or(Error::Unavailable)?
                    .try_into()
                    .map_err(|_| Error::Unavailable)?,
            },
        })
    }
}
async fn category(conn: &mut AsyncPgConnection, name: &str) -> Result<CategoryRow, Error> {
    diesel::sql_query(format!(
        "{CATEGORY} WHERE c.name=$1 AND octet_length(to_jsonb(c)::text)<=1048576"
    ))
    .bind::<Text, _>(name)
    .get_result(conn)
    .await
    .optional()?
    .ok_or(Error::Missing)
}
impl Database {
    pub async fn moderation_software_list(
        &self,
        s: &AuthenticatedSession,
        query: &str,
        locked: bool,
        page: u32,
    ) -> Result<dto::SoftwarePage, Error> {
        policy::query(query, page)?;
        let mut conn = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        (&mut *conn).transaction(async move|conn|{
            admin(conn,s,false).await?;
            let mut items=diesel::sql_query(format!("{SELECT} WHERE (NOT $2 OR coalesce(t.locked,false)) AND ($1='' OR strpos(lower(s.name),lower($1))>0 OR strpos(lower(s.display_name),lower($1))>0) ORDER BY s.display_order,lower(s.display_name),s.name LIMIT 25 OFFSET $3"))
                .bind::<Text,_>(query).bind::<Bool,_>(locked).bind::<BigInt,_>(i64::from(page)*24).load::<SoftwareRow>(conn).await?;
            let has_next=items.len()>24;items.truncate(24);Ok(dto::SoftwarePage{items:items.into_iter().map(Into::into).collect(),page,has_next})
        }).await
    }
    pub async fn moderation_software(
        &self,
        s: &AuthenticatedSession,
        name: &str,
    ) -> Result<dto::Software, Error> {
        domain::existing_name(name)?;
        let mut conn = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        (&mut *conn)
            .transaction(async move |conn| {
                admin(conn, s, false).await?;
                software(conn, name).await
            })
            .await
    }
    pub async fn moderation_software_editable(
        &self,
        s: &AuthenticatedSession,
        name: &str,
    ) -> Result<EditableSoftware, Error> {
        domain::existing_name(name)?;
        let mut conn = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        (&mut *conn)
            .transaction(async move |conn| {
                admin(conn, s, false).await?;
                row(conn, name, false).await?.dto()
            })
            .await
    }
    pub async fn moderation_software_save(
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
                admin(conn, s, true).await?;
                writable(conn, s).await?;
                edit_locked(conn, s, name, revision, value, "admin_edit", true).await
            })
            .await
    }
    pub async fn moderate_software(
        &self,
        s: &AuthenticatedSession,
        request: dto::Request,
    ) -> Result<dto::Software, Error> {
        let r = policy::software(request)?;
        let mut conn = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        (&mut *conn).transaction(async move|conn|{
            admin(conn,s,true).await?;writable(conn,s).await?;
            let before=row(conn,&r.name,true).await?;
            if before.revision!=r.revision{return Err(Error::Conflict)}
            let snap:Value=serde_json::from_str(&before.snapshot).map_err(|_|Error::Unavailable)?;
            let (action,old,new)=match &r.action {
                dto::Action::Locked(v)=>("locked",json!(before.locked),json!(v)),
                dto::Action::BrandColor(v)=>("brand_color",snap["brand_color"].clone(),json!(v)),
                dto::Action::Featured(v)=>("featured",snap["is_featured"].clone(),json!(v)),
                dto::Action::DisplayOrder(v)=>("display_order",snap["display_order"].clone(),json!(v)),
            };
            if old==new{return software(conn,&r.name).await}
            if before.revision==0 {
                diesel::sql_query("INSERT INTO catalog_software_edits(software_id,revision,action,summary,snapshot) VALUES($1,0,'baseline','기존 카탈로그',$2::jsonb)").bind::<BigInt,_>(before.id).bind::<Text,_>(&before.snapshot).execute(conn).await?;
            }
            diesel::sql_query("INSERT INTO catalog_software_state(software_id) VALUES($1) ON CONFLICT DO NOTHING").bind::<BigInt,_>(before.id).execute(conn).await?;
            match &r.action {
                dto::Action::Locked(v)=>{diesel::sql_query("UPDATE catalog_software_state SET locked=$2 WHERE software_id=$1").bind::<BigInt,_>(before.id).bind::<Bool,_>(v).execute(conn).await?;},
                dto::Action::BrandColor(v)=>{diesel::sql_query("UPDATE catalog_software SET brand_color=$2 WHERE id=$1").bind::<BigInt,_>(before.id).bind::<Nullable<Text>,_>(v).execute(conn).await?;},
                dto::Action::Featured(v)=>{diesel::sql_query("UPDATE catalog_software SET is_featured=$2 WHERE id=$1").bind::<BigInt,_>(before.id).bind::<Bool,_>(v).execute(conn).await?;},
                dto::Action::DisplayOrder(v)=>{diesel::sql_query("UPDATE catalog_software SET display_order=$2 WHERE id=$1").bind::<BigInt,_>(before.id).bind::<Integer,_>(v).execute(conn).await?;},
            }
            diesel::sql_query("UPDATE catalog_software_state SET revision=revision+1 WHERE software_id=$1").bind::<BigInt,_>(before.id).execute(conn).await?;
            diesel::sql_query("UPDATE catalog_software SET updated_at=now() AT TIME ZONE 'UTC' WHERE id=$1").bind::<BigInt,_>(before.id).execute(conn).await?;
            let after=row(conn,&r.name,false).await?;
            append(conn,&after,s,"admin_settings",r.action.label()).await?;
            audit(conn,s,(Some(before.id),None),after.revision,action,&r.note,&old,&new).await?;
            software(conn,&r.name).await
        }).await
    }
    pub async fn moderation_categories(
        &self,
        s: &AuthenticatedSession,
        query: &str,
        page: u32,
    ) -> Result<dto::CategoryPage, Error> {
        policy::query(query, page)?;
        let mut conn = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        (&mut *conn).transaction(async move|conn|{
            admin(conn,s,false).await?;
            // List fields are bounded; editing reads the complete row separately.
            let mut rows=diesel::sql_query("SELECT c.id,c.name,coalesce(t.revision,0) AS revision,jsonb_build_object('label',left(c.label,200),'emoji',left(coalesce(c.emoji,''),32),'display_order',c.display_order)::text AS snapshot FROM catalog_categories c LEFT JOIN catalog_category_state t ON t.category_id=c.id WHERE $1='' OR strpos(lower(c.name),lower($1))>0 OR strpos(lower(c.label),lower($1))>0 ORDER BY c.display_order,c.name LIMIT 25 OFFSET $2")
                .bind::<Text,_>(query).bind::<BigInt,_>(i64::from(page)*24).load::<CategoryRow>(conn).await?;
            let has_next=rows.len()>24;rows.truncate(24);Ok(dto::CategoryPage{items:rows.into_iter().map(|v|v.dto()).collect::<Result<_,_>>()?,page,has_next})
        }).await
    }
    pub async fn moderation_category(
        &self,
        s: &AuthenticatedSession,
        name: &str,
    ) -> Result<dto::Category, Error> {
        domain::existing_name(name)?;
        let mut conn = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        (&mut *conn)
            .transaction(async move |conn| {
                admin(conn, s, false).await?;
                category(conn, name).await?.dto()
            })
            .await
    }
    pub async fn moderate_category(
        &self,
        s: &AuthenticatedSession,
        request: dto::CategoryRequest,
    ) -> Result<dto::Category, Error> {
        let r = policy::category(request)?;
        let mut conn = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        (&mut *conn).transaction(async move|conn|{
            admin(conn,s,true).await?;
            let count=diesel::sql_query("SELECT count(*) AS value FROM catalog_admin_events WHERE actor_id=$1 AND created_at>now()-interval '1 minute'").bind::<SqlUuid,_>(s.member.id).get_result::<Count>(conn).await?.value;
            if count>=30{return Err(Error::RateLimited)}
            let (id,revision,before,action)=if let Some(revision)=r.revision {
                diesel::sql_query("SELECT id FROM catalog_categories WHERE name=$1 FOR UPDATE").bind::<Text,_>(&r.name).execute(conn).await?;
                let before=category(conn,&r.name).await?;
                if before.revision!=revision{return Err(Error::Conflict)}
                if before.dto()?.edit==r.edit{return before.dto()}
                (before.id,revision.checked_add(1).ok_or(Error::Invalid)?,serde_json::from_str(&before.snapshot).map_err(|_|Error::Unavailable)?,"category_edit")
            }else{
                // Imported IDs have no sequence; never rename an existing kind.
                diesel::sql_query("SELECT pg_advisory_xact_lock(6810476213316)").execute(conn).await?;
                if diesel::sql_query("SELECT EXISTS(SELECT 1 FROM catalog_categories WHERE lower(name)=lower($1)) AS value").bind::<Text,_>(&r.name).get_result::<Flag>(conn).await?.value{return Err(Error::Duplicate)}
                let id=diesel::sql_query("INSERT INTO catalog_categories(id,name,label,display_order,inserted_at,updated_at) SELECT greatest(coalesce(max(id),0),0)+1,$1,$2,0,now() AT TIME ZONE 'UTC',now() AT TIME ZONE 'UTC' FROM catalog_categories RETURNING id AS value").bind::<Text,_>(&r.name).bind::<Text,_>(&r.edit.label).get_result::<Count>(conn).await?.value;
                (id,1,Value::Null,"category_create")
            };
            diesel::sql_query("UPDATE catalog_categories SET label=$2,emoji=nullif($3,''),display_order=$4,updated_at=now() AT TIME ZONE 'UTC' WHERE id=$1").bind::<BigInt,_>(id).bind::<Text,_>(&r.edit.label).bind::<Text,_>(&r.edit.emoji).bind::<Integer,_>(r.edit.display_order).execute(conn).await?;
            diesel::sql_query("INSERT INTO catalog_category_state(category_id,revision) VALUES($1,$2) ON CONFLICT(category_id) DO UPDATE SET revision=EXCLUDED.revision").bind::<BigInt,_>(id).bind::<BigInt,_>(revision).execute(conn).await?;
            let after=category(conn,&r.name).await?;
            audit(conn,s,(None,Some(id)),revision,action,&r.note,&before,&serde_json::from_str(&after.snapshot).map_err(|_|Error::Unavailable)?).await?;
            after.dto()
        }).await
    }
    pub async fn moderation_catalog_history(
        &self,
        s: &AuthenticatedSession,
        name: &str,
        is_category: bool,
        page: u32,
    ) -> Result<dto::History, Error> {
        domain::existing_name(name)?;
        policy::query("", page)?;
        let mut conn = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        #[derive(QueryableByName)]
        struct Entry {
            #[diesel(sql_type=BigInt)]
            revision: i64,
            #[diesel(sql_type=Text)]
            action: String,
            #[diesel(sql_type=Text)]
            note: String,
            #[diesel(sql_type=Text)]
            before: String,
            #[diesel(sql_type=Text)]
            after: String,
            #[diesel(sql_type=Bool)]
            truncated: bool,
            #[diesel(sql_type=Text)]
            created_at: String,
        }
        (&mut *conn).transaction(async move|conn|{
            admin(conn,s,false).await?;
            let (column,id)=if is_category{("category_id",category(conn,name).await?.id)}else{("software_id",row(conn,name,false).await?.id)};
            let mut items=diesel::sql_query(format!("SELECT revision,action,note,left(before_value::text,600) AS before,left(after_value::text,600) AS after,char_length(before_value::text)>600 OR char_length(after_value::text)>600 AS truncated,to_char(created_at AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at FROM catalog_admin_events WHERE {column}=$1 ORDER BY revision DESC LIMIT 21 OFFSET $2"))
                .bind::<BigInt,_>(id).bind::<BigInt,_>(i64::from(page)*20).load::<Entry>(conn).await?;
            let has_next=items.len()>20;items.truncate(20);Ok(dto::History{items:items.into_iter().map(|e|dto::Event{revision:e.revision,action:e.action,note:e.note,before:e.before,after:e.after,truncated:e.truncated,created_at:e.created_at}).collect(),page,has_next})
        }).await
    }
}
#[cfg(test)]
mod tests;
