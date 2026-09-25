use super::*;
use crate::backend::directory::Icon;
impl Database {
    pub async fn public_site_icon(&self, domain: &str) -> Result<Option<Icon>, StoreError> {
        #[derive(QueryableByName)]
        struct Row {
            #[diesel(sql_type=Text)]
            mime: String,
            #[diesel(sql_type=diesel::sql_types::Binary)]
            bytes: Vec<u8>,
        }
        let mut conn = self.pool.get().await.map_err(|_| StoreError)?;
        let row=diesel::sql_query("SELECT i.mime,i.bytes FROM directory_icons i JOIN directory_sites s ON s.id=i.site_id WHERE s.domain=$1 AND NOT s.is_hidden AND NOT s.is_force_hidden AND octet_length(i.bytes)<=524288")
            .bind::<Text,_>(domain).get_result::<Row>(&mut conn).await.optional()?;
        Ok(row.and_then(|r| {
            let mime = crate::backend::media::image_mime(&r.bytes)?;
            (mime == r.mime).then_some(Icon {
                mime,
                bytes: r.bytes,
            })
        }))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::crawler::{self, FetchFuture, Reply, SiteTransport};
    use crate::backend::db::{
        fixtures,
        schema::{directory_icons as icons, directory_jobs as jobs, directory_sites as sites},
    };
    use chrono::{Duration, Utc};
    use std::collections::HashMap;
    use url::Url;

    struct Fake(HashMap<String, Reply>);
    impl SiteTransport for Fake {
        fn get<'a>(&'a self, url: &'a Url, _: usize) -> FetchFuture<'a> {
            Box::pin(async move { self.0.get(url.as_str()).cloned().ok_or("offline") })
        }
    }

    #[tokio::test]
    #[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
    async fn collected_svg_icon_round_trips_to_the_public_cache() {
        let db = fixtures::database().await;
        let domain = format!("svg-icon-{}.example.org", uuid::Uuid::new_v4().simple());
        let id = db.add_site(&domain).await.unwrap();
        let mut conn = db.pool.get().await.unwrap();
        diesel::insert_into(jobs::table)
            .values((
                jobs::id.eq(uuid::Uuid::new_v4()),
                jobs::site_id.eq(id),
                jobs::scheduled_at.eq(Utc::now() - Duration::days(365)),
            ))
            .execute(&mut conn)
            .await
            .unwrap();
        drop(conn);
        let job = db.claim_site().await.unwrap().unwrap();
        assert_eq!(job.site_id, id);
        assert!(job.needs_icon);
        let root = Url::parse(&format!("https://{domain}/")).unwrap();
        let icon = Url::parse(&format!("https://{domain}/site.svg")).unwrap();
        let svg = b"<svg><style>rect { fill: red }</style><rect/></svg>".to_vec();
        let observation = crawler::collect(
            &Fake(HashMap::from([
                (
                    root.to_string(),
                    Reply {
                        url: root,
                        status: 200,
                        body: Ok(b"<link rel='icon' href='/site.svg'>".to_vec()),
                    },
                ),
                (
                    icon.to_string(),
                    Reply {
                        url: icon,
                        status: 200,
                        body: Ok(svg.clone()),
                    },
                ),
            ])),
            &job,
        )
        .await;
        assert!(db.finish_site(&job, &observation).await.unwrap());
        let cached = db.public_site_icon(&domain).await.unwrap().unwrap();
        assert_eq!(cached.mime, "image/svg+xml");
        assert_eq!(cached.bytes, svg);
        let mut conn = db.pool.get().await.unwrap();
        diesel::delete(sites::table.find(id))
            .execute(&mut conn)
            .await
            .unwrap();
    }

    #[tokio::test]
    #[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
    async fn icons_recheck_visibility_and_byte_type_on_every_read() {
        let db = fixtures::database().await;
        let domain = format!("icon-{}.example.org", uuid::Uuid::new_v4().simple());
        let id = db.add_site(&domain).await.unwrap();
        let mut conn = db.pool.get().await.unwrap();
        let bytes = b"\x89PNG\r\n\x1a\n".to_vec();
        diesel::insert_into(icons::table)
            .values((
                icons::site_id.eq(id),
                icons::mime.eq("image/png"),
                icons::bytes.eq(&bytes),
            ))
            .execute(&mut conn)
            .await
            .unwrap();
        assert_eq!(
            db.public_site_icon(&domain).await.unwrap().unwrap().bytes,
            bytes
        );
        let svg = b"<svg><style>rect { fill: red }</style><rect/></svg>".to_vec();
        diesel::update(icons::table.find(id))
            .set((icons::mime.eq("image/svg+xml"), icons::bytes.eq(&svg)))
            .execute(&mut conn)
            .await
            .unwrap();
        let cached = db.public_site_icon(&domain).await.unwrap().unwrap();
        assert_eq!(cached.mime, "image/svg+xml");
        assert_eq!(cached.bytes, svg);
        diesel::update(sites::table.find(id))
            .set(sites::is_hidden.eq(true))
            .execute(&mut conn)
            .await
            .unwrap();
        assert!(db.public_site_icon(&domain).await.unwrap().is_none());
        diesel::update(sites::table.find(id))
            .set((sites::is_hidden.eq(false), sites::is_force_hidden.eq(true)))
            .execute(&mut conn)
            .await
            .unwrap();
        assert!(db.public_site_icon(&domain).await.unwrap().is_none());
        diesel::update(sites::table.find(id))
            .set(sites::is_force_hidden.eq(false))
            .execute(&mut conn)
            .await
            .unwrap();
        for (mime, bytes) in [
            ("image/png", b"<svg onload='alert(1)'/>".to_vec()),
            ("image/jpeg", b"\x89PNG\r\n\x1a\n".to_vec()),
            ("image/svg+xml", b"<html><svg/></html>".to_vec()),
            ("image/svg+xml", b"<!DOCTYPE svg><svg/>".to_vec()),
            (
                "image/svg+xml",
                b"<svg><!ENTITY remote SYSTEM 'https://example.invalid/a'></svg>".to_vec(),
            ),
        ] {
            diesel::update(icons::table.find(id))
                .set((icons::mime.eq(mime), icons::bytes.eq(bytes)))
                .execute(&mut conn)
                .await
                .unwrap();
            assert!(db.public_site_icon(&domain).await.unwrap().is_none());
        }
        diesel::delete(sites::table.find(id))
            .execute(&mut conn)
            .await
            .unwrap();
    }
}
