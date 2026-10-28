use super::*;
use crate::backend::legacy_assets::tests::Files;
use std::path::PathBuf;

async fn fixture(files: &Files) -> ObjectRef {
    files.write("avatars/test.png", b"private synthetic payload");
    let object = files.paths().read("avatars/test.png").await.unwrap();
    files.paths().put(&object).await.unwrap();
    ObjectRef {
        hash: object.hash,
        bytes: object.bytes.len() as i64,
    }
}

fn env<'a>(entries: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
    move |name| {
        entries
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| (*value).to_string())
    }
}

#[test]
fn storage_configuration_rejects_implicit_or_ambiguous_s3() {
    let lock_dir = Some(PathBuf::from("/var/lib/fedkr2/media-lock"));
    assert!(
        parse_storage_config(lock_dir.clone(), env(&[("FEDKR_S3_BUCKET", "fedkr-test")]),).is_err()
    );
    assert!(
        parse_storage_config(lock_dir.clone(), env(&[("FEDKR_MEDIA_BACKEND", "s3")]),).is_err()
    );
    assert!(parse_storage_config(
        lock_dir,
        env(&[
            ("FEDKR_MEDIA_BACKEND", "fs"),
            ("FEDKR_S3_BUCKET", "fedkr-test"),
        ]),
    )
    .is_err());
}

#[test]
fn storage_configuration_accepts_explicit_fs_and_validates_s3_contract() {
    let lock_dir = PathBuf::from("/var/lib/fedkr2/media-lock");
    assert!(matches!(
        parse_storage_config(Some(lock_dir.clone()), env(&[])).unwrap(),
        Some(StorageConfig::Fs { .. })
    ));
    assert!(matches!(
        parse_storage_config(
            Some(lock_dir),
            env(&[
                ("FEDKR_MEDIA_BACKEND", "s3"),
                (
                    "FEDKR_S3_ENDPOINT",
                    "https://account.r2.cloudflarestorage.com"
                ),
                ("FEDKR_S3_BUCKET", "fedkr-media"),
                ("FEDKR_S3_REGION", "auto"),
                ("FEDKR_S3_ACCESS_KEY_ID", "synthetic-access"),
                ("FEDKR_S3_SECRET_ACCESS_KEY", "synthetic-secret"),
                ("FEDKR_S3_ROOT", "fedkr2/"),
            ]),
        )
        .unwrap(),
        Some(StorageConfig::S3 { .. })
    ));
}

#[tokio::test]
async fn opendal_deletion_is_idempotent_and_preserves_source_export() {
    let files = Files::new();
    let object = fixture(&files).await;
    let store = ObjectStore::open(&files.bundle).unwrap();
    store.delete_unreferenced(&object).await.unwrap();
    store.delete_unreferenced(&object).await.unwrap();
    assert_eq!(files.objects(), 0);
    assert_eq!(
        std::fs::read(files.source.join("avatars/test.png")).unwrap(),
        b"private synthetic payload"
    );
}

#[tokio::test]
async fn deletion_refuses_noncanonical_hash_wrong_length_and_changed_payload() {
    let files = Files::new();
    let object = fixture(&files).await;
    let store = ObjectStore::open(&files.bundle).unwrap();
    for hash in [
        "../source/avatars/test.png".into(),
        object.hash.to_uppercase(),
        "".into(),
    ] {
        assert_eq!(
            store
                .delete_unreferenced(&ObjectRef {
                    hash,
                    bytes: object.bytes
                })
                .await,
            Err(Error::UnsafePath)
        );
    }
    assert_eq!(
        store
            .delete_unreferenced(&ObjectRef {
                hash: object.hash.clone(),
                bytes: object.bytes + 1
            })
            .await,
        Err(Error::Changed)
    );
    let target = files.bundle.join("objects").join(&object.hash);
    std::fs::write(&target, b"changed private fixture").unwrap();
    assert_eq!(
        store.delete_unreferenced(&object).await,
        Err(Error::Changed)
    );
    assert_eq!(std::fs::read(&target).unwrap(), b"changed private fixture");
}

#[tokio::test]
async fn deletion_does_not_acknowledge_an_absent_store_mount() {
    let files = Files::new();
    let store = ObjectStore::open(&files.bundle).unwrap();
    let object = ObjectRef {
        hash: "a".repeat(64),
        bytes: 1,
    };
    assert_eq!(
        store.delete_unreferenced(&object).await,
        Err(Error::Missing)
    );
    std::fs::create_dir(files.bundle.join("objects")).unwrap();
    store.delete_unreferenced(&object).await.unwrap();
}

#[cfg(unix)]
#[tokio::test]
async fn deletion_refuses_linked_file_and_objects_directory() {
    use std::os::unix::fs::symlink;
    let files = Files::new();
    let object = fixture(&files).await;
    let store = ObjectStore::open(&files.bundle).unwrap();
    let target = files.bundle.join("objects").join(&object.hash);
    std::fs::remove_file(&target).unwrap();
    symlink(files.source.join("avatars/test.png"), &target).unwrap();
    assert_eq!(
        store.delete_unreferenced(&object).await,
        Err(Error::UnsafePath)
    );
    std::fs::remove_file(&target).unwrap();
    std::fs::remove_dir(files.bundle.join("objects")).unwrap();
    symlink(&files.source, files.bundle.join("objects")).unwrap();
    assert_eq!(
        store.delete_unreferenced(&object).await,
        Err(Error::UnsafePath)
    );
    assert_eq!(
        std::fs::read(files.source.join("avatars/test.png")).unwrap(),
        b"private synthetic payload"
    );
    std::fs::remove_file(files.bundle.join("objects")).unwrap();
}

#[tokio::test]
#[ignore = "requires an isolated S3/R2 prefix and the FEDKR_S3_* credentials documented in media-serving.md"]
async fn configured_s3_round_trip_uses_only_one_synthetic_object() {
    let store = ObjectStore::from_env()
        .expect("S3 environment must be complete")
        .expect("S3 environment must enable media storage");
    assert!(store.is_s3(), "set FEDKR_MEDIA_BACKEND=s3 for this test");
    let bytes = format!("fedkr isolated storage test {}", uuid::Uuid::new_v4()).into_bytes();
    let object = ObjectRef::from_bytes(&bytes).unwrap();
    let mutation = store.try_mutation().unwrap();
    assert!(mutation
        .publish(object.clone(), bytes.clone())
        .await
        .unwrap());
    assert_eq!(store.read(&object).await.unwrap(), bytes);
    assert!(!mutation
        .publish(object.clone(), bytes.clone())
        .await
        .unwrap());
    mutation.delete_unreferenced(object.clone()).await.unwrap();
    assert_eq!(store.read(&object).await, Err(Error::Missing));
}
