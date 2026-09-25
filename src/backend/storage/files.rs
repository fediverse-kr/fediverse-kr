//! Local path policy around OpenDAL. The private directory must not be writable
//! by untrusted processes; Fs is not an OS capability sandbox against path swaps.
use super::{Error, MAX_BYTES};
use std::{
    fs,
    path::{Path, PathBuf},
};
pub(crate) fn metadata(path: &Path) -> Result<fs::Metadata, Error> {
    let m = fs::symlink_metadata(path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            Error::Missing
        } else {
            Error::Io
        }
    })?;
    if m.file_type().is_symlink() {
        return Err(Error::UnsafePath);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if m.file_attributes() & 0x400 != 0 {
            return Err(Error::UnsafePath);
        }
    }
    Ok(m)
}
pub(crate) fn directory(path: &Path) -> Result<PathBuf, Error> {
    if !path.is_absolute() {
        return Err(Error::Configuration);
    }
    // Check every component, not just the final directory (Windows junctions too).
    for ancestor in path.ancestors() {
        if !metadata(ancestor)?.is_dir() {
            return Err(Error::UnsafePath);
        }
    }
    fs::canonicalize(path).map_err(|_| Error::Io)
}
pub(crate) fn operator(root: &Path) -> Result<opendal::Operator, Error> {
    opendal::Operator::new(
        opendal::services::Fs::default().root(root.to_str().ok_or(Error::Configuration)?),
    )
    .map_err(|_| Error::Configuration)
}
pub(crate) async fn read_regular(
    path: &Path,
    root: &Path,
    operator: &opendal::Operator,
) -> Result<Vec<u8>, Error> {
    let (relative, before) = before_read(path, root)?;
    let bytes = operator
        .read_with(&relative)
        .range(0..before.len())
        .await
        .map_err(io_error)?
        .to_vec();
    after_read(path, root, before, bytes)
}

pub(crate) fn read_blocking(
    path: &Path,
    root: &Path,
    operator: &opendal::blocking::Operator,
) -> Result<Vec<u8>, Error> {
    let (relative, before) = before_read(path, root)?;
    let bytes = operator
        .reader(&relative)
        .map_err(io_error)?
        .read(0..before.len())
        .map_err(io_error)?
        .to_vec();
    after_read(path, root, before, bytes)
}

fn io_error(e: opendal::Error) -> Error {
    if e.kind() == opendal::ErrorKind::NotFound {
        Error::Missing
    } else {
        Error::Io
    }
}

fn before_read(path: &Path, root: &Path) -> Result<(String, fs::Metadata), Error> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| Error::UnsafePath)?
        .to_str()
        .ok_or(Error::UnsafePath)?
        .replace('\\', "/");
    for ancestor in path.parent().ok_or(Error::UnsafePath)?.ancestors() {
        if !metadata(ancestor)?.is_dir() {
            return Err(Error::UnsafePath);
        }
        if ancestor == root {
            break;
        }
    }
    let before = metadata(path)?;
    if !before.is_file() {
        return Err(Error::UnsafePath);
    }
    if before.len() > MAX_BYTES as u64 {
        return Err(Error::TooLarge);
    }
    if !fs::canonicalize(path)
        .map_err(|_| Error::Io)?
        .starts_with(root)
    {
        return Err(Error::UnsafePath);
    }
    Ok((relative, before))
}

fn after_read(
    path: &Path,
    root: &Path,
    before: fs::Metadata,
    bytes: Vec<u8>,
) -> Result<Vec<u8>, Error> {
    if bytes.len() > MAX_BYTES {
        return Err(Error::TooLarge);
    }
    let after = metadata(path)?;
    if before.len() != bytes.len() as u64
        || before.modified().ok() != after.modified().ok()
        || before.len() != after.len()
        || !fs::canonicalize(path)
            .map_err(|_| Error::Io)?
            .starts_with(root)
    {
        return Err(Error::Changed);
    }
    Ok(bytes)
}
