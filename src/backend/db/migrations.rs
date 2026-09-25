use super::{connect, StoreError};
use diesel::{
    connection::SimpleConnection,
    prelude::*,
    sql_types::{BigInt, Binary, Bool, Text},
};
use diesel_async::async_connection_wrapper::AsyncConnectionWrapper;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use sha2::{Digest, Sha384};

const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations/diesel");
const LEGACY: [(i64, &[u8]); 2] = [
    (
        202609120001,
        include_bytes!("../../../migrations/202609120001_members.sql"),
    ),
    (
        202609120002,
        include_bytes!("../../../migrations/202609120002_instance_key.sql"),
    ),
];
type MigrationError = Box<dyn std::error::Error + Send + Sync>;

#[derive(QueryableByName)]
struct Present {
    #[diesel(sql_type = Bool)]
    present: bool,
}
#[derive(QueryableByName)]
struct LegacyMigration {
    #[diesel(sql_type = BigInt)]
    version: i64,
    #[diesel(sql_type = Binary)]
    checksum: Vec<u8>,
    #[diesel(sql_type = Bool)]
    success: bool,
}

pub(super) async fn run(url: &str) -> Result<(), StoreError> {
    let connection = connect(url).await.map_err(|_| StoreError)?;
    apply(connection).await
}
async fn apply(connection: diesel_async::AsyncPgConnection) -> Result<(), StoreError> {
    // Diesel's synchronous migration harness runs on a blocking thread, using
    // the async connection adapter. No libpq or extra executable is required.
    tokio::task::spawn_blocking(move || {
        let mut conn = AsyncConnectionWrapper::<diesel_async::AsyncPgConnection>::from(connection);
        conn.transaction::<(), MigrationError, _>(|conn| {
            // One transaction serializes concurrent startup and makes adoption
            // plus all pending migrations atomic. No HTTP/worker starts early.
            conn.batch_execute("SET LOCAL statement_timeout = '60s'; SET LOCAL lock_timeout = '60s'; SET LOCAL idle_in_transaction_session_timeout = '60s'; SELECT pg_advisory_xact_lock(6810476213301);")?;
            conn.batch_execute("CREATE TABLE IF NOT EXISTS __diesel_schema_migrations (version VARCHAR(50) PRIMARY KEY NOT NULL, run_on TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP);")?;
            let old = diesel::sql_query("SELECT to_regclass('_sqlx_migrations') IS NOT NULL AS present").get_result::<Present>(conn)?.present;
            if old {
                let rows = diesel::sql_query("SELECT version, checksum, success FROM _sqlx_migrations ORDER BY version").load::<LegacyMigration>(conn)?;
                for row in rows {
                    let source = LEGACY.iter().find(|(v,_)| *v == row.version).ok_or("Unknown legacy migration; manual mapping required")?.1;
                    let lf = std::str::from_utf8(source)?.replace("\r\n", "\n");
                    let crlf = lf.replace('\n', "\r\n");
                    if !row.success || ![source, lf.as_bytes(), crlf.as_bytes()].iter().any(|bytes| Sha384::digest(bytes).as_slice() == row.checksum) {
                        return Err("Legacy migration checksum mismatch; database left unchanged".into());
                    }
                    diesel::sql_query("INSERT INTO __diesel_schema_migrations (version) VALUES ($1) ON CONFLICT DO NOTHING")
                        .bind::<Text,_>(row.version.to_string()).execute(conn)?;
                }
            }
            conn.run_pending_migrations(MIGRATIONS)?;
            Ok(())
        }).map_err(|_| StoreError)
    }).await.map_err(|_| StoreError)?
}

#[cfg(test)]
mod tests {
    use super::{apply, connect, Present};
    use crate::backend::db::Database;
    use diesel::sql_types::{Binary, Bool, Text, Uuid as SqlUuid};
    use diesel_async::{RunQueryDsl as AsyncRunQueryDsl, SimpleAsyncConnection};

