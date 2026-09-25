//! Fixed SQL is limited to this offline adapter: the source is a different
//! application's schema. No schema names or SQL come from a dump or CLI input.
use super::{connect, Database};
use crate::backend::{
    federation::{http_signature::RsaHttpSigner, webfinger::AccountHandle},
    legacy::{ImportError as Error, ImportMode, ImportReport, TableCount},
};
use diesel::{
    prelude::*,
    sql_types::{BigInt, Bool, Nullable, Text},
};
use diesel_async::{AsyncPgConnection, RunQueryDsl, SimpleAsyncConnection};
use rsa::{pkcs8::DecodePublicKey, RsaPublicKey};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
pub(super) mod activation;
pub(super) mod assets;

struct Mapping {
    source: &'static str,
    target: &'static str,
    id_type: &'static str,
}
const TABLES: &[Mapping] = &[
    Mapping {
        source: "users",
        target: "legacy_members",
        id_type: "uuid",
    },
    Mapping {
        source: "servers",
        target: "legacy_sites",
        id_type: "uuid",
    },
    Mapping {
        source: "software_categories",
        target: "catalog_categories",
        id_type: "bigint",
    },
    Mapping {
        source: "softwares",
        target: "catalog_software",
        id_type: "bigint",
    },
    Mapping {
        source: "comments",
        target: "community_comments",
        id_type: "uuid",
    },
    Mapping {
        source: "reports",
        target: "community_reports",
        id_type: "uuid",
    },
    Mapping {
        source: "health_checks",
        target: "legacy_health_checks",
        id_type: "uuid",
    },
    Mapping {
        source: "instance_keys",
        target: "legacy_instance_keys",
        id_type: "uuid",
    },
];
const EXCLUDED: &[&str] = &[
    "schema_migrations",
    "verifications",
    "oban_jobs",
    "oban_peers",
];
const TARGET_TABLES: &[&str] = &[
    "__diesel_schema_migrations",
    "member_users",
    "member_profile_media",
    "profile_refresh_batches",
    "profile_refresh_jobs",
    "member_sessions",
    "member_linked_accounts",
    "member_link_challenges",
    "member_auth_rate_limits",
    "federation_instance_keys",
    "directory_sites",
    "directory_observations",
    "directory_jobs",
    "directory_health_checks",
    "directory_icons",
    "directory_site_details",
    "directory_owner_challenges",
    "directory_site_edits",
    "directory_site_registrations",
    "maintenance_schedule",
    "legacy_members",
    "member_legacy_claims",
    "legacy_sites",
    "catalog_categories",
    "catalog_software",
    "catalog_software_state",
    "catalog_software_edits",
    "catalog_category_state",
    "catalog_admin_events",
    "community_comments",
    "community_reports",
    "community_report_evidence",
    "member_admin_roles",
    "moderation_events",
    "legacy_health_checks",
    "legacy_instance_keys",
    "legacy_import_state",
    "legacy_asset_manifest",
    "legacy_asset_files",
    "stored_files",
    "media_deletions",
    "legacy_runtime_activation",
];
const PAGE_SIZE: i64 = 256;
const MAX_ROW_BYTES: usize = 1024 * 1024;
const MAX_PAGE_BYTES: usize = 16 * 1024 * 1024;

impl From<diesel::result::Error> for Error {
    fn from(_: diesel::result::Error) -> Self {
        Self::Database
    }
}

async fn private_import_connection(url: &str) -> Result<AsyncPgConnection, Error> {
    let mut conn = connect(url).await.map_err(|_| Error::Connection)?;
    let expected = url::Url::parse(url).map_err(|_| Error::Configuration)?;
    let actual = diesel::sql_query("SELECT current_database()::text AS name")
        .get_result::<Name>(&mut conn)
        .await?;
    if actual.name != expected.path().trim_start_matches('/')
        || !flag(
            &mut conn,
            "SELECT inet_server_addr()='127.0.0.1'::inet AS value",
        )
        .await?
    {
        return Err(Error::Configuration);
    }
    // A constraint failure can otherwise send a full bound JSON page to the
    // PostgreSQL log even though the CLI redacts its error. Fail closed if this
    // disposable cluster role cannot disable statement / parameter error logs.
    conn.batch_execute("SET log_statement='none'; SET log_min_error_statement='panic'; SET log_min_messages='panic'; SET log_parameter_max_length=0; SET log_parameter_max_length_on_error=0;").await?;
    Ok(conn)
}

