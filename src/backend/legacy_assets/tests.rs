use super::*;

/// Entirely synthetic files, outside version control, with an exact owned root.
pub(crate) struct Files {
    pub root: PathBuf,
    pub source: PathBuf,
    pub bundle: PathBuf,
    parent: PathBuf,
}
impl Files {
    pub fn new() -> Self {
        let parent = fs::canonicalize(std::env::temp_dir()).unwrap();
        let root = parent.join(format!(
            "fedkr-assets-test-{}",
            uuid::Uuid::new_v4().simple()
        ));
        fs::create_dir(&root).unwrap();
        let source = root.join("source");
        let bundle = root.join("bundle");
        fs::create_dir(&source).unwrap();
        fs::create_dir(&bundle).unwrap();
        Self {
            root,
            source,
            bundle,
            parent,
        }
    }
    pub fn paths(&self) -> Bundle {
        Bundle::new(&self.source, &self.bundle).unwrap()
    }
    pub fn write(&self, name: &str, bytes: &[u8]) {
        key(name).unwrap();
        let path = self.source.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }
    pub fn fixture(&self) {
        self.write("avatars/fixture.png", b"same synthetic bytes");
        self.write("favicons/fixture.png", b"same synthetic bytes");
        self.write("emojis/social.example.com/smile.png", b"emoji bytes");
        self.write(
            "software-logos/fixture.png",
            b"<svg>raw preserved bytes, not public</svg>",
        );
    }
    pub fn objects(&self) -> usize {
        match fs::read_dir(self.bundle.join("objects")) {
            Ok(entries) => entries.count(),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => 0,
            Err(e) => panic!("{e}"),
        }
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        // Never recursively remove a supplied source/bundle or a broad root.
        assert_eq!(self.root.parent(), Some(self.parent.as_path()));
        assert!(self
            .root
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("fedkr-assets-test-"));
        assert_eq!(fs::canonicalize(&self.root).unwrap(), self.root);
        fs::remove_dir_all(&self.root).unwrap();
    }
}

#[test]
fn storage_keys_exclude_traversal_windows_aliases_and_remote_urls() {
    for good in [
        "avatars/@alice@social.example.com.png",
        "emojis/social.example.com/wave.svg",
        "favicons/example.com.ico",
        "software-logos/pixelfed.webp",
        "software/mastodon.svg",
    ] {
        key(good).unwrap();
    }
    for bad in [
        "",
        "/avatars/a.png",
        "avatars/../key",
        "avatars/./key",
        "avatars//key",
        "avatars\\a.png",
        "avatars/a:stream",
        "avatars/%2e%2e/key",
        "avatars/NUL.png",
        "avatars/com1.jpg",
        "avatars/lpt9",
        "avatars/trailing.",
        "avatars/trailing ",
        "https://example.com/a",
        "avatars/a?b",
        "avatars/a\n",
        "avatars/*",
        "unknown/a.png",
    ] {
        assert_eq!(key(bad), Err(Error::UnsafePath), "{bad:?}");
    }
    assert_eq!(
        key(&format!("avatars/{}", "x".repeat(1024))),
        Err(Error::UnsafePath)
    );
}

#[test]
fn roots_must_be_existing_absolute_disjoint_directories() {
    let f = Files::new();
    assert!(matches!(
        Bundle::new(Path::new("relative"), &f.bundle),
        Err(Error::Configuration)
    ));
    assert!(matches!(
        Bundle::new(&f.source, &f.source),
        Err(Error::Configuration)
    ));
    assert!(matches!(
        Bundle::new(&f.root, &f.bundle),
        Err(Error::Configuration)
    ));
    assert!(matches!(
        Bundle::new(&f.source, &f.root),
        Err(Error::Configuration)
    ));
    assert!(matches!(
        Bundle::new(&f.root.join("missing"), &f.bundle),
        Err(Error::Missing)
    ));
    f.write("avatars/a", b"file");
    assert!(matches!(
        Bundle::new(&f.source.join("avatars/a"), &f.bundle),
        Err(Error::UnsafePath)
    ));
}

