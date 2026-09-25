//! Private, bounded administrator views. No federated identities or raw snapshots.
pub mod api;
pub mod pages;
use crate::directory::management::RefreshStatus;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Site {
    pub id: String,
    pub domain: String,
    pub name: String,
    pub revision: i64,
    pub hidden: bool,
    pub force_hidden: bool,
    pub closed: bool,
    pub invite_only: Option<bool>,
    pub tags: String,
    pub tags_truncated: bool,
    pub software: Option<String>,
    pub users: Option<i64>,
    pub owner_name: Option<String>,
    pub refresh: Option<RefreshStatus>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SitePage {
    pub sites: Vec<Site>,
    pub page: u32,
    pub has_next: bool,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DeleteImpact {
    pub site: Site,
    pub comments: i64,
    pub reports: i64,
    pub health_checks: i64,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DeleteRequest {
    pub id: String,
    pub revision: i64,
    pub domain: String,
    pub note: String,
    pub confirmed: bool,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum Action {
    Hidden(bool),
    ForceHidden(bool),
    Closed(bool),
    InviteOnly(Option<bool>),
    Tags(String),
    Refresh,
}
impl Action {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Hidden(true) => "일반 숨김",
            Self::Hidden(false) => "일반 숨김 해제",
            Self::ForceHidden(true) => "강제 숨김",
            Self::ForceHidden(false) => "강제 숨김 해제",
            Self::Closed(true) => "운영 종료로 표시",
            Self::Closed(false) => "운영 중으로 표시",
            Self::InviteOnly(Some(true)) => "초대제로 표시",
            Self::InviteOnly(Some(false)) => "초대제 해제",
            Self::InviteOnly(None) => "초대제 미확인으로 표시",
            Self::Tags(_) => "태그 수정",
            Self::Refresh => "지금 수집 요청",
        }
    }
    pub fn explanation(&self) -> &'static str {
        match self {
            Self::Hidden(_) => "일반 숨김만 바꿉니다. 서버 운영자도 이 상태를 변경할 수 있습니다. 강제 숨김과 수집 여부는 바뀌지 않습니다.",
            Self::ForceHidden(_) => "관리자 강제 숨김만 바꿉니다. 운영자는 강제 숨김을 해제할 수 없습니다. 일반 숨김과 수집 여부는 바뀌지 않습니다.",
            Self::Closed(_) => "운영 종료 여부만 바꿉니다. 종료한 서버는 정기 수집과 운영 중 통계에서 빠지지만, 별도로 숨기지 않으면 목록에는 남습니다.",
            Self::InviteOnly(_) => "안내에 쓰이는 초대제 여부만 바꿉니다. 실제 서버의 가입 설정을 변경하지 않습니다.",
            Self::Tags(_) => "태그만 교체합니다. 소개·규칙·코멘트·소유권은 유지합니다. 쉼표로 구분하며 전체 256자, 태그당 32자, 최대 12개입니다.",
            Self::Refresh => "기존 작업 큐에 한 번 수집을 요청합니다. 서버별 1시간 제한은 운영자 요청과 공유합니다. 종료 서버도 이번 한 번만 확인하며 운영 상태는 유지합니다.",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Request {
    pub id: String,
    pub revision: i64,
    pub action: Action,
    pub note: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Change {
    pub field: String,
    pub before: String,
    pub after: String,
    pub truncated: bool,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Event {
    pub id: String,
    pub revision: i64,
    pub actor_name: Option<String>,
    pub note: String,
    pub change: Change,
    pub created_at: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct History {
    pub events: Vec<Event>,
    pub page: u32,
    pub has_next: bool,
}
