//! Administration uses the same software editor and revision as member edits.
pub mod api;
pub mod pages;
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Software {
    pub name: String,
    pub display_name: String,
    pub revision: i64,
    pub locked: bool,
    pub brand_color: Option<String>,
    pub color_truncated: bool,
    pub featured: bool,
    pub display_order: i32,
    pub logo_available: bool,
    #[serde(default)]
    pub logo: Option<crate::directory::SoftwareMedia>,
    #[serde(default)]
    pub header: Option<crate::directory::SoftwareMedia>,
}
pub const LOGO_MAX_BYTES: usize = 512_000;
pub const HEADER_MAX_BYTES: usize = 8 * 1024 * 1024;
pub const LOGO_BODY_LIMIT: usize = 700_000;
pub const HEADER_BODY_LIMIT: usize = 12_000_000;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaRole {
    Logo,
    Header,
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct MediaRequest {
    pub name: String,
    pub revision: i64,
    pub role: MediaRole,
    pub note: String,
    /// Standard base64. None with metadata updates presentation only;
    /// None without metadata explicitly removes the selected role.
    pub data: Option<String>,
    #[serde(default)]
    pub metadata: Option<crate::directory::SoftwareMediaMetadata>,
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct LogoRequest {
    pub name: String,
    pub revision: i64,
    pub note: String,
    /// Standard base64; None explicitly removes the logo. Never a URL or path.
    pub data: Option<String>,
    #[serde(default)]
    pub metadata: Option<crate::directory::SoftwareMediaMetadata>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SoftwarePage {
    pub items: Vec<Software>,
    pub page: u32,
    pub has_next: bool,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum Action {
    Locked(bool),
    BrandColor(Option<String>),
    Featured(bool),
    DisplayOrder(i32),
}
impl Action {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Locked(true) => "회원 편집 잠금",
            Self::Locked(false) => "회원 편집 잠금 해제",
            Self::BrandColor(_) => "브랜드 색상 변경",
            Self::Featured(_) => "대표 제품 표식 변경",
            Self::DisplayOrder(_) => "표시 순서 변경",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Request {
    pub name: String,
    pub revision: i64,
    pub action: Action,
    pub note: String,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CategoryEdit {
    pub label: String,
    pub emoji: String,
    pub display_order: i32,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Category {
    pub name: String,
    pub revision: i64,
    pub edit: CategoryEdit,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CategoryPage {
    pub items: Vec<Category>,
    pub page: u32,
    pub has_next: bool,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CategoryRequest {
    pub name: String,
    pub revision: Option<i64>,
    pub edit: CategoryEdit,
    pub note: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Event {
    pub revision: i64,
    pub action: String,
    pub note: String,
    pub before: String,
    pub after: String,
    pub truncated: bool,
    pub created_at: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct History {
    pub items: Vec<Event>,
    pub page: u32,
    pub has_next: bool,
}
