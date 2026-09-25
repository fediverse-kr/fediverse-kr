pub mod api;
#[cfg(feature = "server")]
pub mod http;
pub mod pages;
pub mod profile;
use serde::{Deserialize, Serialize};

#[derive(Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ProfileMedia {
    pub avatar_available: bool,
    pub emojis: Vec<String>,
    #[serde(default)]
    pub revision: i64,
    #[serde(default)]
    pub refreshing: bool,
    #[serde(default)]
    pub refresh_failed: bool,
    #[serde(default)]
    pub source_account_id: Option<String>,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct Member {
    pub id: String,
    pub login_id: Option<String>,
    pub display_name: String,
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct LinkedAccount {
    pub id: String,
    pub handle: String,
    pub profile_url: String,
    pub display_name: String,
    pub is_public: bool,
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct AccountState {
    pub member: Option<Member>,
    pub linked_accounts: Vec<LinkedAccount>,
    pub federation_available: bool,
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct Challenge {
    pub id: String,
    pub code: String,
    pub handle: String,
    pub expires_at: String,
}
