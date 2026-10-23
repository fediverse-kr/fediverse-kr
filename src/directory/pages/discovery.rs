//! Editorial entry points, not rankings. Unknown categories and projects stay visible.
use super::*;

pub(super) fn activity(category: &Category) -> &str {
    match category.name.as_str() {
        "microblogging" => "대화 · 짧은 글",
        "blog" => "긴 글 · 블로그",
        "media" => "사진 · 영상",
        "image" => "사진",
        "video" => "영상",
        "forum" | "discussion" => "주제별 토론",
        "reading" | "book" => "독서 기록",
        "event" => "모임 · 행사",
        _ => &category.label,
    }
}

pub(super) fn examples(items: &[Software], category: &str) -> Vec<Software> {
    // A small, explicitly editorial introduction. The full directory retains its order.
    let order = [
        "mastodon",
        "misskey",
        "pixelfed",
        "peertube",
        "writefreely",
        "wordpress",
        "lemmy",
        "bookwyrm",
        "mobilizon",
    ];
    let mut selected: Vec<Software> = order
        .iter()
        .filter_map(|name| {
            items
                .iter()
                .find(|item| {
                    item.name.eq_ignore_ascii_case(name)
                        && (category.is_empty() || item.categories.iter().any(|c| c == category))
                })
                .cloned()
        })
        .take(3)
        .collect();
    // Do not fill an arbitrary third slot with the first fork in database order.
    // New categories still get examples without another code change.
    if !selected.is_empty() {
        return selected;
    }
    for item in items {
        if selected.len() == 3 {
            break;
        }
        if (category.is_empty() || item.categories.contains(&category.to_owned()))
            && !selected
                .iter()
                .any(|s| super::families::key(s) == super::families::key(item))
        {
            selected.push(item.clone());
        }
    }
    selected
}

pub(super) fn summary(item: &Software) -> String {
    let text = item.description.trim();
    if text.is_empty() {
        return "아직 소개가 없어요. 알고 있는 내용을 함께 채워 주세요.".into();
    }
    let short: String = text.chars().take(120).collect();
    if text.chars().count() > 120 {
        format!("{short}…")
    } else {
        short
    }
}

pub(super) fn has_more_description(item: &Software) -> bool {
    item.description.trim().chars().count() > 120
        || item.description_html.matches("<p>").count() > 1
        || ["<h", "<ul", "<ol", "<blockquote", "<pre", "<table"]
            .iter()
            .any(|tag| item.description_html.contains(tag))
}

/// Optional editorial format; existing plain feature strings remain valid.
pub(super) fn feature_parts(feature: &str) -> (&str, Option<&str>) {
    match feature.split_once(" — ") {
        Some((title, body)) if !title.trim().is_empty() && !body.trim().is_empty() => {
            (title.trim(), Some(body.trim()))
        }
        _ => (feature.trim(), None),
    }
}

pub(super) fn category_names(item: &Software, categories: &[Category]) -> String {
    if item.categories.is_empty() {
        return "분류 미등록".into();
    }
    item.categories
        .iter()
        .map(|name| {
            categories
                .iter()
                .find(|c| &c.name == name)
                .map(activity)
                .unwrap_or(name)
                .to_owned()
        })
        .collect::<Vec<_>>()
        .join(" · ")
}

pub(super) fn family_name(item: &Software) -> Option<&str> {
    item.family
        .as_deref()
        .map(str::trim)
        .filter(|family| !family.is_empty() && !family.eq_ignore_ascii_case(&item.name))
}

#[component]
pub(super) fn Examples(items: Vec<Software>, category: String) -> Element {
    let selected = examples(&items, &category);
    if selected.is_empty() {
        return rsx! {};
    }
    let columns = selected.len();
    rsx! {
        section { class:"software-introduction", aria_labelledby:"software-introduction-title",
            div { class:"software-section-heading",
                div { h2 { id:"software-introduction-title", "이런 차이가 있어요" }
                    p { "쓰는 모습을 비교해 볼 몇 가지 예시예요. 전체 목록은 아래에 있어요." }
                }
                a { class:"text-link", href:"#software-all", "전체 목록으로" ArrowRight {size:16} }
            }
            div { class:"software-example-grid", style:format!("--software-example-columns:{columns}"),
                for item in selected {
                    Link { key:"{item.name}", class:"software-example", to:Route::SoftwareDetail {name:item.name.clone()},
                        brand::SoftwareArtwork {item:item.clone(),compact:true}
                        div {class:"software-example-copy",
                            div {class:"software-example-name",brand::SoftwareMark {item:item.clone(),size:38} h3 {"{item.display_name}"}}
                            p {{summary(&item)}}
                            if !item.features.is_empty() {
                                div {class:"software-example-points",for feature in item.features.iter().filter(|s|!s.trim().is_empty()).take(2) {span {{feature_parts(feature).0}}}}
                            }
                            span {class:"text-link","어떻게 쓰는지 보기" ArrowRight {size:16}}
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn entry_points_use_actual_membership_and_never_replace_the_full_catalog() {
        let data = crate::directory::preview::catalog();
        let selected = examples(&data.software, "");
        assert_eq!(
            selected.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
            ["mastodon", "misskey", "pixelfed"]
        );
        assert!(examples(&data.software, "not-registered").is_empty());
        let mut photo = data
            .software
            .iter()
            .find(|s| s.name == "pixelfed")
            .unwrap()
            .clone();
        photo.categories = vec!["media".into()];
        assert_eq!(examples(&[photo], "media").len(), 1);
        assert_eq!(data.software.len(), 28);
        assert_eq!(
            examples(&data.software, "microblogging")
                .iter()
                .map(|s| s.name.as_str())
                .collect::<Vec<_>>(),
            ["mastodon", "misskey"]
        );
    }
    #[test]
    fn production_and_preview_taxonomies_keep_their_ids_and_unknown_labels() {
        for (id, label) in [
            ("reading", "독서 기록"),
            ("book", "독서 기록"),
            ("forum", "주제별 토론"),
            ("media", "사진 · 영상"),
            ("new-kind", "새 종류"),
        ] {
            let category = Category {
                name: id.into(),
                label: "새 종류".into(),
                emoji: String::new(),
            };
            assert_eq!(activity(&category), label);
            assert_eq!(category.name, id);
        }
    }
    #[test]
    fn member_content_is_used_even_for_known_products_and_new_products() {
        let mut item = crate::directory::preview::catalog().software[0].clone();
        item.description = "회원이 고친 새 소개".into();
        assert_eq!(summary(&item), "회원이 고친 새 소개");
        item.name = "new-community-product".into();
        item.categories = vec!["new-kind".into()];
        assert_eq!(examples(&[item.clone()], "new-kind"), vec![item]);
        assert_eq!(
            feature_parts("화면 꾸미기 — 칼럼을 배치해요"),
            ("화면 꾸미기", Some("칼럼을 배치해요"))
        );
        assert_eq!(feature_parts("기존 일반 문장"), ("기존 일반 문장", None));
        assert_eq!(feature_parts(" — 설명"), ("— 설명", None));
    }
}
