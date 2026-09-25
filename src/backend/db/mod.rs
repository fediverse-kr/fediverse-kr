//! Diesel is an infrastructure detail: callers receive domain models, not rows,
//! SQL strings, connections, or Diesel expressions.
mod catalog_editing;
mod community;
mod directory;
#[cfg(test)]
pub(crate) mod fixtures;
mod legacy;
mod media;
mod media_cleanup;
mod members;
mod moderation;
mod profile_refresh;
mod public_directory;
mod site_management;
mod site_registration;
mod transport;
mod worker_monitor;
pub(crate) use legacy::activation::activate_legacy;
pub(crate) use legacy::assets::preserve_legacy_assets;
pub(crate) use legacy::ensure_runtime_allowed;
pub(crate) use legacy::import_snapshot;
mod migrations;
mod schema;

use diesel::result::ConnectionError;
use diesel_async::{
    pooled_connection::{bb8::Pool, AsyncDieselConnectionManager, ManagerConfig},
    AsyncPgConnection,
};
use std::time::Duration;

#[derive(Clone)]
pub struct Database {
    pool: Pool<AsyncPgConnection>,
}

/// Never includes SQL, remote messages, credentials or query parameters.
#[derive(Debug, Clone, Copy)]
pub struct StoreError;
impl From<diesel::result::Error> for StoreError {
    fn from(_: diesel::result::Error) -> Self {
        Self
    }
}
impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Database operation failed")
    }
}
impl std::error::Error for StoreError {}

pub(super) async fn connect(url: &str) -> Result<AsyncPgConnection, ConnectionError> {
    // diesel-async's default establish() uses NoTls. Supply an explicitly
    // validating TLS connector; remote connections cannot silently downgrade.
    let transport = transport::from_environment(url).map_err(|_| {
        ConnectionError::BadConnection("Database transport configuration failed".into())
    })?;
    let (client, connection) = transport
        .config
        .connect(tokio_postgres_rustls::MakeRustlsConnect::new(transport.tls))
        .await
        .map_err(|_| ConnectionError::BadConnection("Database connection failed".into()))?;
    let mut connection =
        AsyncPgConnection::try_from_client_and_connection(client, connection).await?;
    use diesel_async::SimpleAsyncConnection;
    connection.batch_execute("SET statement_timeout = '15s'; SET lock_timeout = '5s'; SET idle_in_transaction_session_timeout = '15s';").await
        .map_err(|_| ConnectionError::BadConnection("Database session setup failed".into()))?;
    Ok(connection)
}

impl Database {
    pub async fn ping(&self) -> Result<(), StoreError> {
        use diesel_async::RunQueryDsl;
        let mut conn = self.pool.get().await.map_err(|_| StoreError)?;
        diesel::sql_query("SELECT 1").execute(&mut conn).await?;
        Ok(())
    }
    pub async fn connect(url: &str) -> Result<Self, StoreError> {
        let mut config = ManagerConfig::default();
        config.custom_setup = Box::new(|url| Box::pin(connect(url)));
        let manager = AsyncDieselConnectionManager::new_with_config(url, config);
        let pool = Pool::builder()
            .max_size(8)
            .connection_timeout(Duration::from_secs(5))
            .build(manager)
            .await
            .map_err(|_| StoreError)?;
        Ok(Self { pool })
    }
    pub async fn migrate(url: &str) -> Result<(), StoreError> {
        migrations::run(url).await
    }
}