#[derive(QueryableByName)]
struct Name {
    #[diesel(sql_type = Text)]
    name: String,
}
#[derive(QueryableByName)]
struct Number {
    #[diesel(sql_type = BigInt)]
    value: i64,
}
#[derive(QueryableByName)]
struct Flag {
    #[diesel(sql_type = Bool)]
    value: bool,
}
#[derive(QueryableByName)]
struct Row {
    #[diesel(sql_type = Text)]
    cursor: String,
    #[diesel(sql_type = Text)]
    payload: String,
}
#[derive(QueryableByName, PartialEq, Eq)]
struct Column {
    #[diesel(sql_type = Text)]
    name: String,
    #[diesel(sql_type = Text)]
    kind: String,
}

async fn tables(conn: &mut AsyncPgConnection) -> Result<BTreeSet<String>, Error> {
    Ok(diesel::sql_query("SELECT table_name::text AS name FROM information_schema.tables WHERE table_schema='public'")
        .load::<Name>(conn).await?.into_iter().map(|n| n.name).collect())
}
async fn columns(conn: &mut AsyncPgConnection, table: &str) -> Result<Vec<Column>, Error> {
    Ok(diesel::sql_query("SELECT column_name::text AS name, CASE udt_name WHEN 'varchar' THEN 'text' WHEN '_varchar' THEN '_text' ELSE udt_name END::text AS kind FROM information_schema.columns WHERE table_schema='public' AND table_name=$1 ORDER BY column_name")
        .bind::<Text,_>(table).load(conn).await?)
}
async fn count(conn: &mut AsyncPgConnection, table: &str) -> Result<i64, Error> {
    // All callers use table names from the constants above, never external SQL.
    Ok(
        diesel::sql_query(format!("SELECT count(*) AS value FROM public.{table}"))
            .get_result::<Number>(conn)
            .await?
            .value,
    )
}
async fn flag(conn: &mut AsyncPgConnection, query: &str) -> Result<bool, Error> {
    Ok(diesel::sql_query(query)
        .get_result::<Flag>(conn)
        .await?
        .value)
}

#[cfg(test)]
pub(super) async fn handle_reserved(
    conn: &mut AsyncPgConnection,
    handle: &str,
) -> Result<bool, super::StoreError> {
    Ok(diesel::sql_query(
        "SELECT EXISTS(SELECT 1 FROM public.member_legacy_claims WHERE handle=$1) AS value",
    )
    .bind::<Text, _>(handle.to_lowercase())
    .get_result::<Flag>(conn)
    .await?
    .value)
}

/// Called BEFORE automatic migrations and before key / HTTP / worker startup.
/// Prefix guard also covers a restored Phoenix database without our marker.
pub(crate) async fn ensure_runtime_allowed(
    url: &str,
    origin: Option<&str>,
) -> Result<(), super::StoreError> {
    let parsed = url::Url::parse(url).map_err(|_| super::StoreError)?;
    if parsed.path().starts_with("/fedkr_snapshot_")
        || parsed.path().starts_with("/fedkr_rehearsal_")
    {
        return Err(super::StoreError);
    }
    let mut conn = connect(url).await.map_err(|_| super::StoreError)?;
    let actual_name = diesel::sql_query("SELECT current_database()::text AS name")
        .get_result::<Name>(&mut conn)
        .await
        .map_err(|_| super::StoreError)?
        .name;
    if actual_name.starts_with("fedkr_snapshot_") || actual_name.starts_with("fedkr_rehearsal_") {
        return Err(super::StoreError);
    }
    let requires_approval = actual_name.starts_with("fedkr_live_");
    // A renamed rehearsal needs explicit approval. Restored Phoenix tables are
    // rejected, instead of allowing the app to silently migrate the source.
    if flag(&mut conn, "SELECT to_regclass('public.users') IS NOT NULL OR to_regclass('public.servers') IS NOT NULL AS value")
        .await.map_err(|_| super::StoreError)? {
        return Err(super::StoreError);
    }
    if flag(
        &mut conn,
        "SELECT to_regclass('public.legacy_import_state') IS NOT NULL AS value",
    )
    .await
    .map_err(|_| super::StoreError)?
    {
        if count(&mut conn, "legacy_import_state")
            .await
            .map_err(|_| super::StoreError)?
            != 0
        {
            if !activation::runtime_permitted(&mut conn, origin)
                .await
                .map_err(|_| super::StoreError)?
            {
                return Err(super::StoreError);
            }
        } else {
            if requires_approval {
                return Err(super::StoreError);
            }
            // Removing a ledger is not an activation method. Fresh installs have
            // these empty tables; ordinary new members do not write legacy rows.
            for name in [
                "legacy_members",
                "legacy_sites",
                "legacy_health_checks",
                "legacy_instance_keys",
            ] {
                if count(&mut conn, name)
                    .await
                    .map_err(|_| super::StoreError)?
                    != 0
                {
                    return Err(super::StoreError);
                }
            }
        }
    } else if requires_approval {
        return Err(super::StoreError);
    }
    Ok(())
}

