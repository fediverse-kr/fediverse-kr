//! Quarantined asset inventory. All SQL and private storage keys stay here.
use super::*;
use crate::backend::legacy_assets::{self, Bundle, Error as AssetError, Report};
use diesel::sql_types::Array;
use std::collections::BTreeMap;

const REFERENCES: &str = "WITH asset_references AS (
 SELECT avatar_key::text AS object_key FROM public.legacy_members WHERE avatar_key IS NOT NULL
 UNION ALL SELECT favicon_key::text FROM public.legacy_sites WHERE favicon_key IS NOT NULL
 UNION ALL SELECT logo_key::text FROM public.catalog_software WHERE logo_key IS NOT NULL
 UNION ALL SELECT e.value #>> '{}' FROM public.legacy_members m
 CROSS JOIN LATERAL jsonb_each(CASE WHEN jsonb_typeof(m.emojis)='object' THEN m.emojis ELSE '{}'::jsonb END) e
)";

fn asset_error(error: Error) -> AssetError {
    match error {
        Error::Connection | Error::Database => AssetError::Database,
        Error::Configuration => AssetError::Configuration,
        _ => AssetError::Inventory,
    }
}

async fn validate_references(conn: &mut AsyncPgConnection) -> Result<(), Error> {
    if flag(conn, "SELECT EXISTS(SELECT 1 FROM public.legacy_members WHERE emojis IS NOT NULL AND jsonb_typeof(emojis) NOT IN ('object','null')) OR EXISTS(SELECT 1 FROM public.legacy_members m CROSS JOIN LATERAL jsonb_each(CASE WHEN jsonb_typeof(m.emojis)='object' THEN m.emojis ELSE '{}'::jsonb END) e WHERE jsonb_typeof(e.value)<>'string') AS value").await? {
        return Err(Error::Data);
    }
    Ok(())
}

pub(super) async fn reference_count(conn: &mut AsyncPgConnection) -> Result<i64, Error> {
    validate_references(conn).await?;
    Ok(diesel::sql_query(format!(
        "{REFERENCES} SELECT count(*) AS value FROM asset_references"
    ))
    .get_result::<Number>(conn)
    .await?
    .value)
}

#[derive(QueryableByName)]
struct Manifest {
    #[diesel(sql_type = Text)]
    source_fingerprint: String,
    #[diesel(sql_type = BigInt)]
    object_count: i64,
    #[diesel(sql_type = BigInt)]
    byte_count: i64,
}

async fn manifest(conn: &mut AsyncPgConnection) -> Result<Option<Manifest>, Error> {
    Ok(diesel::sql_query("SELECT source_fingerprint,object_count,byte_count FROM public.legacy_asset_manifest WHERE singleton")
        .get_result(conn).await.optional()?)
}

/// Importer may revalidate a supplementary manifest without claiming to have
/// read the bundle bytes. Only --legacy-assets with the two roots does that.
pub(super) async fn verify_manifest_metadata(
    conn: &mut AsyncPgConnection,
    fingerprint: &str,
) -> Result<(), Error> {
    let Some(m) = manifest(conn).await? else {
        return if count(conn, "legacy_asset_files").await? == 0
            && count(conn, "stored_files").await? == 0
        {
            Ok(())
        } else {
            Err(Error::Mismatch)
        };
    };
    validate_references(conn).await?;
    let totals = diesel::sql_query("SELECT ''::text AS source_fingerprint,count(*) AS object_count,coalesce(sum(byte_count),0)::bigint AS byte_count FROM public.legacy_asset_files")
        .get_result::<Manifest>(conn).await?;
    if m.source_fingerprint != fingerprint || m.object_count != totals.object_count || m.byte_count != totals.byte_count
        || flag(conn, &format!("{REFERENCES} SELECT EXISTS((SELECT object_key FROM asset_references EXCEPT SELECT object_key FROM public.legacy_asset_files) UNION ALL (SELECT object_key FROM public.legacy_asset_files EXCEPT SELECT object_key FROM asset_references)) AS value")).await?
        || flag(conn,"SELECT EXISTS(SELECT 1 FROM public.legacy_asset_files GROUP BY sha256 HAVING min(byte_count)<>max(byte_count)) AS value").await? {
        return Err(Error::Mismatch);
    }
    if flag(conn,"SELECT EXISTS((SELECT object_key,sha256,byte_count FROM public.legacy_asset_files EXCEPT SELECT object_key,sha256,byte_count FROM public.stored_files) UNION ALL (SELECT object_key,sha256,byte_count FROM public.stored_files EXCEPT SELECT object_key,sha256,byte_count FROM public.legacy_asset_files)) AS value").await? {return Err(Error::Mismatch)}
    Ok(())
}

