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
#[derive(Clone)]
pub struct ObjectStore {
    root: PathBuf,
    operator: opendal::Operator,
}
impl ObjectStore {
    pub fn open(root: &Path) -> Result<Self, Error> {
        let root = files::directory(root)?;
        Ok(Self {
            operator: files::operator(&root)?,
            root,
        })
    }
    pub fn from_env() -> Result<Option<Self>, &'static str> {
        std::env::var_os("FEDKR_MEDIA_DIR")
            .map(|p| {
                Self::open(Path::new(&p)).map_err(|_| {
                    "Media directory must be an existing absolute private directory without links"
                })
            })
            .transpose()
    }
    pub async fn read(&self, object: &ObjectRef) -> Result<Vec<u8>, Error> {
        if !object.valid() {
            return Err(Error::UnsafePath);
        }
        let objects = self.root.join("objects");
        if !files::metadata(&objects)?.is_dir() {
            return Err(Error::UnsafePath);
        }
        let bytes =
            files::read_regular(&objects.join(&object.hash), &self.root, &self.operator).await?;
        object.matches(&bytes)?;
        Ok(bytes)
    }

    #[cfg(test)]
    pub(crate) async fn delete_unreferenced(&self, object: &ObjectRef) -> Result<(), Error> {
        self.try_mutation()?
            .delete_unreferenced(object.clone())
            .await
    }
}
