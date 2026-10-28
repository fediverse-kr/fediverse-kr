//! Offline, data-only software media import. The manifest is operator input;
//! media publication still goes through ObjectStore and the catalog mutation fence.
use super::{catalog_logos, db::Database};
use crate::{
    directory::SoftwareMediaMetadata,
    moderation::catalog::{MediaRequest, MediaRole},
};
use base64::{engine::general_purpose::STANDARD, Engine};
use diesel::{
    prelude::*,
    sql_types::{BigInt, Nullable, Text},
};
use diesel_async::RunQueryDsl;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fs,
    io::Read,
    path::{Component, Path, PathBuf},
};

const MANIFEST_MAX_BYTES: u64 = 2 * 1024 * 1024;
const MAX_ENTRIES: usize = 500;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    version: u32,
    entries: Vec<Entry>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    name: String,
    revision: i64,
    logo: Option<Asset>,
    header: Option<Asset>,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Asset {
    file: String,
    sha256: String,
    metadata: SoftwareMediaMetadata,
}
struct PreparedAsset {
    bytes: Vec<u8>,
    object: catalog_logos::Logo,
    metadata: SoftwareMediaMetadata,
}
struct PreparedEntry {
    entry: Entry,
    logo: Option<PreparedAsset>,
    header: Option<PreparedAsset>,
    noop: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ImportReport {
    pub dry_run: bool,
    pub entries: usize,
    pub assets: usize,
    pub noops: usize,
}

#[derive(QueryableByName)]
struct LiveRow {
    #[diesel(sql_type=Text)]
    name: String,
    #[diesel(sql_type=BigInt)]
    revision: i64,
    #[diesel(sql_type=Nullable<Text>)]
    logo_key: Option<String>,
    #[diesel(sql_type=Nullable<Text>)]
    logo_hash: Option<String>,
    #[diesel(sql_type=Nullable<BigInt>)]
    logo_bytes: Option<i64>,
    #[diesel(sql_type=Nullable<Text>)]
    logo_metadata: Option<String>,
    #[diesel(sql_type=Nullable<Text>)]
    header_key: Option<String>,
    #[diesel(sql_type=Nullable<Text>)]
    header_hash: Option<String>,
    #[diesel(sql_type=Nullable<BigInt>)]
    header_bytes: Option<i64>,
    #[diesel(sql_type=Nullable<Text>)]
    header_metadata: Option<String>,
}

fn fail(message: &str) -> String {
    message.to_owned()
}
fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value.bytes().all(|b| b.is_ascii_hexdigit())
        && value.bytes().all(|b| !b.is_ascii_uppercase())
}
fn safe_relative(root: &Path, value: &str) -> Result<PathBuf, String> {
    let path = Path::new(value);
    if value.is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(fail(
            "software media manifest contains an unsafe relative file path",
        ));
    }
    let mut current = root.to_path_buf();
    for component in path.components() {
        let Component::Normal(part) = component else {
            return Err(fail("software media manifest contains an unsafe path"));
        };
        current.push(part);
        let metadata =
            fs::symlink_metadata(&current).map_err(|_| fail("software media asset is missing"))?;
        if metadata.file_type().is_symlink() || (!metadata.is_file() && !metadata.is_dir()) {
            return Err(fail(
                "software media asset path contains a symlink or non-regular component",
            ));
        }
    }
    if !current.is_file() {
        return Err(fail("software media asset is not a regular file"));
    }
    Ok(current)
}
fn read_bounded(path: &Path, max: u64) -> Result<Vec<u8>, String> {
    let file =
        fs::File::open(path).map_err(|_| fail("software media asset could not be opened"))?;
    let mut bytes = Vec::new();
    file.take(max + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| fail("software media asset could not be read"))?;
    if bytes.is_empty() || bytes.len() as u64 > max {
        return Err(fail("software media asset exceeds its role size limit"));
    }
    Ok(bytes)
}
fn metadata_matches(current: Option<&str>, value: &SoftwareMediaMetadata) -> Result<bool, String> {
    Ok(
        current.and_then(|v| serde_json::from_str::<serde_json::Value>(v).ok())
            == Some(
                serde_json::to_value(value)
                    .map_err(|_| fail("software media metadata could not be encoded"))?,
            ),
    )
}