async fn reference_page(
    conn: &mut AsyncPgConnection,
    cursor: Option<&str>,
) -> Result<Vec<String>, AssetError> {
    Ok(diesel::sql_query(format!("{REFERENCES} SELECT DISTINCT object_key COLLATE \"C\" AS name FROM asset_references WHERE $1::text IS NULL OR object_key COLLATE \"C\">$1 COLLATE \"C\" ORDER BY name LIMIT $2"))
        .bind::<Nullable<Text>,_>(cursor).bind::<BigInt,_>(PAGE_SIZE)
        .load::<Name>(conn).await.map_err(|_|AssetError::Database)?.into_iter().map(|r|r.name).collect())
}

/// Verify retained rows against the original import's digest before reading any
/// files. A changed local reference must not silently define a new inventory.
async fn verify_retained(conn: &mut AsyncPgConnection, fingerprint: &str) -> Result<(), Error> {
    let mut digest = Sha256::new();
    for m in TABLES {
        digest.update((m.source.len() as u64).to_be_bytes());
        digest.update(m.source.as_bytes());
        let mut cursor = None;
        loop {
            let page = read_page(conn, m.target, m.id_type, &cursor).await?;
            if page.is_empty() {
                break;
            }
            for row in &page {
                digest.update((row.payload.len() as u64).to_be_bytes());
                digest.update(row.payload.as_bytes());
            }
            cursor = page.last().map(|r| r.cursor.clone());
        }
    }
    if format!("{:x}", digest.finalize()) != fingerprint {
        return Err(Error::Mismatch);
    }
    prepare_identities(conn, false).await?;
    validate_projection(conn).await
}

#[derive(QueryableByName)]
struct Stored {
    #[diesel(sql_type = Text)]
    object_key: String,
    #[diesel(sql_type = Text)]
    sha256: String,
    #[diesel(sql_type = BigInt)]
    byte_count: i64,
}

pub(crate) async fn preserve_legacy_assets(
    target_url: &str,
    paths: &Bundle,
    apply: bool,
) -> Result<Report, AssetError> {
    if !crate::backend::legacy::isolated_url(target_url, "fedkr_rehearsal_") {
        return Err(AssetError::Configuration);
    }
    let mut conn = private_import_connection(target_url)
        .await
        .map_err(asset_error)?;
    conn.batch_execute("SET statement_timeout='60s'; SELECT pg_advisory_lock(6810476213302)")
        .await
        .map_err(|_| AssetError::Database)?;
    let available = tables(&mut conn).await.map_err(asset_error)?;
    if !available.contains("legacy_import_state")
        || available
            .iter()
            .any(|t| !TARGET_TABLES.contains(&t.as_str()))
    {
        return Err(AssetError::Inventory);
    }
    if !flag(&mut conn,"SELECT count(*)=1 AND bool_and(singleton AND state='quarantined' AND format_version=1) AS value FROM public.legacy_import_state").await.map_err(asset_error)? {
        return Err(AssetError::Inventory);
    }
    if available.contains("legacy_runtime_activation")
        && count(&mut conn, "legacy_runtime_activation")
            .await
            .map_err(asset_error)?
            != 0
    {
        return Err(AssetError::Inventory);
    }
    // Older verified rehearsals can acquire the new empty embedded schema.
    // No runtime, sessions or work queue is initialized by this path.
    Database::migrate(target_url)
        .await
        .map_err(|_| AssetError::Database)?;
    conn.batch_execute("BEGIN ISOLATION LEVEL REPEATABLE READ; SET LOCAL TIME ZONE 'UTC'; SET LOCAL statement_timeout='60s'; SET LOCAL idle_in_transaction_session_timeout='120s'; SET LOCAL search_path=pg_catalog,public;").await.map_err(|_|AssetError::Database)?;
    let result = preserve_transaction(&mut conn, paths, apply).await;
    match &result {
        Ok(report) if apply && report.outcome == "preserved_quarantined" => {
            conn.batch_execute("COMMIT")
                .await
                .map_err(|_| AssetError::Database)?;
        }
        _ => {
            conn.batch_execute("ROLLBACK")
                .await
                .map_err(|_| AssetError::Database)?;
        }
    }
    result
}

