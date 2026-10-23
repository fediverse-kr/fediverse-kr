//! Never inserted into a database. Reserved domains and explicit preview state.
use super::*;
use std::{collections::HashMap, sync::LazyLock};

#[derive(serde::Deserialize)]
struct CatalogSnapshot {
    fetched_at: String,
    #[allow(dead_code)]
    source: String,
    #[allow(dead_code)]
    family_source: String,
    catalog: Catalog,
    #[serde(default)]
    families: HashMap<String, String>,
}

static CATALOG_SNAPSHOT: LazyLock<CatalogSnapshot> = LazyLock::new(|| {
    serde_json::from_str(include_str!("preview-catalog.json"))
        .expect("the checked-in preview catalog snapshot is valid JSON")
});

static CATALOG: LazyLock<Catalog> = LazyLock::new(|| {
    let snapshot = &*CATALOG_SNAPSHOT;
    let mut catalog = snapshot.catalog.clone();
    catalog.preview = true;
    for software in &mut catalog.software {
        software.family = preview_family(
            &software.name,
            software.family.as_deref(),
            &snapshot.families,
        );
        software.website_url = software.website_url.as_deref().and_then(web_link);
        #[cfg(feature = "server")]
        {
            let description_source = if software.description_html.trim().is_empty() {
                &software.description
            } else {
                &software.description_html
            };
            software.description_html =
                super::rich_text::render_description(description_source).html;
        }
    }
    catalog
});

fn preview_family(
    name: &str,
    source_family: Option<&str>,
    known_families: &HashMap<String, String>,
) -> Option<String> {
    if let Some(family) = source_family
        .map(str::trim)
        .filter(|family| !family.is_empty())
    {
        return Some(family.to_owned());
    }
    known_families
        .iter()
        .find(|(software, _)| software.eq_ignore_ascii_case(name))
        .map(|(_, family)| family.clone())
}

pub(crate) fn catalog_snapshot_date() -> String {
    CATALOG_SNAPSHOT
        .fetched_at
        .split('T')
        .next()
        .unwrap_or_default()
        .to_owned()
}
pub fn health_history(domain: &str) -> Option<health::History> {
    all_sites().iter().find(|site| site.domain == domain)?;
    Some(health::History {
        preview: true,
        checks: (0..8)
            .map(|hour| health::Check {
                checked_at: format!("2026-09-14T0{hour}:00:00+00:00"),
                checked_at_kst: format!("2026-09-14 {:02}:00:00", hour + 9),
                alive: hour != 3,
                response_ms: match hour {
                    3 => None,
                    4 => Some(2100),
                    _ => Some(180),
                },
            })
            .collect(),
    })
}