async fn live_rows(db: &Database) -> Result<HashMap<String, LiveRow>, String> {
    let mut conn = db
        .pool
        .get()
        .await
        .map_err(|_| fail("database unavailable"))?;
    let rows = diesel::sql_query("SELECT s.name,coalesce(t.revision,0) AS revision,s.logo_key,lf.sha256 AS logo_hash,lf.byte_count AS logo_bytes,t.logo_metadata::text AS logo_metadata,t.header_key,hf.sha256 AS header_hash,hf.byte_count AS header_bytes,t.header_metadata::text AS header_metadata FROM catalog_software s LEFT JOIN catalog_software_state t ON t.software_id=s.id LEFT JOIN stored_files lf ON lf.object_key=s.logo_key LEFT JOIN stored_files hf ON hf.object_key=t.header_key")
        .load::<LiveRow>(&mut conn).await.map_err(|_| fail("database unavailable"))?;
    Ok(rows
        .into_iter()
        .map(|row| (row.name.clone(), row))
        .collect())
}

fn prepare_asset(root: &Path, asset: Asset, header: bool) -> Result<PreparedAsset, String> {
    if !valid_sha256(&asset.sha256) {
        return Err(fail("software media manifest contains an invalid SHA256"));
    }
    let metadata = catalog_logos::validate_metadata(Some(asset.metadata), true)
        .map_err(|_| fail("software media metadata is invalid"))?
        .ok_or_else(|| fail("software media metadata is missing"))?;
    let max = if header {
        crate::moderation::catalog::HEADER_MAX_BYTES
    } else {
        crate::moderation::catalog::LOGO_MAX_BYTES
    };
    let path = safe_relative(root, &asset.file)?;
    let bytes = read_bounded(&path, max as u64)?;
    let actual = format!("{:x}", Sha256::digest(&bytes));
    if actual != asset.sha256 {
        return Err(fail("software media SHA256 does not match the asset bytes"));
    }
    let object = catalog_logos::validate_role(STANDARD.encode(&bytes), header)
        .map_err(|_| fail("software media bytes failed image validation"))?;
    Ok(PreparedAsset {
        bytes,
        object,
        metadata,
    })
}