// Caller owns transaction completion so activation can verify files and record
// approval under the same target locks, without a check/commit race.
pub(super) async fn preserve_transaction(
    conn: &mut AsyncPgConnection,
    paths: &Bundle,
    apply: bool,
) -> Result<Report, AssetError> {
    let fingerprint=diesel::sql_query("SELECT source_fingerprint AS name FROM public.legacy_import_state WHERE singleton AND state='quarantined' AND format_version=1")
        .get_result::<Name>(conn).await.map_err(|_|AssetError::Inventory)?.name;
    verify_retained(conn, &fingerprint)
        .await
        .map_err(asset_error)?;
    let references = reference_count(conn).await.map_err(asset_error)?;
    verify_manifest_metadata(conn, &fingerprint)
        .await
        .map_err(asset_error)?;
    let existing = manifest(conn).await.map_err(asset_error)?.is_some();
    // Reject oversized references inside PG, before a keyset page is allocated.
    if flag(conn,&format!("{REFERENCES} SELECT EXISTS(SELECT 1 FROM asset_references WHERE octet_length(object_key) NOT BETWEEN 1 AND 1024) AS value")).await.map_err(asset_error)? {
        return Err(AssetError::UnsafePath);
    }
    // A Windows export may already have lost distinct case-sensitive S3 keys.
    // Do not impose this restriction on the intended case-sensitive Linux export.
    if cfg!(windows) && flag(conn,&format!("{REFERENCES} SELECT EXISTS(SELECT 1 FROM (SELECT DISTINCT object_key FROM asset_references) keys GROUP BY lower(object_key COLLATE \"C\") HAVING count(*)>1) AS value")).await.map_err(asset_error)? {
        return Err(AssetError::Inventory);
    }
    let mut cursor = None;
    let mut report = Report {
        outcome: if existing {
            "already_preserved_verified"
        } else if apply {
            "preserved_quarantined"
        } else {
            "dry_run_verified"
        },
        references,
        objects: 0,
        bytes: 0,
        copied_objects: 0,
        runtime_enabled: false,
    };
    loop {
        let page = reference_page(conn, cursor.as_deref()).await?;
        if page.is_empty() {
            break;
        }
        let stored:BTreeMap<_,_>=diesel::sql_query("SELECT object_key,sha256,byte_count FROM public.legacy_asset_files WHERE object_key=ANY($1)")
            .bind::<Array<Text>,_>(&page).load::<Stored>(conn).await.map_err(|_|AssetError::Database)?
            .into_iter().map(|r|(r.object_key,(r.sha256,r.byte_count))).collect();
        for name in &page {
            legacy_assets::key(name)?;
            let expected = stored.get(name).cloned();
            if existing != expected.is_some() {
                return Err(AssetError::Inventory);
            }
            let (hash, bytes, copied) = async {
                let object = paths.read(name).await?;
                let size = object.bytes.len() as i64;
                let copied = if let Some((hash, len)) = expected {
                    if hash != object.hash || len != size {
                        return Err(AssetError::Changed);
                    }
                    paths.verify(&object).await?;
                    false
                } else if apply {
                    paths.put(&object).await?
                } else {
                    paths.check_destination(&object).await?;
                    false
                };
                Ok::<_, AssetError>((object.hash, size, copied))
            }
            .await?;
            if apply && !existing {
                diesel::sql_query("INSERT INTO public.legacy_asset_files(object_key,sha256,byte_count) VALUES($1,$2,$3)")
                    .bind::<Text,_>(name).bind::<Text,_>(&hash).bind::<BigInt,_>(bytes)
                    .execute(conn).await.map_err(|_|AssetError::Database)?;
            }
            report.objects += 1;
            report.bytes = report
                .bytes
                .checked_add(bytes)
                .ok_or(AssetError::Inventory)?;
            report.copied_objects += i64::from(copied);
        }
        cursor = page.last().cloned();
    }
    if apply && !existing {
        diesel::sql_query("INSERT INTO public.stored_files(object_key,sha256,byte_count) SELECT object_key,sha256,byte_count FROM public.legacy_asset_files").execute(conn).await.map_err(|_|AssetError::Database)?;
        diesel::sql_query("INSERT INTO public.legacy_asset_manifest(singleton,source_fingerprint,object_count,byte_count) VALUES(true,$1,$2,$3)")
            .bind::<Text,_>(&fingerprint).bind::<BigInt,_>(report.objects).bind::<BigInt,_>(report.bytes)
            .execute(conn).await.map_err(|_|AssetError::Database)?;
        verify_manifest_metadata(conn, &fingerprint)
            .await
            .map_err(asset_error)?;
    }
    Ok(report)
}
