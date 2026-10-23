//! A view of explicitly registered families, not an inferred fork genealogy.
use super::*;

pub(super) fn key(item: &Software) -> String {
    item.family
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(&item.name)
        .to_lowercase()
}

#[derive(Debug, PartialEq)]
struct Family {
    label: String,
    anchor: Option<Software>,
    members: Vec<Software>,
}

fn groups(items: &[Software], category: &str, current: Option<&str>) -> Vec<Family> {
    let selected = current
        .and_then(|name| items.iter().find(|s| s.name == name))
        .map(key);
    if current.is_some() && selected.is_none() {
        return vec![];
    }
    let mut map = std::collections::BTreeMap::<String, Vec<Software>>::new();
    for item in items {
        if !category.is_empty() && !item.categories.iter().any(|c| c == category) {
            continue;
        }
        let family = key(item);
        if selected.as_ref().is_none_or(|selected| *selected == family) {
            map.entry(family).or_default().push(item.clone());
        }
    }
    map.into_iter()
        .filter_map(|(key, mut members)| {
            if members.len() < 2 {
                return None;
            }
            members.sort_by_key(|s| s.display_name.to_lowercase());
            // The matching catalog entry identifies this family. This is not a parent edge.
            let anchor = members
                .iter()
                .position(|s| s.name.eq_ignore_ascii_case(&key))
                .map(|index| members.remove(index));
            let label = anchor
                .as_ref()
                .map(|s| s.display_name.clone())
                .unwrap_or(key);
            Some(Family {
                label,
                anchor,
                members,
            })
        })
        .collect()
}

#[component]
pub(super) fn FamilyIndex(
    items: Vec<Software>,
    category: String,
    current: Option<String>,
) -> Element {
    let families = groups(&items, &category, current.as_deref());
    if families.is_empty() {
        return rsx! {};
    }
    let single = families.len() == 1;
    rsx! {
        section {class:"software-family-index",aria_labelledby:"software-family-title",
            div {class:"software-section-heading",
                div {h2 {id:"software-family-title",if current.is_some(){"같은 계열의 소프트웨어"}else{"같은 계열도 살펴보세요"}}
                    p {"계열의 기준 이름을 보고, 그 안의 소프트웨어로 이어서 둘러보세요."}
                }
            }
            div {class:if single {"software-family-rows software-family-single"}else{"software-family-rows"},
                for family in families {
                    section {class:"software-family-row",aria_label:format!("{} 계열",family.label),
                        div {class:"software-family-origin",
                            if let Some(anchor)=family.anchor {
                                Link {class:"software-family-anchor",to:Route::SoftwareDetail{name:anchor.name.clone()},
                                    aria_current:if current.as_deref()==Some(anchor.name.as_str()) {"page"}else{"false"},
                                    brand::SoftwareMark{item:anchor.clone(),size:44}
                                    div {span {class:"software-family-overline","계열 기준"}h3 {"{family.label}"}}
                                    ArrowRight{size:16}
                                }
                                p {class:"software-family-description",{super::discovery::summary(&anchor)}}
                            } else {
                                div {class:"software-family-unregistered",
                                    span {class:"software-family-overline","등록된 계열 이름"}h3 {"{family.label}"}
                                    p {"이 목록에는 기준 이름에 해당하는 소개가 없어요."}
                                }
                            }
                        }
                        div {class:"software-family-related",
                            p {class:"software-family-overline","같은 계열로 등록된 이름"}
                            div {class:"software-family-members",
                                for item in family.members {
                                    Link {key:"{item.name}",to:Route::SoftwareDetail{name:item.name.clone()},
                                        title:item.description.clone(),
                                        aria_current:if current.as_deref()==Some(item.name.as_str()) {"page"}else{"false"},
                                        "{item.display_name}"
                                    }
                                }
                            }
                        }
                    }
                }
            }
            p {class:"software-family-footnote","직계 포크 관계를 그린 계보도는 아니에요. 과거 이름이나 개발 종료 항목도 포함되므로, 현재 상태는 각 소개에서 확인하세요."}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn complete_source_families_have_distinct_anchors_and_all_registered_members() {
        let data = crate::directory::preview::catalog();
        let all = groups(&data.software, "", None);
        assert_eq!(all.len(), 4);
        for (label, anchor, count) in [
            ("Mastodon", "mastodon", 4),
            ("Misskey", "misskey", 8),
            ("Pleroma", "pleroma", 1),
            ("Wildebeest", "wildebeest", 1),
        ] {
            let family = all.iter().find(|g| g.label == label).unwrap();
            assert_eq!(family.anchor.as_ref().unwrap().name, anchor);
            assert_eq!(family.members.len(), count);
            assert!(!family.members.iter().any(|s| s.name == anchor));
        }
        let micro = groups(&data.software, "microblogging", None);
        assert_eq!(micro.len(), 3);
        let cherry = groups(&data.software, "", Some("cherrypick"));
        assert_eq!(cherry.len(), 1);
        assert_eq!(cherry[0].label, "Misskey");
        assert_eq!(cherry[0].members.len(), 8);
        assert!(groups(&data.software, "", Some("missing")).is_empty());
        assert!(groups(&data.software, "", Some("hollo")).is_empty());
    }
    #[test]
    fn missing_base_is_not_fabricated_and_filtered_groups_do_not_leak_categories() {
        let mut one = crate::directory::preview::catalog().software[0].clone();
        one.name = "new-one".into();
        one.family = Some(" New-Base ".into());
        one.categories = vec!["kind-one".into()];
        let mut two = one.clone();
        two.name = "new-two".into();
        two.family = Some("new-base".into());
        two.categories = vec!["kind-two".into()];
        let items = [one, two];
        let all = groups(&items, "", None);
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].label, "new-base");
        assert!(all[0].anchor.is_none());
        assert_eq!(all[0].members.len(), 2);
        assert!(groups(&items, "kind-one", None).is_empty());
    }
}
