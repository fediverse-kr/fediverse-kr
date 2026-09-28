use serde::{Deserialize, Serialize};
pub mod api;
pub mod pages;

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
