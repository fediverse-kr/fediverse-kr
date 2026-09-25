//! Shareable, bounded directory criteria. No recommendation scores or inferred data.
use super::{Category, SitePage};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerQuery {
    pub query: String,
    pub category: String,
    pub family: String,
    pub software: String,
    pub tag: String,
    pub registration: String,
    pub sort: String,
    pub direction: String,
    pub alive: String,
    pub page: u32,
    pub page_size: usize,
    pub invalid: bool,
}
impl Default for ServerQuery {
    fn default() -> Self {
        Self {
            query: String::new(),
            category: String::new(),
            family: String::new(),
            software: String::new(),
            tag: String::new(),
            registration: "all".into(),
            sort: "recommended".into(),
            direction: "desc".into(),
            alive: "all".into(),
            page: 0,
            page_size: 24,
            invalid: false,
        }
    }
}
impl ServerQuery {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.invalid
            || self.query.len() > 256
            || self.category.len() > 128
            || self.family.len() > 128
            || self.software.len() > 128
            || self.tag.len() > 128
            || self.page > 10_000
            || ![24, 25, 50, 100, 200].contains(&self.page_size)
            || [
                &self.query,
                &self.category,
                &self.family,
                &self.software,
                &self.tag,
            ]
            .iter()
            .any(|s| s.chars().any(char::is_control))
            || ![
                "all",
                "open",
                "approval",
                "invite_only",
                "closed",
                "unknown",
            ]
            .contains(&self.registration.as_str())
            || ![
                "recommended",
                "name",
                "domain",
                "users",
                "recent",
                "newest",
                "response_time",
            ]
            .contains(&self.sort.as_str())
            || !["asc", "desc"].contains(&self.direction.as_str())
            || !["all", "yes", "no", "unknown", "closed"].contains(&self.alive.as_str())
        {
            Err("검색 조건을 확인해 주세요.")
        } else {
            Ok(())
        }
    }
    pub fn parse(raw: &str) -> Result<Self, &'static str> {
        if raw.len() > 4096 {
            return Err("검색 조건이 너무 길어요.");
        }
        let mut result = Self::default();
        let mut keys = std::collections::HashSet::new();
        let mut explicit_dir = false;
        for (k, v) in url::form_urlencoded::parse(raw.as_bytes()) {
            if !keys.insert(k.to_string()) {
                return Err("검색 조건이 중복됐어요.");
            }
            match k.as_ref() {
                "q" => result.query = v.into_owned(),
                "cat" => result.category = v.into_owned(),
                "family" => result.family = v.into_owned(),
                "software" => result.software = v.into_owned(),
                "tag" => result.tag = v.into_owned(),
                "reg" => result.registration = v.into_owned(),
                "sort" => result.sort = v.into_owned(),
                "dir" => {
                    result.direction = v.into_owned();
                    explicit_dir = true;
                }
                "alive" => result.alive = v.into_owned(),
                "page" => {
                    result.page = v
                        .parse::<u32>()
                        .ok()
                        .and_then(|n| n.checked_sub(1))
                        .ok_or("페이지 번호를 확인해 주세요.")?
                }
                "page_size" => {
                    result.page_size = v.parse().map_err(|_| "페이지 크기를 확인해 주세요.")?
                }
                // Unknown tracking parameters cannot change the query or authority.
                _ => {}
            }
        }
        if result.sort == "oldest" {
            result.sort = "newest".into();
            if !explicit_dir {
                result.direction = "asc".into();
                explicit_dir = true;
            }
        }
        if !explicit_dir {
            result.direction =
                if ["recommended", "users", "recent", "newest"].contains(&result.sort.as_str()) {
                    "desc".into()
                } else {
                    "asc".into()
                };
        }
        result.validate()?;
        Ok(result)
    }
}
impl From<&str> for ServerQuery {
    fn from(value: &str) -> Self {
        Self::parse(value).unwrap_or(Self {
            invalid: true,
            ..Self::default()
        })
    }
}
impl std::fmt::Display for ServerQuery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.invalid {
            return f.write_str("page=invalid");
        }
        let mut out = url::form_urlencoded::Serializer::new(String::new());
        for (key, value) in [
            ("q", &self.query),
            ("cat", &self.category),
            ("family", &self.family),
            ("software", &self.software),
            ("tag", &self.tag),
        ] {
            if !value.is_empty() {
                out.append_pair(key, value);
            }
        }
        if self.registration != "all" {
            out.append_pair("reg", &self.registration);
        }
        if self.sort != "recommended" {
            out.append_pair("sort", &self.sort);
        }
        let default_dir =
            if ["recommended", "users", "recent", "newest"].contains(&self.sort.as_str()) {
                "desc"
            } else {
                "asc"
            };
        if self.direction != default_dir {
            out.append_pair("dir", &self.direction);
        }
        if self.alive != "all" {
            out.append_pair("alive", &self.alive);
        }
        if self.page != 0 {
            out.append_pair("page", &self.page.saturating_add(1).to_string());
        }
        if self.page_size != 24 {
            out.append_pair("page_size", &self.page_size.to_string());
        }
        f.write_str(&out.finish())
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SearchPage {
    pub listing: SitePage,
    pub total: i64,
    pub without_registration: i64,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FilterSoftware {
    pub name: String,
    pub label: String,
    pub family: String,
    pub categories: Vec<String>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SearchOptions {
    pub categories: Vec<Category>,
    pub software: Vec<FilterSoftware>,
    pub tags: Vec<String>,
    pub truncated: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn query_roundtrip_keeps_korean_literals_and_rejects_ambiguous_input() {
        let q=ServerQuery::parse("q=%EA%B3%A0%EC%96%91%EC%9D%B4%25&tag=C%2B%2B&cat=image&reg=invite_only&sort=users&page=3").unwrap();
        assert_eq!(q.query, "고양이%");
        assert_eq!(q.tag, "C++");
        assert_eq!(q.page, 2);
        assert_eq!(q.direction, "desc");
        assert_eq!(ServerQuery::parse(&q.to_string()).unwrap(), q);
        for bad in [
            "page=0",
            "page=10002",
            "page=-1",
            "q=a&q=b",
            "sort=users;drop",
            "dir=sideways",
            "reg=magic",
            "q=%00",
        ] {
            assert!(ServerQuery::parse(bad).is_err(), "{bad}");
        }
        assert_eq!(
            ServerQuery::parse("sort=recommended").unwrap().sort,
            "recommended"
        );
        assert_eq!(ServerQuery::parse("sort=oldest").unwrap().direction, "asc");
        assert_eq!(ServerQuery::default().to_string(), "");
        for sort in [
            "recommended",
            "name",
            "domain",
            "users",
            "recent",
            "newest",
            "response_time",
        ] {
            for direction in ["asc", "desc"] {
                let q = ServerQuery {
                    sort: sort.into(),
                    direction: direction.into(),
                    ..Default::default()
                };
                assert_eq!(ServerQuery::parse(&q.to_string()).unwrap(), q);
            }
        }
    }

    #[test]
    fn phoenix_recommended_default_survives_parse_and_url_roundtrip() {
        let default = ServerQuery::default();
        assert_eq!(default.sort, "recommended");
        assert_eq!(default.direction, "desc");
        assert_eq!(default.to_string(), "");
        assert_eq!(ServerQuery::parse("sort=recommended").unwrap(), default);
        assert_eq!(ServerQuery::parse(&default.to_string()).unwrap(), default);
    }

    #[test]
    fn phoenix_recommendation_score_uses_active_users_registration_and_response_time() {
        use super::super::ranking::{recommendation_score, RecommendationMetrics};

        let highly_active = RecommendationMetrics {
            active_users: Some(20),
            users: Some(2_000),
            registration_open: Some(true),
            average_response_ms: Some(499),
        };
        let fallback_to_total_users = RecommendationMetrics {
            active_users: Some(0),
            users: Some(100),
            registration_open: Some(false),
            average_response_ms: Some(999),
        };
        let response_at_first_legacy_cutoff = RecommendationMetrics {
            active_users: None,
            users: None,
            registration_open: None,
            average_response_ms: Some(500),
        };
        let response_at_second_legacy_cutoff = RecommendationMetrics {
            active_users: None,
            users: None,
            registration_open: None,
            average_response_ms: Some(1_000),
        };

        assert!(
            (recommendation_score(highly_active) - (21_f64.ln() * 10.0 + 8.0)).abs() < f64::EPSILON
        );
        assert!(
            (recommendation_score(fallback_to_total_users) - (21_f64.ln() * 10.0 + 1.0)).abs()
                < f64::EPSILON
        );
        assert_eq!(recommendation_score(response_at_first_legacy_cutoff), 1.0);
        assert_eq!(recommendation_score(response_at_second_legacy_cutoff), 0.0);
    }

    #[test]
    fn phoenix_recommendation_diversity_breaks_a_fourth_same_family_streak() {
        use super::super::ranking::diversify_by_family;

        let ordered = vec![
            ("micro-1", "micro"),
            ("micro-2", "micro"),
            ("micro-3", "micro"),
            ("micro-4", "micro"),
            ("micro-5", "micro"),
            ("video-1", "video"),
        ];
        let diversified = diversify_by_family(ordered, |(_, family)| family.to_string());

        assert_eq!(
            diversified
                .into_iter()
                .map(|(name, _)| name)
                .collect::<Vec<_>>(),
            vec!["micro-1", "micro-2", "micro-3", "video-1", "micro-4", "micro-5"]
        );
    }
}
