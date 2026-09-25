//! Public read models. No credentials, ownership claims, or moderation notes.
pub mod api;
pub mod catalog;
pub mod catalog_editing;
pub mod health;
pub mod management;
pub mod pages;
mod preview;
pub(crate) mod ranking;
pub mod registration;
#[cfg(feature = "server")]
pub(crate) mod rich_text;
pub mod search;
use serde::{Deserialize, Serialize};

pub const PAGE_SIZE: usize = 24;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Category {
    pub name: String,
    pub label: String,
    pub emoji: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Software {
    pub logo_available: bool,
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub description_html: String,
    pub categories: Vec<String>,
    pub features: Vec<String>,
    pub website_url: Option<String>,
    pub tech_stack: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Catalog {
    pub preview: bool,
    pub categories: Vec<Category>,
    pub software: Vec<Software>,
    pub truncated: bool,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SoftwareInfo {
    pub preview: bool,
    pub software: Option<Software>,
    pub categories: Vec<Category>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PublicSite {
    pub icon_available: bool,
    #[serde(default)]
    pub guidance: SiteGuidance,
    pub domain: String,
    pub name: String,
    pub description: String,
    pub description_html: String,
    #[serde(default)]
    pub software: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    pub closed: bool,
    #[serde(default)]
    pub alive: Option<bool>,
    #[serde(default)]
    pub registration_open: Option<bool>,
    #[serde(default)]
    pub users: Option<i64>,
    #[serde(default)]
    pub active_users: Option<i64>,
    #[serde(default)]
    pub checked_at: Option<String>,
    #[serde(default)]
    pub metadata_checked_at: Option<String>,
    #[serde(default)]
    pub average_response_ms: Option<i32>,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SiteGuidance {
    pub rules: String,
    pub language: String,
    pub tags: Vec<String>,
    pub owner_comment: String,
    #[serde(default)]
    pub invite_only: Option<bool>,
    #[serde(default)]
    pub approval_required: Option<bool>,
}
impl PublicSite {
    pub fn status(&self) -> &'static str {
        if self.closed {
            "운영 종료"
        } else {
            match self.alive {
                Some(true) => "최근 응답 확인",
                Some(false) => "최근 응답 없음",
                None => "아직 확인 전",
            }
        }
    }
    pub fn registration(&self) -> &'static str {
        if self.closed {
            "운영 종료"
        } else if self.guidance.invite_only == Some(true) {
            "초대 필요"
        } else if self.registration_open == Some(true)
            && self.guidance.approval_required == Some(true)
        {
            "가입 승인 필요"
        } else {
            match self.registration_open {
                Some(true) => "가입 접수 중",
                Some(false) => "가입 접수 닫힘",
                None => "가입 방식 확인 필요",
            }
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SitePage {
    pub preview: bool,
    pub sites: Vec<PublicSite>,
    pub page: u32,
    pub has_next: bool,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SiteDetail {
    pub preview: bool,
    pub site: Option<PublicSite>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Statistics {
    pub preview: bool,
    pub sites: i64,
    pub accounts: Option<i64>,
    pub counted_sites: i64,
    pub oldest_observation: Option<String>,
}

/// HTTP links only; reject userinfo, escapes, whitespace and non-web schemes.
/// This validates navigation, not server-side fetching (which has stricter SSRF checks).
pub fn web_link(input: &str) -> Option<String> {
    let rest = input
        .strip_prefix("https://")
        .or_else(|| input.strip_prefix("http://"))?;
    if input.chars().any(|c| c.is_control() || c.is_whitespace()) || input.contains('\\') {
        return None;
    }
    let authority = rest.split(['/', '?', '#']).next()?;
    if authority.is_empty() || authority.contains(['@', '%']) {
        return None;
    }
    Some(input.to_owned())
}

pub fn site_link(domain: &str) -> Option<String> {
    if domain.is_empty() || domain.contains(['/', ':', '?', '#', '@', '%', '\\']) {
        return None;
    }
    web_link(&format!("https://{domain}/"))
}

pub fn number(value: i64) -> String {
    let text = value.to_string();
    let mut result = String::new();
    for (i, ch) in text.chars().enumerate() {
        if i > 0 && (text.len() - i) % 3 == 0 && ch != '-' {
            result.push(',');
        }
        result.push(ch);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn navigation_is_not_arbitrary_markup_or_scheme() {
        for value in [
            "javascript:alert(1)",
            "data:text/html,hi",
            "https://",
            "https://safe.example@evil.example",
            "https://x\\evil",
            "https://x\n",
        ] {
            assert!(web_link(value).is_none(), "{value:?}");
        }
        assert!(web_link("https://example.org/path?q=hello#read").is_some());
        assert!(site_link("evil.example/path").is_none());
    }
    #[test]
    fn totals_are_formatted_without_calling_them_people() {
        assert_eq!(number(97269), "97,269");
        assert_eq!(number(0), "0");
    }

    #[test]
    fn nodeinfo_less_public_site_deserializes_without_nullable_fields() {
        let site: PublicSite = serde_json::from_value(serde_json::json!({
            "icon_available": false,
            "domain": "oeee.cafe",
            "name": "oeee.cafe",
            "description": "",
            "description_html": "",
            "closed": false
        }))
        .expect("minimal public site should deserialize");

        assert_eq!(site.software, None);
        assert_eq!(site.version, None);
        assert_eq!(site.alive, None);
        assert_eq!(site.registration_open, None);
        assert_eq!(site.users, None);
        assert_eq!(site.active_users, None);
        assert_eq!(site.checked_at, None);
        assert_eq!(site.metadata_checked_at, None);
        assert_eq!(site.average_response_ms, None);
        assert_eq!(site.guidance.invite_only, None);
        assert_eq!(site.guidance.approval_required, None);
    }

    #[cfg(feature = "server")]
    #[test]
    fn public_description_renders_markdown_and_sanitizes_raw_html() {
        let rendered = rich_text::render_description(
            "## 소개\n\n**굵게** 쓰고 <em>기울여</em>요. [안전한 링크](https://example.org)\n\n<script>alert(1)</script><a href=\"java&#x73;cript:alert(1)\" onclick=\"alert(1)\">나쁜 링크</a><iframe src=\"https://evil.example\"></iframe>",
        );

        assert!(rendered.html.contains("<h2>소개</h2>"));
        assert!(rendered.html.contains("<strong>굵게</strong>"));
        assert!(rendered.html.contains("<em>기울여</em>"));
        assert!(rendered.html.contains("href=\"https://example.org\""));
        for forbidden in ["script", "onclick", "javascript:", "iframe"] {
            assert!(!rendered.html.contains(forbidden), "{forbidden}");
        }
        assert!(rendered.plain.contains("소개"));
        assert!(rendered.plain.contains("굵게"));
        assert!(!rendered.plain.contains('<'));
    }
}
