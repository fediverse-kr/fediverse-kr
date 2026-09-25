use super::*;
use crate::directory::ranking::diversify_by_family;
use crate::directory::search::{FilterSoftware, SearchOptions, SearchPage, ServerQuery};
use diesel_async::AsyncConnection;

const FILTERS:&str=" AND ($1='' OR strpos(lower(concat_ws(' ',s.domain,s.name,o.observed_name,o.software,s.description,o.observed_description)),lower($1))>0)
    AND ($2='' OR lower(o.software)=lower($2))
    AND ($3='' OR EXISTS (SELECT 1 FROM catalog_software c WHERE lower(c.name)=lower(o.software) AND $3=ANY(CASE WHEN cardinality(c.categories)>0 THEN c.categories ELSE ARRAY[c.category_tag] END)))
    AND ($4='' OR EXISTS (SELECT 1 FROM catalog_software c WHERE lower(c.name)=lower(o.software) AND coalesce(nullif(c.family,''),c.name)=$4)
         OR ($4=o.software AND NOT EXISTS (SELECT 1 FROM catalog_software c WHERE lower(c.name)=lower(o.software))))
    AND ($5='' OR $5=ANY(d.tags))
    AND ($6='all' OR ($6='yes' AND NOT s.is_closed AND o.is_alive IS TRUE) OR ($6='no' AND NOT s.is_closed AND o.is_alive IS FALSE) OR ($6='unknown' AND NOT s.is_closed AND o.is_alive IS NULL) OR ($6='closed' AND s.is_closed))";
const REGISTRATION:&str="($7='all' OR (NOT s.is_closed AND (
    ($7='open' AND o.registration_open IS TRUE AND d.invite_only IS NOT TRUE)
    OR ($7='approval' AND o.registration_open IS TRUE AND d.approval_required IS TRUE AND d.invite_only IS NOT TRUE)
    OR ($7='invite_only' AND d.invite_only IS TRUE)
    OR ($7='closed' AND o.registration_open IS FALSE AND d.invite_only IS NOT TRUE)
    OR ($7='unknown' AND o.registration_open IS NULL AND d.invite_only IS NOT TRUE))))";
const RECOMMENDED_SCORE: &str = "ln(GREATEST(CASE WHEN COALESCE(o.active_user_count,0)>0 THEN o.active_user_count::double precision ELSE COALESCE(o.user_count,0)::double precision*0.20 END,0.0)+1.0)*10.0 + CASE WHEN o.registration_open IS TRUE THEN 5.0 ELSE 0.0 END + CASE WHEN COALESCE(o.avg_response_time_7d,9999)< 500 THEN 3.0 WHEN COALESCE(o.avg_response_time_7d,9999)< 1000 THEN 1.0 ELSE 0.0 END";

