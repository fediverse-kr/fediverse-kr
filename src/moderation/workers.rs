//! Administrator-only snapshots. No raw worker errors, credentials, or profile data.
pub mod api;
pub mod pages;
use serde::{Deserialize, Serialize};

pub const ISSUE_LIMIT: usize = 20;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorkerOverview {
    pub captured_at: String,
    pub queue: QueueSummary,
    pub job_issues_total: i64,
    pub job_issues: Vec<JobIssue>,
    pub site_issues_total: i64,
    pub site_issues: Vec<SiteIssue>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct QueueSummary {
    pub ready: i64,
    pub scheduled: i64,
    pub paused: i64,
    pub running: i64,
    pub expired: i64,
    pub dead: i64,
    pub completed_24h: i64,
    pub latest_completion: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobIssueKind {
    LeaseExpired,
    Dead,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct JobIssue {
    pub site_id: String,
    pub domain: String,
    pub scheduled_at: String,
    pub attempts: i32,
    pub kind: JobIssueKind,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SiteIssue {
    pub site_id: String,
    pub domain: String,
    pub checked_at: String,
    pub alive: bool,
    pub status_code: Option<i32>,
    pub nodeinfo_failed: bool,
}