pub async fn run(path: &Path, dry_run: bool) -> Result<ImportReport, String> {
    if !path.is_absolute() {
        return Err(fail("software media manifest path must be absolute"));
    }
    let manifest_meta =
        fs::symlink_metadata(path).map_err(|_| fail("software media manifest is missing"))?;
    if manifest_meta.file_type().is_symlink() || !manifest_meta.is_file() {
        return Err(fail(
            "software media manifest must be a regular file without symlinks",
        ));
    }
    let bytes = read_bounded(path, MANIFEST_MAX_BYTES)?;
    let manifest: Manifest = serde_json::from_slice(&bytes)
        .map_err(|_| fail("software media manifest is invalid JSON"))?;
    if manifest.version != 1 || manifest.entries.is_empty() || manifest.entries.len() > MAX_ENTRIES
    {
        return Err(fail(
            "software media manifest version or entry count is invalid",
        ));
    }
    let root = path
        .parent()
        .ok_or_else(|| fail("software media manifest has no parent directory"))?;
    let mut seen = HashMap::new();
    let mut prepared = Vec::with_capacity(manifest.entries.len());
    for entry in manifest.entries {
        if entry.revision < 0
            || entry.logo.is_none() && entry.header.is_none()
            || seen.insert(entry.name.clone(), ()).is_some()
        {
            return Err(fail(
                "software media manifest contains an invalid or duplicate software entry",
            ));
        }
        let logo = entry
            .logo
            .clone()
            .map(|asset| prepare_asset(root, asset, false))
            .transpose()?;
        let header = entry
            .header
            .clone()
            .map(|asset| prepare_asset(root, asset, true))
            .transpose()?;
        prepared.push(PreparedEntry {
            entry,
            logo,
            header,
            noop: false,
        });
    }
    let db = crate::backend::config::state()
        .await
        .map_err(str::to_owned)?
        .db
        .clone();
    let rows = live_rows(&db).await?;
    let mut assets = 0;
    let mut noops = 0;
    for item in &mut prepared {
        let live = rows
            .get(&item.entry.name)
            .ok_or_else(|| fail("software media manifest names an unknown software"))?;
        if live.revision != item.entry.revision && live.revision != item.entry.revision + 1 {
            return Err(fail("software media manifest revision conflict"));
        }
        if let Some(asset) = &item.logo {
            assets += 1;
            let same = live.logo_key.is_some()
                && live.logo_hash.as_deref() == Some(asset.object.object.hash.as_str())
                && live.logo_bytes == Some(asset.object.object.bytes)
                && metadata_matches(live.logo_metadata.as_deref(), &asset.metadata)?;
            if live.revision == item.entry.revision && live.logo_key.is_some() && !same {
                return Err(fail(
                    "software media import would replace an existing logo without a bound prestate",
                ));
            }
            if live.revision == item.entry.revision + 1 && !same {
                return Err(fail(
                    "software media import is not idempotent for the existing logo",
                ));
            }
        }
        if let Some(asset) = &item.header {
            assets += 1;
            let same = live.header_key.is_some()
                && live.header_hash.as_deref() == Some(asset.object.object.hash.as_str())
                && live.header_bytes == Some(asset.object.object.bytes)
                && metadata_matches(live.header_metadata.as_deref(), &asset.metadata)?;
            if live.revision == item.entry.revision && live.header_key.is_some() && !same {
                return Err(fail("software media import would replace an existing header without a bound prestate"));
            }
            if live.revision == item.entry.revision + 1 && !same {
                return Err(fail(
                    "software media import is not idempotent for the existing header",
                ));
            }
        }
        item.noop = live.revision == item.entry.revision + 1;
        if item.noop {
            noops += 1;
        }
    }
    if dry_run {
        return Ok(ImportReport {
            dry_run,
            entries: prepared.len(),
            assets,
            noops,
        });
    }
    let store = crate::backend::config::state()
        .await
        .map_err(str::to_owned)?
        .media
        .clone()
        .ok_or_else(|| fail("software media import requires the configured ObjectStore"))?;
    for item in prepared.into_iter().filter(|item| !item.noop) {
        let logo = item.logo.map(|asset| {
            (
                MediaRequest {
                    name: item.entry.name.clone(),
                    revision: item.entry.revision,
                    role: MediaRole::Logo,
                    note: "operator software media import".into(),
                    data: Some(STANDARD.encode(&asset.bytes)),
                    metadata: Some(asset.metadata),
                },
                asset.object,
            )
        });
        let header = item.header.map(|asset| {
            (
                MediaRequest {
                    name: item.entry.name.clone(),
                    revision: item.entry.revision,
                    role: MediaRole::Header,
                    note: "operator software media import".into(),
                    data: Some(STANDARD.encode(&asset.bytes)),
                    metadata: Some(asset.metadata),
                },
                asset.object,
            )
        });
        db.moderate_software_media_batch_operator(
            &store,
            item.entry.name,
            item.entry.revision,
            logo,
            header,
        )
        .await
        .map_err(|_| fail("software media import transaction failed"))?;
    }
    Ok(ImportReport {
        dry_run,
        entries: seen.len(),
        assets,
        noops,
    })
}
