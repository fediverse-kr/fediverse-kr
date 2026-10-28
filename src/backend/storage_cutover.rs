//! Explicit Fs -> configured S3 cutover. Never deletes the preservation source.
//! Run after the old writer has stopped, before the new HTTP/workers start.
use super::{
    db::StoreError,
    storage::{Backend, Error, ObjectRef, ObjectStore},
};
use diesel::{
    prelude::*,
    sql_types::{BigInt, Text},
};
use diesel_async::{AsyncConnection, RunQueryDsl};

#[derive(QueryableByName)]
struct StoredObject {
    #[diesel(sql_type = Text)]
    hash: String,
    #[diesel(sql_type = BigInt)]
    bytes: i64,
}

#[derive(serde::Serialize)]
pub(crate) struct Report {
    dry_run: bool,
    objects: usize,
    already_present: usize,
    copied: usize,
    source_preserved: bool,
}

pub(crate) async fn run(dry_run: bool) -> Result<Report, &'static str> {
    let destination = ObjectStore::from_env()?.ok_or("Configured S3 storage required")?;
    if destination.backend != Backend::S3 {
        return Err("Media cutover requires the configured S3 backend; Fs is not a destination");
    }
    let source = ObjectStore::open(&destination.root)
        .map_err(|_| "Existing local preservation directory is unavailable")?;
    // Normal application initialization verifies the destination before serving.
    // This explicit offline command must populate that destination first; it
    // never starts HTTP/workers or initializes/changes the signing identity.
    let config = super::settings::Config::from_env()?;
    config.migrate().await?;
    let db = config.connect().await?;
    // Same cross-process lock used by web mutations and GC; no reverse lock order.
    let mutation = destination
        .try_mutation()
        .map_err(|_| "Media cutover lock unavailable")?;
    let mut connection = db.pool.get().await.map_err(|_| "Database unavailable")?;
    let result = (&mut *connection).transaction::<_, StoreError, _>(async |conn| {
        // The offline transfer keeps this lock while verifying/copying remote
        // objects. The ordinary HTTP pool's 15-second idle limit is too short.
        diesel::sql_query("SET LOCAL idle_in_transaction_session_timeout = '10min'")
            .execute(conn).await?;
        diesel::sql_query("LOCK TABLE stored_files IN SHARE ROW EXCLUSIVE MODE")
            .execute(conn).await?;
        let rows = diesel::sql_query("SELECT DISTINCT sha256 AS hash,byte_count AS bytes FROM stored_files ORDER BY sha256,byte_count")
            .load::<StoredObject>(conn).await?;
        let total = rows.len();
        let mut missing = Vec::new();
        let mut previous = None;
        // Verify every referenced object before the first publication. Already
        // migrated objects need no local copy, including new S3-only uploads.
        for row in rows {
            if previous.as_ref() == Some(&row.hash) {
                return Err(StoreError);
            }
            previous = Some(row.hash.clone());
            let object = ObjectRef { hash: row.hash, bytes: row.bytes };
            match destination.read(&object).await {
                Ok(_) => {},
                Err(Error::Missing) => {
                    source.read(&object).await.map_err(|_| StoreError)?;
                    missing.push(object);
                },
                Err(_) => return Err(StoreError),
            }
        }
        let count = missing.len();
        if !dry_run {
            for object in missing {
                let bytes = source.read(&object).await.map_err(|_| StoreError)?;
                mutation.publish(object.clone(), bytes).await.map_err(|_| StoreError)?;
                destination.read(&object).await.map_err(|_| StoreError)?;
            }
        }
        Ok(Report { dry_run, objects: total, already_present: total - count,
            copied: if dry_run { 0 } else { count }, source_preserved: true })
    }).await.map_err(|_| "Media cutover verification failed; local originals were preserved");
    drop(mutation);
    result
}
