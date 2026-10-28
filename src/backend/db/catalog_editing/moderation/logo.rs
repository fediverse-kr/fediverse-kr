//! Storage guard -> admin gate -> member/session -> catalog -> mapping locks.
//! Durable orphan intent precedes IO. Catalog/mapping/audit/old-key retirement
//! then commit together; neither HTTP cancellation nor PG loss abandons IO locks.
use super::*;
use crate::backend::{
    catalog_logos::Logo,
    storage::{self, ObjectStore},
};
use crate::directory::SoftwareMediaMetadata;
use serde_json::{json, Value};

#[derive(QueryableByName)]
struct CurrentMedia {
    #[diesel(sql_type = Nullable<Text>)]
    key: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    metadata: Option<String>,
}

fn metadata_value(value: Option<&SoftwareMediaMetadata>) -> Result<Option<String>, Error> {
    value
        .map(|value| serde_json::to_string(value).map_err(|_| Error::Invalid))
        .transpose()
}

fn metadata_equal(current: Option<&str>, next: Option<&str>) -> bool {
    match (current, next) {
        (None, None) => true,
        (Some(a), Some(b)) => serde_json::from_str::<Value>(a).ok() == serde_json::from_str(b).ok(),
        _ => false,
    }
}

async fn current_media(
    conn: &mut AsyncPgConnection,
    id: i64,
    role: dto::MediaRole,
) -> Result<CurrentMedia, Error> {
    let sql = match role {
        dto::MediaRole::Logo => {
            "SELECT s.logo_key AS key,t.logo_metadata::text AS metadata FROM catalog_software s LEFT JOIN catalog_software_state t ON t.software_id=s.id WHERE s.id=$1"
        }
        dto::MediaRole::Header => {
            "SELECT t.header_key AS key,t.header_metadata::text AS metadata FROM catalog_software_state t WHERE t.software_id=$1"
        }
    };
    diesel::sql_query(sql)
        .bind::<BigInt, _>(id)
        .get_result(conn)
        .await
        .optional()?
        .ok_or(Error::Missing)
}

async fn operator_audit(
    conn: &mut AsyncPgConnection,
    software_id: i64,
    revision: i64,
    action: &str,
    note: &str,
    before: &Value,
    after: &Value,
) -> Result<(), Error> {
    diesel::sql_query("INSERT INTO catalog_admin_events(software_id,revision,action,note,before_value,after_value) VALUES($1,$2,$3,$4,$5::jsonb,$6::jsonb)")
        .bind::<BigInt, _>(software_id)
        .bind::<BigInt, _>(revision)
        .bind::<Text, _>(action)
        .bind::<Text, _>(note)
        .bind::<Text, _>(before.to_string())
        .bind::<Text, _>(after.to_string())
        .execute(conn)
        .await?;
    Ok(())
}

async fn checks(
    conn: &mut AsyncPgConnection,
    s: Option<&AuthenticatedSession>,
    operator: bool,
) -> Result<(), Error> {
    if operator {
        Ok(())
    } else {
        let s = s.ok_or(Error::Forbidden)?;
        admin(conn, s, true).await?;
        writable(conn, s).await
    }
}