    #[tokio::test]
    #[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
    async fn postgres_fresh_embedded_migrations_repeat_and_retain_key() {
        let url = std::env::var("FEDKR_TEST_DATABASE_URL").unwrap();
        assert!(crate::backend::settings::local_database(&url) && url.ends_with("/fedkr_test"));
        let schema = format!("migration_test_{}", uuid::Uuid::new_v4().simple());
        let mut admin = connect(&url).await.unwrap();
        admin
            .batch_execute(&format!("CREATE SCHEMA {schema}"))
            .await
            .unwrap();
        for _ in 0..2 {
            let mut conn = connect(&url).await.unwrap();
            conn.batch_execute(&format!("SET search_path TO {schema}"))
                .await
                .unwrap();
            let result = apply(conn).await;
            if result.is_err() {
                admin
                    .batch_execute(&format!("DROP SCHEMA {schema} CASCADE"))
                    .await
                    .unwrap();
                panic!("fresh embedded migration failed");
            }
        }
        let present = diesel::sql_query(format!(
            "SELECT (SELECT count(*) FROM {schema}.__diesel_schema_migrations)=27 AND EXISTS(SELECT 1 FROM {schema}.__diesel_schema_migrations WHERE version='202609260027') AND EXISTS(SELECT 1 FROM pg_constraint WHERE conrelid='{schema}.member_auth_rate_limits'::regclass AND conname='member_auth_rate_limits_scope_check' AND pg_get_constraintdef(oid) LIKE '%account_age_retry%') AND to_regclass('{schema}.profile_refresh_batches') IS NOT NULL AND to_regclass('{schema}.profile_refresh_jobs') IS NOT NULL AND EXISTS(SELECT 1 FROM pg_constraint WHERE conrelid='{schema}.moderation_events'::regclass AND conname='moderation_events_action_check' AND pg_get_constraintdef(oid) LIKE '%delete_site%') AND EXISTS(SELECT 1 FROM pg_constraint WHERE conrelid='{schema}.directory_icons'::regclass AND conname='directory_icons_mime_check' AND pg_get_constraintdef(oid) LIKE '%image/svg+xml%' AND pg_get_constraintdef(oid) LIKE '%image/avif%' AND pg_get_constraintdef(oid) LIKE '%image/bmp%' AND pg_get_constraintdef(oid) LIKE '%image/tiff%') AND EXISTS(SELECT 1 FROM information_schema.columns WHERE table_schema='{schema}' AND table_name='directory_icons' AND column_name='collection_version' AND column_default='0') AS present"
        ))
        .get_result::<Present>(&mut admin)
        .await
        .unwrap()
        .present;
        admin
            .batch_execute(&format!("DROP SCHEMA {schema} CASCADE"))
            .await
            .unwrap();
        assert!(present);
        let db = Database::connect(&url).await.unwrap();
        db.ensure_signing_key().await.unwrap();
        let before = db.signing_key().await.unwrap();
        Database::migrate(&url).await.unwrap();
        db.ensure_signing_key().await.unwrap();
        assert_eq!(before, db.signing_key().await.unwrap());
    }

    #[tokio::test]
    #[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
    async fn postgres_legacy_mismatch_rolls_back_adoption_without_touching_data() {
        let url = std::env::var("FEDKR_TEST_DATABASE_URL").unwrap();
        assert!(crate::backend::settings::local_database(&url) && url.ends_with("/fedkr_test"));
        let schema = format!("migration_test_{}", uuid::Uuid::new_v4().simple());
        let mut admin = connect(&url).await.unwrap();
        admin.batch_execute(&format!("CREATE SCHEMA {schema}; CREATE TABLE {schema}._sqlx_migrations (version bigint, checksum bytea, success boolean); INSERT INTO {schema}._sqlx_migrations VALUES (202609120001,decode('ff','hex'),true);")).await.unwrap();
        let mut conn = connect(&url).await.unwrap();
        conn.batch_execute(&format!("SET search_path TO {schema}"))
            .await
            .unwrap();
        let rejected = apply(conn).await.is_err();
        let untouched=diesel::sql_query(format!("SELECT to_regclass('{schema}.__diesel_schema_migrations') IS NULL AND (SELECT count(*) FROM {schema}._sqlx_migrations)=1 AS present"))
            .get_result::<Present>(&mut admin).await.unwrap().present;
        admin
            .batch_execute(&format!("DROP SCHEMA {schema} CASCADE"))
            .await
            .unwrap();
        assert!(rejected && untouched);
    }

    #[tokio::test]
    #[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
    async fn postgres_site_deletion_downgrade_guard_preserves_audit() {
        let url = std::env::var("FEDKR_TEST_DATABASE_URL").unwrap();
        assert!(crate::backend::settings::local_database(&url) && url.ends_with("/fedkr_test"));
        Database::migrate(&url).await.unwrap();
        let db = Database::connect(&url).await.unwrap();
        let event = uuid::Uuid::new_v4();
        let target = uuid::Uuid::new_v4();
        let mut conn = db.pool.get().await.unwrap();
        diesel::sql_query("INSERT INTO moderation_events(id,target_kind,target_id,action,before_state,after_state,note) VALUES($1,'site',$2,'delete_site','before','after','fixture')")
            .bind::<SqlUuid, _>(event)
            .bind::<SqlUuid, _>(target)
            .execute(&mut conn)
            .await
            .unwrap();
        assert!(conn
            .batch_execute(include_str!(
                "../../../migrations/diesel/202609140024_site_deletion/down.sql"
            ))
            .await
            .is_err());
        #[derive(diesel::QueryableByName)]
        struct Retained {
            #[diesel(sql_type = Bool)]
            value: bool,
        }
        assert!(
            diesel::sql_query(
                "SELECT EXISTS(SELECT 1 FROM moderation_events WHERE id=$1) AS value"
            )
            .bind::<SqlUuid, _>(event)
            .get_result::<Retained>(&mut conn)
            .await
            .unwrap()
            .value
        );
        diesel::sql_query("DELETE FROM moderation_events WHERE id=$1")
            .bind::<SqlUuid, _>(event)
            .execute(&mut conn)
            .await
            .unwrap();
    }