pub(crate) async fn import_snapshot(
    source_url: &str,
    target_url: &str,
    mode: ImportMode,
) -> Result<ImportReport, Error> {
    // Keep this check at the persistence entry too, so a future caller cannot
    // bypass the CLI's environment validation.
    if !crate::backend::legacy::isolated_url(source_url, "fedkr_snapshot_")
        || !crate::backend::legacy::isolated_url(target_url, "fedkr_rehearsal_")
    {
        return Err(Error::Configuration);
    }
    let mut source = private_import_connection(source_url).await?;
    source.batch_execute("BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY; SET LOCAL TIME ZONE 'UTC'; SET LOCAL statement_timeout='60s'; SET LOCAL idle_in_transaction_session_timeout='120s'; SET LOCAL search_path=pg_catalog,public;").await?;
    let mut target = private_import_connection(target_url).await?;
    // Session lock includes schema preparation and the preflight emptiness
    // check. A competing importer cannot change the destination between them.
    target
        .batch_execute("SET statement_timeout='60s'; SELECT pg_advisory_lock(6810476213302)")
        .await?;
    let result = async {
        let source_tables = tables(&mut source).await?;
        if TABLES.iter().any(|m| !source_tables.contains(m.source))
            || source_tables.iter().any(|t| !TABLES.iter().any(|m| m.source == t) && !EXCLUDED.contains(&t.as_str())) {
            return Err(Error::Schema);
        }
        let target_tables = tables(&mut target).await?;
        if target_tables.iter().any(|t| !TARGET_TABLES.contains(&t.as_str())) {
            return Err(Error::TargetNotEmpty);
        }
        let prior_import = target_tables.contains("legacy_import_state")
            && count(&mut target,"legacy_import_state").await? == 1;
        if target_tables.contains("legacy_runtime_activation") && count(&mut target,"legacy_runtime_activation").await? != 0 {
            return Err(Error::TargetNotEmpty);
        }
        if !prior_import {
            for name in &target_tables {
                if name != "__diesel_schema_migrations" && count(&mut target,name).await? != 0 {
                    return Err(Error::TargetNotEmpty);
                }
            }
        }
        // Schema preparation only. Dry-run rolls back ALL transferred rows and
        // the ledger, but intentionally leaves empty embedded tables reusable.
        Database::migrate(target_url).await.map_err(|_| Error::Database)?;
        target.batch_execute("BEGIN; SET LOCAL TIME ZONE 'UTC'; SET LOCAL statement_timeout='60s'; SET LOCAL idle_in_transaction_session_timeout='120s'; SET LOCAL search_path=pg_catalog,public;").await?;
        for m in TABLES {
            if columns(&mut source, m.source).await? != columns(&mut target, m.target).await? {
                return Err(Error::Schema);
            }
            let valid = diesel::sql_query("SELECT EXISTS (SELECT 1 FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace JOIN pg_index i ON i.indrelid=c.oid JOIN pg_attribute a ON a.attrelid=c.oid AND a.attnum=ANY(i.indkey) WHERE n.nspname='public' AND c.relname=$1 AND c.relkind='r' AND i.indisprimary AND i.indnkeyatts=1 AND a.attname='id') AS value")
                .bind::<Text,_>(m.source).get_result::<Flag>(&mut source).await?.value;
            if !valid { return Err(Error::Schema); }
        }
        let ledger = diesel::sql_query("SELECT source_fingerprint AS name FROM public.legacy_import_state WHERE singleton")
            .get_result::<Name>(&mut target).await.optional()?;
        if ledger.is_none() {
            for name in TARGET_TABLES.iter().filter(|t| **t != "__diesel_schema_migrations") {
                if count(&mut target, name).await? != 0 { return Err(Error::TargetNotEmpty); }
            }
        }
        let mut digest = Sha256::new();
        let mut counts = Vec::new();
        for m in TABLES {
            let rows = transfer_table(&mut source, &mut target, m, ledger.is_none(), &mut digest).await?;
            counts.push(TableCount { table: m.source, rows });
        }
        let fingerprint = format!("{:x}", digest.finalize());
        if ledger.as_ref().is_some_and(|old| old.name != fingerprint) { return Err(Error::Mismatch); }
        let reserved = prepare_identities(&mut target, ledger.is_none()).await?;
        validate_key(&mut target).await?;
        if ledger.is_none() {
            for p in PROJECTIONS {
                diesel::sql_query(format!("INSERT INTO public.{} ({}) {}", p.table, p.columns, p.select))
                    .execute(&mut target).await.map_err(|_| Error::Data)?;
            }
            target.batch_execute("SET CONSTRAINTS ALL IMMEDIATE").await.map_err(|_| Error::Data)?;
        }
        validate_projection(&mut target).await?;
        assets::verify_manifest_metadata(&mut target,&fingerprint).await?;
        let report = ImportReport {
            outcome: if ledger.is_some() { "already_imported_verified" } else if mode == ImportMode::Apply { "applied_quarantined" } else { "dry_run_rolled_back" },
            tables: counts, reserved_members: reserved,
            display_names_adapted: diesel::sql_query("SELECT count(*) AS value FROM public.legacy_members WHERE display_name IS NULL OR display_name='' OR char_length(display_name)>64")
                .get_result::<Number>(&mut target).await?.value,
            external_asset_references: assets::reference_count(&mut target).await?,
            retained_signing_keys: count(&mut target, "legacy_instance_keys").await?,
            runtime_enabled: false,
        };
        if ledger.is_none() && mode == ImportMode::Apply {
            diesel::sql_query("INSERT INTO public.legacy_import_state(singleton,source_fingerprint,format_version,state) VALUES(true,$1,1,'quarantined')")
                .bind::<Text,_>(&fingerprint).execute(&mut target).await?;
        }
        if ledger.is_some() || mode == ImportMode::DryRun { target.batch_execute("ROLLBACK").await?; }
        else { target.batch_execute("COMMIT").await?; }
        Ok(report)
    }.await;
    // Explicit cleanup also covers schema, key and FK validation failures. Raw
    // database errors must never be printed (they can contain a full source row).
    if result.is_err() {
        let _ = target.batch_execute("ROLLBACK").await;
    }
    let _ = source.batch_execute("ROLLBACK").await;
    result
}