async fn apply_media(
    db: &Database,
    s: Option<&AuthenticatedSession>,
    operator: bool,
    store: &ObjectStore,
    request: dto::MediaRequest,
    media: Option<Logo>,
) -> Result<dto::Software, Error> {
    let mutation = store.try_mutation().map_err(|e| match e {
        storage::Error::Busy => Error::RateLimited,
        _ => Error::Unavailable,
    })?;
    let mut conn = db.pool.get().await.map_err(|_| Error::Unavailable)?;

    if let Some(media) = media.as_ref() {
        (&mut *conn).transaction::<_, Error, _>(async |conn| {
            checks(conn, s, operator).await?;
            let before = row(conn, &request.name, true).await?;
            if before.revision != request.revision {
                return Err(Error::Conflict);
            }
            diesel::sql_query("LOCK TABLE stored_files IN SHARE ROW EXCLUSIVE MODE")
                .execute(conn)
                .await?;
            let inconsistent = diesel::sql_query("SELECT EXISTS(SELECT 1 FROM stored_files WHERE sha256=$1 AND byte_count<>$2 UNION ALL SELECT 1 FROM media_deletions WHERE sha256=$1 AND byte_count<>$2) AS value")
                .bind::<Text, _>(&media.object.hash)
                .bind::<BigInt, _>(media.object.bytes)
                .get_result::<Flag>(conn)
                .await?
                .value;
            if inconsistent {
                return Err(Error::Unavailable);
            }
            let already_stored = diesel::sql_query("SELECT EXISTS(SELECT 1 FROM stored_files WHERE sha256=$1 AND byte_count=$2) AS value")
                .bind::<Text, _>(&media.object.hash)
                .bind::<BigInt, _>(media.object.bytes)
                .get_result::<Flag>(conn)
                .await?
                .value;
            if already_stored {
                return Ok(());
            }
            diesel::sql_query("INSERT INTO media_deletions(sha256,byte_count) VALUES($1,$2) ON CONFLICT DO NOTHING")
                .bind::<Text, _>(&media.object.hash)
                .bind::<BigInt, _>(media.object.bytes)
                .execute(conn)
                .await?;
            Ok(())
        })
        .await?;
    }

    let result = (&mut *conn)
        .transaction(async |conn| {
            checks(conn, s, operator).await?;
            let before = row(conn, &request.name, true).await?;
            if before.revision != request.revision {
                return Err(Error::Conflict);
            }
            diesel::sql_query("LOCK TABLE stored_files IN SHARE ROW EXCLUSIVE MODE")
                .execute(conn)
                .await?;
            diesel::sql_query("INSERT INTO catalog_software_state(software_id) VALUES($1) ON CONFLICT DO NOTHING")
                .bind::<BigInt, _>(before.id)
                .execute(conn)
                .await?;
            let current = current_media(conn, before.id, request.role).await?;
            let metadata_only = media.is_none() && request.metadata.is_some();
            if metadata_only && current.key.is_none() {
                return Err(Error::Invalid);
            }
            let next_metadata = if media.is_some() || metadata_only {
                metadata_value(request.metadata.as_ref())?
                    .or_else(|| current.metadata.clone())
            } else {
                None
            };
            let current_object = if let Some(key) = current.key.as_deref() {
                diesel::sql_query("SELECT sha256 AS hash,byte_count AS bytes FROM stored_files WHERE object_key=$1")
                    .bind::<Text, _>(key)
                    .get_result::<HashRow>(conn)
                    .await
                    .optional()?
            } else {
                None
            };
            let same_object = media.as_ref().is_some_and(|media| {
                current_object.as_ref().is_some_and(|old| {
                    old.hash == media.object.hash && old.bytes == media.object.bytes
                })
            });
            if !metadata_only && media.is_none() && current.key.is_none() && current.metadata.is_none()
                || (same_object || metadata_only) && metadata_equal(current.metadata.as_deref(), next_metadata.as_deref())
            {
                return software(conn, &request.name).await;
            }

            let old_key = current.key.clone();
            let (key, after_value) = if metadata_only {
                (current.key.clone(), json!({"present":true,"metadata":request.metadata}))
            } else if let Some(media) = media {
                let key = if same_object {
                    current.key.clone().ok_or(Error::Unavailable)?
                } else {
                    mutation
                        .publish(media.object.clone(), media.bytes)
                        .await
                        .map_err(|_| Error::Unavailable)?;
                    let prefix = match request.role {
                        dto::MediaRole::Logo => "software-logos",
                        dto::MediaRole::Header => "software-headers",
                    };
                    let key = format!("{prefix}/{}", uuid::Uuid::new_v4().simple());
                    diesel::sql_query("INSERT INTO stored_files(object_key,sha256,byte_count) VALUES($1,$2,$3)")
                        .bind::<Text, _>(&key)
                        .bind::<Text, _>(&media.object.hash)
                        .bind::<BigInt, _>(media.object.bytes)
                        .execute(conn)
                        .await?;
                    key
                };
                // Publication and its durable mapping succeeded in this same
                // transaction. Retire only the provisional orphan intent.
                diesel::sql_query("DELETE FROM media_deletions WHERE sha256=$1")
                    .bind::<Text, _>(&media.object.hash)
                    .execute(conn)
                    .await?;
                (
                    Some(key),
                    json!({"present":true,"mime":media.mime,"bytes":media.object.bytes}),
                )
            } else {
                (None, Value::Null)
            };
            if before.revision == 0 {
                diesel::sql_query("INSERT INTO catalog_software_edits(software_id,revision,action,summary,snapshot) VALUES($1,0,'baseline','기존 카탈로그',$2::jsonb)")
                    .bind::<BigInt, _>(before.id)
                    .bind::<Text, _>(&before.snapshot)
                    .execute(conn)
                    .await?;
            }
            match request.role {
                dto::MediaRole::Logo => {
                    diesel::sql_query("UPDATE catalog_software SET logo_key=$2,updated_at=now() AT TIME ZONE 'UTC' WHERE id=$1")
                        .bind::<BigInt, _>(before.id)
                        .bind::<Nullable<Text>, _>(&key)
                        .execute(conn)
                        .await?;
                    diesel::sql_query("UPDATE catalog_software_state SET logo_metadata=$2::jsonb WHERE software_id=$1")
                        .bind::<BigInt, _>(before.id)
                        .bind::<Nullable<Text>, _>(&next_metadata)
                        .execute(conn)
                        .await?;
                }
                dto::MediaRole::Header => {
                    diesel::sql_query("UPDATE catalog_software_state SET header_key=$2,header_metadata=$3::jsonb WHERE software_id=$1")
                        .bind::<BigInt, _>(before.id)
                        .bind::<Nullable<Text>, _>(&key)
                        .bind::<Nullable<Text>, _>(&next_metadata)
                        .execute(conn)
                        .await?;
                    diesel::sql_query("UPDATE catalog_software SET updated_at=now() AT TIME ZONE 'UTC' WHERE id=$1")
                        .bind::<BigInt, _>(before.id)
                        .execute(conn)
                        .await?;
                }
            }
            diesel::sql_query("UPDATE catalog_software_state SET revision=revision+1 WHERE software_id=$1")
                .bind::<BigInt, _>(before.id)
                .execute(conn)
                .await?;
            let after = row(conn, &request.name, false).await?;
            if !operator {
                append(
                    conn,
                    &after,
                    s.ok_or(Error::Forbidden)?,
                    "admin_settings",
                    match (request.role, key.is_some()) {
                        (dto::MediaRole::Logo, true) => "로고 변경",
                        (dto::MediaRole::Logo, false) => "로고 제거",
                        (dto::MediaRole::Header, true) => "헤더 변경",
                        (dto::MediaRole::Header, false) => "헤더 제거",
                    },
                )
                .await?;
                audit(
                    conn,
                    s.ok_or(Error::Forbidden)?,
                    (Some(before.id), None),
                    after.revision,
                    match request.role {
                        dto::MediaRole::Logo => "logo",
                        dto::MediaRole::Header => "header",
                    },
                    &request.note,
                    &json!({"present":old_key.is_some()}),
                    &after_value,
                )
                .await?;
            } else {
                operator_audit(
                    conn,
                    before.id,
                    after.revision,
                    match request.role {
                        dto::MediaRole::Logo => "logo",
                        dto::MediaRole::Header => "header",
                    },
                    &request.note,
                    &json!({"present":old_key.is_some()}),
                    &after_value,
                )
                .await?;
            }
            if let Some(old) = old_key {
                crate::backend::db::media_cleanup::retire_keys(conn, &[old])
                    .await
                    .map_err(|_| Error::Unavailable)?;
            }
            software(conn, &request.name).await
        })
        .await;
    drop(mutation);
    result
}

