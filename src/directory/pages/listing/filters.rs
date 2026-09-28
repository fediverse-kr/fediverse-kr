use crate::directory::search::{SearchOptions, ServerQuery};

/// A shareable removal link, derived from applied URL criteria, never a draft.
pub(super) fn active_filters(
    criteria: &ServerQuery,
    options: Option<&SearchOptions>,
) -> Vec<(String, ServerQuery)> {
    let mut chips = Vec::new();
    let mut add = |label: String, clear: fn(&mut ServerQuery)| {
        let mut next = criteria.clone();
        next.page = 0;
        clear(&mut next);
        chips.push((label, next));
    };
    if !criteria.query.is_empty() {
        add(format!("검색: {}", criteria.query), |q| q.query.clear());
    }
    if !criteria.category.is_empty() {
        let label = options
            .and_then(|o| o.categories.iter().find(|c| c.name == criteria.category))
            .map(|c| c.label.as_str())
            .unwrap_or(&criteria.category);
        add(format!("종류: {label}"), |q| {
            q.category.clear();
            q.family.clear();
            q.software.clear();
        });
    }
    if !criteria.family.is_empty() {
        add(format!("계열: {}", criteria.family), |q| {
            q.family.clear();
            q.software.clear();
        });
    }
    if !criteria.software.is_empty() {
        let label = options
            .and_then(|o| o.software.iter().find(|s| s.name == criteria.software))
            .map(|s| s.label.as_str())
            .unwrap_or(&criteria.software);
        add(format!("소프트웨어: {label}"), |q| q.software.clear());
    }
    if !criteria.tag.is_empty() {
        add(format!("태그: {}", criteria.tag), |q| q.tag.clear());
    }
    if criteria.registration != "all" {
        let label = match criteria.registration.as_str() {
            "open" => "가입 접수 중",
            "approval" => "가입 승인 필요",
            "invite_only" => "초대 필요",
            "closed" => "가입 접수 닫힘",
            _ => "가입 방식 확인 전",
        };
        add(format!("가입 방식: {label}"), |q| {
            q.registration = "all".into()
        });
    }
    if criteria.alive != "all" {
        let label = match criteria.alive.as_str() {
            "yes" => "최근 응답 확인",
            "no" => "최근 응답 없음",
            "closed" => "운영 종료",
            _ => "아직 확인 전",
        };
        add(format!("응답 상태: {label}"), |q| {
            q.alive = "all".into()
        });
    }
    if criteria.sort != "recommended" || criteria.direction != "desc" {
        let label = super::sort_options()
            .into_iter()
            .find(|(k, _)| k == &criteria.sort)
            .map(|(_, l)| l)
            .unwrap_or_else(|| criteria.sort.clone());
        let direction = if criteria.direction == "asc" {
            "오름차순"
        } else {
            "내림차순"
        };
        add(format!("정렬: {label} · {direction}"), |q| {
            q.sort = "recommended".into();
            q.direction = "desc".into();
        });
    }
    if criteria.page_size != 24 {
        add(format!("페이지당 {}개", criteria.page_size), |q| {
            q.page_size = 24
        });
    }
    chips
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn single_removal_preserves_unrelated_criteria_and_resets_pagination() {
        let q =
            ServerQuery::parse("software=mastodon&reg=open&sort=name&dir=desc&page=3&page_size=25")
                .unwrap();
        let chips = active_filters(&q, None);
        assert_eq!(chips.len(), 4);
        let (_, removed) = chips
            .iter()
            .find(|(label, _)| label.starts_with("소프트웨어:"))
            .unwrap();
        assert_eq!(
            removed,
            &ServerQuery {
                software: String::new(),
                page: 0,
                ..q
            }
        );
        assert!(active_filters(&ServerQuery::default(), None).is_empty());
    }
    #[test]
    fn category_removal_clears_dependent_filters_but_keeps_literal_search() {
        let q = ServerQuery::parse(
            "cat=microblog&family=misskey&software=cherrypick&q=%EA%B3%A0%EC%96%91%EC%9D%B4%26%2B",
        )
        .unwrap();
        let chips = active_filters(&q, None);
        let (_, removed) = chips
            .iter()
            .find(|(label, _)| label.starts_with("종류:"))
            .unwrap();
        assert!(
            removed.category.is_empty() && removed.family.is_empty() && removed.software.is_empty()
        );
        assert_eq!(removed.query, "고양이&+");
        assert_eq!(ServerQuery::parse(&removed.to_string()).unwrap(), *removed);
    }
}
