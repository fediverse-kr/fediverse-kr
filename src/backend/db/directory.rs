use super::{
    schema::{
        directory_health_checks as health, directory_icons as icons, directory_jobs as jobs,
        directory_observations as observations, directory_sites as sites,
    },
    Database, StoreError,
};
use crate::backend::directory::{canonical_domain, CrawlJob, Observation};
use diesel::{
    dsl::now,
    prelude::*,
    sql_types::{Bool, Text, Uuid as SqlUuid},
};
use diesel_async::{AsyncConnection, AsyncPgConnection, RunQueryDsl};
use uuid::Uuid;

#[derive(QueryableByName)]
struct Claimed {
    #[diesel(sql_type=SqlUuid)]
    id: Uuid,
    #[diesel(sql_type=SqlUuid)]
    site_id: Uuid,
    #[diesel(sql_type=Text)]
    domain: String,
    #[diesel(sql_type=Bool)]
    needs_icon: bool,
}

fn icon_quality(mime: &str, bytes: &[u8]) -> Option<(u8, u64)> {
    if crate::backend::media::image_mime(bytes)? != mime {
        return None;
    }
    if mime == "image/svg+xml" {
        Some((2, u64::MAX))
    } else {
        crate::backend::media::validation::decoded_raster_area(bytes).map(|area| (1, area))
    }
}

fn icon_improves(new_mime: &str, new_bytes: &[u8], old_mime: &str, old_bytes: &[u8]) -> bool {
    matches!(
        (icon_quality(new_mime, new_bytes), icon_quality(old_mime, old_bytes)),
        (Some(new), Some(old)) if new > old
    )
}