#[tokio::test]
async fn exact_raw_bytes_empty_files_deduplication_and_tamper_detection() {
    let f = Files::new();
    let paths = f.paths();
    let raw = b"<svg><script>not safe to serve</script></svg>\0\xff";
    f.write("software/raw.svg", raw);
    f.write("avatars/copy.svg", raw);
    f.write("avatars/empty", b"");
    let a = paths.read("software/raw.svg").await.unwrap();
    assert!(!paths.check_destination(&a).await.unwrap());
    assert_eq!(f.objects(), 0);
    assert!(paths.put(&a).await.unwrap());
    assert!(!paths
        .put(&paths.read("avatars/copy.svg").await.unwrap())
        .await
        .unwrap());
    assert!(paths
        .put(&paths.read("avatars/empty").await.unwrap())
        .await
        .unwrap());
    assert_eq!(f.objects(), 2);
    assert_eq!(
        fs::read(f.bundle.join("objects").join(&a.hash)).unwrap(),
        raw
    );
    fs::write(f.bundle.join("objects").join(&a.hash), b"corrupt").unwrap();
    assert_eq!(paths.verify(&a).await, Err(Error::Changed));
    assert_eq!(paths.put(&a).await, Err(Error::Changed));
    assert_eq!(paths.check_destination(&a).await, Err(Error::Changed));
    assert_eq!(
        fs::read(f.bundle.join("objects").join(&a.hash)).unwrap(),
        b"corrupt"
    );
    assert!(matches!(
        paths.read("avatars/missing").await,
        Err(Error::Missing)
    ));
    let large = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(f.source.join("avatars/large"))
        .unwrap();
    large.set_len(MAX_BYTES as u64 + 1).unwrap();
    drop(large);
    assert!(matches!(
        paths.read("avatars/large").await,
        Err(Error::TooLarge)
    ));
}

#[tokio::test]
async fn linked_directory_cannot_escape_the_export_or_bundle() {
    let f = Files::new();
    let paths = f.paths();
    let outside = f.root.join("outside");
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("a"), b"outside").unwrap();
    for link in [f.source.join("avatars"), f.bundle.join("objects")] {
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside, &link).unwrap();
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            let result=std::process::Command::new("powershell.exe").args(["-NoProfile","-NonInteractive","-Command","$ErrorActionPreference='Stop'; New-Item -ItemType Junction -Path $env:FEDKR_TEST_LINK -Target $env:FEDKR_TEST_LINK_TARGET | Out-Null"])
            .env("FEDKR_TEST_LINK",&link).env("FEDKR_TEST_LINK_TARGET",&outside).creation_flags(0x08000000).output().unwrap();
            assert!(result.status.success(), "synthetic junction setup failed");
        }
        if link == f.source.join("avatars") {
            assert!(matches!(
                paths.read("avatars/a").await,
                Err(Error::UnsafePath)
            ));
            assert!(matches!(
                Bundle::new(&link, &f.bundle),
                Err(Error::UnsafePath)
            ));
        } else {
            let object = Object {
                hash: format!("{:x}", Sha256::digest(b"synthetic")),
                bytes: b"synthetic".to_vec(),
            };
            assert_eq!(
                paths.check_destination(&object).await,
                Err(Error::UnsafePath)
            );
            assert_eq!(paths.put(&object).await, Err(Error::UnsafePath));
            assert_eq!(paths.verify(&object).await, Err(Error::UnsafePath));
            assert_eq!(fs::read_dir(&outside).unwrap().count(), 1);
        }
        // Remove only the exact test link, never its target.
        #[cfg(windows)]
        fs::remove_dir(&link).unwrap();
        #[cfg(unix)]
        fs::remove_file(&link).unwrap();
    }
    assert_eq!(fs::read(outside.join("a")).unwrap(), b"outside");
}

#[test]
fn errors_and_report_do_not_expose_source_details() {
    for error in [
        Error::Configuration,
        Error::Database,
        Error::Inventory,
        Error::Missing,
        Error::UnsafePath,
        Error::TooLarge,
        Error::Changed,
        Error::Io,
    ] {
        let text = format!("{error} {error:?}");
        for secret in ["avatars/", "PRIVATE KEY", "postgres://", "@social"] {
            assert!(!text.contains(secret));
        }
    }
}
