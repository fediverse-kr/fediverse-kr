use super::*;
use crate::directory::catalog::{
    CatalogPage, CatalogQuery, CategoryPage, CATEGORY_PAGE_SIZE, SOFTWARE_PAGE_SIZE,
};

const MATCH: &str = "($1='' OR strpos(lower(concat_ws(' ',name,display_name)),lower($1))>0)
    AND ($2='' OR $2=ANY(CASE WHEN cardinality(categories)>0 THEN categories ELSE ARRAY[category_tag] END))";
#[derive(QueryableByName)]
struct Count {
    #[diesel(sql_type=BigInt)]
    value: i64,
}
impl From<CategoryRow> for Category {
    fn from(r: CategoryRow) -> Self {
        Self {
            name: r.name,
            label: r.label,
            emoji: r.emoji,
        }
    }
}
impl Database {
    pub async fn search_catalog(&self, query: &CatalogQuery) -> Result<CatalogPage, StoreError> {
        query.validate().map_err(|_| StoreError)?;
        let mut conn = self.pool.get().await.map_err(|_| StoreError)?;
        (&mut *conn).build_transaction().repeatable_read().read_only().run(async |conn| {
            let total=diesel::sql_query(format!("SELECT count(*) AS value FROM catalog_software WHERE {MATCH}"))
                .bind::<Text,_>(&query.query).bind::<Text,_>(&query.category).get_result::<Count>(conn).await?.value;
            let software=diesel::sql_query(format!("{SOFTWARE_SELECT} WHERE {MATCH} ORDER BY display_order,lower(display_name),name LIMIT $3 OFFSET $4"))
                .bind::<Text,_>(&query.query).bind::<Text,_>(&query.category).bind::<BigInt,_>(SOFTWARE_PAGE_SIZE as i64)
                .bind::<BigInt,_>(i64::from(query.page)*SOFTWARE_PAGE_SIZE as i64).load::<SoftwareRow>(conn).await?;
            let kinds_total=diesel::sql_query("SELECT count(*) AS value FROM catalog_categories").get_result::<Count>(conn).await?.value;
            let categories=diesel::sql_query("SELECT name,left(label,200) AS label,left(coalesce(emoji,''),32) AS emoji FROM catalog_categories ORDER BY display_order,name LIMIT $1 OFFSET $2")
                .bind::<BigInt,_>(CATEGORY_PAGE_SIZE as i64).bind::<BigInt,_>(i64::from(query.kind_page)*CATEGORY_PAGE_SIZE as i64)
                .load::<CategoryRow>(conn).await?.into_iter().map(Category::from).collect();
            let software:Vec<Software>=software.into_iter().map(Software::from).collect();
            let mut names:Vec<_>=software.iter().flat_map(|s|s.categories.clone()).collect();
            names.push(query.category.clone());names.sort();names.dedup();
            let labels=diesel::sql_query("SELECT name,left(label,200) AS label,left(coalesce(emoji,''),32) AS emoji FROM catalog_categories WHERE name=ANY($1) ORDER BY display_order,name")
                .bind::<Array<Text>,_>(names).load::<CategoryRow>(conn).await?.into_iter().map(Category::from).collect();
            Ok(CatalogPage {
                preview:false, software, total, page:query.page,
                has_next:(i64::from(query.page)+1)*(SOFTWARE_PAGE_SIZE as i64)<total,
                kinds:CategoryPage {categories,total:kinds_total,page:query.kind_page,has_next:(i64::from(query.kind_page)+1)*(CATEGORY_PAGE_SIZE as i64)<kinds_total},
                labels,
            })
        }).await
    }
}

#[cfg(test)]
mod tests;
