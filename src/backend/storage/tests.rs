use super::*;
use crate::backend::legacy_assets::tests::Files;

async fn fixture(files: &Files) -> ObjectRef {
    files.write("avatars/test.png", b"private synthetic payload");
    let object = files.paths().read("avatars/test.png").await.unwrap();
    files.paths().put(&object).await.unwrap();
    ObjectRef {
        hash: object.hash,
        bytes: object.bytes.len() as i64,
    }
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
