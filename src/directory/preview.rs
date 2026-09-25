//! Never inserted into a database. Reserved domains and explicit preview state.
use super::*;
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
    let kinds = [
        ("microblogging", "짧은 글 · SNS", "✳"),
        ("image", "사진", "▧"),
        ("video", "영상", "▷"),
        ("discussion", "게시판", "☷"),
        ("book", "독서", "▤"),
        ("event", "모임 · 행사", "◷"),
    ];
    Catalog {
        preview: true,
        truncated: false,
        categories: kinds
            .into_iter()
            .map(|(name, label, emoji)| Category {
                name: name.into(),
                label: label.into(),
                emoji: emoji.into(),
            })
            .collect(),
        software: [
            (
                "mastodon",
                "Mastodon",
                "microblogging",
                "짧은 글로 일상을 나누고, 다른 곳의 이웃을 팔로우해요.",
                "https://joinmastodon.org/",
            ),
            (
                "misskey",
                "Misskey",
                "microblogging",
                "노트와 리액션으로 이야기를 나누는 소셜 네트워크예요.",
                "https://misskey-hub.net/ko/",
            ),
            (
                "pixelfed",
                "Pixelfed",
                "image",
                "사진을 중심으로 기록하고 감상하는 공간이에요.",
                "https://pixelfed.org/",
            ),
            (
                "peertube",
                "PeerTube",
                "video",
                "영상을 올리고 채널을 구독하는 공간이에요.",
                "https://joinpeertube.org/",
            ),
            (
                "lemmy",
                "Lemmy",
                "discussion",
                "주제별 커뮤니티에서 링크와 이야기를 나눠요.",
                "https://join-lemmy.org/",
            ),
            (
                "bookwyrm",
                "BookWyrm",
                "book",
                "읽는 책을 기록하고, 서평을 나눠요.",
                "https://joinbookwyrm.com/",
            ),
            (
                "mobilizon",
                "Mobilizon",
                "event",
                "행사를 알리고, 함께할 사람을 모아요.",
                "https://joinmobilizon.org/",
            ),
        ]
        .into_iter()
        .map(
            |(name, display_name, category, description, url)| Software {
                logo_available: false,
                name: name.into(),
                display_name: display_name.into(),
                description: description.into(),
                description_html: format!("<p>{description}</p>"),
                categories: vec![category.into()],
                features: vec![],
                website_url: Some(url.into()),
                tech_stack: None,
            },
        )
        .collect(),
    }
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
            .map(|s| search::FilterSoftware {
                family: s.name.clone(),
                name: s.name,
                label: s.display_name,
                categories: s.categories,
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
