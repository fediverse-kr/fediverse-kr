//! Phoenix-compatible recommendation metrics for the public directory.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RecommendationMetrics {
    pub active_users: Option<i64>,
    pub users: Option<i64>,
    pub registration_open: Option<bool>,
    pub average_response_ms: Option<i32>,
}

pub(crate) fn recommendation_score(metrics: RecommendationMetrics) -> f64 {
    let population = metrics
        .active_users
        .filter(|users| *users > 0)
        .map(|users| users as f64)
        .or_else(|| {
            metrics
                .users
                .filter(|users| *users > 0)
                .map(|users| users as f64 * 0.20)
        })
        .unwrap_or(0.0);
    let activity = (population.max(0.0) + 1.0).ln() * 10.0;
    let registration = (metrics.registration_open == Some(true)) as u8 as f64 * 5.0;
    let response = match metrics.average_response_ms {
        Some(ms) if ms < 500 => 3.0,
        Some(ms) if ms < 1_000 => 1.0,
        _ => 0.0,
    };

    activity + registration + response
}

struct Tagged<T> {
    value: T,
    family: String,
}

/// Preserve the Phoenix directory's page-local family diversity rule.
pub(crate) fn diversify_by_family<T>(items: Vec<T>, family: impl Fn(&T) -> String) -> Vec<T> {
    if items.len() < 3 {
        return items;
    }
    let tagged = items
        .into_iter()
        .map(|value| Tagged {
            family: family(&value),
            value,
        })
        .collect();
    interleave_by_streak(tagged, 3)
        .into_iter()
        .map(|item| item.value)
        .collect()
}

fn interleave_by_streak<T>(mut remaining: Vec<Tagged<T>>, max: usize) -> Vec<Tagged<T>> {
    let mut result = Vec::with_capacity(remaining.len());
    while !remaining.is_empty() {
        let current = remaining.remove(0);
        if streak_ok(&result, &current.family, max) {
            result.push(current);
        } else if let Some(index) = remaining
            .iter()
            .position(|candidate| candidate.family != current.family)
        {
            let breaker = remaining.remove(index);
            result.push(breaker);
            remaining.insert(0, current);
        } else {
            result.push(current);
        }
    }
    repair_streaks(result, max)
}

fn streak_ok<T>(result: &[Tagged<T>], family: &str, max: usize) -> bool {
    result.len() < max
        || result[result.len() - max..]
            .iter()
            .any(|item| item.family != family)
}

fn repair_streaks<T>(mut items: Vec<Tagged<T>>, max: usize) -> Vec<Tagged<T>> {
    while let Some((violation, family)) = first_violation(&items, max) {
        let donor = (0..violation)
            .rev()
            .find(|&index| items[index].family != family && safe_to_remove(&items, index));
        let Some(donor) = donor else {
            break;
        };
        let item = items.remove(donor);
        items.insert(violation - 1, item);
    }
    items
}

fn first_violation<T>(items: &[Tagged<T>], max: usize) -> Option<(usize, String)> {
    let mut start = 0;
    while start < items.len() {
        let family = &items[start].family;
        let mut end = start + 1;
        while end < items.len() && items[end].family == *family {
            end += 1;
        }
        if end - start > max {
            return Some((start + max, family.clone()));
        }
        start = end;
    }
    None
}

fn safe_to_remove<T>(items: &[Tagged<T>], index: usize) -> bool {
    let before = index.checked_sub(1).map(|index| &items[index].family);
    let after = items.get(index + 1).map(|item| &item.family);
    before != after
}