    #[tokio::test]
    #[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
    async fn postgres_directory_icon_mime_upgrade_and_downgrade_preserve_rows() {
        #[derive(diesel::QueryableByName)]
        struct IconBytes {
            #[diesel(sql_type = Binary)]
            bytes: Vec<u8>,
        }

        let url = std::env::var("FEDKR_TEST_DATABASE_URL").unwrap();
        assert!(crate::backend::settings::local_database(&url) && url.ends_with("/fedkr_test"));
        let schema = format!("migration_test_{}", uuid::Uuid::new_v4().simple());
        let mut admin = connect(&url).await.unwrap();
        admin
            .batch_execute(&format!("CREATE SCHEMA {schema}"))
            .await
            .unwrap();
        let mut conn = connect(&url).await.unwrap();
        conn.batch_execute(&format!("SET search_path TO {schema}"))
            .await
            .unwrap();
        apply(conn).await.unwrap();
        let mut conn = connect(&url).await.unwrap();
        conn.batch_execute(&format!("SET search_path TO {schema}"))
            .await
            .unwrap();
        let png_site = uuid::Uuid::new_v4();
        let svg_site = uuid::Uuid::new_v4();
        let unsupported_site = uuid::Uuid::new_v4();
        let png = b"\x89PNG\r\n\x1a\n".to_vec();
        let svg = b"<svg><style>rect { fill: red }</style><rect/></svg>".to_vec();
        conn.batch_execute(include_str!(
            "../../../migrations/diesel/202609140025_directory_icon_mime/down.sql"
        ))
        .await
        .unwrap();
        diesel::sql_query("INSERT INTO directory_sites(id,domain) VALUES($1,$2),($3,$4),($5,$6)")
            .bind::<SqlUuid, _>(png_site)
            .bind::<Text, _>(format!("migration-png-{}.example.org", png_site.simple()))
            .bind::<SqlUuid, _>(svg_site)
            .bind::<Text, _>(format!("migration-svg-{}.example.org", svg_site.simple()))
            .bind::<SqlUuid, _>(unsupported_site)
            .bind::<Text, _>(format!(
                "migration-unsupported-{}.example.org",
                unsupported_site.simple()
            ))
            .execute(&mut conn)
            .await
            .unwrap();
        diesel::sql_query(
            "INSERT INTO directory_icons(site_id,mime,bytes) VALUES($1,'image/png',$2)",
        )
        .bind::<SqlUuid, _>(png_site)
        .bind::<Binary, _>(&png)
        .execute(&mut conn)
        .await
        .unwrap();
        conn.batch_execute(include_str!(
            "../../../migrations/diesel/202609140025_directory_icon_mime/up.sql"
        ))
        .await
        .unwrap();
        assert_eq!(
            diesel::sql_query("SELECT bytes FROM directory_icons WHERE site_id=$1")
                .bind::<SqlUuid, _>(png_site)
                .get_result::<IconBytes>(&mut conn)
                .await
                .unwrap()
                .bytes,
            png
        );
        diesel::sql_query(
            "INSERT INTO directory_icons(site_id,mime,bytes) VALUES($1,'image/svg+xml',$2)",
        )
        .bind::<SqlUuid, _>(svg_site)
        .bind::<Binary, _>(&svg)
        .execute(&mut conn)
        .await
        .unwrap();
        assert!(diesel::sql_query(
            "INSERT INTO directory_icons(site_id,mime,bytes) VALUES($1,'image/heic',$2)"
        )
        .bind::<SqlUuid, _>(unsupported_site)
        .bind::<Binary, _>(b"not an image")
        .execute(&mut conn)
        .await
        .is_err());
        assert!(conn
            .batch_execute(include_str!(
                "../../../migrations/diesel/202609140025_directory_icon_mime/down.sql"
            ))
            .await
            .is_err());
        assert_eq!(
            diesel::sql_query("SELECT bytes FROM directory_icons WHERE site_id=$1")
                .bind::<SqlUuid, _>(svg_site)
                .get_result::<IconBytes>(&mut conn)
                .await
                .unwrap()
                .bytes,
            svg
        );
        diesel::sql_query("DELETE FROM directory_icons WHERE site_id=$1")
            .bind::<SqlUuid, _>(svg_site)
            .execute(&mut conn)
            .await
            .unwrap();
        conn.batch_execute(include_str!(
            "../../../migrations/diesel/202609140025_directory_icon_mime/down.sql"
        ))
        .await
        .unwrap();
        conn.batch_execute(include_str!(
            "../../../migrations/diesel/202609140025_directory_icon_mime/up.sql"
        ))
        .await
        .unwrap();
        diesel::sql_query("DELETE FROM directory_sites WHERE id=ANY($1)")
            .bind::<diesel::sql_types::Array<SqlUuid>, _>(vec![
                png_site,
                svg_site,
                unsupported_site,
            ])
            .execute(&mut conn)
            .await
            .unwrap();
        admin
            .batch_execute(&format!("DROP SCHEMA {schema} CASCADE"))
            .await
            .unwrap();
    }
}
