//! Site directory domain: editorial text is not crawler-owned data.
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct CrawlJob {
    pub id: Uuid,
    pub site_id: Uuid,
    pub domain: String,
    pub lease_token: Uuid,
    pub needs_icon: bool,
}
#[derive(Clone, Debug, Default)]
pub struct NodeInfo {
    pub software: Option<String>,
    pub version: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub registration_open: Option<bool>,
    pub users: Option<i64>,
    pub active_users: Option<i64>,
    pub posts: Option<i64>,
}
#[derive(Clone, Debug)]
pub struct Icon {
    pub mime: &'static str,
    pub bytes: Vec<u8>,
}
#[derive(Clone, Debug)]
pub struct Observation {
    pub alive: bool,
    pub response_ms: Option<i32>,
    pub status: Option<i32>,
    pub health_error: Option<String>,
    pub checked_at: DateTime<Utc>,
    pub nodeinfo: Option<NodeInfo>,
    pub nodeinfo_error: Option<String>,
    pub icon: Option<Icon>,
    /// True only when the bounded icon collection phase completed.
    pub icon_collection_complete: bool,
}
impl Observation {
    pub fn failed(error: &str) -> Self {
        Self {
            alive: false,
            response_ms: None,
            status: None,
            health_error: Some(error.to_owned()),
            checked_at: Utc::now(),
            nodeinfo: None,
            nodeinfo_error: None,
            icon: None,
            icon_collection_complete: false,
        }
    }
}

/// For controlled imports only; no public registration or editing policy yet.
pub fn canonical_domain(input: &str) -> Result<String, &'static str> {
    let url = super::federation::transport::safe_url(&format!("https://{input}/"))
        .map_err(|_| "Invalid public site domain")?;
    if url.path() != "/" || url.query().is_some() || input.contains(['/', ':', '@', '?', '#']) {
        return Err("Supply a domain, not a URL");
    }
    Ok(url.domain().ok_or("Missing site domain")?.to_owned())
}
