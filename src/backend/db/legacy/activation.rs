//! Approval changes one row in the renamed copy, never the snapshot or traffic.
use super::*;
use crate::backend::{
    activation::{Error as ActivationError, Report, Request},
    storage::{ObjectRef, ObjectStore},
};

fn import_error(error: Error) -> ActivationError {
    match error {
        Error::Connection => ActivationError::Connection,
        Error::Database => ActivationError::Database,
        _ => ActivationError::Inventory,
    }
}
impl From<diesel::result::Error> for ActivationError {
    fn from(_: diesel::result::Error) -> Self {
        Self::Database
    }
}

#[derive(QueryableByName)]
struct Approval {
    #[diesel(sql_type=BigInt)]
    asset_objects: i64,
    #[diesel(sql_type=BigInt)]
    asset_bytes: i64,
}
pub(super) async fn runtime_permitted(
    conn: &mut AsyncPgConnection,
    origin: Option<&str>,
) -> Result<bool, Error> {
    let Some(origin) = origin else {
        return Ok(false);
    };
    if !flag(
        conn,
        "SELECT to_regclass('public.legacy_runtime_activation') IS NOT NULL AS value",
    )
    .await?
    {
        return Ok(false);
    }
    Ok(diesel::sql_query("SELECT (SELECT count(*) FROM public.legacy_import_state)=1 AND EXISTS(SELECT 1 FROM public.legacy_runtime_activation a JOIN public.legacy_import_state i USING(singleton) WHERE a.format_version=1 AND i.format_version=1 AND i.state='quarantined' AND a.source_fingerprint=i.source_fingerprint AND a.database_name=current_database() AND a.public_origin=$1) AS value")
        .bind::<Text,_>(origin).get_result::<Flag>(conn).await?.value)
}

pub(crate) async fn activate_legacy(request: &Request) -> Result<Report, ActivationError> {
    crate::backend::activation::validate(&request.source, &request.target, &request.origin)?;
    let mut target = private_import_connection(&request.target)
        .await
        .map_err(import_error)?;
    // Serialize with import / preservation and with competing activation calls.
    target.batch_execute("SET statement_timeout='60s'; SET lock_timeout='60s'; SELECT pg_advisory_lock(6810476213302)").await?;
    let available = tables(&mut target).await.map_err(import_error)?;
    if !available.contains("legacy_import_state")
        || available
            .iter()
            .any(|t| !TARGET_TABLES.contains(&t.as_str()))
    {
        return Err(ActivationError::Inventory);
    }
    if !flag(&mut target,"SELECT count(*)=1 AND bool_and(singleton AND format_version=1 AND state='quarantined') AS value FROM public.legacy_import_state").await.map_err(import_error)? {
        return Err(ActivationError::Inventory);
    }
    Database::migrate(&request.target)
        .await
        .map_err(|_| ActivationError::Database)?;
    target.batch_execute("BEGIN ISOLATION LEVEL REPEATABLE READ; SET LOCAL TIME ZONE 'UTC'; SET LOCAL idle_in_transaction_session_timeout='120s'; SET LOCAL search_path=pg_catalog,public;").await?;
    let result = async {
        // All names are fixed schema constants. Prevent writes between the
        // source/projection/file checks and recording approval, not just other CLIs.
        target.batch_execute(&format!("LOCK TABLE {} IN SHARE ROW EXCLUSIVE MODE",TARGET_TABLES.iter().map(|t|format!("public.{t}")).collect::<Vec<_>>().join(","))).await?;
        let old = diesel::sql_query("SELECT asset_objects,asset_bytes FROM public.legacy_runtime_activation WHERE singleton")
            .get_result::<Approval>(&mut target).await.optional()?;
        if let Some(old) = old {
            if !runtime_permitted(&mut target,Some(&request.origin)).await.map_err(import_error)? {
                return Err(ActivationError::Conflict);
            }
            // A retry after COMMIT may find legitimate new runtime writes. Do
            // not overwrite them or pretend the old snapshot was revalidated.
            return Ok(Report { outcome:"already_activated",runtime_enabled:true,validated_now:false,asset_objects:old.asset_objects,asset_bytes:old.asset_bytes });
        }
        let mut source = private_import_connection(&request.source).await.map_err(import_error)?;
        source.batch_execute("BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY; SET LOCAL TIME ZONE 'UTC'; SET LOCAL statement_timeout='60s'; SET LOCAL idle_in_transaction_session_timeout='120s'; SET LOCAL search_path=pg_catalog,public;").await?;
        let source_tables = tables(&mut source).await.map_err(import_error)?;
        if TABLES.iter().any(|m| !source_tables.contains(m.source)) || source_tables.iter().any(|t| !TABLES.iter().any(|m| m.source==t) && !EXCLUDED.contains(&t.as_str())) {
            return Err(ActivationError::Inventory);
        }
        let mut digest = Sha256::new();
        for mapping in TABLES {
            if columns(&mut source,mapping.source).await.map_err(import_error)? != columns(&mut target,mapping.target).await.map_err(import_error)? {
                return Err(ActivationError::Inventory);
            }
            transfer_table(&mut source,&mut target,mapping,false,&mut digest).await.map_err(import_error)?;
        }
        source.batch_execute("ROLLBACK").await?;
        let fingerprint = format!("{:x}",digest.finalize());
        let expected = diesel::sql_query("SELECT source_fingerprint AS name FROM public.legacy_import_state WHERE singleton")
            .get_result::<Name>(&mut target).await?.name;
        if fingerprint != expected { return Err(ActivationError::Inventory); }
        validate_key(&mut target).await.map_err(import_error)?;
        if count(&mut target,"legacy_asset_manifest").await.map_err(import_error)? != 1 {
            return Err(ActivationError::Files);
        }
        // Reuses the full retained/projection/hash/byte verification. The helper
        // does not commit, so these checks and approval share the same locks.
        let files = assets::preserve_transaction(&mut target,&request.paths,false).await.map_err(|_|ActivationError::Files)?;
        if request.apply {
            diesel::sql_query("INSERT INTO public.legacy_runtime_activation(singleton,format_version,source_fingerprint,database_name,public_origin,asset_objects,asset_bytes) VALUES(true,1,$1,current_database(),$2,$3,$4)")
                .bind::<Text,_>(&fingerprint).bind::<Text,_>(&request.origin).bind::<BigInt,_>(files.objects).bind::<BigInt,_>(files.bytes).execute(&mut target).await?;
        }
        Ok(Report { outcome:if request.apply {"activated"}else{"verified_not_activated"}, runtime_enabled:request.apply, validated_now:true, asset_objects:files.objects, asset_bytes:files.bytes })
    }.await;
    match &result {
        Ok(report) if request.apply && report.outcome == "activated" => {
            target.batch_execute("COMMIT").await?
        }
        _ => target.batch_execute("ROLLBACK").await?,
    }
    result
}