pub fn search_catalog(query: &catalog::CatalogQuery) -> catalog::CatalogPage {
    use catalog::{CatalogPage, CategoryPage, CATEGORY_PAGE_SIZE, SOFTWARE_PAGE_SIZE};
    let data = catalog();
    let items: Vec<_> = data
        .software
        .into_iter()
        .filter(|s| {
            (query.category.is_empty() || s.categories.contains(&query.category))
                && format!("{} {}", s.name, s.display_name)
                    .to_lowercase()
                    .contains(&query.query.to_lowercase())
        })
        .collect();
    CatalogPage {
        preview: true,
        total: items.len() as i64,
        page: query.page,
        has_next: (query.page as usize + 1) * SOFTWARE_PAGE_SIZE < items.len(),
        software: items
            .into_iter()
            .skip(query.page as usize * SOFTWARE_PAGE_SIZE)
            .take(SOFTWARE_PAGE_SIZE)
            .collect(),
        kinds: CategoryPage {
            total: data.categories.len() as i64,
            page: query.kind_page,
            has_next: (query.kind_page as usize + 1) * CATEGORY_PAGE_SIZE < data.categories.len(),
            categories: data
                .categories
                .iter()
                .skip(query.kind_page as usize * CATEGORY_PAGE_SIZE)
                .take(CATEGORY_PAGE_SIZE)
                .cloned()
                .collect(),
        },
        labels: data.categories,
    }
}
pub fn catalog() -> Catalog {
    CATALOG.clone()
}
pub fn all_sites() -> Vec<PublicSite> {
    [
        (
            "dog-town.example",
            "멍멍.타운",
            "mastodon",
            "소소한 하루를 나누는 동네.",
        ),
        (
            "cat-tower.example",
            "냥냥.타워",
            "misskey",
            "각자의 속도로 느긋하게 이야기해요.",
        ),
        (
            "light.example",
            "빛 모으는 곳",
            "pixelfed",
            "산책 중 만난 풍경을 나누어요.",
        ),
        (
            "screen.example",
            "동네 상영관",
            "peertube",
            "직접 만든 영상이 모이는 상영관.",
        ),
    ]
    .into_iter()
    .map(|(domain, name, software, description)| PublicSite {
        icon_available: false,
        header_available: false,
        guidance: SiteGuidance {
            tags: vec![if software == "pixelfed" {
                "사진"
            } else {
                "일상"
            }
            .into()],
            invite_only: Some(software == "misskey"),
            approval_required: Some(software == "mastodon"),
            ..Default::default()
        },
        domain: domain.into(),
        name: name.into(),
        description: description.into(),
        description_html: format!("<p>{description}</p>"),
        software: Some(software.into()),
        version: None,
        closed: false,
        alive: None,
        registration_open: if software == "peertube" {
            None
        } else {
            Some(software != "misskey")
        },
        users: None,
        active_users: None,
        checked_at: None,
        metadata_checked_at: None,
        average_response_ms: None,
    })
    .collect()
}
pub fn search_options() -> search::SearchOptions {
    let catalog = catalog();
    search::SearchOptions {
        categories: catalog.categories,
        software: catalog
            .software
            .into_iter()
            .map(|s| {
                let family = s
                    .family
                    .filter(|family| {
                        !family.trim().is_empty() && !family.eq_ignore_ascii_case(&s.name)
                    })
                    .unwrap_or_else(|| s.name.clone());
                search::FilterSoftware {
                    family,
                    name: s.name,
                    label: s.display_name,
                    categories: s.categories,
                }
            })
            .collect(),
        tags: vec!["사진".into(), "일상".into()],
        truncated: false,
    }
}
pub fn search(q: &search::ServerQuery) -> search::SearchPage {
    let options = search_options();
    let mut sites: Vec<_> = all_sites()
        .into_iter()
        .filter(|s| {
            let software = options
                .software
                .iter()
                .find(|c| Some(c.name.as_str()) == s.software.as_deref());
            (q.query.trim().is_empty()
                || format!(
                    "{} {} {} {}",
                    s.domain,
                    s.name,
                    s.description,
                    s.software.as_deref().unwrap_or("")
                )
                .to_lowercase()
                .contains(&q.query.trim().to_lowercase()))
                && (q.software.is_empty()
                    || s.software
                        .as_deref()
                        .is_some_and(|v| v.eq_ignore_ascii_case(&q.software)))
                && (q.category.is_empty()
                    || software.is_some_and(|c| c.categories.contains(&q.category)))
                && (q.family.is_empty() || software.is_some_and(|c| c.family == q.family))
                && (q.tag.is_empty() || s.guidance.tags.contains(&q.tag))
                && match q.alive.as_str() {
                    "yes" => !s.closed && s.alive == Some(true),
                    "no" => !s.closed && s.alive == Some(false),
                    "unknown" => !s.closed && s.alive.is_none(),
                    "closed" => s.closed,
                    _ => true,
                }
        })
        .collect();
    let without_registration = sites.len() as i64;
    sites.retain(|s| {
        q.registration == "all"
            || (!s.closed
                && match q.registration.as_str() {
                    "open" => {
                        s.registration_open == Some(true) && s.guidance.invite_only != Some(true)
                    }
                    "approval" => {
                        s.registration_open == Some(true)
                            && s.guidance.approval_required == Some(true)
                            && s.guidance.invite_only != Some(true)
                    }
                    "invite_only" => s.guidance.invite_only == Some(true),
                    "closed" => {
                        s.registration_open == Some(false) && s.guidance.invite_only != Some(true)
                    }
                    "unknown" => {
                        s.registration_open.is_none() && s.guidance.invite_only != Some(true)
                    }
                    _ => false,
                })
    });
    sites.sort_by(|a, b| {
        if q.sort == "recommended" {
            return std::cmp::Ordering::Equal;
        }
        let cmp = if q.sort == "name" {
            a.name.to_lowercase().cmp(&b.name.to_lowercase())
        } else {
            a.domain.cmp(&b.domain)
        };
        if q.direction == "desc" {
            cmp.reverse()
        } else {
            cmp
        }
    });
    let total = sites.len() as i64;
    let start = q.page as usize * q.page_size;
    search::SearchPage {
        total,
        without_registration,
        listing: SitePage {
            preview: true,
            page: q.page,
            has_next: start.saturating_add(q.page_size) < sites.len(),
            sites: sites.into_iter().skip(start).take(q.page_size).collect(),
        },
    }
}
pub fn servers(query: &str, software: &str, page: u32) -> SitePage {
    let filtered: Vec<_> = all_sites()
        .into_iter()
        .filter(|s| {
            (software.is_empty() || s.software.as_deref() == Some(software))
                && format!("{} {} {}", s.domain, s.name, s.description)
                    .to_lowercase()
                    .contains(&query.trim().to_lowercase())
        })
        .collect();
    SitePage {
        preview: true,
        sites: filtered
            .into_iter()
            .skip(page as usize * PAGE_SIZE)
            .take(PAGE_SIZE)
            .collect(),
        page,
        has_next: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_catalog_counts_pages_and_keeps_unique_products() {
        let data = catalog();
        assert!(data.preview);
        assert_eq!(data.software.len(), 28);
        assert_eq!(data.categories.len(), 5);
        assert_eq!(
            data.software
                .iter()
                .filter(|software| software
                    .categories
                    .iter()
                    .any(|category| category == "microblogging"))
                .count(),
            20
        );
        let mut microblogging = catalog::CatalogQuery::default();
        microblogging.category = "microblogging".into();
        assert_eq!(search_catalog(&microblogging).total, 20);
        let mut query = catalog::CatalogQuery::default();
        let first = search_catalog(&query);
        assert_eq!(first.total, 28);
        assert_eq!(first.software.len(), 24);
        assert!(first.has_next);
        query.page = 1;
        let second = search_catalog(&query);
        assert_eq!(second.software.len(), 4);
        assert!(!second.has_next);

        let names: std::collections::HashSet<_> = data
            .software
            .iter()
            .map(|software| software.name.as_str())
            .collect();
        assert_eq!(names.len(), 28);
    }

    #[test]
    fn snapshot_preserves_source_descriptions_and_features_and_known_families() {
        let snapshot: CatalogSnapshot =
            serde_json::from_str(include_str!("preview-catalog.json")).unwrap();
        let data = catalog();
        for source in &snapshot.catalog.software {
            let software = data
                .software
                .iter()
                .find(|software| software.name == source.name)
                .unwrap();
            assert_eq!(software.description, source.description, "{}", source.name);
            assert_eq!(software.features, source.features, "{}", source.name);
            assert_eq!(
                software.website_url,
                source.website_url.as_deref().and_then(web_link),
                "{}",
                source.name
            );
        }
        for (name, family) in [
            ("mastodon", "mastodon"),
            ("misskey", "misskey"),
            ("pleroma", "pleroma"),
            ("wildebeest", "wildebeest"),
        ] {
            assert_eq!(
                data.software
                    .iter()
                    .find(|software| software.name == name)
                    .and_then(|software| software.family.as_deref()),
                Some(family)
            );
        }
        #[cfg(feature = "server")]
        {
            let kosmo = data
                .software
                .iter()
                .find(|software| software.name == "kosmo")
                .unwrap();
            assert!(kosmo
                .description_html
                .contains("href=\"https://planet.moe/@kosmo/116985757378791889\""));
            assert!(kosmo.description_html.contains("개발 배경"));
        }
        assert_eq!(catalog_snapshot_date(), "2026-10-23");
    }

    #[test]
    fn explicit_source_families_and_fallbacks_are_preserved() {
        let families = HashMap::from([
            ("known".into(), "mapped-family".into()),
            ("self".into(), "self".into()),
        ]);
        assert_eq!(
            preview_family("known", Some(" source-family "), &families).as_deref(),
            Some("source-family")
        );
        assert_eq!(
            preview_family("known", Some("known"), &families).as_deref(),
            Some("known")
        );
        assert_eq!(
            preview_family("known", Some("  "), &families).as_deref(),
            Some("mapped-family")
        );
        assert_eq!(preview_family("unknown", None, &families), None);
        assert_eq!(
            preview_family("self", None, &families).as_deref(),
            Some("self")
        );

        let options = search_options();
        assert_eq!(
            options
                .software
                .iter()
                .find(|software| software.name == "cherrypick")
                .map(|software| software.family.as_str()),
            Some("misskey")
        );
        assert_eq!(
            options
                .software
                .iter()
                .find(|software| software.name == "bookwyrm")
                .map(|software| software.family.as_str()),
            Some("bookwyrm")
        );
    }

    #[test]
    fn snapshot_family_groups_match_the_public_software_edit_data() {
        let data = catalog();
        for (family, expected) in [
            ("mastodon", 5),
            ("misskey", 9),
            ("pleroma", 2),
            ("wildebeest", 2),
        ] {
            assert_eq!(
                data.software
                    .iter()
                    .filter(|software| software.family.as_deref() == Some(family))
                    .count(),
                expected,
                "{family}"
            );
        }
    }

    #[test]
    fn reserved_preview_sites_remain_unchanged() {
        let sites = all_sites();
        assert_eq!(sites.len(), 4);
        assert!(sites.iter().all(|site| site.domain.ends_with(".example")));
        assert_eq!(
            sites
                .iter()
                .map(|site| site.domain.as_str())
                .collect::<Vec<_>>(),
            [
                "dog-town.example",
                "cat-tower.example",
                "light.example",
                "screen.example"
            ]
        );
    }
    #[test]
    fn recommended_preview_keeps_the_curated_order() {
        let actual = search(&search::ServerQuery::default())
            .listing
            .sites
            .into_iter()
            .map(|site| site.domain)
            .collect::<Vec<_>>();
        let expected = all_sites()
            .into_iter()
            .map(|site| site.domain)
            .collect::<Vec<_>>();
        assert_eq!(actual, expected);
    }
}
