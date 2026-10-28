//! File storage is independent of HTTP authorization and the PostgreSQL adapter.
//! Runtime and offline preservation share the exact same checked file reader.
pub(crate) mod files;
mod mutation;
pub(crate) use mutation::Mutation;
pub(crate) mod publication;
#[cfg(test)]
mod tests;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

pub const MAX_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Configuration,
    Missing,
    UnsafePath,
    TooLarge,
    Changed,
    Io,
    Busy,
}

#[derive(Clone, PartialEq, Eq)]
pub struct ObjectRef {
    pub hash: String,
    pub bytes: i64,
}

impl ObjectRef {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() > MAX_BYTES {
            return Err(Error::TooLarge);
        }
        Ok(Self {
            hash: format!("{:x}", Sha256::digest(bytes)),
            bytes: bytes.len() as i64,
        })
    }

    pub(crate) fn matches(&self, bytes: &[u8]) -> Result<(), Error> {
        if !self.valid() {
            return Err(Error::UnsafePath);
        }
        if &Self::from_bytes(bytes)? != self {
            return Err(Error::Changed);
        }
        Ok(())
    }

    pub fn valid(&self) -> bool {
        self.hash.len() == 64
            && self
                .hash
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            && (0..=MAX_BYTES as i64).contains(&self.bytes)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Backend {
    Fs,
    S3,
}

#[derive(Clone)]
pub struct ObjectStore {
    pub(crate) root: PathBuf,
    pub(crate) operator: opendal::Operator,
    pub(crate) backend: Backend,
}

impl ObjectStore {
    pub fn open(root: &Path) -> Result<Self, Error> {
        let root = files::directory(root)?;
        Ok(Self {
            operator: files::operator(&root)?,
            root,
            backend: Backend::Fs,
        })
    }

    pub fn from_env() -> Result<Option<Self>, &'static str> {
        let config = parse_storage_config(
            std::env::var_os("FEDKR_MEDIA_DIR").map(PathBuf::from),
            |name| std::env::var(name).ok(),
        )?;
        match config {
            None => Ok(None),
            Some(StorageConfig::Fs { media_dir }) => {
                Self::open(&media_dir).map(Some).map_err(|_| {
                    "Media directory must be an existing absolute private directory without links"
                })
            }
            Some(StorageConfig::S3 {
                media_dir,
                endpoint,
                bucket,
                region,
                access_key_id,
                secret_access_key,
                root_prefix,
            }) => {
                let root = files::directory(&media_dir).map_err(|_| {
                    "S3 media storage requires an existing absolute private lock directory without links"
                })?;
                let remote_root = format!("/{root_prefix}");
                let builder = opendal::services::S3::default()
                    .root(&remote_root)
                    .bucket(&bucket)
                    .endpoint(&endpoint)
                    .region(&region)
                    .access_key_id(&access_key_id)
                    .secret_access_key(&secret_access_key)
                    .disable_config_load();
                // The transport disables its default crypto provider so all
                // application TLS keeps using the already selected ring backend.
                let _ = rustls::crypto::ring::default_provider().install_default();
                let operator = opendal::Operator::new(builder)
                    .map_err(|_| "S3 media storage configuration is invalid")?
                    .with_context(opendal::OperationContext::new().with_http_transport(
                        opendal::HttpTransporter::new(
                            opendal_http_transport_reqwest::ReqwestTransport::default(),
                        ),
                    ));
                Ok(Some(Self {
                    root,
                    operator,
                    backend: Backend::S3,
                }))
            }
        }
    }

    pub async fn read(&self, object: &ObjectRef) -> Result<Vec<u8>, Error> {
        if !object.valid() {
            return Err(Error::UnsafePath);
        }
        match self.backend {
            Backend::Fs => {
                let objects = self.root.join("objects");
                if !files::metadata(&objects)?.is_dir() {
                    return Err(Error::UnsafePath);
                }
                let bytes =
                    files::read_regular(&objects.join(&object.hash), &self.root, &self.operator)
                        .await?;
                object.matches(&bytes)?;
                Ok(bytes)
            }
            Backend::S3 => publication::read_remote(&self.operator, object).await,
        }
    }

    #[cfg(test)]
    pub(crate) async fn delete_unreferenced(&self, object: &ObjectRef) -> Result<(), Error> {
        self.try_mutation()?
            .delete_unreferenced(object.clone())
            .await
    }

    #[cfg(test)]
    pub(crate) fn is_s3(&self) -> bool {
        self.backend == Backend::S3
    }
}

#[derive(Clone)]
enum StorageConfig {
    Fs {
        media_dir: PathBuf,
    },
    S3 {
        media_dir: PathBuf,
        endpoint: String,
        bucket: String,
        region: String,
        access_key_id: String,
        secret_access_key: String,
        root_prefix: String,
    },
}

const S3_ENV_NAMES: [&str; 6] = [
    "FEDKR_S3_ENDPOINT",
    "FEDKR_S3_BUCKET",
    "FEDKR_S3_REGION",
    "FEDKR_S3_ACCESS_KEY_ID",
    "FEDKR_S3_SECRET_ACCESS_KEY",
    "FEDKR_S3_ROOT",
];

