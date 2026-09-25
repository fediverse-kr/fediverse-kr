//! Runtime file retirement. The import inventory is deliberately not edited.
//! No storage IO happens in the membership transaction: only durable intent.
use super::{Database, StoreError};
use crate::backend::storage::{self, ObjectRef, ObjectStore};
use diesel::{
    prelude::*,
    sql_types::{Array, BigInt, Bool, Text, Uuid as SqlUuid},
};
use diesel_async::{AsyncConnection, AsyncPgConnection, RunQueryDsl};
use uuid::Uuid;

#[cfg(test)]
mod tests;

#[derive(QueryableByName)]
struct Key {
    #[diesel(sql_type = Text)]
    key: String,
}
#[derive(QueryableByName)]
struct Job {
    #[diesel(sql_type = Text)]
    hash: String,
    #[diesel(sql_type = BigInt)]
    bytes: i64,
}
#[derive(QueryableByName)]
struct Flag {
    #[diesel(sql_type = Bool)]
    value: bool,
}

pub(super) async fn member_keys(
    conn: &mut AsyncPgConnection,
    member: Uuid,
) -> Result<Vec<String>, StoreError> {
    Ok(diesel::sql_query("SELECT avatar_key AS key FROM legacy_members WHERE id=$1 AND avatar_key LIKE 'avatars/%'
        UNION SELECT e.value #>> '{}' AS key FROM legacy_members m
        CROSS JOIN LATERAL jsonb_each(CASE WHEN jsonb_typeof(m.emojis)='object' THEN m.emojis ELSE '{}'::jsonb END) e
        WHERE m.id=$1 AND jsonb_typeof(e.value)='string' AND (e.value #>> '{}') LIKE 'emojis/%'
        UNION SELECT avatar_key FROM member_profile_media WHERE member_id=$1 AND avatar_key IS NOT NULL
        UNION SELECT e.value FROM member_profile_media m CROSS JOIN LATERAL jsonb_each_text(m.emojis) e WHERE m.member_id=$1")
        .bind::<SqlUuid,_>(member).load::<Key>(conn).await?.into_iter().map(|r|r.key).collect())
}

/// Called after the member's legacy row is removed, within that same transaction.
/// Serialize mapping retirement, so two members retiring one shared key cannot
/// both observe the other's uncommitted reference and leave it behind forever.
pub(super) async fn retire_keys(
    conn: &mut AsyncPgConnection,
    keys: &[String],
) -> Result<(), StoreError> {
    if keys.is_empty() {
        return Ok(());
    }
    diesel::sql_query("LOCK TABLE stored_files IN SHARE ROW EXCLUSIVE MODE")
        .execute(conn)
        .await?;
    diesel::sql_query("WITH retired AS (
        DELETE FROM stored_files f WHERE f.object_key=ANY($1)
        AND NOT EXISTS(SELECT 1 FROM legacy_members m WHERE m.avatar_key=f.object_key)
        AND NOT EXISTS(SELECT 1 FROM legacy_members m CROSS JOIN LATERAL jsonb_each(
            CASE WHEN jsonb_typeof(m.emojis)='object' THEN m.emojis ELSE '{}'::jsonb END) e
            WHERE jsonb_typeof(e.value)='string' AND e.value #>> '{}' = f.object_key)
        AND NOT EXISTS(SELECT 1 FROM legacy_sites s WHERE s.favicon_key=f.object_key)
        AND NOT EXISTS(SELECT 1 FROM member_profile_media m WHERE m.avatar_key=f.object_key)
        AND NOT EXISTS(SELECT 1 FROM member_profile_media m CROSS JOIN LATERAL jsonb_each_text(m.emojis) e WHERE e.value=f.object_key)
        AND NOT EXISTS(SELECT 1 FROM catalog_software s WHERE s.logo_key=f.object_key)
        RETURNING sha256,byte_count)
        INSERT INTO media_deletions(sha256,byte_count) SELECT DISTINCT sha256,byte_count FROM retired
        ON CONFLICT(sha256) DO NOTHING")
        .bind::<Array<Text>,_>(keys).execute(conn).await?;
    Ok(())
}

impl Database {
    pub(crate) async fn collect_retired_media(
        &self,
        store: &ObjectStore,
    ) -> Result<bool, StoreError> {
        // FS before PG. The handle also remains in the blocking OpenDAL job if
        // this future or the database connection disappears during deletion.
        let mutation = match store.try_mutation() {
            Ok(mutation) => mutation,
            Err(storage::Error::Busy) => return Ok(false),
            Err(_) => return Err(StoreError),
        };
        self.collect_retired_media_with(|object| mutation.delete_unreferenced(object))
            .await
    }

    /// One item, one bounded-size Fs operation. Transaction lock is the ownership
    /// DB fence; abort/crash rolls back acknowledgement, so a missing file is retried
    /// idempotently. No timeout may detach IO and then acknowledge its job.
    /// Real IO additionally owns the storage mutation guard above. Writers need
    /// both fences, in FS -> PG order, through mapping commit.
    async fn collect_retired_media_with<F, Fut>(&self, delete: F) -> Result<bool, StoreError>
    where
        F: FnOnce(ObjectRef) -> Fut + Send,
        Fut: std::future::Future<Output = Result<(), storage::Error>> + Send,
    {
        let mut conn = self.pool.get().await.map_err(|_| StoreError)?;
        if !diesel::sql_query(
            "SELECT EXISTS(SELECT 1 FROM media_deletions WHERE available_at<=now()) AS value",
        )
        .get_result::<Flag>(&mut conn)
        .await?
        .value
        {
            return Ok(false);
        }
        (&mut *conn).transaction(async move |conn| {
            // NOWAIT keeps cleanup from holding up a busy metadata writer. The
            // supervisor backs off instead of spinning or widening the lock set.
            diesel::sql_query("LOCK TABLE stored_files IN SHARE ROW EXCLUSIVE MODE NOWAIT").execute(conn).await?;
            let Some(job) = diesel::sql_query("SELECT sha256 AS hash,byte_count AS bytes FROM media_deletions
                WHERE available_at<=now() ORDER BY available_at,sha256 LIMIT 1 FOR UPDATE SKIP LOCKED")
                .get_result::<Job>(conn).await.optional()? else { return Ok(false); };
            let referenced = diesel::sql_query("SELECT EXISTS(SELECT 1 FROM stored_files WHERE sha256=$1) AS value")
                .bind::<Text,_>(&job.hash).get_result::<Flag>(conn).await?.value;
            let result = if referenced { Ok(()) } else {
                delete(ObjectRef { hash:job.hash.clone(), bytes:job.bytes }).await
            };
            if result.is_ok() {
                diesel::sql_query("DELETE FROM media_deletions WHERE sha256=$1")
                    .bind::<Text,_>(&job.hash).execute(conn).await?;
            } else {
                diesel::sql_query("UPDATE media_deletions SET
                    available_at=now()+make_interval(secs=>least(3600,60*power(2,least(attempts,6)))::double precision),
                    attempts=least(attempts+1,65535) WHERE sha256=$1")
                    .bind::<Text,_>(&job.hash).execute(conn).await?;
                // No error text, original paths, member identities or hashes.
                eprintln!("Media cleanup deferred; retry scheduled");
            }
            Ok(true)
        }).await
    }
}