#[derive(QueryableByName)]
struct HashRow {
    #[diesel(sql_type = Text)]
    hash: String,
    #[diesel(sql_type = BigInt)]
    bytes: i64,
}

impl Database {
    pub(crate) async fn moderate_software_logo(
        &self,
        s: &AuthenticatedSession,
        store: &ObjectStore,
        request: dto::LogoRequest,
        logo: Option<Logo>,
    ) -> Result<dto::Software, Error> {
        let request = policy::logo(request)?;
        apply_media(
            self,
            Some(s),
            false,
            store,
            dto::MediaRequest {
                name: request.name,
                revision: request.revision,
                role: dto::MediaRole::Logo,
                note: request.note,
                data: request.data,
                metadata: request.metadata,
            },
            logo,
        )
        .await
    }

    pub(crate) async fn moderate_software_media(
        &self,
        s: &AuthenticatedSession,
        store: &ObjectStore,
        request: dto::MediaRequest,
        media: Option<Logo>,
    ) -> Result<dto::Software, Error> {
        apply_media(self, Some(s), false, store, request, media).await
    }

    pub(crate) async fn moderate_software_media_batch_operator(
        &self,
        store: &ObjectStore,
        name: String,
        revision: i64,
        logo: Option<(dto::MediaRequest, Logo)>,
        header: Option<(dto::MediaRequest, Logo)>,
    ) -> Result<dto::Software, Error> {
        apply_operator_batch(self, store, name, revision, logo, header).await
    }
}