async fn read_page(
    conn: &mut AsyncPgConnection,
    table: &str,
    id_type: &str,
    cursor: &Option<String>,
) -> Result<Vec<Row>, Error> {
    // Enforce byte limits before transferring the payload to Rust, not only
    // after allocating the page. An over-limit row must fail, never be skipped.
    let page: Vec<Row> = diesel::sql_query(format!("WITH page AS (SELECT id,to_jsonb(t)::text AS data FROM public.{table} t WHERE ($1::{id_type} IS NULL OR id>$1::{id_type}) ORDER BY id LIMIT $2) SELECT id::text AS cursor,CASE WHEN octet_length(data)<=$3 AND sum(octet_length(data)) OVER()<=$4 THEN data ELSE '' END AS payload FROM page ORDER BY id"))
        .bind::<Nullable<Text>,_>(cursor.as_deref()).bind::<BigInt,_>(PAGE_SIZE)
        .bind::<BigInt,_>(MAX_ROW_BYTES as i64).bind::<BigInt,_>(MAX_PAGE_BYTES as i64).load(conn).await?;
    if page.iter().any(|r| r.payload.is_empty()) {
        return Err(Error::Data);
    }
    Ok(page)
}

async fn transfer_table(
    source: &mut AsyncPgConnection,
    target: &mut AsyncPgConnection,
    m: &Mapping,
    write: bool,
    digest: &mut Sha256,
) -> Result<i64, Error> {
    let mut cursor = None;
    let expected = count(source, m.source).await?;
    let mut transferred = 0;
    digest.update((m.source.len() as u64).to_be_bytes());
    digest.update(m.source.as_bytes());
    loop {
        let page = read_page(source, m.source, m.id_type, &cursor).await?;
        if page.is_empty() {
            break;
        }
        if page.iter().any(|r| r.payload.len() > MAX_ROW_BYTES)
            || page.iter().map(|r| r.payload.len()).sum::<usize>() > MAX_PAGE_BYTES
        {
            return Err(Error::Data);
        }
        let previous = cursor.clone();
        for row in &page {
            digest.update((row.payload.len() as u64).to_be_bytes());
            digest.update(row.payload.as_bytes());
        }
        if write {
            let payload = format!(
                "[{}]",
                page.iter()
                    .map(|r| r.payload.as_str())
                    .collect::<Vec<_>>()
                    .join(",")
            );
            diesel::sql_query(format!("INSERT INTO public.{} SELECT * FROM jsonb_populate_recordset(NULL::public.{},$1::jsonb)", m.target,m.target))
                .bind::<Text,_>(&payload).execute(target).await.map_err(|_| Error::Data)?;
        }
        // Compare every retained field after the PostgreSQL type conversion,
        // including NULLs, timestamps, private profile fields and key material.
        let retained = read_page(target, m.target, m.id_type, &previous).await?;
        if page.len() != retained.len()
            || page
                .iter()
                .zip(&retained)
                .any(|(a, b)| a.cursor != b.cursor || a.payload != b.payload)
        {
            return Err(Error::Mismatch);
        }
        transferred += page.len() as i64;
        cursor = page.last().map(|r| r.cursor.clone());
    }
    if transferred != expected || count(target, m.target).await? != expected {
        return Err(Error::Mismatch);
    }
    Ok(transferred)
}

