use serde::{Deserialize, Serialize};
pub mod api;
pub mod pages;

/// The provider's base of operations, not its data-center location.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperatorRegion {
    Korea,
    Overseas,
    #[default]
    Unknown,
}
impl OperatorRegion {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Korea => "korea",
            Self::Overseas => "overseas",
            Self::Unknown => "unknown",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Korea => "한국 서비스",
            Self::Overseas => "해외 서비스",
            Self::Unknown => "운영 지역 미확인",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct HostingEdit {
    pub name: String,
    pub website_url: String,
    pub scope: String,
    pub software: String,
    pub provider_responsibilities: String,
    pub customer_responsibilities: String,
    pub source_url: String,
    pub checked_on: String,
    #[serde(default)]
    pub operator_region: OperatorRegion,
    #[serde(default)]
    pub korean_support: Option<bool>,
    #[serde(default)]
    pub classification_source_url: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HostingService {
    pub slug: String,
    pub revision: i64,
    pub edit: HostingEdit,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HostingPage {
    pub items: Vec<HostingService>,
    pub page: u32,
    /// Pin the same KST day while paging, even across midnight.
    pub rotation_day: u32,
    pub has_next: bool,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HostingRevision {
    pub revision: i64,
    pub action: String,
    pub summary: String,
    pub created_at: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HostingHistory {
    pub entries: Vec<HostingRevision>,
    pub page: u32,
    pub has_next: bool,
}
#[cfg(test)]
mod tests {
    use crate::Route;
    #[test]
    fn hosting_classification_survives_serialization_and_old_snapshots_remain_readable() {
        let old = serde_json::to_value(super::HostingEdit::default()).unwrap();
        let mut tagged = old.clone();
        tagged["operator_region"] = serde_json::json!("korea");
        tagged["korean_support"] = serde_json::json!(true);
        tagged["classification_source_url"] = serde_json::json!("https://example.org/about");
        let saved: super::HostingEdit = serde_json::from_value(tagged).unwrap();
        let roundtrip = serde_json::to_value(saved).unwrap();
        assert_eq!(roundtrip["operator_region"], "korea");
        assert_eq!(roundtrip["korean_support"], true);
        assert_eq!(
            roundtrip["classification_source_url"],
            "https://example.org/about"
        );
        let mut legacy = old;
        for field in [
            "operator_region",
            "korean_support",
            "classification_source_url",
        ] {
            legacy.as_object_mut().unwrap().remove(field);
        }
        let restored: super::HostingEdit = serde_json::from_value(legacy).unwrap();
        let restored = serde_json::to_value(restored).unwrap();
        assert_eq!(restored["operator_region"], "unknown");
        assert!(restored["korean_support"].is_null());
    }

    #[test]
    fn managed_hosting_urls_are_distinct_and_discoverable() {
        assert_eq!(Route::HostingList {}.to_string(), "/hosting");
        assert_eq!(
            Route::HostingDetail {
                slug: "example".into()
            }
            .to_string(),
            "/hosting/example"
        );
        assert_eq!(
            Route::HostingHistory {
                slug: "example".into()
            }
            .to_string(),
            "/hosting/example/history"
        );
        assert_eq!(Route::HostingNew {}.to_string(), "/account/hosting/new");
    }
}