async fn apply_operator_batch(
    db: &Database,
    store: &ObjectStore,
    name: String,
    revision: i64,
    logo: Option<(dto::MediaRequest, Logo)>,
    header: Option<(dto::MediaRequest, Logo)>,
) -> Result<dto::Software, Error> {
    let mutation = store.try_mutation().map_err(|e| match e {
        storage::Error::Busy => Error::RateLimited,
        _ => Error::Unavailable,
    })?;
    let mut conn = db.pool.get().await.map_err(|_| Error::Unavailable)?;
    let candidates = [
        logo.as_ref().map(|(_, v)| v),
        header.as_ref().map(|(_, v)| v),
    ];
    (&mut *conn).transaction::<_, Error, _>(async |conn| {
        let before = row(conn, &name, true).await?;
        if before.revision != revision {
            return Err(Error::Conflict);
        }
        diesel::sql_query("LOCK TABLE stored_files IN SHARE ROW EXCLUSIVE MODE")
            .execute(conn)
            .await?;
        for media in candidates.into_iter().flatten() {
            let inconsistent = diesel::sql_query("SELECT EXISTS(SELECT 1 FROM stored_files WHERE sha256=$1 AND byte_count<>$2 UNION ALL SELECT 1 FROM media_deletions WHERE sha256=$1 AND byte_count<>$2) AS value")
                .bind::<Text, _>(&media.object.hash)
                .bind::<BigInt, _>(media.object.bytes)
                .get_result::<Flag>(conn)
                .await?
                .value;
            if inconsistent {
                return Err(Error::Unavailable);
            }
            let already_stored = diesel::sql_query("SELECT EXISTS(SELECT 1 FROM stored_files WHERE sha256=$1 AND byte_count=$2) AS value")
                .bind::<Text, _>(&media.object.hash)
                .bind::<BigInt, _>(media.object.bytes)
                .get_result::<Flag>(conn)
                .await?
                .value;
            if !already_stored {
                diesel::sql_query("INSERT INTO media_deletions(sha256,byte_count) VALUES($1,$2) ON CONFLICT DO NOTHING")
                    .bind::<Text, _>(&media.object.hash)
                    .bind::<BigInt, _>(media.object.bytes)
                    .execute(conn)
                    .await?;
            }
        }
        Ok(())
    }).await?;
    let result = (&mut *conn).transaction(async |conn| {
        let before = row(conn, &name, true).await?;
        if before.revision != revision {
            return Err(Error::Conflict);
        }
        diesel::sql_query("LOCK TABLE stored_files IN SHARE ROW EXCLUSIVE MODE")
            .execute(conn)
            .await?;
        diesel::sql_query("INSERT INTO catalog_software_state(software_id) VALUES($1) ON CONFLICT DO NOTHING")
            .bind::<BigInt, _>(before.id)
            .execute(conn)
            .await?;
        let mut changed = false;
        let mut old_keys = Vec::new();
        let mut audit_after = serde_json::Map::new();
        for (role, operation) in [
            (dto::MediaRole::Logo, logo),
            (dto::MediaRole::Header, header),
        ] {
            let Some((request, media)) = operation else { continue };
            let current = current_media(conn, before.id, role).await?;
            let current_object = if let Some(key) = current.key.as_deref() {
                diesel::sql_query("SELECT sha256 AS hash,byte_count AS bytes FROM stored_files WHERE object_key=$1")
                    .bind::<Text, _>(key)
                    .get_result::<HashRow>(conn)
                    .await
                    .optional()?
            } else { None };
            let metadata = metadata_value(request.metadata.as_ref())?;
            if current_object.as_ref().is_some_and(|old| old.hash == media.object.hash && old.bytes == media.object.bytes)
                && metadata_equal(current.metadata.as_deref(), metadata.as_deref())
            {
                continue;
            }
            changed = true;
            if let Some(old) = current.key { old_keys.push(old); }
            mutation.publish(media.object.clone(), media.bytes).await.map_err(|_| Error::Unavailable)?;
            let prefix = match role { dto::MediaRole::Logo => "software-logos", dto::MediaRole::Header => "software-headers" };
            let key = format!("{prefix}/{}", uuid::Uuid::new_v4().simple());
            diesel::sql_query("INSERT INTO stored_files(object_key,sha256,byte_count) VALUES($1,$2,$3)")
                .bind::<Text, _>(&key).bind::<Text, _>(&media.object.hash).bind::<BigInt, _>(media.object.bytes)
                .execute(conn).await?;
            diesel::sql_query("DELETE FROM media_deletions WHERE sha256=$1")
                .bind::<Text, _>(&media.object.hash).execute(conn).await?;
            match role {
                dto::MediaRole::Logo => {
                    diesel::sql_query("UPDATE catalog_software SET logo_key=$2,updated_at=now() AT TIME ZONE 'UTC' WHERE id=$1")
                        .bind::<BigInt, _>(before.id).bind::<Text, _>(&key).execute(conn).await?;
                    diesel::sql_query("UPDATE catalog_software_state SET logo_metadata=$2::jsonb WHERE software_id=$1")
                        .bind::<BigInt, _>(before.id).bind::<Nullable<Text>, _>(&metadata).execute(conn).await?;
                }
                dto::MediaRole::Header => {
                    diesel::sql_query("UPDATE catalog_software_state SET header_key=$2,header_metadata=$3::jsonb WHERE software_id=$1")
                        .bind::<BigInt, _>(before.id).bind::<Text, _>(&key).bind::<Nullable<Text>, _>(&metadata).execute(conn).await?;
                    diesel::sql_query("UPDATE catalog_software SET updated_at=now() AT TIME ZONE 'UTC' WHERE id=$1")
                        .bind::<BigInt, _>(before.id).execute(conn).await?;
                }
            }
            audit_after.insert(match role { dto::MediaRole::Logo => "logo", dto::MediaRole::Header => "header" }.into(), json!({"present":true,"mime":media.mime,"bytes":media.object.bytes}));
        }
        if !changed { return software(conn, &name).await; }
        if before.revision == 0 {
            diesel::sql_query("INSERT INTO catalog_software_edits(software_id,revision,action,summary,snapshot) VALUES($1,0,'baseline','기존 카탈로그',$2::jsonb)")
                .bind::<BigInt, _>(before.id).bind::<Text, _>(&before.snapshot).execute(conn).await?;
        }
        diesel::sql_query("UPDATE catalog_software_state SET revision=revision+1 WHERE software_id=$1")
            .bind::<BigInt, _>(before.id).execute(conn).await?;
        let after = row(conn, &name, false).await?;
        operator_audit(conn, before.id, after.revision, "media_import", "operator software media import", &json!({}), &Value::Object(audit_after)).await?;
        if !old_keys.is_empty() { crate::backend::db::media_cleanup::retire_keys(conn, &old_keys).await.map_err(|_| Error::Unavailable)?; }
        software(conn, &name).await
    }).await;
    drop(mutation);
    result
}