async fn prepare_identities(conn: &mut AsyncPgConnection, write: bool) -> Result<i64, Error> {
    let mut cursor = None;
    let mut total = 0;
    loop {
        let page = read_page(conn, "legacy_members", "uuid", &cursor).await?;
        if page.is_empty() {
            break;
        }
        for row in &page {
            let v: serde_json::Value =
                serde_json::from_str(&row.payload).map_err(|_| Error::Data)?;
            let handle =
                AccountHandle::parse(v["fediverse_handle"].as_str().ok_or(Error::Identity)?)
                    .map_err(|_| Error::Identity)?;
            let domain = crate::backend::directory::canonical_domain(
                v["fediverse_domain"].as_str().ok_or(Error::Identity)?,
            )
            .map_err(|_| Error::Identity)?;
            if handle.domain != domain {
                return Err(Error::Identity);
            }
            let canonical = handle.display().to_lowercase();
            if write {
                // Deferred FK is necessary until the membership projection is inserted.
                diesel::sql_query(
                    "INSERT INTO public.member_legacy_claims(member_id,handle) VALUES($1::uuid,$2)",
                )
                .bind::<Text, _>(&row.cursor)
                .bind::<Text, _>(&canonical)
                .execute(conn)
                .await
                .map_err(|_| Error::Identity)?;
            } else {
                let found = diesel::sql_query("SELECT EXISTS(SELECT 1 FROM public.member_legacy_claims WHERE member_id=$1::uuid AND handle=$2 AND actor_url IS NULL AND claimed_at IS NULL) AS value")
                    .bind::<Text,_>(&row.cursor).bind::<Text,_>(&canonical).get_result::<Flag>(conn).await?.value;
                if !found {
                    return Err(Error::Mismatch);
                }
            }
            total += 1;
        }
        cursor = page.last().map(|r| r.cursor.clone());
    }
    if count(conn, "member_legacy_claims").await? != total {
        return Err(Error::Mismatch);
    }
    Ok(total)
}

async fn validate_key(conn: &mut AsyncPgConnection) -> Result<(), Error> {
    let page = read_page(conn, "legacy_instance_keys", "uuid", &None).await?;
    if page.len() > 1 {
        return Err(Error::SigningKey);
    }
    for row in page {
        let v: serde_json::Value =
            serde_json::from_str(&row.payload).map_err(|_| Error::SigningKey)?;
        let private = v["private_key_pem"].as_str().ok_or(Error::SigningKey)?;
        let public = v["public_key_pem"].as_str().ok_or(Error::SigningKey)?;
        let signer = RsaHttpSigner::from_pem("https://fediverse.kr/actor#main-key".into(), private)
            .map_err(|_| Error::SigningKey)?;
        let derived = RsaPublicKey::from_public_key_pem(
            &signer.public_key_pem().map_err(|_| Error::SigningKey)?,
        )
        .map_err(|_| Error::SigningKey)?;
        // Match the private-key parser's Phoenix compatibility without
        // rewriting the retained source value.
        let supplied = RsaPublicKey::from_public_key_pem(public.trim_end_matches(['\r', '\n']))
            .map_err(|_| Error::SigningKey)?;
        if derived != supplied {
            return Err(Error::SigningKey);
        }
    }
    Ok(())
}

