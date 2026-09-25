//! Offline, immutable file bundle. Logical storage keys stay private in PG.
//! This module never downloads a remote URL or publishes imported bytes.
#[cfg(test)]
pub use super::storage::MAX_BYTES;
use serde::Serialize;
use sha2::{Digest, Sha256};
#[cfg(test)]
use std::fs::OpenOptions;
use std::{
    fs,
    path::{Path, PathBuf},
};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Configuration,
    Database,
    Inventory,
    Missing,
    UnsafePath,
    TooLarge,
    Changed,
    Io,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self{
        Self::Configuration=>"Asset import requires a quarantined rehearsal database and separate absolute source/bundle directories",
        Self::Database=>"Private asset inventory transaction failed; no source details are logged",
        Self::Inventory=>"Asset references or the existing bundle index are inconsistent; nothing was overwritten",
        Self::Missing=>"A referenced asset is missing from the supplied storage export",
        Self::UnsafePath=>"Asset key or path is unsafe, linked, or outside the supplied directory",
        Self::TooLarge=>"An asset exceeds the 16 MiB preservation limit; it was not skipped or truncated",
        Self::Changed=>"Asset content differs from the existing bundle or changed while reading; nothing was overwritten",
        Self::Io=>"Asset bundle IO failed; validated content-addressed files may remain for a safe retry",
    })
    }
}
impl std::error::Error for Error {}
#[derive(Serialize, Debug)]
pub struct Report {
    pub outcome: &'static str,
    pub references: i64,
    pub objects: i64,
    pub bytes: i64,
    pub copied_objects: i64,
    pub runtime_enabled: bool,
}
pub async fn run_from_env(apply: bool) -> Result<Report, Error> {
    let target = std::env::var("FEDKR_IMPORT_TARGET_URL").map_err(|_| Error::Configuration)?;
    let source = std::env::var_os("FEDKR_ASSET_SOURCE_DIR").ok_or(Error::Configuration)?;
    let bundle = std::env::var_os("FEDKR_ASSET_BUNDLE_DIR").ok_or(Error::Configuration)?;
    let paths = Bundle::new(Path::new(&source), Path::new(&bundle))?;
    super::db::preserve_legacy_assets(&target, &paths, apply).await
}
pub(crate) fn key(value: &str) -> Result<(), Error> {
    if value.is_empty()
        || value.len() > 1024
        || value.contains(['\\', ':', '%', '?', '*', '"', '<', '>', '|'])
        || value.chars().any(char::is_control)
    {
        return Err(Error::UnsafePath);
    }
    let mut parts = value.split('/');
    if !matches!(
        parts.next(),
        Some("avatars" | "emojis" | "favicons" | "software-logos" | "software")
    ) {
        return Err(Error::UnsafePath);
    }
    let rest: Vec<_> = parts.collect();
    if rest.is_empty() {
        return Err(Error::UnsafePath);
    }
    for part in rest {
        if part.is_empty() || matches!(part, "." | "..") || part.ends_with(['.', ' ']) {
            return Err(Error::UnsafePath);
        }
        let stem = part.split('.').next().unwrap_or("").to_ascii_uppercase();
        if matches!(
            stem.as_str(),
            "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
        ) || (stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && matches!(stem.as_bytes()[3], b'1'..=b'9'))
        {
            return Err(Error::UnsafePath);
        }
    }
    Ok(())
}
fn metadata(path: &Path) -> Result<fs::Metadata, Error> {
    super::storage::files::metadata(path).map_err(Into::into)
}
fn directory(path: &Path) -> Result<PathBuf, Error> {
    super::storage::files::directory(path).map_err(Into::into)
}
impl From<super::storage::Error> for Error {
    fn from(value: super::storage::Error) -> Self {
        match value {
            super::storage::Error::Configuration => Self::Configuration,
            super::storage::Error::Missing => Self::Missing,
            super::storage::Error::UnsafePath => Self::UnsafePath,
            super::storage::Error::TooLarge => Self::TooLarge,
            super::storage::Error::Changed => Self::Changed,
            super::storage::Error::Io | super::storage::Error::Busy => Self::Io,
        }
    }
}
#[derive(Clone)]
pub(crate) struct Bundle {
    source: PathBuf,
    root: PathBuf,
    source_operator: opendal::Operator,
    operator: opendal::Operator,
}
pub(crate) struct Object {
    pub hash: String,
    pub bytes: Vec<u8>,
}
impl Bundle {
    pub fn new(source: &Path, root: &Path) -> Result<Self, Error> {
        let source = directory(source)?;
        let root = directory(root)?;
        if source.starts_with(&root) || root.starts_with(&source) {
            return Err(Error::Configuration);
        }
        Ok(Self {
            source_operator: super::storage::files::operator(&source)?,
            operator: super::storage::files::operator(&root)?,
            source,
            root,
        })
    }
    pub async fn read(&self, name: &str) -> Result<Object, Error> {
        key(name)?;
        let mut path = self.source.clone();
        let parts: Vec<_> = name.split('/').collect();
        for (i, part) in parts.iter().enumerate() {
            path.push(part);
            let m = metadata(&path)?;
            if i + 1 < parts.len() && !m.is_dir() {
                return Err(Error::UnsafePath);
            }
        }
        let bytes =
            super::storage::files::read_regular(&path, &self.source, &self.source_operator).await?;
        Ok(Object {
            hash: format!("{:x}", Sha256::digest(&bytes)),
            bytes,
        })
    }
    pub async fn verify(&self, object: &Object) -> Result<(), Error> {
        let dir = self.root.join("objects");
        if !metadata(&dir)?.is_dir() {
            return Err(Error::UnsafePath);
        }
        let bytes = super::storage::files::read_regular(
            &dir.join(&object.hash),
            &self.root,
            &self.operator,
        )
        .await?;
        if bytes != object.bytes {
            return Err(Error::Changed);
        }
        Ok(())
    }
    pub async fn check_destination(&self, object: &Object) -> Result<bool, Error> {
        let dir = self.root.join("objects");
        match metadata(&dir) {
            Err(Error::Missing) => return Ok(false),
            Err(e) => return Err(e),
            Ok(m) if !m.is_dir() => return Err(Error::UnsafePath),
            Ok(_) => {}
        }
        match metadata(&dir.join(&object.hash)) {
            Err(Error::Missing) => Ok(false),
            Err(e) => Err(e),
            Ok(_) => {
                self.verify(object).await?;
                Ok(true)
            }
        }
    }
    pub async fn put(&self, object: &Object) -> Result<bool, Error> {
        let root = self.root.clone();
        let reference = super::storage::ObjectRef {
            hash: object.hash.clone(),
            bytes: object.bytes.len() as i64,
        };
        let bytes = object.bytes.clone();
        let op = opendal::blocking::Operator::new(self.operator.clone())
            .map_err(|_| Error::Configuration)?;
        tokio::task::spawn_blocking(move || {
            super::storage::publication::publish(&root, &op, &reference, bytes)
        })
        .await
        .map_err(|_| Error::Io)?
        .map_err(Into::into)
    }
}
#[cfg(test)]
pub(crate) mod tests;
