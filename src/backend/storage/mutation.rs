//! OS ownership outlives a dropped HTTP future or PostgreSQL connection.
//! All runtime mutations acquire this before any database locks. Never unlink
//! the lock file: replacements would let two processes lock different inodes.
use super::{files, publication, Error, ObjectRef, ObjectStore};
use std::{
    fs::{File, OpenOptions, TryLockError},
    sync::Arc,
};

pub(crate) struct Mutation {
    store: ObjectStore,
    lock: Arc<File>,
}

impl ObjectStore {
    pub(crate) fn try_mutation(&self) -> Result<Mutation, Error> {
        if files::directory(&self.root)? != self.root {
            return Err(Error::UnsafePath);
        }
        let path = self.root.join(".runtime.lock");
        match files::metadata(&path) {
            Ok(m) if !m.is_file() => return Err(Error::UnsafePath),
            Ok(_) | Err(Error::Missing) => {}
            Err(e) => return Err(e),
        }
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = options.open(&path).map_err(|_| Error::Io)?;
        match file.try_lock() {
            Ok(()) => Ok(Mutation {
                store: self.clone(),
                lock: Arc::new(file),
            }),
            Err(TryLockError::WouldBlock) => Err(Error::Busy),
            Err(TryLockError::Error(_)) => Err(Error::Io),
        }
    }
}

impl Mutation {
    async fn run<T: Send + 'static>(
        &self,
        f: impl FnOnce(&ObjectStore, &opendal::blocking::Operator) -> Result<T, Error> + Send + 'static,
    ) -> Result<T, Error> {
        let op = opendal::blocking::Operator::new(self.store.operator.clone())
            .map_err(|_| Error::Configuration)?;
        let store = self.store.clone();
        let lock = self.lock.clone();
        tokio::task::spawn_blocking(move || {
            let _lock = lock;
            f(&store, &op)
        })
        .await
        .map_err(|_| Error::Io)?
    }

    /// Keep this Mutation alive until the DB mapping transaction finishes.
    /// Reserve durable orphan cleanup in PG *before* publishing any bytes.
    pub(crate) async fn publish(&self, object: ObjectRef, bytes: Vec<u8>) -> Result<bool, Error> {
        self.run(move |store, op| publication::publish(&store.root, op, &object, bytes))
            .await
    }

    /// Caller must exclude live mappings under the shared stored_files PG lock.
    pub(crate) async fn delete_unreferenced(&self, object: ObjectRef) -> Result<(), Error> {
        self.run(move |store, op| {
            if !object.valid() {
                return Err(Error::UnsafePath);
            }
            if files::directory(&store.root)? != store.root
                || !files::metadata(&store.root.join("objects"))?.is_dir()
            {
                return Err(Error::UnsafePath);
            }
            match publication::verify(&store.root, op, &object) {
                Ok(()) => {}
                Err(Error::Missing)
                    if matches!(
                        files::metadata(&store.root.join("objects").join(&object.hash)),
                        Err(Error::Missing)
                    ) =>
                {
                    return Ok(())
                }
                Err(e) => return Err(e),
            }
            op.delete(&format!("objects/{}", object.hash))
                .map_err(|_| Error::Io)
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::legacy_assets::tests::Files;

    #[test]
    fn child_process_cannot_acquire_live_store_lock() {
        let files = Files::new();
        let store = ObjectStore::open(&files.bundle).unwrap();
        let held = store.try_mutation().unwrap();
        let mut cmd = std::process::Command::new(std::env::current_exe().unwrap());
        cmd.args([
            "backend::storage::mutation::tests::child_lock_probe",
            "--exact",
        ])
        .env("FEDKR_SYNTHETIC_LOCK_PROBE", &files.bundle);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000000);
        }
        assert!(cmd.output().unwrap().status.success());
        drop(held);
        assert!(store.try_mutation().is_ok());
    }

    #[test]
    fn child_lock_probe() {
        let Some(path) = std::env::var_os("FEDKR_SYNTHETIC_LOCK_PROBE") else {
            return;
        };
        let store = ObjectStore::open(std::path::Path::new(&path)).unwrap();
        assert!(matches!(store.try_mutation(), Err(Error::Busy)));
    }

    #[tokio::test]
    async fn runtime_publication_is_checked_deduplicated_and_non_overwriting() {
        let files = Files::new();
        let store = ObjectStore::open(&files.bundle).unwrap();
        let mutation = store.try_mutation().unwrap();
        let bytes = b"synthetic runtime payload".to_vec();
        let object = ObjectRef::from_bytes(&bytes).unwrap();
        assert!(mutation
            .publish(object.clone(), bytes.clone())
            .await
            .unwrap());
        assert!(!mutation
            .publish(object.clone(), bytes.clone())
            .await
            .unwrap());
        assert_eq!(
            mutation
                .publish(object.clone(), b"different".to_vec())
                .await,
            Err(Error::Changed)
        );
        assert_eq!(
            mutation
                .publish(
                    ObjectRef {
                        hash: "../escape".into(),
                        bytes: bytes.len() as i64
                    },
                    bytes.clone()
                )
                .await,
            Err(Error::UnsafePath)
        );
        assert_eq!(files.objects(), 1);
        assert_eq!(store.read(&object).await.unwrap(), bytes);
        std::fs::write(files.bundle.join("objects").join(&object.hash), b"corrupt").unwrap();
        assert_eq!(mutation.publish(object, bytes).await, Err(Error::Changed));
    }

    #[tokio::test]
    async fn cancellation_keeps_os_lock_until_io_finishes() {
        let files = Files::new();
        let store = ObjectStore::open(&files.bundle).unwrap();
        let mutation = store.try_mutation().unwrap();
        let (entered, ready) = tokio::sync::oneshot::channel();
        let (release, wait) = std::sync::mpsc::channel();
        let task = tokio::spawn(async move {
            mutation
                .run(move |_, _| {
                    entered.send(()).unwrap();
                    wait.recv_timeout(std::time::Duration::from_secs(10))
                        .unwrap();
                    Ok(())
                })
                .await
        });
        ready.await.unwrap();
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        assert!(matches!(store.try_mutation(), Err(Error::Busy)));
        release.send(()).unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                match store.try_mutation() {
                    Ok(_) => break,
                    Err(Error::Busy) => tokio::task::yield_now().await,
                    Err(e) => panic!("{e:?}"),
                }
            }
        })
        .await
        .unwrap();
    }
}
