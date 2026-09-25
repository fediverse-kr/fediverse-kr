//! Storage guard -> admin gate -> member/session -> catalog -> mapping locks.
//! Durable orphan intent precedes IO. Catalog/mapping/audit/old-key retirement
//! then commit together; neither HTTP cancellation nor PG loss abandons IO locks.
use super::*;
use crate::backend::{
    catalog_logos::Logo,
    storage::{self, ObjectStore},
};

impl Database {
    pub(crate) async fn moderate_software_logo(
        &self,
        s: &AuthenticatedSession,
        store: &ObjectStore,
        request: dto::LogoRequest,
        logo: Option<Logo>,
    ) -> Result<dto::Software, Error> {
        let request = policy::logo(request)?;
        let mutation = store.try_mutation().map_err(|e| match e {
            storage::Error::Busy => Error::RateLimited,
            _ => Error::Unavailable,
        })?;
        let mut conn = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        if let Some(logo) = logo.as_ref() {
            // Committed independently before writing. A later rollback leaves
            // this candidate for GC; a successful mapping makes GC skip it.
            (&mut *conn).transaction::<_, Error, _>(async |conn| {
                admin(conn, s, true).await?;
                writable(conn, s).await?;
                if row(conn, &request.name, true).await?.revision != request.revision { return Err(Error::Conflict); }
                diesel::sql_query("LOCK TABLE stored_files IN SHARE ROW EXCLUSIVE MODE").execute(conn).await?;
                let inconsistent = diesel::sql_query("SELECT EXISTS(SELECT 1 FROM stored_files WHERE sha256=$1 AND byte_count<>$2 UNION ALL SELECT 1 FROM media_deletions WHERE sha256=$1 AND byte_count<>$2) AS value")
                    .bind::<Text,_>(&logo.object.hash).bind::<BigInt,_>(logo.object.bytes).get_result::<Flag>(conn).await?.value;
                if inconsistent { return Err(Error::Unavailable); }
                diesel::sql_query("INSERT INTO media_deletions(sha256,byte_count) VALUES($1,$2) ON CONFLICT DO NOTHING")
                    .bind::<Text,_>(&logo.object.hash).bind::<BigInt,_>(logo.object.bytes).execute(conn).await?;
                Ok(())
            }).await?;
        }
        let result = (&mut *conn).transaction(async |conn| {
            admin(conn, s, true).await?;
            writable(conn, s).await?;
            let before = row(conn, &request.name, true).await?;
            if before.revision != request.revision { return Err(Error::Conflict); }
            diesel::sql_query("LOCK TABLE stored_files IN SHARE ROW EXCLUSIVE MODE").execute(conn).await?;
            let snapshot: Value = serde_json::from_str(&before.snapshot).map_err(|_| Error::Unavailable)?;
            let old_key = snapshot["logo_key"].as_str().map(str::to_owned);
            let (key, new_value) = if let Some(logo) = logo {
                mutation.publish(logo.object.clone(), logo.bytes).await.map_err(|_| Error::Unavailable)?;
                // A repeated save of identical bytes is not a new edit. Still
                // verify/repair its object before treating it as successful.
                let same = diesel::sql_query("SELECT EXISTS(SELECT 1 FROM stored_files WHERE object_key=$1 AND sha256=$2 AND byte_count=$3) AS value")
                    .bind::<Nullable<Text>,_>(&old_key).bind::<Text,_>(&logo.object.hash).bind::<BigInt,_>(logo.object.bytes).get_result::<Flag>(conn).await?.value;
                if same { return software(conn, &request.name).await; }
                let key = format!("software-logos/{}", uuid::Uuid::new_v4().simple());
                diesel::sql_query("INSERT INTO stored_files(object_key,sha256,byte_count) VALUES($1,$2,$3)")
                    .bind::<Text,_>(&key).bind::<Text,_>(&logo.object.hash).bind::<BigInt,_>(logo.object.bytes).execute(conn).await?;
                (Some(key), json!({"present":true,"mime":logo.mime,"bytes":logo.object.bytes}))
            } else {
                if old_key.is_none() { return software(conn, &request.name).await; }
                (None, Value::Null)
            };
            if before.revision == 0 {
                diesel::sql_query("INSERT INTO catalog_software_edits(software_id,revision,action,summary,snapshot) VALUES($1,0,'baseline','기존 카탈로그',$2::jsonb)")
                    .bind::<BigInt,_>(before.id).bind::<Text,_>(&before.snapshot).execute(conn).await?;
            }
            diesel::sql_query("INSERT INTO catalog_software_state(software_id) VALUES($1) ON CONFLICT DO NOTHING").bind::<BigInt,_>(before.id).execute(conn).await?;
            diesel::sql_query("UPDATE catalog_software SET logo_key=$2,updated_at=now() AT TIME ZONE 'UTC' WHERE id=$1").bind::<BigInt,_>(before.id).bind::<Nullable<Text>,_>(&key).execute(conn).await?;
            diesel::sql_query("UPDATE catalog_software_state SET revision=revision+1 WHERE software_id=$1").bind::<BigInt,_>(before.id).execute(conn).await?;
            let after = row(conn, &request.name, false).await?;
            append(conn, &after, s, "admin_settings", if key.is_some() { "로고 변경" } else { "로고 제거" }).await?;
            audit(conn, s, (Some(before.id), None), after.revision, "logo", &request.note, &json!({"present":old_key.is_some()}), &new_value).await?;
            if let Some(old) = old_key {
                crate::backend::db::media_cleanup::retire_keys(conn, &[old]).await.map_err(|_| Error::Unavailable)?;
            }
            software(conn, &request.name).await
        }).await;
        // Explicitly keep the OS guard through DB commit/rollback, not just IO.
        drop(mutation);
        result
    }
}
