//! Database-only TLS policy and private CA loading.
//!
//! This deliberately does not alter HTTP or federation trust stores.

use rustls::{
    pki_types::{
        pem::{PemObject, SectionKind},
        CertificateDer,
    },
    ClientConfig, RootCertStore,
};
use std::{
    env,
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

const DATABASE_CA_FILE: &str = "FEDKR_DATABASE_CA_FILE";
const MAX_CA_BYTES: usize = 256 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TlsPolicy {
    Disable,
    Require,
}

/// Carries the validated database connector parts only.
pub(super) struct Transport {
    pub(super) config: tokio_postgres::Config,
    pub(super) tls: ClientConfig,
}

/// Deliberately has no path, certificate, URL, or operating-system detail.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct TransportError;

pub(super) fn from_environment(url: &str) -> Result<Transport, TransportError> {
    let ca_file = env::var_os(DATABASE_CA_FILE).map(PathBuf::from);
    build(url, ca_file.as_deref())
}

/// Pure input boundary for tests and non-pool connection callers.
pub(super) fn build(url: &str, ca_file: Option<&Path>) -> Result<Transport, TransportError> {
    let policy = tls_policy(url, ca_file.is_some())?;
    let mut config: tokio_postgres::Config = url.parse().map_err(|_| TransportError)?;
    config.ssl_mode(match policy {
        TlsPolicy::Disable => tokio_postgres::config::SslMode::Disable,
        TlsPolicy::Require => tokio_postgres::config::SslMode::Require,
    });
    config.connect_timeout(Duration::from_secs(5));

    let mut roots = RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    if let Some(ca_file) = ca_file {
        add_private_roots(&mut roots, ca_file)?;
    }
    let tls =
        ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .with_safe_default_protocol_versions()
            .map_err(|_| TransportError)?
            .with_root_certificates(roots)
            .with_no_client_auth();
    Ok(Transport { config, tls })
}

/// An explicitly configured CA is an intentional remote/TLS deployment even
/// when its database URL happens to use a literal loopback address.
pub(super) fn tls_policy(
    url: &str,
    private_ca_configured: bool,
) -> Result<TlsPolicy, TransportError> {
    let parsed = url::Url::parse(url).map_err(|_| TransportError)?;
    let literal_loopback = parsed
        .host_str()
        .is_some_and(|host| host == "127.0.0.1" || host == "[::1]" || host == "::1")
        && parsed.query().is_none();
    Ok(if literal_loopback && !private_ca_configured {
        TlsPolicy::Disable
    } else {
        TlsPolicy::Require
    })
}

fn add_private_roots(roots: &mut RootCertStore, path: &Path) -> Result<(), TransportError> {
    let pem = read_private_ca(path)?;
    // Decode every PEM section first. CertificateDer's iterator intentionally
    // skips other section kinds, so this separate generic PemObject pass makes
    // a key/CSR/CRL bundle fail closed rather than silently ignoring it.
    let sections = <(SectionKind, Vec<u8>)>::pem_slice_iter(&pem)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| TransportError)?;
    if sections.is_empty()
        || sections
            .iter()
            .any(|(kind, _)| *kind != SectionKind::Certificate)
    {
        return Err(TransportError);
    }
    let certificates = CertificateDer::pem_slice_iter(&pem)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| TransportError)?;
    if certificates.len() != sections.len() {
        return Err(TransportError);
    }
    for certificate in certificates {
        roots.add(certificate).map_err(|_| TransportError)?;
    }
    Ok(())
}

fn read_private_ca(path: &Path) -> Result<Vec<u8>, TransportError> {
    if !path.is_absolute() {
        return Err(TransportError);
    }
    let metadata = fs::metadata(path).map_err(|_| TransportError)?;
    if !metadata.file_type().is_file()
        || metadata.len() == 0
        || metadata.len() > MAX_CA_BYTES as u64
    {
        return Err(TransportError);
    }
    let file = File::open(path).map_err(|_| TransportError)?;
    let mut pem = Vec::with_capacity(metadata.len() as usize);
    file.take(MAX_CA_BYTES as u64 + 1)
        .read_to_end(&mut pem)
        .map_err(|_| TransportError)?;
    if pem.is_empty() || pem.len() > MAX_CA_BYTES {
        return Err(TransportError);
    }
    Ok(pem)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs::OpenOptions, io::Write};

    const LOOPBACK: &str = "postgresql://fedkr@127.0.0.1/fedkr";

    struct TestFile(PathBuf);
    impl Drop for TestFile {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.0);
        }
    }

    fn fixture(contents: &[u8]) -> TestFile {
        let path = env::temp_dir().join(format!("fedkr-db-ca-{}", uuid::Uuid::new_v4()));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .expect("create unique synthetic CA fixture");
        file.write_all(contents)
            .expect("write synthetic CA fixture");
        TestFile(path)
    }

    #[test]
    fn loopback_without_a_private_ca_preserves_plain_local_policy() {
        assert_eq!(tls_policy(LOOPBACK, false), Ok(TlsPolicy::Disable));
    }

    #[test]
    fn explicit_private_ca_forces_tls_even_on_loopback() {
        assert_eq!(tls_policy(LOOPBACK, true), Ok(TlsPolicy::Require));
    }

    #[test]
    fn non_loopback_always_requires_tls() {
        assert_eq!(
            tls_policy("postgresql://fedkr@db.internal/fedkr", false),
            Ok(TlsPolicy::Require)
        );
    }

    #[test]
    fn explicit_hostaddr_remains_a_valid_tls_transport_configuration() {
        let transport = build(
            "postgresql://fedkr@localhost/fedkr?hostaddr=127.0.0.1",
            None,
        )
        .unwrap();
        assert_eq!(transport.config.get_hostaddrs().len(), 1);
        assert_eq!(
            transport.config.get_ssl_mode(),
            tokio_postgres::config::SslMode::Require
        );
    }

    #[test]
    fn private_ca_rejects_relative_empty_missing_and_oversized_inputs() {
        assert!(build(LOOPBACK, Some(Path::new("ca.pem"))).is_err());
        let empty = fixture(b"");
        assert!(build(LOOPBACK, Some(&empty.0)).is_err());
        let missing = env::temp_dir().join(format!("fedkr-db-ca-missing-{}", uuid::Uuid::new_v4()));
        assert!(build(LOOPBACK, Some(&missing)).is_err());
        let oversized = fixture(&vec![b'x'; MAX_CA_BYTES + 1]);
        assert!(build(LOOPBACK, Some(&oversized.0)).is_err());
    }

    #[test]
    fn private_ca_rejects_non_certificate_pem() {
        let key = fixture(b"-----BEGIN PRIVATE KEY-----\nAA==\n-----END PRIVATE KEY-----\n");
        assert!(build(LOOPBACK, Some(&key.0)).is_err());
    }

    #[test]
    fn private_ca_rejects_malformed_pem() {
        let malformed = fixture(b"-----BEGIN CERTIFICATE-----\nnot base64\n");
        assert!(build(LOOPBACK, Some(&malformed.0)).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn private_ca_loader_accepts_kubernetes_secret_projection_symlink() {
        let target = fixture(b"synthetic CA bytes");
        let link = TestFile(
            env::temp_dir().join(format!("fedkr-db-ca-projection-{}", uuid::Uuid::new_v4())),
        );
        std::os::unix::fs::symlink(&target.0, &link.0)
            .expect("create synthetic Secret projection symlink");
        assert_eq!(
            read_private_ca(&link.0).expect("follow Secret projection symlink"),
            b"synthetic CA bytes"
        );
    }
}
