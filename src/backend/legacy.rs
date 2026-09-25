//! Offline import boundary. No source rows, handles, keys, URLs or credentials
//! are returned to the CLI or shared with the HTTP / federation runtime.
use serde::Serialize;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ImportMode {
    DryRun,
    Apply,
}

#[derive(Serialize, Debug, PartialEq, Eq)]
pub struct TableCount {
    pub table: &'static str,
    pub rows: i64,
}

#[derive(Serialize, Debug)]
pub struct ImportReport {
    pub outcome: &'static str,
    pub tables: Vec<TableCount>,
    pub reserved_members: i64,
    pub display_names_adapted: i64,
    pub external_asset_references: i64,
    pub retained_signing_keys: i64,
    pub runtime_enabled: bool,
}

// Deliberately excludes original database errors, query values and environment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportError {
    Configuration,
    Connection,
    Schema,
    TargetNotEmpty,
    Data,
    Identity,
    SigningKey,
    Mismatch,
    Database,
}
impl std::fmt::Display for ImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Configuration => "Import requires distinct literal-loopback fedkr_snapshot_* and fedkr_rehearsal_* databases; URLs must be supplied only in the import environment variables",
            Self::Connection => "Isolated import database connection failed",
            Self::Schema => "Snapshot schema differs from the supported Phoenix contract; no rows imported",
            Self::TargetNotEmpty => "Destination is not empty or is not a matching quarantined import; no existing data overwritten",
            Self::Data => "Snapshot has incompatible data or relationships; import rolled back",
            Self::Identity => "Legacy identity normalization failed or collides; import rolled back",
            Self::SigningKey => "Legacy signing key count, format or public/private match failed; import rolled back",
            Self::Mismatch => "Snapshot, retained rows or projected data changed; import refused without overwriting",
            Self::Database => "Offline import failed or its result could not be confirmed; retry verifies existing data without overwriting",
        })
    }
}
impl std::error::Error for ImportError {}
pub(crate) fn isolated_url(value: &str, prefix: &str) -> bool {
    url::Url::parse(value).is_ok_and(|u| {
        let suffix = u.path().strip_prefix(&format!("/{prefix}"));
        matches!(u.scheme(), "postgres" | "postgresql")
            && u.host_str() == Some("127.0.0.1")
            && u.port().is_some_and(|p| p != 0)
            && u.query().is_none()
            && u.fragment().is_none()
            && suffix.is_some_and(|s| {
                !s.is_empty()
                    && s.len() <= 32
                    && s.bytes()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_')
            })
    })
}

pub async fn run_from_env(mode: ImportMode) -> Result<ImportReport, ImportError> {
    let source =
        std::env::var("FEDKR_IMPORT_SOURCE_URL").map_err(|_| ImportError::Configuration)?;
    let target =
        std::env::var("FEDKR_IMPORT_TARGET_URL").map_err(|_| ImportError::Configuration)?;
    if !isolated_url(&source, "fedkr_snapshot_") || !isolated_url(&target, "fedkr_rehearsal_") {
        return Err(ImportError::Configuration);
    }
    super::db::import_snapshot(&source, &target, mode).await
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn import_targets_exclude_production_development_and_connection_overrides() {
        assert!(isolated_url(
            "postgresql://u@127.0.0.1:16440/fedkr_snapshot_test",
            "fedkr_snapshot_"
        ));
        for value in [
            "postgresql://u@localhost:16440/fedkr_snapshot_test",
            "postgresql://u@192.168.0.25:5432/fedkr_snapshot_test",
            "postgresql://u@127.0.0.1:16439/fedkr_dev",
            "postgresql://u@127.0.0.1:16439/fedkr_test",
            "postgresql://u@127.0.0.1:16439/fedkr_snapshot_",
            "postgresql://u@127.0.0.1:16439/fedkr_snapshot_test?host=production",
            "postgresql://u@127.0.0.1:16439/fedkr_snapshot_test#x",
            "postgresql://u@127.0.0.1:16439/fedkr_snapshot_a%2fb",
        ] {
            assert!(!isolated_url(value, "fedkr_snapshot_"));
        }
    }
}
