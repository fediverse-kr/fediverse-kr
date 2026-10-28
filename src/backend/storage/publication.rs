//! Shared create-if-absent publication for offline bundles and runtime uploads.
//! Call only on a blocking worker. Payload IO belongs to OpenDAL; the local Fs
//! adapter supplies atomic publication and durability, while S3 uses its
//! service-side create-if-absent condition.
use super::{files, map_opendal_error, Backend, Error, ObjectRef, MAX_BYTES};
use std::{
    fs::{self, OpenOptions},
    path::Path,
};

pub(super) fn object_key(object: &ObjectRef) -> String {
    format!("objects/{}", object.hash)
}

pub(super) async fn read_remote(
    operator: &opendal::Operator,
    object: &ObjectRef,
) -> Result<Vec<u8>, Error> {
    let key = object_key(object);
    let metadata = operator.stat(&key).await.map_err(map_opendal_error)?;
    let length = metadata.content_length();
    if length > MAX_BYTES as u64 {
        return Err(Error::TooLarge);
    }
    if length != object.bytes as u64 {
        return Err(Error::Changed);
    }
    let bytes = operator
        .read_with(&key)
        .range(0..length)
        .await
        .map_err(map_opendal_error)?
        .to_vec();
    if bytes.len() > MAX_BYTES {
        return Err(Error::TooLarge);
    }
    object.matches(&bytes)?;
    Ok(bytes)
}

pub(super) fn verify(
    backend: Backend,
    root: &Path,
    op: &opendal::blocking::Operator,
    object: &ObjectRef,
) -> Result<(), Error> {
    match backend {
        Backend::Fs => {
            let bytes = files::read_blocking(&root.join(object_key(object)), root, op)?;
            object.matches(&bytes)
        }
        Backend::S3 => verify_remote(op, object),
    }
}

fn verify_remote(op: &opendal::blocking::Operator, object: &ObjectRef) -> Result<(), Error> {
    let key = object_key(object);
    let metadata = op.stat(&key).map_err(map_opendal_error)?;
    let length = metadata.content_length();
    if length > MAX_BYTES as u64 {
        return Err(Error::TooLarge);
    }
    if length != object.bytes as u64 {
        return Err(Error::Changed);
    }
    let bytes = op
        .read_options(
            &key,
            opendal::options::ReadOptions {
                range: (0..length).into(),
                ..Default::default()
            },
        )
        .map_err(map_opendal_error)?
        .to_vec();
    if bytes.len() > MAX_BYTES {
        return Err(Error::TooLarge);
    }
    object.matches(&bytes)
}

/// Offline bundles always use the local Fs publication path.
pub(crate) fn publish(
    root: &Path,
    op: &opendal::blocking::Operator,
    object: &ObjectRef,
    bytes: Vec<u8>,
) -> Result<bool, Error> {
    publish_backend(Backend::Fs, root, op, object, bytes)
}

pub(crate) fn publish_backend(
    backend: Backend,
    root: &Path,
    op: &opendal::blocking::Operator,
    object: &ObjectRef,
    bytes: Vec<u8>,
) -> Result<bool, Error> {
    object.matches(&bytes)?;
    match backend {
        Backend::Fs => publish_fs(root, op, object, bytes),
        Backend::S3 => publish_s3(op, object, bytes),
    }
}

fn publish_s3(
    op: &opendal::blocking::Operator,
    object: &ObjectRef,
    bytes: Vec<u8>,
) -> Result<bool, Error> {
    let key = object_key(object);
    let mut options = opendal::options::WriteOptions::default();
    options.if_not_exists = true;
    match op.write_options(&key, bytes, options) {
        Ok(_) => {
            // A successful conditional write is not publication until the
            // object can be read back and still matches its content address.
            verify_remote(op, object)?;
            Ok(true)
        }
        Err(error)
            if matches!(
                error.kind(),
                opendal::ErrorKind::ConditionNotMatch | opendal::ErrorKind::AlreadyExists
            ) =>
        {
            verify_remote(op, object)?;
            Ok(false)
        }
        Err(error) => Err(map_opendal_error(error)),
    }
}

fn publish_fs(
    root: &Path,
    op: &opendal::blocking::Operator,
    object: &ObjectRef,
    bytes: Vec<u8>,
) -> Result<bool, Error> {
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
            verify(Backend::Fs, root, op, object)?;
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
                verify(Backend::Fs, root, op, object)?;
                Ok(false)
            }
            Err(_) => Err(Error::Io),
        }
    })();
    fs::remove_file(&temp).map_err(|_| Error::Io)?;
    let created = result?;
    verify(Backend::Fs, root, op, object)?;
    #[cfg(unix)]
    {
        fs::File::open(&dir)
            .and_then(|f| f.sync_all())
            .map_err(|_| Error::Io)?;
    }
    Ok(created)
}
