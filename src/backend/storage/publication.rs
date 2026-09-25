//! Shared create-if-absent publication for offline bundles and runtime uploads.
//! Call only on a blocking worker. Payload IO belongs to OpenDAL; the local Fs
//! adapter supplies atomic publication and durability, not a second storage API.
use super::{files, Error, ObjectRef};
use std::{
    fs::{self, OpenOptions},
    path::Path,
};

pub(super) fn verify(
    root: &Path,
    op: &opendal::blocking::Operator,
    object: &ObjectRef,
) -> Result<(), Error> {
    let bytes = files::read_blocking(&root.join("objects").join(&object.hash), root, op)?;
    object.matches(&bytes)
}

pub(crate) fn publish(
    root: &Path,
    op: &opendal::blocking::Operator,
    object: &ObjectRef,
    bytes: Vec<u8>,
) -> Result<bool, Error> {
    object.matches(&bytes)?;
    if files::directory(root)? != root {
        return Err(Error::UnsafePath);
    }
    let dir = root.join("objects");
    match fs::create_dir(&dir) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(_) => return Err(Error::Io),
    }
    if !files::metadata(&dir)?.is_dir()
        || !fs::canonicalize(&dir)
            .map_err(|_| Error::Io)?
            .starts_with(root)
    {
        return Err(Error::UnsafePath);
    }
    let dest = dir.join(&object.hash);
    match files::metadata(&dest) {
        Ok(_) => {
            verify(root, op, object)?;
            return Ok(false);
        }
        Err(Error::Missing) => {}
        Err(e) => return Err(e),
    }
    let name = format!("objects/.partial-{}", uuid::Uuid::new_v4().simple());
    let temp = root.join(&name);
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    drop(options.open(&temp).map_err(|_| Error::Io)?);
    let result = (|| {
        op.write(&name, bytes).map_err(|_| Error::Io)?;
        // OpenDAL completes the write before this sync and create-if-absent.
        OpenOptions::new()
            .read(true)
            .write(true)
            .open(&temp)
            .and_then(|f| f.sync_all())
            .map_err(|_| Error::Io)?;
        match fs::hard_link(&temp, &dest) {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                verify(root, op, object)?;
                Ok(false)
            }
            Err(_) => Err(Error::Io),
        }
    })();
    fs::remove_file(&temp).map_err(|_| Error::Io)?;
    let created = result?;
    verify(root, op, object)?;
    #[cfg(unix)]
    {
        fs::File::open(&dir)
            .and_then(|f| f.sync_all())
            .map_err(|_| Error::Io)?;
    }
    Ok(created)
}
