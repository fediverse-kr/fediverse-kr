//! One server process owns HTTP, embedded schema setup, and background services.
use super::db::Database;
use super::federation::http_signature::RsaHttpSigner;
pub use super::settings::Config;
use std::sync::Arc;
use tokio::sync::OnceCell;

pub struct State {
    pub config: Config,
    pub db: Database,
    pub signer: Arc<RsaHttpSigner>,
    pub media: Option<super::storage::ObjectStore>,
}
static STATE: OnceCell<State> = OnceCell::const_new();
pub async fn state() -> Result<&'static State, &'static str> {
    STATE
        .get_or_try_init(|| async {
            let config = Config::from_env()?;
            let media = super::storage::ObjectStore::from_env()?;
            config.migrate().await?;
            let db = config.connect().await?;
            db.verify_activated_store(media.as_ref())
                .await
                .map_err(|_| {
                    "Activated database requires its intact private media store before startup"
                })?;
            db.ensure_signing_key()
                .await
                .map_err(|_| "Signing key initialization failed")?;
            let pem = db
                .signing_key()
                .await
                .map_err(|_| "Signing key lookup failed")?;
            let signer = RsaHttpSigner::from_pem(format!("{}/actor#main-key", config.origin), &pem)
                .map_err(|_| "Invalid instance signing key")?;
            Ok(State {
                config,
                db,
                signer: Arc::new(signer),
                media,
            })
        })
        .await
}
