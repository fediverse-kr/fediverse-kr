//! Private transport models; no federated addresses or login credentials.
pub mod api;
pub mod catalog;
pub mod layout;
pub mod overview;
pub mod pages;
pub mod profiles;
pub mod sites;
pub mod workers;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProfileBatch {
    pub id: String,
    pub state: String,
    pub total: i64,
    pub pending: i64,
    pub running: i64,
    pub succeeded: i64,
    pub partial: i64,
    pub failed: i64,
    pub skipped: i64,
    pub cancelled: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Resolve,
    Dismiss,
    Reopen,
    DeleteComment,
    Ban,
    Unban,
}
impl Action {
    pub fn key(self) -> &'static str {
        match self {
            Self::Resolve => "resolve",
            Self::Dismiss => "dismiss",
            Self::Reopen => "reopen",
            Self::DeleteComment => "delete_comment",
            Self::Ban => "ban",
            Self::Unban => "unban",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Resolve => "처리 완료",
            Self::Dismiss => "신고 기각",
            Self::Reopen => "다시 검토",
            Self::DeleteComment => "댓글 삭제 표시",
            Self::Ban => "작성자 이용 차단",
            Self::Unban => "작성자 차단 해제",
        }
    }
    pub fn explanation(self) -> &'static str {
        match self {
        Self::Resolve=>"이 신고를 처리 완료로 표시합니다. 댓글이나 회원 상태는 바꾸지 않습니다.",
        Self::Dismiss=>"이 신고를 기각합니다. 댓글이나 회원 상태는 바꾸지 않습니다.",
        Self::Reopen=>"이 신고를 다시 미처리 상태로 돌립니다. 이전 조치 기록은 남습니다.",
        Self::DeleteComment=>"댓글을 공개 화면에서 숨깁니다. 원문과 답글은 보존하며 신고는 따로 처리합니다.",
        Self::Ban=>"이 회원의 fediverse.kr 이용을 차단하고 모든 로그인을 해제합니다. 외부 SNS 계정에는 영향이 없습니다.",
        Self::Unban=>"fediverse.kr 이용 차단을 해제합니다. 다시 로그인해야 하며 삭제된 댓글은 복구하지 않습니다.",
    }
    }
}
pub fn status_label(status: &str) -> &str {
    match status {
        "pending" => "미처리",
        "resolved" => "처리 완료",
        "dismissed" => "기각",
        "all" => "전체",
        _ => "기존 상태",
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReportSummary {
    pub id: String,
    pub status: String,
    pub reason: String,
    pub domain: Option<String>,
    pub reporter_name: Option<String>,
    pub excerpt: String,
    pub created_at: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReportPage {
    pub reports: Vec<ReportSummary>,
    pub page: u32,
    pub has_next: bool,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReportComment {
    pub id: String,
    pub revision: String,
    pub body: String,
    pub author_id: Option<String>,
    pub author_name: Option<String>,
    pub author_banned: Option<bool>,
    pub author_revision: Option<String>,
    pub author_admin: bool,
    pub deleted: bool,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Evidence {
    pub body: String,
    pub author_name: String,
    pub saved_revision: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Event {
    pub id: String,
    pub action: String,
    pub actor_name: Option<String>,
    pub before: String,
    pub after: String,
    pub note: String,
    pub created_at: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EventPage {
    pub events: Vec<Event>,
    pub page: u32,
    pub has_next: bool,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReportDetail {
    pub summary: ReportSummary,
    pub revision: String,
    pub detail: String,
    pub detail_truncated: bool,
    pub admin_note: String,
    pub note_truncated: bool,
    pub resolver_name: Option<String>,
    pub resolved_at: Option<String>,
    pub comment: Option<ReportComment>,
    pub evidence: Option<Evidence>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ActionRequest {
    pub report: String,
    pub revision: String,
    pub comment_revision: Option<String>,
    pub author_id: Option<String>,
    pub author_banned: Option<bool>,
    pub author_revision: Option<String>,
    pub action: Action,
    pub note: String,
}
