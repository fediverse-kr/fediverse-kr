//! Transport models for the signed-in owner, never reused by the public API.
pub mod api;
pub mod pages;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SiteEdit {
    pub name: String,
    pub description: String,
    pub rules: String,
    pub language: String,
    pub tags: String,
    pub owner_comment: String,
    pub invite_only: Option<bool>,
    pub approval_required: Option<bool>,
    pub hidden: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OwnedSite {
    pub domain: String,
    pub revision: i64,
    pub owner_method: Option<String>,
    pub force_hidden: bool,
    pub closed: bool,
    pub edit: SiteEdit,
    pub refresh: Option<RefreshStatus>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RefreshStatus {
    pub state: RefreshState,
    pub requested_at: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefreshState {
    Pending,
    Running,
    Complete,
    Failed,
}
impl RefreshState {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Pending => "갱신 대기 중",
            Self::Running => "서버 정보 확인 중",
            Self::Complete => "서버 정보를 갱신했어요",
            Self::Failed => "갱신 중 가져오지 못한 정보가 있어요. 기존 값은 유지합니다.",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OwnedPage {
    pub sites: Vec<OwnedSite>,
    pub page: u32,
    pub has_next: bool,
}

// Deliberately not Debug: the one-time value is shown only after an explicit POST.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct DnsChallenge {
    pub id: String,
    pub domain: String,
    pub record: String,
    pub value: String,
}
