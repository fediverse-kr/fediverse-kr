//! Public projection is an allowlist, not serialization of a storage row.
//! Visibility is enforced for lists, direct URLs, and aggregate counts here.
use super::{Database, StoreError};
use crate::directory::search::ServerQuery;
use crate::directory::{
    rich_text::render_description, web_link, Catalog, Category, PublicSite, SitePage, Software,
    SoftwareInfo, Statistics, PAGE_SIZE,
};
use chrono::{DateTime, Utc};
use diesel::{
    prelude::*,
    sql_types::{Array, BigInt, Bool, Integer, Nullable, Text, Timestamptz},
};
use diesel_async::RunQueryDsl;
mod catalog;
mod health;
mod icons;
mod search;

#[derive(QueryableByName)]
struct CategoryRow {
    #[diesel(sql_type=Text)]
    name: String,
    #[diesel(sql_type=Text)]
    label: String,
    #[diesel(sql_type=Text)]
    emoji: String,
}
#[derive(QueryableByName)]
struct SoftwareRow {
    #[diesel(sql_type=Bool)]
    logo_available: bool,
    #[diesel(sql_type=Text)]
    name: String,
    #[diesel(sql_type=Text)]
    display_name: String,
    #[diesel(sql_type=Text)]
    description: String,
    #[diesel(sql_type=Array<Text>)]
    categories: Vec<String>,
    #[diesel(sql_type=Array<Text>)]
    features: Vec<String>,
    #[diesel(sql_type=Nullable<Text>)]
    website_url: Option<String>,
    #[diesel(sql_type=Nullable<Text>)]
    tech_stack: Option<String>,
}
#[derive(QueryableByName)]
struct SiteRow {
    #[diesel(sql_type=Bool)]
    icon_available: bool,
    #[diesel(sql_type=Text)]
    guidance: String,
    #[diesel(sql_type=Text)]
    domain: String,
    #[diesel(sql_type=Text)]
    name: String,
    #[diesel(sql_type=Text)]
    description: String,
    #[diesel(sql_type=Nullable<Text>)]
    software: Option<String>,
    #[diesel(sql_type=Text)]
    family: String,
    #[diesel(sql_type=Nullable<Text>)]
    version: Option<String>,
    #[diesel(sql_type=Bool)]
    closed: bool,
    #[diesel(sql_type=Nullable<Bool>)]
    alive: Option<bool>,
    #[diesel(sql_type=Nullable<Bool>)]
    registration_open: Option<bool>,
    #[diesel(sql_type=Nullable<BigInt>)]
    users: Option<i64>,
    #[diesel(sql_type=Nullable<BigInt>)]
    active_users: Option<i64>,
    #[diesel(sql_type=Nullable<Timestamptz>)]
    checked_at: Option<DateTime<Utc>>,
    #[diesel(sql_type=Nullable<Timestamptz>)]
    metadata_checked_at: Option<DateTime<Utc>>,
    #[diesel(sql_type=Nullable<Integer>)]
    average_response_ms: Option<i32>,
}
impl TryFrom<SiteRow> for PublicSite {
    type Error = StoreError;
    fn try_from(s: SiteRow) -> Result<Self, StoreError> {
        let description = render_description(&s.description);
        Ok(Self {
            icon_available: s.icon_available,
            guidance: serde_json::from_str(&s.guidance).map_err(|_| StoreError)?,
            domain: s.domain,
            name: s.name,
            description: description.plain,
            description_html: description.html,
            software: s.software,
            version: s.version,
            closed: s.closed,
            alive: s.alive,
            registration_open: s.registration_open,
            users: s.users,
            active_users: s.active_users,
            checked_at: s.checked_at.map(|t| t.to_rfc3339()),
            metadata_checked_at: s.metadata_checked_at.map(|t| t.to_rfc3339()),
            average_response_ms: s.average_response_ms,
        })
    }
}

