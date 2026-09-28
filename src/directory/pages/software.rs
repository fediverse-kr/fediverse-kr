use super::*;

#[component]
pub(super) fn Overview(item: Software, categories: Vec<Category>) -> Element {
    rsx! {
        header { class:"software-overview",
            div { class:"software-overview-copy",
                div { class:"software-categories",
                    for category in &item.categories {
                        Link { to:Route::Platforms { filters:super::super::catalog::CatalogQuery::default().category(category.clone()) },
                            "{categories.iter().find(|c|&c.name==category).map(|c|c.label.as_str()).unwrap_or(category)}"
                        }
                    }
                }
                h1 { class:"software-title",
                    if item.logo_available {
                        crate::media_ui::ImageMark {
                            source:crate::media_ui::software_source(&item.name),
                            fallback:item.display_name.chars().next().unwrap_or('·').to_string(),
                            class:"software-mark", size:64
                        }
                    }
                    span { "{item.display_name}" }
                }
                if !item.description.is_empty() { p { class:"software-lead", "{item.description}" } }
            }
            div { class:"software-next-step",
                Link { class:"primary-button", to:Route::SoftwareServers { name:item.name.clone() }, "이 소프트웨어의 서버 찾기" ArrowRight { size:18 } }
                if let Some(url)=item.website_url.as_deref().and_then(web_link) {
                    a { class:"text-link", href:url, target:"_blank", rel:"noopener noreferrer ugc", "프로젝트 웹사이트" ArrowRight { size:16 } }
                }
                p { "가입 방식과 운영 규칙은 서버마다 달라요." }
            }
        }
        if !item.features.is_empty() {
            section { class:"software-features", aria_labelledby:"software-features-title",
                h2 { id:"software-features-title", "주요 기능" }
                ul { for feature in &item.features { li { "{feature}" } } }
            }
        }
        if !item.description_html.is_empty() {
            details { class:"software-more",
                summary { "소개 자세히 보기" }
                div { class:"rich-description", dangerous_inner_html:"{item.description_html}" }
            }
        }
        if let Some(tech)=item.tech_stack.as_deref().filter(|s|!s.trim().is_empty()) {
            details { class:"software-more",
                summary { "개발에 쓰인 기술" }
                p { "{tech}" }
            }
        }
        footer { class:"software-contribute",
            p { "빠지거나 바뀐 정보가 있나요?" }
            div { class:"membership-actions",
                Link { class:"text-link", to:Route::SoftwareEditor { name:item.name.clone() }, "정보 수정" }
                Link { class:"text-link", to:Route::SoftwareHistory { name:item.name }, "변경 이력" }
            }
        }
    }
}