#[derive(QueryableByName)]
struct FileRow {
    #[diesel(sql_type=Text)]
    hash: String,
    #[diesel(sql_type=BigInt)]
    bytes: i64,
}
impl Database {
    /// Activated deployments must mount the actual current store before serving.
    /// Validate current mappings, not historical totals (later deletions are valid).
    pub(crate) async fn verify_activated_store(
        &self,
        store: Option<&ObjectStore>,
    ) -> Result<(), super::super::StoreError> {
        use super::super::StoreError;
        let mut conn = self.pool.get().await.map_err(|_| StoreError)?;
        if count(&mut conn, "legacy_runtime_activation")
            .await
            .map_err(|_| StoreError)?
            == 0
        {
            return Ok(());
        }
        if store.is_none()
            && count(&mut conn, "media_deletions")
                .await
                .map_err(|_| StoreError)?
                != 0
        {
            return Err(StoreError);
        }
        if flag(&mut conn,"SELECT EXISTS(SELECT 1 FROM public.stored_files GROUP BY sha256 HAVING min(byte_count)<>max(byte_count)) AS value").await.map_err(|_|StoreError)? { return Err(StoreError); }
        let mut cursor = String::new();
        loop {
            let page=diesel::sql_query("SELECT sha256 AS hash,min(byte_count) AS bytes FROM public.stored_files WHERE sha256>$1 GROUP BY sha256 ORDER BY sha256 LIMIT $2")
                .bind::<Text,_>(&cursor).bind::<BigInt,_>(PAGE_SIZE).load::<FileRow>(&mut conn).await?;
            if page.is_empty() {
                return Ok(());
            }
            let store = store.ok_or(StoreError)?;
            for file in page {
                store
                    .read(&ObjectRef {
                        hash: file.hash.clone(),
                        bytes: file.bytes,
                    })
                    .await
                    .map_err(|_| StoreError)?;
                cursor = file.hash;
            }
        }
    }
}