const SITE_SELECT: &str = "SELECT s.domain,
    (EXISTS(SELECT 1 FROM directory_icons i WHERE i.site_id=s.id) OR EXISTS(SELECT 1 FROM legacy_sites l JOIN stored_files f ON f.object_key=l.favicon_key WHERE l.id=s.id AND l.favicon_key LIKE 'favicons/%')) AS icon_available,
    jsonb_build_object('rules',left(coalesce(d.rules,''),8000),'language',left(coalesce(d.language,''),128),'tags',ARRAY(SELECT left(v,128) FROM unnest(d.tags) v WHERE v IS NOT NULL LIMIT 32),'owner_comment',left(coalesce(d.owner_comment,''),2000),'invite_only',d.invite_only,'approval_required',d.approval_required)::text AS guidance,
    left(coalesce(nullif(s.name,''),nullif(o.observed_name,''),s.domain),200) AS name,
    left(coalesce(nullif(s.description,''),o.observed_description,''),4000) AS description,
    left(o.software,128) AS software,
    coalesce(nullif((SELECT c.family FROM catalog_software c WHERE lower(c.name)=lower(o.software) ORDER BY (c.name=o.software) DESC,c.name LIMIT 1),''),o.software,'') AS family,
    left(o.software_version,128) AS version,
    s.is_closed AS closed, o.is_alive AS alive, o.registration_open,
    CASE WHEN o.user_count >= 0 THEN o.user_count END AS users,
    CASE WHEN o.active_user_count >= 0 THEN o.active_user_count END AS active_users,
    o.checked_at, o.nodeinfo_checked_at AS metadata_checked_at, o.avg_response_time_7d AS average_response_ms
    FROM directory_sites s LEFT JOIN directory_observations o ON o.site_id=s.id
    LEFT JOIN directory_site_details d ON d.site_id=s.id
    WHERE NOT s.is_hidden AND NOT s.is_force_hidden";

fn legacy_public_servers_query(query: &str, software: &str, page: u32) -> ServerQuery {
    ServerQuery {
        query: query.trim().into(),
        software: software.trim().into(),
        page,
        page_size: PAGE_SIZE,
        ..Default::default()
    }
}

impl From<SoftwareRow> for Software {
    fn from(r: SoftwareRow) -> Self {
        let description = render_description(&r.description);
        Self {
            logo_available: r.logo_available,
            name: r.name,
            display_name: r.display_name,
            description: description.plain,
            description_html: description.html,
            categories: r.categories,
            features: r.features,
            website_url: r.website_url.as_deref().and_then(web_link),
            tech_stack: r.tech_stack,
        }
    }
}
const SOFTWARE_SELECT: &str = "SELECT EXISTS(SELECT 1 FROM stored_files f WHERE f.object_key=catalog_software.logo_key AND (catalog_software.logo_key LIKE 'software-logos/%' OR catalog_software.logo_key LIKE 'software/%')) AS logo_available,name,left(display_name,200) AS display_name,left(coalesce(description,''),4000) AS description,
            ARRAY(SELECT left(v,128) FROM unnest(CASE WHEN cardinality(categories)>0 THEN categories ELSE ARRAY[category_tag] END) v WHERE v IS NOT NULL LIMIT 32) AS categories,
            ARRAY(SELECT left(v,300) FROM unnest(features) v WHERE v IS NOT NULL LIMIT 32) AS features,
            left(website_url,2048) AS website_url,left(tech_stack,1000) AS tech_stack
            FROM catalog_software";

impl Database {
    pub async fn public_software(&self, name: &str) -> Result<SoftwareInfo, StoreError> {
        if name.is_empty() || name.len() > 128 || name.chars().any(char::is_control) {
            return Err(StoreError);
        }
        let mut conn = self.pool.get().await.map_err(|_| StoreError)?;
        let software = diesel::sql_query(format!("{SOFTWARE_SELECT} WHERE name=$1"))
            .bind::<Text, _>(name)
            .get_result::<SoftwareRow>(&mut conn)
            .await
            .optional()?
            .map(Software::from);
        let names = software
            .as_ref()
            .map(|s| s.categories.clone())
            .unwrap_or_default();
        let categories = diesel::sql_query("SELECT name,left(label,200) AS label,left(coalesce(emoji,''),32) AS emoji FROM catalog_categories WHERE name=ANY($1) ORDER BY display_order,name")
            .bind::<Array<Text>,_>(names).load::<CategoryRow>(&mut conn).await?
            .into_iter().map(|r|Category{name:r.name,label:r.label,emoji:r.emoji}).collect();
        Ok(SoftwareInfo {
            preview: false,
            software,
            categories,
        })
    }

