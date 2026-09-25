//! Offline, explicit cutover approval. Never starts HTTP/workers or alters source.
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Configuration,
    Connection,
    Inventory,
    Files,
    Conflict,
    Database,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Configuration => "Activation requires an isolated snapshot and a renamed fedkr_live_* copy, canonical HTTPS origin and private file roots; apply additionally requires the cutover acknowledgment",
            Self::Connection => "Offline activation connection failed",
            Self::Inventory => "Snapshot, import ledger, schema or projected data did not match; activation refused",
            Self::Files => "The complete preserved file inventory could not be verified; activation refused",
            Self::Conflict => "Existing activation belongs to a different database or public origin; no approval changed",
            Self::Database => "Activation result could not be confirmed; repeat the same command to inspect the durable approval",
        })
    }
}
impl std::error::Error for Error {}

#[derive(Serialize, Debug)]
pub struct Report {
    pub outcome: &'static str,
    pub runtime_enabled: bool,
    pub validated_now: bool,
    pub asset_objects: i64,
    pub asset_bytes: i64,
}

pub(crate) struct Request {
    pub source: String,
    pub target: String,
    pub origin: String,
    pub paths: super::legacy_assets::Bundle,
    pub apply: bool,
}
pub(crate) fn validate(source: &str, target: &str, origin: &str) -> Result<(), Error> {
    if !super::legacy::isolated_url(source, "fedkr_snapshot_")
        || !super::legacy::isolated_url(target, "fedkr_live_")
        || !super::settings::canonical_origin(origin)
            .is_ok_and(|(canonical, secure)| secure && canonical == origin && origin.len() <= 2048)
    {
        return Err(Error::Configuration);
    }
    Ok(())
}
pub async fn run_from_env(apply: bool) -> Result<Report, Error> {
    let read = |name| std::env::var(name).map_err(|_| Error::Configuration);
    let source = read("FEDKR_IMPORT_SOURCE_URL")?;
    let target = read("FEDKR_ACTIVATION_TARGET_URL")?;
    let origin = read("FEDKR_PUBLIC_ORIGIN")?;
    validate(&source, &target, &origin)?;
    if apply && read("FEDKR_ACTIVATION_ACK")? != "source-frozen-and-reviewed" {
        return Err(Error::Configuration);
    }
    let source_dir = read("FEDKR_ASSET_SOURCE_DIR")?;
    let bundle_dir = read("FEDKR_ASSET_BUNDLE_DIR")?;
    let paths = super::legacy_assets::Bundle::new(source_dir.as_ref(), bundle_dir.as_ref())
        .map_err(|_| Error::Files)?;
    super::db::activate_legacy(&Request {
        source,
        target,
        origin,
        paths,
        apply,
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn activation_is_not_a_general_database_override() {
        let source = "postgres://u@127.0.0.1:16440/fedkr_snapshot_test";
        let target = "postgres://u@127.0.0.1:16440/fedkr_live_test";
        assert!(validate(source, target, "https://fediverse.kr").is_ok());
        for bad in [
            target.replace("fedkr_live_", "fedkr_rehearsal_"),
            target.replace("fedkr_live_test", "fedkr_dev"),
            format!("{target}?host=elsewhere"),
            target.replace("127.0.0.1", "db.example.org"),
        ] {
            assert_eq!(
                validate(source, &bad, "https://fediverse.kr"),
                Err(Error::Configuration)
            );
        }
        for bad in [
            "http://127.0.0.1:12239",
            "http://fediverse.kr",
            "https://fediverse.kr/",
            "https://fediverse.kr/path",
            "https://user@fediverse.kr",
        ] {
            assert_eq!(validate(source, target, bad), Err(Error::Configuration));
        }
        for error in [
            Error::Configuration,
            Error::Connection,
            Error::Inventory,
            Error::Files,
            Error::Conflict,
            Error::Database,
        ] {
            assert!(!error.to_string().contains("postgres://"));
        }
    }
}