/// authorization. Registration uses the same representation as background work.
pub(super) async fn store_observation(
    conn: &mut AsyncPgConnection,
    sample: Uuid,
    site: Uuid,
    result: &Observation,
    replace_icon: bool,
    mark_icon_collection: bool,
) -> Result<(), StoreError> {
    diesel::insert_into(health::table)
        .values((
            health::job_id.eq(sample),
            health::site_id.eq(site),
            health::is_alive.eq(result.alive),
            health::response_time_ms.eq(result.response_ms),
            health::status_code.eq(result.status),
            health::error.eq(&result.health_error),
            health::checked_at.eq(result.checked_at),
        ))
        .on_conflict(health::job_id)
        .do_nothing()
        .execute(conn)
        .await?;
    let values = (
        observations::is_alive.eq(result.alive),
        observations::response_time_ms.eq(result.response_ms),
        observations::status_code.eq(result.status),
        observations::health_error.eq(&result.health_error),
        observations::checked_at.eq(result.checked_at),
        observations::nodeinfo_error.eq(&result.nodeinfo_error),
    );
    diesel::insert_into(observations::table)
        .values((observations::site_id.eq(site), values.clone()))
        .on_conflict(observations::site_id)
        .do_update()
        .set(values)
        .execute(conn)
        .await?;
    if let Some(n) = &result.nodeinfo {
        diesel::update(observations::table.find(site))
            .set((
                observations::software.eq(&n.software),
                observations::software_version.eq(&n.version),
                observations::observed_name.eq(&n.name),
                observations::observed_description.eq(&n.description),
                observations::registration_open.eq(n.registration_open),
                observations::user_count.eq(n.users),
                observations::active_user_count.eq(n.active_users),
                observations::status_count.eq(n.posts),
                observations::nodeinfo_checked_at.eq(Some(result.checked_at)),
            ))
            .execute(conn)
            .await?;
    }
    let existing_icon = icons::table
        .find(site)
        .select((icons::mime, icons::bytes))
        .first::<(String, Vec<u8>)>(conn)
        .await
        .optional()?;
    if let Some(icon) = &result.icon {
        if icon_quality(icon.mime, &icon.bytes).is_some() {
            match existing_icon {
                None => {
                    diesel::insert_into(icons::table)
                        .values((
                            icons::site_id.eq(site),
                            icons::mime.eq(icon.mime),
                            icons::bytes.eq(&icon.bytes),
                        ))
                        .on_conflict(icons::site_id)
                        .do_nothing()
                        .execute(conn)
                        .await?;
                }
                Some((old_mime, old_bytes))
                    if replace_icon
                        && icon_improves(icon.mime, &icon.bytes, &old_mime, &old_bytes) =>
                {
                    diesel::update(icons::table.find(site))
                        .set((
                            icons::mime.eq(icon.mime),
                            icons::bytes.eq(&icon.bytes),
                            icons::fetched_at.eq(now),
                        ))
                        .execute(conn)
                        .await?;
                }
                _ => {}
            }
        }
    }
    if mark_icon_collection {
        diesel::sql_query("UPDATE directory_icons SET collection_version=1 WHERE site_id=$1")
            .bind::<SqlUuid, _>(site)
            .execute(conn)
            .await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::{
        db::fixtures,
        directory::{Icon, NodeInfo},
    };
    use chrono::{Duration, Utc};

    fn valid_png() -> Vec<u8> {
        use std::io::Cursor;

        let mut bytes = Vec::new();
        image::DynamicImage::new_rgba8(32, 32)
            .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)
            .unwrap();
        bytes
    }

    #[test]
    fn automatic_icon_replacement_requires_a_verified_quality_improvement() {
        use std::io::Cursor;
        fn png(width: u32, height: u32) -> Vec<u8> {
            let mut bytes = Vec::new();
            image::DynamicImage::new_rgba8(width, height)
                .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)
                .unwrap();
            bytes
        }

        let large = png(512, 512);
        let small = png(32, 32);
        assert!(icon_improves("image/png", &large, "image/png", &small));
        assert!(!icon_improves("image/png", &small, "image/png", &large));
        assert!(!icon_improves("image/png", &small, "image/png", b"legacy"));
    }

    #[tokio::test]
    #[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
    async fn postgres_queue_leases_fencing_editorial_isolation_and_maintenance() {
        let db = fixtures::database().await;
        let domain = format!("crawl-{}.example.com", Uuid::new_v4().simple());
        let site = db.add_site(&domain).await.unwrap();
        let closed = db
            .add_site(&format!("closed-{}.example.com", Uuid::new_v4().simple()))
            .await
            .unwrap();
        let mut conn = db.pool.get().await.unwrap();
        diesel::update(sites::table.find(site))
            .set((
                sites::name.eq(Some("owner name")),
                sites::description.eq(Some("owner service details")),
                sites::is_hidden.eq(true),
            ))
            .execute(&mut conn)
            .await
            .unwrap();
        diesel::update(sites::table.find(closed))
            .set(sites::is_closed.eq(true))
            .execute(&mut conn)
            .await
            .unwrap();
        db.enqueue_sites().await.unwrap();
        db.enqueue_sites().await.unwrap();
        assert_eq!(
            jobs::table
                .filter(jobs::site_id.eq(site))
                .count()
                .get_result::<i64>(&mut conn)
                .await
                .unwrap(),
            1
        );
        assert_eq!(
            jobs::table
                .filter(jobs::site_id.eq(closed))
                .count()
                .get_result::<i64>(&mut conn)
                .await
                .unwrap(),
            0
        );
        let first = db.claim_site().await.unwrap().unwrap();
        assert_eq!(first.site_id, site);
        assert!(db.claim_site().await.unwrap().is_none());
        diesel::update(jobs::table.find(first.id))
            .set(jobs::lease_until.eq(Some(Utc::now() - Duration::seconds(1))))
            .execute(&mut conn)
            .await
            .unwrap();
        let recovered = db.claim_site().await.unwrap().unwrap();
        assert_eq!(first.id, recovered.id);
        assert_ne!(first.lease_token, recovered.lease_token);
        let mut result = Observation {
            alive: true,
            response_ms: Some(150),
            status: Some(200),
            health_error: None,
            checked_at: Utc::now(),
            nodeinfo: Some(NodeInfo {
                name: Some("remote name".into()),
                description: Some("remote description".into()),
                users: Some(78),
                ..Default::default()
            }),
            nodeinfo_error: None,
            icon: Some(Icon {
                mime: "image/png",
                bytes: valid_png(),
            }),
            icon_collection_complete: true,
        };
        assert!(!db.finish_site(&first, &result).await.unwrap());
        assert!(db.finish_site(&recovered, &result).await.unwrap());
        assert!(!db.finish_site(&recovered, &result).await.unwrap());
        assert_eq!(
            health::table
                .filter(health::site_id.eq(site))
                .count()
                .get_result::<i64>(&mut conn)
                .await
                .unwrap(),
            1
        );
        assert_eq!(
            sites::table
                .find(site)
                .select((sites::name, sites::description))
                .first::<(Option<String>, Option<String>)>(&mut conn)
                .await
                .unwrap(),
            (
                Some("owner name".into()),
                Some("owner service details".into())
            )
        );
        assert_eq!(
            observations::table
                .find(site)
                .select(observations::user_count)
                .first::<Option<i64>>(&mut conn)
                .await
                .unwrap(),
            Some(78)
        );
        diesel::update(jobs::table.find(first.id))
            .set(jobs::scheduled_at.eq(Utc::now() - Duration::hours(1)))
            .execute(&mut conn)
            .await
            .unwrap();
        db.enqueue_sites().await.unwrap();
        let next = db.claim_site().await.unwrap().unwrap();
        assert!(!next.needs_icon);
        result.nodeinfo = None;
        result.nodeinfo_error = Some("nodeinfo_http_status".into());
        assert!(db.finish_site(&next, &result).await.unwrap());
        assert_eq!(
            observations::table
                .find(site)
                .select((
                    observations::is_alive,
                    observations::user_count,
                    observations::nodeinfo_error
                ))
                .first::<(bool, Option<i64>, Option<String>)>(&mut conn)
                .await
                .unwrap(),
            (true, Some(78), Some("nodeinfo_http_status".into()))
        );
        // Seed exactly this fixture's health history; no broad database cleanup.
        diesel::delete(health::table.filter(health::site_id.eq(site)))
            .execute(&mut conn)
            .await
            .unwrap();
        for n in 1..=20 {
            diesel::insert_into(health::table)
                .values((
                    health::job_id.eq(Uuid::new_v4()),
                    health::site_id.eq(site),
                    health::is_alive.eq(true),
                    health::response_time_ms.eq(Some(n)),
                    health::checked_at.eq(Utc::now()),
                ))
                .execute(&mut conn)
                .await
                .unwrap();
        }
        diesel::sql_query("UPDATE maintenance_schedule SET last_slot=-1")
            .execute(&mut conn)
            .await
            .unwrap();
        db.maintain_directory().await.unwrap();
        assert_eq!(
            observations::table
                .find(site)
                .select(observations::avg_response_time_7d)
                .first::<Option<i32>>(&mut conn)
                .await
                .unwrap(),
            Some(11)
        );
        diesel::update(health::table.filter(health::site_id.eq(site)))
            .set(health::checked_at.eq(Utc::now() - Duration::days(31)))
            .execute(&mut conn)
            .await
            .unwrap();
        diesel::sql_query("UPDATE maintenance_schedule SET last_slot=-1")
            .execute(&mut conn)
            .await
            .unwrap();
        db.maintain_directory().await.unwrap();
        assert_eq!(
            observations::table
                .find(site)
                .select(observations::avg_response_time_7d)
                .first::<Option<i32>>(&mut conn)
                .await
                .unwrap(),
            None
        );
        assert_eq!(
            health::table
                .filter(health::site_id.eq(site))
                .count()
                .get_result::<i64>(&mut conn)
                .await
                .unwrap(),
            0
        );
        // A closure during HTTP collection discards the result.
        diesel::update(jobs::table.find(next.id))
            .set(jobs::scheduled_at.eq(Utc::now() - Duration::minutes(40)))
            .execute(&mut conn)
            .await
            .unwrap();
        db.enqueue_sites().await.unwrap();
        let last = db.claim_site().await.unwrap().unwrap();
        diesel::update(sites::table.find(site))
            .set(sites::is_closed.eq(true))
            .execute(&mut conn)
            .await
            .unwrap();
        assert!(!db.finish_site(&last, &result).await.unwrap());
        diesel::delete(sites::table.filter(sites::id.eq_any([site, closed])))
            .execute(&mut conn)
            .await
            .unwrap();
    }

    #[tokio::test]
    #[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
    async fn imported_favicon_file_suppresses_only_automatic_recrawls() {
        async fn queue(
            db: &Database,
            site: Uuid,
            owner_requested: bool,
            schedule_offset: i32,
        ) -> Uuid {
            let id = Uuid::new_v4();
            let mut conn = db.pool.get().await.unwrap();
            diesel::sql_query("INSERT INTO directory_jobs(id,site_id,scheduled_at,owner_requested) VALUES($1,$2,now()-interval '10 years'+($4::integer * interval '1 second'),$3)")
                .bind::<SqlUuid,_>(id)
                .bind::<SqlUuid,_>(site)
                .bind::<Bool,_>(owner_requested)
                .bind::<diesel::sql_types::Integer,_>(schedule_offset)
                .execute(&mut conn)
                .await
                .unwrap();
            id
        }
        async fn complete(db: &Database, id: Uuid) {
            let mut conn = db.pool.get().await.unwrap();
            diesel::update(jobs::table.find(id))
                .set((
                    jobs::state.eq("complete"),
                    jobs::lease_token.eq(None::<Uuid>),
                    jobs::lease_until.eq(None::<chrono::DateTime<Utc>>),
                    jobs::completed_at.eq(Some(Utc::now())),
                ))
                .execute(&mut conn)
                .await
                .unwrap();
        }

        let db = fixtures::database().await;
        let valid_domain = format!("legacy-icon-{}.example.org", Uuid::new_v4().simple());
        let missing_domain = format!("legacy-empty-{}.example.org", Uuid::new_v4().simple());
        let absent_domain = format!("legacy-absent-{}.example.org", Uuid::new_v4().simple());
        let wrong_domain = format!("legacy-wrong-{}.example.org", Uuid::new_v4().simple());
        let valid = db.add_site(&valid_domain).await.unwrap();
        let missing = db.add_site(&missing_domain).await.unwrap();
        let absent = db.add_site(&absent_domain).await.unwrap();
        let wrong = db.add_site(&wrong_domain).await.unwrap();
        let favicon = format!("favicons/{valid_domain}.ico");
        let absent_key = format!("favicons/{absent_domain}.ico");
        let wrong_key = format!("avatars/{wrong_domain}.ico");
        let mut conn = db.pool.get().await.unwrap();
        diesel::sql_query("INSERT INTO legacy_sites(id,domain,is_force_hidden,favicon_key,inserted_at,updated_at) VALUES($1,$2,false,$3,now(),now()),($4,$5,false,NULL,now(),now()),($6,$7,false,$8,now(),now()),($9,$10,false,$11,now(),now())")
            .bind::<SqlUuid,_>(valid).bind::<Text,_>(&valid_domain).bind::<Text,_>(&favicon)
            .bind::<SqlUuid,_>(missing).bind::<Text,_>(&missing_domain)
            .bind::<SqlUuid,_>(absent).bind::<Text,_>(&absent_domain).bind::<Text,_>(&absent_key)
            .bind::<SqlUuid,_>(wrong).bind::<Text,_>(&wrong_domain).bind::<Text,_>(&wrong_key)
            .execute(&mut conn).await.unwrap();
        diesel::sql_query(
            "INSERT INTO stored_files(object_key,sha256,byte_count) VALUES($1,$2,1),($3,$2,1)",
        )
        .bind::<Text, _>(&favicon)
        .bind::<Text, _>(&"a".repeat(64))
        .bind::<Text, _>(&wrong_key)
        .execute(&mut conn)
        .await
        .unwrap();
        drop(conn);

        let automatic = queue(&db, valid, false, 0).await;
        let job = db.claim_site().await.unwrap().unwrap();
        assert_eq!(job.site_id, valid);
        assert!(!job.needs_icon);
        complete(&db, automatic).await;
        let owner = queue(&db, valid, true, 1).await;
        let job = db.claim_site().await.unwrap().unwrap();
        assert_eq!(job.site_id, valid);
        assert!(job.needs_icon);
        complete(&db, owner).await;
        for (offset, expected) in [missing, absent, wrong].into_iter().enumerate() {
            let id = queue(&db, expected, false, (offset + 2) as i32).await;
            let job = db.claim_site().await.unwrap().unwrap();
            assert_eq!(job.site_id, expected);
            assert!(job.needs_icon);
            complete(&db, id).await;
        }

        let mut conn = db.pool.get().await.unwrap();
        diesel::sql_query("DELETE FROM legacy_sites WHERE id=ANY($1)")
            .bind::<diesel::sql_types::Array<SqlUuid>, _>(vec![valid, missing, absent, wrong])
            .execute(&mut conn)
            .await
            .unwrap();
        diesel::delete(sites::table.filter(sites::id.eq_any([valid, missing, absent, wrong])))
            .execute(&mut conn)
            .await
            .unwrap();
        diesel::sql_query("DELETE FROM stored_files WHERE object_key=ANY($1)")
            .bind::<diesel::sql_types::Array<Text>, _>(vec![favicon, wrong_key])
            .execute(&mut conn)
            .await
            .unwrap();
    }

    #[tokio::test]
    #[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
    async fn crawler_icon_collection_policy_refreshes_once_without_erasing_old_bytes() {
        #[derive(QueryableByName)]
        struct StoredIcon {
            #[diesel(sql_type = diesel::sql_types::Binary)]
            bytes: Vec<u8>,
            #[diesel(sql_type = diesel::sql_types::SmallInt)]
            collection_version: i16,
        }

        let db = fixtures::database().await;
        let domain = format!("icon-policy-{}.example.org", Uuid::new_v4().simple());
        let site = db.add_site(&domain).await.unwrap();
        let mut conn = db.pool.get().await.unwrap();
        diesel::insert_into(icons::table)
            .values((
                icons::site_id.eq(site),
                icons::mime.eq("image/png"),
                icons::bytes.eq(b"old".to_vec()),
            ))
            .execute(&mut conn)
            .await
            .unwrap();
        db.enqueue_sites().await.unwrap();
        let first = db.claim_site().await.unwrap().unwrap();
        assert_eq!(first.site_id, site);
        assert!(first.needs_icon);
        assert!(db
            .finish_site(&first, &Observation::failed("offline"))
            .await
            .unwrap());
        assert_eq!(
            diesel::sql_query(
                "SELECT bytes,collection_version FROM directory_icons WHERE site_id=$1",
            )
            .bind::<SqlUuid, _>(site)
            .get_result::<StoredIcon>(&mut conn)
            .await
            .map(|icon| (icon.bytes, icon.collection_version))
            .unwrap(),
            (b"old".to_vec(), 0)
        );
        let second_job = Uuid::new_v4();
        diesel::sql_query(
            "INSERT INTO directory_jobs(id,site_id,scheduled_at) VALUES($1,$2,now()-interval '1 second')",
        )
        .bind::<SqlUuid, _>(second_job)
        .bind::<SqlUuid, _>(site)
        .execute(&mut conn)
        .await
        .unwrap();
        let second = db.claim_site().await.unwrap().unwrap();
        assert_eq!(second.id, second_job);
        assert!(second.needs_icon);
        diesel::delete(sites::table.find(site))
            .execute(&mut conn)
            .await
            .unwrap();
    }
}
impl Database {
    /// Controlled-import seam, not a public registration endpoint. Existing
    /// editorial records are never overwritten by discovery or repeat import.
    pub async fn add_site(&self, domain: &str) -> Result<Uuid, StoreError> {
        let domain = canonical_domain(domain).map_err(|_| StoreError)?;
        let mut conn = self.pool.get().await.map_err(|_| StoreError)?;
        diesel::insert_into(sites::table)
            .values((sites::id.eq(Uuid::new_v4()), sites::domain.eq(&domain)))
            .on_conflict(sites::domain)
            .do_nothing()
            .execute(&mut conn)
            .await?;
        Ok(sites::table
            .filter(sites::domain.eq(domain))
            .select(sites::id)
            .first(&mut conn)
            .await?)
    }
    pub async fn enqueue_sites(&self) -> Result<usize, StoreError> {
        let mut conn = self.pool.get().await.map_err(|_| StoreError)?;
        // Queue primitives are PG-specific infrastructure, not domain queries.
        // Only the current half-hour is scheduled after downtime (no catch-up storm).
        Ok(diesel::sql_query("INSERT INTO directory_jobs (id,site_id,scheduled_at) SELECT gen_random_uuid(),s.id,to_timestamp(floor(extract(epoch FROM now())/1800)*1800) FROM directory_sites s WHERE NOT s.is_closed AND NOT EXISTS (SELECT 1 FROM directory_jobs j WHERE j.site_id=s.id AND j.scheduled_at=to_timestamp(floor(extract(epoch FROM now())/1800)*1800)) ORDER BY s.id LIMIT 1000 ON CONFLICT (site_id,scheduled_at) DO NOTHING")
            .execute(&mut conn).await?)
    }
    pub async fn claim_site(&self) -> Result<Option<CrawlJob>, StoreError> {
        let mut conn = self.pool.get().await.map_err(|_| StoreError)?;
        let token = Uuid::new_v4();
        (&mut *conn).transaction(async move |conn| {
            // Reclaim expired process leases, not failed HTTP checks. A remote
            // failure is completed normally and sampled again in the next period.
            diesel::sql_query("UPDATE directory_jobs SET state=CASE WHEN attempts>=3 THEN 'dead' ELSE 'pending' END,lease_token=NULL,lease_until=NULL,last_error='worker_lease_expired' WHERE state='running' AND lease_until<=now()")
                .execute(conn).await?;
            let row=diesel::sql_query("WITH candidate AS (SELECT j.id,s.domain,(j.owner_requested OR (NOT EXISTS(SELECT 1 FROM directory_icons i WHERE i.site_id=s.id) AND NOT EXISTS(SELECT 1 FROM legacy_sites l JOIN stored_files f ON f.object_key=l.favicon_key WHERE l.id=s.id AND l.favicon_key LIKE 'favicons/%')) OR EXISTS(SELECT 1 FROM directory_icons i WHERE i.site_id=s.id AND i.collection_version<1)) AS needs_icon FROM directory_jobs j JOIN directory_sites s ON s.id=j.site_id WHERE j.state='pending' AND j.attempts<3 AND j.scheduled_at<=now() AND (NOT s.is_closed OR j.owner_requested) AND NOT EXISTS (SELECT 1 FROM directory_jobs running WHERE running.site_id=s.id AND running.state='running') ORDER BY j.scheduled_at,j.id LIMIT 1 FOR UPDATE OF j SKIP LOCKED) UPDATE directory_jobs j SET state='running',attempts=j.attempts+1,lease_token=$1,lease_until=now()+interval '120 seconds' FROM candidate c WHERE j.id=c.id RETURNING j.id,j.site_id,c.domain,c.needs_icon")
                .bind::<SqlUuid,_>(token).get_result::<Claimed>(conn).await.optional()?;
            Ok(row.map(|r|CrawlJob {id:r.id,site_id:r.site_id,domain:r.domain,lease_token:token,needs_icon:r.needs_icon}))
        }).await
    }
    /// Lease token and deadline fence late results from a previous worker.
    pub async fn finish_site(
        &self,
        job: &CrawlJob,
        result: &Observation,
    ) -> Result<bool, StoreError> {
        let mut conn = self.pool.get().await.map_err(|_| StoreError)?;
        (&mut *conn)
            .transaction(async move |conn| {
                // Lock the parent before its job. Site deletion holds this same
                // lock while it cascades jobs, so an in-flight worker waits and
                // then sees the old UUID missing instead of deadlocking or
                // writing observations into a later same-domain registration.
                let site = sites::table
                    .find(job.site_id)
                    .for_update()
                    .select((sites::domain, sites::is_closed))
                    .first::<(String, bool)>(conn)
                    .await
                    .optional()?;
                let Some((domain, closed)) = site else {
                    return Ok(false);
                };
                let locked = jobs::table
                    .find(job.id)
                    .filter(jobs::site_id.eq(job.site_id))
                    .filter(jobs::state.eq("running"))
                    .filter(jobs::lease_token.eq(Some(job.lease_token)))
                    .filter(jobs::lease_until.gt(now))
                    .for_update()
                    .select(jobs::owner_requested)
                    .first::<bool>(conn)
                    .await
                    .optional()?;
                let Some(owner_requested) = locked else {
                    return Ok(false);
                };
                // Explicit owner requests may inspect a closed site once; they
                // never reopen it or enable automatic periodic collection.
                let usable = (!closed || owner_requested) && domain == job.domain;
                if usable {
                    let icon_collection_complete =
                        job.needs_icon && result.icon_collection_complete;
                    store_observation(
                        conn,
                        job.id,
                        job.site_id,
                        result,
                        icon_collection_complete,
                        icon_collection_complete,
                    )
                    .await?;
                }
                diesel::update(jobs::table.find(job.id))
                    .set((
                        jobs::state.eq("complete"),
                        jobs::lease_token.eq(None::<Uuid>),
                        jobs::lease_until.eq(None::<chrono::DateTime<chrono::Utc>>),
                        jobs::completed_at.eq(Some(chrono::Utc::now())),
                        jobs::last_error.eq(if usable {
                            result
                                .health_error
                                .as_deref()
                                .or(result.nodeinfo_error.as_deref())
                        } else {
                            Some("site_changed_or_closed")
                        }),
                    ))
                    .execute(conn)
                    .await?;
                Ok(usable)
            })
            .await
    }
    /// Hourly maintenance and six-hour trimmed averages are elected atomically.
    pub async fn maintain_directory(&self) -> Result<(), StoreError> {
        let mut conn = self.pool.get().await.map_err(|_| StoreError)?;
        (&mut *conn).transaction(async move |conn| {
            #[derive(QueryableByName)] struct Acquired {#[diesel(sql_type=Bool)] acquired:bool}
            let acquired=diesel::sql_query("SELECT pg_try_advisory_xact_lock(6810476213302) AS acquired").get_result::<Acquired>(conn).await?.acquired;
            if !acquired {return Ok(());}
            let cleanup=diesel::sql_query("INSERT INTO maintenance_schedule (name,last_slot) VALUES ('cleanup',floor(extract(epoch FROM now())/3600)::bigint) ON CONFLICT(name) DO UPDATE SET last_slot=EXCLUDED.last_slot WHERE maintenance_schedule.last_slot<EXCLUDED.last_slot").execute(conn).await?;
            if cleanup==1 {
                diesel::sql_query("DELETE FROM directory_owner_challenges WHERE id IN (SELECT id FROM directory_owner_challenges WHERE expires_at<=now() ORDER BY expires_at LIMIT 1000)").execute(conn).await?;
                diesel::sql_query("DELETE FROM member_auth_rate_limits WHERE (scope,key_hash) IN (SELECT scope,key_hash FROM member_auth_rate_limits WHERE resets_at<=now() ORDER BY resets_at LIMIT 1000)").execute(conn).await?;
                diesel::sql_query("DELETE FROM member_link_challenges WHERE id IN (SELECT id FROM member_link_challenges WHERE expires_at<=now() ORDER BY expires_at LIMIT 1000)").execute(conn).await?;
                diesel::sql_query("DELETE FROM member_sessions WHERE id IN (SELECT id FROM member_sessions WHERE expires_at<=now() ORDER BY expires_at LIMIT 1000)").execute(conn).await?;
                diesel::sql_query("DELETE FROM directory_health_checks WHERE job_id IN (SELECT job_id FROM directory_health_checks WHERE checked_at<now()-interval '30 days' ORDER BY checked_at LIMIT 10000)").execute(conn).await?;
                diesel::sql_query("DELETE FROM directory_jobs WHERE id IN (SELECT id FROM directory_jobs WHERE scheduled_at<now()-interval '30 days' AND state<>'running' ORDER BY scheduled_at LIMIT 10000)").execute(conn).await?;
            }
            let averages=diesel::sql_query("INSERT INTO maintenance_schedule (name,last_slot) VALUES ('averages',floor((extract(epoch FROM now())-900)/21600)::bigint) ON CONFLICT(name) DO UPDATE SET last_slot=EXCLUDED.last_slot WHERE maintenance_schedule.last_slot<EXCLUDED.last_slot").execute(conn).await?;
            if averages==1 {
                // Same minimum 20 live samples and rounded 10% trim as Phoenix.
                // Every observation is updated, clearing stale averages as well.
                diesel::sql_query("WITH ranked AS (SELECT site_id,response_time_ms,row_number() OVER(PARTITION BY site_id ORDER BY response_time_ms) AS rank,count(*) OVER(PARTITION BY site_id) AS n FROM directory_health_checks WHERE is_alive AND response_time_ms IS NOT NULL AND checked_at>=now()-interval '7 days'), means AS (SELECT site_id,round(avg(response_time_ms))::integer AS value FROM ranked WHERE n>=20 AND rank>greatest(1,round(n*0.1)) AND rank<=n-greatest(1,round(n*0.1)) GROUP BY site_id) UPDATE directory_observations o SET avg_response_time_7d=(SELECT value FROM means WHERE means.site_id=o.site_id)").execute(conn).await?;
            }
            Ok(())
        }).await
    }
}