    pub async fn public_catalog(&self) -> Result<Catalog, StoreError> {
        let mut conn = self.pool.get().await.map_err(|_| StoreError)?;
        let categories:Vec<CategoryRow> = diesel::sql_query("SELECT name,left(label,200) AS label,left(coalesce(emoji,''),32) AS emoji FROM catalog_categories ORDER BY display_order,name LIMIT 201").load(&mut conn).await?;
        let software: Vec<SoftwareRow> = diesel::sql_query(format!(
            "{SOFTWARE_SELECT} ORDER BY display_order,lower(display_name),name LIMIT 501"
        ))
        .load(&mut conn)
        .await?;
        Ok(Catalog {
            preview: false,
            truncated: categories.len() > 200 || software.len() > 500,
            categories: categories
                .into_iter()
                .take(200)
                .map(|r| Category {
                    name: r.name,
                    label: r.label,
                    emoji: r.emoji,
                })
                .collect(),
            software: software.into_iter().take(500).map(Software::from).collect(),
        })
    }
    pub async fn public_sites(
        &self,
        query: &str,
        software: &str,
        page: u32,
    ) -> Result<SitePage, StoreError> {
        if query.len() > 256 || software.len() > 128 || page > 10_000 {
            return Err(StoreError);
        }
        Ok(self
            .search_sites(&legacy_public_servers_query(query, software, page))
            .await?
            .listing)
    }
    pub async fn public_site(&self, domain: &str) -> Result<Option<PublicSite>, StoreError> {
        let mut conn = self.pool.get().await.map_err(|_| StoreError)?;
        Ok(diesel::sql_query(format!("{SITE_SELECT} AND s.domain=$1"))
            .bind::<Text, _>(domain)
            .get_result::<SiteRow>(&mut conn)
            .await
            .optional()?
            .map(TryInto::try_into)
            .transpose()?)
    }
    pub async fn public_statistics(&self) -> Result<Statistics, StoreError> {
        #[derive(QueryableByName)]
        struct Counts {
            #[diesel(sql_type=BigInt)]
            sites: i64,
            #[diesel(sql_type=Nullable<BigInt>)]
            accounts: Option<i64>,
            #[diesel(sql_type=BigInt)]
            counted_sites: i64,
            #[diesel(sql_type=Nullable<Timestamptz>)]
            oldest: Option<DateTime<Utc>>,
        }
        let mut conn = self.pool.get().await.map_err(|_| StoreError)?;
        // Unknown counts are not zero. A failed/stale observation is not fresh data.
        let r:Counts = diesel::sql_query("SELECT count(*) AS sites, sum(CASE WHEN o.user_count>=0 THEN o.user_count END)::bigint AS accounts,
            count(CASE WHEN o.user_count>=0 THEN 1 END) AS counted_sites,
            CASE WHEN count(CASE WHEN o.user_count>=0 THEN 1 END) = count(CASE WHEN o.user_count>=0 THEN o.nodeinfo_checked_at END)
                 THEN min(CASE WHEN o.user_count>=0 THEN o.nodeinfo_checked_at END) END AS oldest
            FROM directory_sites s LEFT JOIN directory_observations o ON o.site_id=s.id
            WHERE NOT s.is_hidden AND NOT s.is_force_hidden AND NOT s.is_closed").get_result(&mut conn).await?;
        Ok(Statistics {
            preview: false,
            sites: r.sites,
            accounts: r.accounts,
            counted_sites: r.counted_sites,
            oldest_observation: r.oldest.map(|t| t.to_rfc3339()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::db::{
        fixtures, schema::directory_observations as obs, schema::directory_sites as sites,
    };
    #[test]
    fn legacy_public_servers_request_uses_the_recommended_directory_query() {
        let query = legacy_public_servers_query("fedi", "mastodon", 3);
        assert_eq!(query.query, "fedi");
        assert_eq!(query.software, "mastodon");
        assert_eq!(query.page, 3);
        assert_eq!(query.page_size, PAGE_SIZE);
        assert_eq!(query.sort, "recommended");
        assert_eq!(query.direction, "desc");
    }

    #[test]
    fn software_public_projection_keeps_markup_out_of_summary_and_exposes_safe_html() {
        let software = Software::from(SoftwareRow {
            logo_available: false,
            name: "test".into(),
            display_name: "Test".into(),
            description: "## 소개\n\n**안전한** 설명<script>alert(1)</script>".into(),
            categories: vec![],
            features: vec![],
            website_url: None,
            tech_stack: None,
        });

        assert!(software.description.contains("소개"));
        assert!(software.description.contains("안전한"));
        assert!(!software.description.contains('<'));
        assert!(
            !software.description.contains("**"),
            "summary={:?}; html={:?}",
            software.description,
            software.description_html
        );
        assert!(software.description_html.contains("<h2>소개</h2>"));
        assert!(software
            .description_html
            .contains("<strong>안전한</strong>"));
        assert!(!software.description_html.contains("script"));
    }

    #[tokio::test]
    #[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
    async fn hidden_sites_cannot_be_read_by_list_direct_url_or_counts() {
        let db = fixtures::database().await;
        let marker = uuid::Uuid::new_v4().to_string();
        let ids: Vec<_> = (0..4).map(|_| uuid::Uuid::new_v4()).collect();
        let mut conn = db.pool.get().await.unwrap();
        let baseline = db.public_statistics().await.unwrap();
        for (i, id) in ids.iter().enumerate() {
            diesel::insert_into(sites::table)
                .values((
                    sites::id.eq(id),
                    sites::domain.eq(format!("{i}.{marker}.example")),
                    sites::is_hidden.eq(i == 1),
                    sites::is_force_hidden.eq(i == 2),
                    sites::is_closed.eq(i == 3),
                ))
                .execute(&mut conn)
                .await
                .unwrap();
            diesel::insert_into(obs::table)
                .values((
                    obs::site_id.eq(id),
                    obs::is_alive.eq(true),
                    obs::checked_at.eq(Utc::now()),
                    obs::software.eq("test-soft"),
                    obs::user_count.eq(if i == 0 { 7 } else { 999 }),
                    obs::nodeinfo_checked_at.eq(Utc::now()),
                ))
                .execute(&mut conn)
                .await
                .unwrap();
        }
        let list = db.public_sites(&marker, "test-soft", 0).await.unwrap();
        assert_eq!(list.sites.len(), 2); // The closed record remains readable, not counted as operating.
        for i in [1, 2] {
            assert!(db
                .public_site(&format!("{i}.{marker}.example"))
                .await
                .unwrap()
                .is_none());
        }
        assert!(db.public_sites("%", "", 0).await.unwrap().sites.is_empty()); // literal search, not SQL wildcard
        assert!(db
            .public_sites(&marker, "test-soft", 1)
            .await
            .unwrap()
            .sites
            .is_empty());
        let after = db.public_statistics().await.unwrap();
        assert_eq!(after.sites, baseline.sites + 1);
        assert_eq!(after.accounts.unwrap(), baseline.accounts.unwrap_or(0) + 7);
        diesel::delete(sites::table.filter(sites::id.eq_any(ids)))
            .execute(&mut conn)
            .await
            .unwrap();
    }
    #[tokio::test]
    #[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
    async fn catalog_is_a_public_field_allowlist_and_rejects_active_urls() {
        let db = fixtures::database().await;
        let mut conn = db.pool.get().await.unwrap();
        let marker = format!("test-{}", uuid::Uuid::new_v4());
        diesel::sql_query("INSERT INTO catalog_software (id,name,display_name,categories,features,website_url,is_featured,display_order,inserted_at,updated_at) VALUES (-98123,$1,'Test',ARRAY['image',NULL],ARRAY['Photo',NULL],'javascript:alert(1)',false,0,now(),now())").bind::<Text,_>(&marker).execute(&mut conn).await.unwrap();
        let data = db.public_catalog().await.unwrap();
        let row = data.software.iter().find(|s| s.name == marker).unwrap();
        assert!(row.website_url.is_none());
        assert_eq!(row.categories, vec!["image"]);
        assert_eq!(row.features, vec!["Photo"]);
        assert_eq!(
            db.public_software(&marker).await.unwrap().software.as_ref(),
            Some(row)
        );
        diesel::sql_query("DELETE FROM catalog_software WHERE id=-98123 AND name=$1")
            .bind::<Text, _>(&marker)
            .execute(&mut conn)
            .await
            .unwrap();
    }

    #[tokio::test]
    #[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
    async fn software_detail_is_independent_of_catalog_list_limit() {
        let db = fixtures::database().await;
        let mut conn = db.pool.get().await.unwrap();
        let marker = format!("large-{}-", uuid::Uuid::new_v4());
        diesel::sql_query("INSERT INTO catalog_software(id,name,display_name,features,categories,is_featured,display_order,inserted_at,updated_at) SELECT -99000-i,$1||i::text,$1||i::text,ARRAY[]::text[],ARRAY[]::text[],false,2000000+i,now(),now() FROM generate_series(1,501) i")
            .bind::<Text,_>(&marker).execute(&mut conn).await.unwrap();
        let last = format!("{marker}501");
        let list = db.public_catalog().await.unwrap();
        let detail = db.public_software(&last).await.unwrap();
        let missing = db
            .public_software(&format!("{marker}missing"))
            .await
            .unwrap();
        diesel::sql_query(
            "DELETE FROM catalog_software WHERE name LIKE $1 AND id BETWEEN -99501 AND -99001",
        )
        .bind::<Text, _>(format!("{marker}%"))
        .execute(&mut conn)
        .await
        .unwrap();
        assert!(list.truncated);
        assert!(!list.software.iter().any(|s| s.name == last));
        assert_eq!(detail.software.unwrap().name, last);
        assert!(missing.software.is_none());
    }
}
