//! Public comments and private controls never carry linked social identities.
pub mod api;
pub mod pages;
pub mod personal;
use serde::{Deserialize, Serialize};

/// Private author history, never returned from the public discussion endpoints.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OwnComment {
    pub id: String,
    pub domain: Option<String>,
    pub server_available: bool,
    pub reply: bool,
    pub body: String,
    pub truncated: bool,
    pub created_at: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OwnCommentPage {
    pub comments: Vec<OwnComment>,
    pub page: u32,
    pub has_next: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Comment {
    pub id: String,
    pub parent_id: Option<String>,
    pub author_name: String,
    pub body: String,
    pub deleted: bool,
    pub truncated: bool,
    pub created_at: String,
    pub updated_at: String,
    pub revision: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Thread {
    pub comment: Comment,
    pub replies: Vec<Comment>,
    pub reply_count: i64,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ThreadPage {
    pub domain: String,
    pub preview: bool,
    pub threads: Vec<Thread>,
    pub page: u32,
    pub has_next: bool,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReplyPage {
    pub domain: String,
    pub parent: String,
    pub replies: Vec<Comment>,
    pub page: u32,
    pub has_next: bool,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Permission {
    pub id: String,
    pub own: bool,
    pub can_edit: bool,
    pub can_report: bool,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Access {
    pub display_name: Option<String>,
    pub available: bool,
    pub permissions: Vec<Permission>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    Spam,
    Abuse,
    Offtopic,
    Other,
}
impl Reason {
    pub fn key(self) -> &'static str {
        match self {
            Self::Spam => "spam",
            Self::Abuse => "abuse",
            Self::Offtopic => "offtopic",
            Self::Other => "other",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Spam => "스팸",
            Self::Abuse => "욕설·비방",
            Self::Offtopic => "주제와 무관",
            Self::Other => "기타",
        }
    }
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "spam" => Some(Self::Spam),
            "abuse" => Some(Self::Abuse),
            "offtopic" => Some(Self::Offtopic),
            "other" => Some(Self::Other),
            _ => None,
        }
    }
}