struct Projection {
    table: &'static str,
    columns: &'static str,
    select: &'static str,
}
const PROJECTIONS: &[Projection] = &[
    Projection { table: "directory_site_details", columns: "site_id,owner_id,owner_method,rules,language,tags,invite_only,approval_required,owner_comment,revision,updated_at,refresh_requested_at,refresh_job_id", select:
        "SELECT id,admin_user_id,admin_verified_via,rules,language,tags,invite_only,approval_required,admin_comment,0::bigint,updated_at AT TIME ZONE 'UTC',NULL::timestamptz,NULL::uuid FROM public.legacy_sites" },
    Projection { table: "member_users", columns: "id,login_id,display_name,password_hash,is_banned,created_at,updated_at", select:
        "SELECT id,NULL::text,left(coalesce(nullif(display_name,''),fediverse_handle),64),NULL::text,coalesce(is_banned,false),inserted_at AT TIME ZONE 'UTC',updated_at AT TIME ZONE 'UTC' FROM public.legacy_members" },
    Projection { table: "directory_sites", columns: "id,domain,name,description,is_hidden,is_force_hidden,is_closed,created_at,updated_at", select:
        "SELECT id,domain,name,description,coalesce(is_hidden,false),is_force_hidden,coalesce(is_closed,false),inserted_at AT TIME ZONE 'UTC',updated_at AT TIME ZONE 'UTC' FROM public.legacy_sites" },
    Projection { table: "directory_observations", columns: "site_id,is_alive,response_time_ms,status_code,health_error,checked_at,software,software_version,observed_name,observed_description,registration_open,user_count,active_user_count,status_count,nodeinfo_checked_at,nodeinfo_error,avg_response_time_7d", select:
        "SELECT id,is_alive,response_time_ms,NULL::integer,NULL::text,last_checked_at AT TIME ZONE 'UTC',software,software_version,NULL::text,NULL::text,registration_open,user_count::bigint,active_user_count::bigint,status_count::bigint,NULL::timestamptz,'Legacy snapshot; NodeInfo observation time unknown'::text,avg_response_time_7d FROM public.legacy_sites WHERE is_alive IS NOT NULL AND last_checked_at IS NOT NULL" },
    Projection { table: "directory_health_checks", columns: "job_id,site_id,is_alive,response_time_ms,status_code,error,checked_at", select:
        "SELECT id,server_id,is_alive,response_time_ms,status_code,error,checked_at AT TIME ZONE 'UTC' FROM public.legacy_health_checks" },
    Projection { table: "federation_instance_keys", columns: "singleton,private_key_pem,created_at", select:
        "SELECT true,private_key_pem,inserted_at AT TIME ZONE 'UTC' FROM public.legacy_instance_keys" },
];

async fn validate_projection(conn: &mut AsyncPgConnection) -> Result<(), Error> {
    for p in PROJECTIONS {
        let q = format!("SELECT NOT EXISTS ((SELECT {} FROM public.{} EXCEPT {}) UNION ALL ({} EXCEPT SELECT {} FROM public.{})) AS value",p.columns,p.table,p.select,p.select,p.columns,p.table);
        if !flag(conn, &q).await? {
            return Err(Error::Mismatch);
        }
    }
    for name in [
        "member_profile_media",
        "profile_refresh_batches",
        "profile_refresh_jobs",
        "member_sessions",
        "member_linked_accounts",
        "member_link_challenges",
        "member_auth_rate_limits",
        "directory_jobs",
        "directory_icons",
        "maintenance_schedule",
        "directory_owner_challenges",
        "directory_site_edits",
        "directory_site_registrations",
        "catalog_software_state",
        "catalog_software_edits",
        "catalog_category_state",
        "catalog_admin_events",
        "community_report_evidence",
        "member_admin_roles",
        "moderation_events",
        "media_deletions",
    ] {
        if count(conn, name).await? != 0 {
            return Err(Error::Mismatch);
        }
    }
    if flag(conn,"SELECT EXISTS(SELECT 1 FROM public.community_comments c JOIN public.community_comments p ON p.id=c.parent_id WHERE c.server_id<>p.server_id OR c.id=p.id) AS value").await? {
        return Err(Error::Data);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
