use super::*;
use crate::backend::db::{fixtures, schema::directory_sites as sites};
use uuid::Uuid;

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn health_history_reads_latest_48_in_order_and_respects_visibility_using_pg() {
    let db = fixtures::database().await;
    let mut conn = db.pool.get().await.unwrap();
    let id = Uuid::new_v4();
    let empty_id = Uuid::new_v4();
    let domain = format!("health-{id}.example.org");
    let empty_domain = format!("empty-{id}.example.org");
    for (id, domain) in [(id, &domain), (empty_id, &empty_domain)] {
        diesel::insert_into(sites::table)
            .values((
                sites::id.eq(id),
                sites::domain.eq(domain),
                sites::name.eq("Health fixture"),
                sites::is_closed.eq(true),
            ))
            .execute(&mut conn)
            .await
            .unwrap();
    }
    let start = DateTime::parse_from_rfc3339("2026-09-13T23:59:00Z")
        .unwrap()
        .with_timezone(&Utc);
    for index in 0..64_u128 {
        // Pairs share a timestamp. UUID ordering makes the latest 48 stable.
        let job_id = Uuid::from_u128((id.as_u128() & !0xff) | index);
        diesel::sql_query("INSERT INTO directory_health_checks(job_id,site_id,is_alive,response_time_ms,status_code,error,checked_at) VALUES($1,$2,$3,$4,503,'private transport detail',$5)")
            .bind::<diesel::sql_types::Uuid,_>(job_id).bind::<diesel::sql_types::Uuid,_>(id)
            .bind::<Bool,_>(index != 20).bind::<Integer,_>(index as i32)
            .bind::<Timestamptz,_>(start + chrono::Duration::minutes((index / 2) as i64))
            .execute(&mut conn).await.unwrap();
    }
    let history = db.public_health_history(&domain).await.unwrap().unwrap();
    assert!(!history.preview);
    assert_eq!(history.checks.len(), HISTORY_LIMIT);
    assert_eq!(
        history
            .checks
            .iter()
            .map(|c| c.response_ms.unwrap())
            .collect::<Vec<_>>(),
        (16..64).collect::<Vec<_>>()
    );
    assert_eq!(history.checks[0].checked_at_kst, "2026-09-14 09:07:00");
    assert_eq!(
        history.checks[4].status(),
        crate::directory::health::Status::Unreachable
    );
    let json = serde_json::to_string(&history).unwrap();
    for private in [
        "private transport detail",
        "status_code",
        "job_id",
        "site_id",
    ] {
        assert!(!json.contains(private));
    }
    assert!(db
        .public_health_history(&empty_domain)
        .await
        .unwrap()
        .unwrap()
        .checks
        .is_empty());
    assert!(db
        .public_health_history("missing-health.example.org")
        .await
        .unwrap()
        .is_none());
    assert!(db.public_health_history("\n").await.is_err());
    for (hidden, force_hidden) in [(true, false), (false, true), (true, true)] {
        diesel::update(sites::table.find(id))
            .set((
                sites::is_hidden.eq(hidden),
                sites::is_force_hidden.eq(force_hidden),
            ))
            .execute(&mut conn)
            .await
            .unwrap();
        assert!(db.public_health_history(&domain).await.unwrap().is_none());
    }
    diesel::delete(sites::table.filter(sites::id.eq_any([id, empty_id])))
        .execute(&mut conn)
        .await
        .unwrap();
}
