//! Shareable catalog browsing criteria; URL encoding belongs to the url crate.
use super::{Category, Software};
use serde::{Deserialize, Serialize};
pub const SOFTWARE_PAGE_SIZE: usize = 24;
pub const CATEGORY_PAGE_SIZE: usize = 12;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CatalogQuery {
    pub query: String,
    pub category: String,
    pub page: u32,
    pub kind_page: u32,
    pub invalid: bool,
}
impl CatalogQuery {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.invalid
            || self.query.len() > 256
            || self.category.len() > 128
            || self.page > 10_000
            || self.kind_page > 10_000
            || self
                .query
                .chars()
                .chain(self.category.chars())
                .any(char::is_control)
        {
            Err("검색 조건을 확인해 주세요.")
        } else {
            Ok(())
        }
    }
    pub fn parse(raw: &str) -> Result<Self, &'static str> {
        if raw.len() > 2048 {
            return Err("검색 조건이 너무 길어요.");
        }
        let mut result = Self::default();
        let mut keys = std::collections::HashSet::new();
        for (key, value) in url::form_urlencoded::parse(raw.as_bytes()) {
            if !keys.insert(key.to_string()) {
                return Err("검색 조건이 중복됐어요.");
            }
            match key.as_ref() {
                "q" => result.query = value.into_owned(),
                "cat" => result.category = value.into_owned(),
                "page" | "kind_page" => {
                    let page = value
                        .parse::<u32>()
                        .ok()
                        .and_then(|n| n.checked_sub(1))
                        .ok_or("페이지 번호를 확인해 주세요.")?;
                    if key == "page" {
                        result.page = page;
                    } else {
                        result.kind_page = page;
                    }
                }
                _ => {}
            }
        }
        result.validate()?;
        result.query = result.query.trim().into();
        Ok(result)
    }
    pub fn category(&self, category: String) -> Self {
        Self {
            category,
            page: 0,
            ..self.clone()
        }
    }
}
impl From<&str> for CatalogQuery {
    fn from(raw: &str) -> Self {
        Self::parse(raw).unwrap_or(Self {
            invalid: true,
            ..Self::default()
        })
    }
}
impl std::fmt::Display for CatalogQuery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.invalid {
            return f.write_str("page=invalid");
        }
        let mut query = url::form_urlencoded::Serializer::new(String::new());
        if !self.query.is_empty() {
            query.append_pair("q", &self.query);
        }
        if !self.category.is_empty() {
            query.append_pair("cat", &self.category);
        }
        if self.page > 0 {
            query.append_pair("page", &self.page.saturating_add(1).to_string());
        }
        if self.kind_page > 0 {
            query.append_pair("kind_page", &self.kind_page.saturating_add(1).to_string());
        }
        f.write_str(&query.finish())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CategoryPage {
    pub categories: Vec<Category>,
    pub total: i64,
    pub page: u32,
    pub has_next: bool,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CatalogPage {
    pub preview: bool,
    pub software: Vec<Software>,
    pub total: i64,
    pub page: u32,
    pub has_next: bool,
    pub kinds: CategoryPage,
    /// Labels for this page's cards/selection, independently of the kinds page.
    pub labels: Vec<Category>,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn catalog_urls_roundtrip_and_reject_ambiguous_or_excessive_criteria() {
        let q = CatalogQuery::parse(
            "q=%EA%B3%A0%EC%96%91%EC%9D%B4%25_%2B&cat=photo&page=3&kind_page=2",
        )
        .unwrap();
        assert_eq!(q.query, "고양이%_+");
        assert_eq!((q.page, q.kind_page), (2, 1));
        assert_eq!(CatalogQuery::parse(&q.to_string()).unwrap(), q);
        assert_eq!(q.category("video".into()).page, 0);
        for raw in [
            "q=a&q=b",
            "q=%00",
            "cat=%0A",
            "page=0",
            "kind_page=-1",
            "page=10002",
            "kind_page=10002",
            "page=hi",
        ] {
            assert!(CatalogQuery::parse(raw).is_err(), "{raw}");
        }
        assert!(CatalogQuery::parse(&format!("q={}", "a".repeat(257))).is_err());
        assert_eq!(CatalogQuery::default().to_string(), "");
        assert!(CatalogQuery::from("page=0").invalid);
    }
}
