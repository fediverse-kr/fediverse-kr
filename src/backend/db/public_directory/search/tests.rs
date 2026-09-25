use super::*;
use crate::backend::db::{
    fixtures,
    schema::{directory_observations as obs, directory_sites as sites},
};
use uuid::Uuid;
#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn search_filters_counts_hidden_facets_sort_nulls_and_pagination_use_actual_pg() {
    let db = fixtures::database().await;
    let mut conn = db.pool.get().await.unwrap();
    let marker = Uuid::new_v4().simple().to_string();
    let sw = format!("search-{marker}");
    let category = format!("cat-{marker}");
    let family = format!("family-{marker}");
    let tag = format!("tag-{marker}");
    let private_tag = format!("private-{marker}");
    diesel::sql_query("INSERT INTO catalog_software(id,name,display_name,family,categories,features,is_featured,display_order,inserted_at,updated_at) VALUES(-98124,$1,$1,$2,ARRAY[$3],ARRAY[]::text[],false,0,now(),now())").bind::<Text,_>(&sw).bind::<Text,_>(&family).bind::<Text,_>(&category).execute(&mut conn).await.unwrap();
    let ids: Vec<_> = (0..28).map(|_| Uuid::new_v4()).collect();
    for (i, id) in ids.iter().enumerate() {
        diesel::insert_into(sites::table)
            .values((
                sites::id.eq(id),
                sites::domain.eq(format!("{i:02}.{marker}.example.org")),
                sites::name.eq(format!("{i:02} fixture {marker}")),
                sites::is_hidden.eq(i == 26),
                sites::is_force_hidden.eq(i == 27),
                sites::is_closed.eq(i == 25),
            ))
            .execute(&mut conn)
            .await
            .unwrap();
        diesel::insert_into(obs::table)
            .values((
                obs::site_id.eq(id),
                obs::is_alive.eq(i != 3),
                obs::checked_at.eq(Utc::now()),
                obs::software.eq(&sw),
                obs::registration_open.eq(if i == 4 { None } else { Some(i != 2 && i != 3) }),
                obs::user_count.eq(if i == 24 { None } else { Some(i as i64) }),
            ))
            .execute(&mut conn)
            .await
            .unwrap();
        diesel::sql_query("INSERT INTO directory_site_details(site_id,invite_only,approval_required,tags) VALUES($1,$2,$3,ARRAY[$4])").bind::<diesel::sql_types::Uuid,_>(id).bind::<Bool,_>(i==2).bind::<Bool,_>(i==1).bind::<Text,_>(if i>=26{&private_tag}else{&tag}).execute(&mut conn).await.unwrap();
    }
    let mut q = ServerQuery {
        query: marker.clone(),
        software: sw.clone(),
        category: category.clone(),
        family: family.clone(),
        tag: tag.clone(),
        ..Default::default()
    };
    let first = db.search_sites(&q).await.unwrap();
    assert_eq!(first.total, 26);
    assert_eq!(first.listing.sites.len(), 24);
    assert!(first.listing.has_next);
    q.page = 1;
    let second = db.search_sites(&q).await.unwrap();
    assert_eq!(second.listing.sites.len(), 2);
    assert!(!second.listing.has_next);
    assert!(second.listing.sites.iter().all(|s| !first
        .listing
        .sites
        .iter()
        .any(|t| t.domain == s.domain)));
    q.page = 0;
    for (reg, count) in [
        ("open", 22),
        ("approval", 1),
        ("invite_only", 1),
        ("closed", 1),
        ("unknown", 1),
    ] {
        q.registration = reg.into();
        let p = db.search_sites(&q).await.unwrap();
        assert_eq!(p.total, count, "{reg}");
        assert_eq!(p.without_registration, 26);
        assert!(p.listing.sites.iter().all(|s| !s.closed));
    }
    q.registration = "all".into();
    q.alive = "no".into();
    assert_eq!(db.search_sites(&q).await.unwrap().total, 1);
    q.alive = "closed".into();
    assert_eq!(db.search_sites(&q).await.unwrap().total, 1);
    q.alive = "all".into();
    q.sort = "users".into();
    q.direction = "desc".into();
    q.page_size = 50;
    let sorted = db.search_sites(&q).await.unwrap();
    assert_eq!(sorted.listing.sites[0].users, Some(23));
    assert!(sorted.listing.sites[24].users.is_none());
    assert!(sorted.listing.sites[25].closed);
    q.tag = private_tag.clone();
    assert_eq!(db.search_sites(&q).await.unwrap().total, 0);
    let options = db.search_options().await.unwrap();
    assert!(!options.tags.contains(&private_tag));
    assert!(options.tags.contains(&tag));
    q.tag = tag.clone();
    q.query = "%".into();
    assert_eq!(db.search_sites(&q).await.unwrap().total, 0);
    q.query = marker;
    q.category = "unknown".into();
    assert_eq!(db.search_sites(&q).await.unwrap().total, 0);
    q.category = category;
    diesel::update(obs::table.filter(obs::site_id.eq_any(&ids)))
        .set(obs::software.eq(sw.to_uppercase()))
        .execute(&mut conn)
        .await
        .unwrap();
    assert_eq!(db.search_sites(&q).await.unwrap().total, 26);
    assert!(db
        .search_options()
        .await
        .unwrap()
        .software
        .iter()
        .any(|s| s.name == sw.to_uppercase() && s.family == family));
    diesel::update(obs::table.filter(obs::site_id.eq_any(&ids)))
        .set(obs::software.eq(&sw))
        .execute(&mut conn)
        .await
        .unwrap();
    // A newly observed, uncatalogued product is still a usable family option.
    diesel::sql_query("DELETE FROM catalog_software WHERE id=-98124 AND name=$1")
        .bind::<Text, _>(&sw)
        .execute(&mut conn)
        .await
        .unwrap();
    q.category.clear();
    q.family = sw.clone();
    assert_eq!(db.search_sites(&q).await.unwrap().total, 26);
    assert!(db
        .search_options()
        .await
        .unwrap()
        .software
        .iter()
        .any(|s| s.name == sw && s.family == sw));
    diesel::delete(sites::table.filter(sites::id.eq_any(ids)))
        .execute(&mut conn)
        .await
        .unwrap();
}