fn parse_storage_config<F>(
    media_dir: Option<PathBuf>,
    get: F,
) -> Result<Option<StorageConfig>, &'static str>
where
    F: Fn(&str) -> Option<String>,
{
    let s3_configured = S3_ENV_NAMES.iter().any(|name| get(name).is_some());
    match get("FEDKR_MEDIA_BACKEND").as_deref() {
        None if s3_configured => Err(
            "S3 media variables require FEDKR_MEDIA_BACKEND=s3; refusing an implicit Fs fallback",
        ),
        None => Ok(media_dir.map(|media_dir| StorageConfig::Fs { media_dir })),
        Some("fs") => {
            if s3_configured {
                return Err("FEDKR_MEDIA_BACKEND=fs cannot be combined with S3 media variables");
            }
            media_dir
                .map(|media_dir| StorageConfig::Fs { media_dir })
                .ok_or("FEDKR_MEDIA_BACKEND=fs requires FEDKR_MEDIA_DIR")
                .map(Some)
        }
        Some("s3") => {
            let media_dir = media_dir.ok_or("FEDKR_MEDIA_BACKEND=s3 requires FEDKR_MEDIA_DIR")?;
            let endpoint = required_env(&get, "FEDKR_S3_ENDPOINT")?;
            let bucket = required_env(&get, "FEDKR_S3_BUCKET")?;
            let region = required_env(&get, "FEDKR_S3_REGION")?;
            let access_key_id = required_env(&get, "FEDKR_S3_ACCESS_KEY_ID")?;
            let secret_access_key = required_env(&get, "FEDKR_S3_SECRET_ACCESS_KEY")?;
            let root_prefix = required_env(&get, "FEDKR_S3_ROOT")?;
            validate_endpoint(&endpoint)?;
            validate_bucket(&bucket)?;
            validate_token(&region, "FEDKR_S3_REGION")?;
            validate_token(&access_key_id, "FEDKR_S3_ACCESS_KEY_ID")?;
            validate_token(&secret_access_key, "FEDKR_S3_SECRET_ACCESS_KEY")?;
            validate_root_prefix(&root_prefix)?;
            Ok(Some(StorageConfig::S3 {
                media_dir,
                endpoint,
                bucket,
                region,
                access_key_id,
                secret_access_key,
                root_prefix,
            }))
        }
        Some(_) => Err("FEDKR_MEDIA_BACKEND must be fs or s3"),
    }
}

fn required_env<F>(get: &F, name: &str) -> Result<String, &'static str>
where
    F: Fn(&str) -> Option<String>,
{
    get(name)
        .filter(|value| !value.is_empty())
        .ok_or("S3 media storage configuration is incomplete")
}

fn validate_token(value: &str, name: &'static str) -> Result<(), &'static str> {
    if value.is_empty() || value.chars().any(|c| c.is_control() || c.is_whitespace()) {
        Err(match name {
            "FEDKR_S3_REGION" => "FEDKR_S3_REGION is invalid",
            "FEDKR_S3_ACCESS_KEY_ID" => "FEDKR_S3_ACCESS_KEY_ID is invalid",
            "FEDKR_S3_SECRET_ACCESS_KEY" => "FEDKR_S3_SECRET_ACCESS_KEY is invalid",
            _ => "S3 media storage configuration is invalid",
        })
    } else {
        Ok(())
    }
}

fn validate_endpoint(value: &str) -> Result<(), &'static str> {
    let url = url::Url::parse(value).map_err(|_| "FEDKR_S3_ENDPOINT must be an HTTPS URL")?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !(url.path().is_empty() || url.path() == "/")
    {
        return Err("FEDKR_S3_ENDPOINT must be an HTTPS URL without credentials or a path");
    }
    Ok(())
}

fn validate_bucket(value: &str) -> Result<(), &'static str> {
    let valid = (3..=63).contains(&value.len())
        && value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'.' || b == b'-')
        && !matches!(value.as_bytes().first(), Some(b'.' | b'-'))
        && !matches!(value.as_bytes().last(), Some(b'.' | b'-'))
        && !value.contains("..")
        && value.parse::<std::net::IpAddr>().is_err();
    if valid {
        Ok(())
    } else {
        Err("FEDKR_S3_BUCKET is invalid")
    }
}

fn validate_root_prefix(value: &str) -> Result<(), &'static str> {
    let parts: Vec<_> = value.split('/').collect();
    let valid = !value.is_empty()
        && !value.starts_with('/')
        && value.ends_with('/')
        && !value.contains('\\')
        && !value.chars().any(|c| c.is_control() || c.is_whitespace())
        && parts
            .iter()
            .take(parts.len().saturating_sub(1))
            .all(|part| !part.is_empty() && *part != "." && *part != "..");
    if valid {
        Ok(())
    } else {
        Err("FEDKR_S3_ROOT must be a relative prefix ending in /")
    }
}

pub(super) fn map_opendal_error(error: opendal::Error) -> Error {
    match error.kind() {
        opendal::ErrorKind::NotFound => Error::Missing,
        opendal::ErrorKind::ConfigInvalid | opendal::ErrorKind::Unsupported => Error::Configuration,
        opendal::ErrorKind::ConditionNotMatch | opendal::ErrorKind::AlreadyExists => Error::Changed,
        _ => Error::Io,
    }
}
