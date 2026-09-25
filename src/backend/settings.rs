//! Configuration shared by the explicit migration CLI and the HTTP server.
use super::db::Database;
use url::{Host, Url};

pub struct Config {
    database_url: String,
    pub origin: String,
    pub secure: bool,
}
impl Config {
    pub fn from_env() -> Result<Self, &'static str> {
        let database_url =
            std::env::var("FEDKR_DATABASE_URL").map_err(|_| "FEDKR_DATABASE_URL is required")?;
        let origin =
            std::env::var("FEDKR_PUBLIC_ORIGIN").map_err(|_| "FEDKR_PUBLIC_ORIGIN is required")?;
        let (origin, secure) = canonical_origin(&origin)?;
        Ok(Self {
            database_url,
            origin,
            secure,
        })
    }
    pub async fn connect(&self) -> Result<Database, &'static str> {
        super::db::ensure_runtime_allowed(&self.database_url, Some(&self.origin))
            .await
            .map_err(|_| "Snapshot or quarantined data cannot be used by the web/worker runtime")?;
        Database::connect(&self.database_url)
            .await
            .map_err(|_| "Database connection failed")
    }
    pub async fn migrate(&self) -> Result<(), &'static str> {
        super::db::ensure_runtime_allowed(&self.database_url, Some(&self.origin))
            .await
            .map_err(|_| "Snapshot or quarantined data cannot be used by the web/worker runtime")?;
        Database::migrate(&self.database_url)
            .await
            .map_err(|_| "Embedded migration failed; database changes rolled back")
    }
    /// A narrow development-only target allowlist, not a production migrator.
    #[allow(dead_code)]
    pub fn is_local_database(&self) -> bool {
        local_database(&self.database_url)
    }
}
pub(crate) fn local_database(value: &str) -> bool {
    Url::parse(value).is_ok_and(|u| {
        matches!(u.scheme(), "postgres" | "postgresql")
            && u.host_str() == Some("127.0.0.1")
            && u.port() == Some(16439)
            && matches!(u.path(), "/fedkr_dev" | "/fedkr_test")
            && u.query().is_none()
            && u.fragment().is_none()
    })
}
pub(crate) fn canonical_origin(value: &str) -> Result<(String, bool), &'static str> {
    let u = Url::parse(value).map_err(|_| "Invalid public origin")?;
    let loopback = match u.host() {
        Some(Host::Ipv4(ip)) => ip.is_loopback(),
        Some(Host::Ipv6(ip)) => ip.is_loopback(),
        _ => false,
    };
    if (u.scheme() != "https" && !(u.scheme() == "http" && loopback))
        || u.host_str().is_none()
        || !u.username().is_empty()
        || u.password().is_some()
        || u.path() != "/"
        || u.query().is_some()
        || u.fragment().is_some()
        || u.port() == Some(0)
    {
        return Err("Public origin must be HTTPS (literal loopback HTTP only for development)");
    }
    let origin = u.origin().ascii_serialization();
    if value.trim_end_matches('/') != origin {
        return Err("Public origin must be canonical");
    }
    Ok((origin, u.scheme() == "https"))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn origin_rejects_ambiguous_or_insecure_configuration() {
        assert!(canonical_origin("https://fediverse.kr").is_ok());
        assert!(canonical_origin("http://127.0.0.1:12239").is_ok());
        for invalid in [
            "http://fediverse.kr",
            "http://localhost:12239",
            "https://user@fediverse.kr",
            "https://fediverse.kr/sub",
            "https://fediverse.kr?x",
            "https://fediverse.kr#x",
            "http://2130706433:12239",
        ] {
            assert!(canonical_origin(invalid).is_err(), "{invalid}");
        }
    }
    #[test]
    fn migration_target_is_only_the_named_isolated_cluster() {
        assert!(local_database(
            "postgresql://fedkr_dev@127.0.0.1:16439/fedkr_dev"
        ));
        assert!(local_database(
            "postgresql://fedkr_dev@127.0.0.1:16439/fedkr_test"
        ));
        for invalid in [
            "postgresql://u@db.example:16439/fedkr_dev",
            "postgresql://u@127.0.0.1:5432/fedkr_dev",
            "postgresql://u@127.0.0.1:16439/production",
            "postgresql://u@127.0.0.1:16439/fedkr_dev?host=db.example",
        ] {
            assert!(!local_database(invalid));
        }
    }
}