fn sort_expression(sort: &str) -> &'static str {
    match sort {
        "recommended" => RECOMMENDED_SCORE,
        "domain" => "s.domain",
        "users" => "o.user_count",
        "recent" => "o.checked_at",
        "newest" => "s.created_at",
        "response_time" => "o.avg_response_time_7d",
        _ => "lower(coalesce(nullif(s.name,''),o.observed_name,s.domain))",
    }
}
fn bindings<'a>(
    sql: String,
    q: &'a ServerQuery,
) -> diesel::query_builder::BoxedSqlQuery<'a, diesel::pg::Pg, diesel::query_builder::SqlQuery> {
    diesel::sql_query(sql)
        .into_boxed()
        .bind::<Text, _>(q.query.trim())
        .bind::<Text, _>(q.software.trim())
        .bind::<Text, _>(q.category.trim())
        .bind::<Text, _>(q.family.trim())
        .bind::<Text, _>(q.tag.trim())
        .bind::<Text, _>(&q.alive)
        .bind::<Text, _>(&q.registration)
}
impl Database {
    pub async fn search_sites(&self, q: &ServerQuery) -> Result<SearchPage, StoreError> {
        q.validate().map_err(|_| StoreError)?;
        let mut conn = self.pool.get().await.map_err(|_| StoreError)?;
        (&mut *conn).transaction(async move|conn|{
            diesel::sql_query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY").execute(conn).await?;
            #[derive(QueryableByName)] struct Counts {#[diesel(sql_type=BigInt)] total:i64, #[diesel(sql_type=BigInt)] without_registration:i64}
            let counts=bindings(format!("SELECT count(*) FILTER (WHERE {REGISTRATION}) AS total,count(*) AS without_registration FROM directory_sites s LEFT JOIN directory_observations o ON o.site_id=s.id LEFT JOIN directory_site_details d ON d.site_id=s.id WHERE NOT s.is_hidden AND NOT s.is_force_hidden {FILTERS}"),q).get_result::<Counts>(conn).await?;
            // Only source-controlled SQL fragments may become ORDER BY.
            let sort = sort_expression(&q.sort);
            let dir=if q.direction=="desc"{"DESC"}else{"ASC"};
            let mut rows=bindings(format!("{SITE_SELECT} {FILTERS} AND {REGISTRATION} ORDER BY s.is_closed,{sort} {dir} NULLS LAST,s.domain ASC LIMIT $8 OFFSET $9"),q)
                .bind::<BigInt,_>(q.page_size as i64+1).bind::<BigInt,_>(i64::from(q.page)*q.page_size as i64)
                .load::<SiteRow>(conn).await?;
            let has_next=rows.len()>q.page_size;
            rows.truncate(q.page_size);
            if q.sort == "recommended" {
                rows = diversify_by_family(rows, |row| row.family.clone());
            }
            Ok(SearchPage {listing:SitePage {preview:false,sites:rows.into_iter().map(TryInto::try_into).collect::<Result<_,_>>()?,page:q.page,has_next},total:counts.total,without_registration:counts.without_registration})
        }).await
    }
    pub async fn search_options(&self) -> Result<SearchOptions, StoreError> {
        let mut conn = self.pool.get().await.map_err(|_| StoreError)?;
        #[derive(QueryableByName)]
        struct Item {
            #[diesel(sql_type=Text)]
            name: String,
            #[diesel(sql_type=Text)]
            label: String,
            #[diesel(sql_type=Text)]
            family: String,
            #[diesel(sql_type=Array<Text>)]
            categories: Vec<String>,
        }
        #[derive(QueryableByName)]
        struct Tag {
            #[diesel(sql_type=Text)]
            tag: String,
        }
        let mut software=diesel::sql_query("SELECT DISTINCT o.software AS name,left(coalesce(c.display_name,o.software),200) AS label,coalesce(nullif(c.family,''),o.software) AS family,ARRAY(SELECT v FROM unnest(CASE WHEN cardinality(c.categories)>0 THEN c.categories ELSE ARRAY[c.category_tag] END) v WHERE v IS NOT NULL LIMIT 32) AS categories FROM directory_sites s JOIN directory_observations o ON o.site_id=s.id LEFT JOIN LATERAL (SELECT c.* FROM catalog_software c WHERE lower(c.name)=lower(o.software) ORDER BY (c.name=o.software) DESC,c.name LIMIT 1) c ON true WHERE NOT s.is_hidden AND NOT s.is_force_hidden AND o.software IS NOT NULL AND octet_length(o.software)<=128 ORDER BY name LIMIT 501").load::<Item>(&mut conn).await?;
        let mut categories=diesel::sql_query("SELECT name,left(label,200) AS label,left(coalesce(emoji,''),32) AS emoji FROM catalog_categories ORDER BY display_order,name LIMIT 201").load::<CategoryRow>(&mut conn).await?;
        let mut tags=diesel::sql_query("SELECT DISTINCT t.tag FROM directory_sites s JOIN directory_site_details d ON d.site_id=s.id CROSS JOIN LATERAL unnest(d.tags) t(tag) WHERE NOT s.is_hidden AND NOT s.is_force_hidden AND t.tag IS NOT NULL AND octet_length(t.tag)<=128 ORDER BY t.tag LIMIT 201").load::<Tag>(&mut conn).await?;
        let truncated = software.len() > 500 || categories.len() > 200 || tags.len() > 200;
        software.truncate(500);
        categories.truncate(200);
        tags.truncate(200);
        Ok(SearchOptions {
            software: software
                .into_iter()
                .map(|r| FilterSoftware {
                    name: r.name,
                    label: r.label,
                    family: r.family,
                    categories: r.categories,
                })
                .collect(),
            categories: categories
                .into_iter()
                .map(|r| Category {
                    name: r.name,
                    label: r.label,
                    emoji: r.emoji,
                })
                .collect(),
            tags: tags.into_iter().map(|r| r.tag).collect(),
            truncated,
        })
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod ordering_tests {
    use super::*;

    #[test]
    fn recommended_sort_selects_the_phoenix_score_expression() {
        assert_eq!(sort_expression("recommended"), RECOMMENDED_SCORE);
        assert_ne!(sort_expression("recommended"), sort_expression("name"));
        assert!(sort_expression("recommended").contains("ln(GREATEST"));
        assert!(sort_expression("recommended").contains("avg_response_time_7d"));
        assert!(sort_expression("recommended").contains("< 500"));
        assert!(sort_expression("recommended").contains("< 1000"));
    }
}
