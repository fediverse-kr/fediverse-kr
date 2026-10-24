use super::*;

#[component]
pub(super) fn Overview(
    item: Software,
    categories: Vec<Category>,
    #[props(default)] related: Vec<Software>,
) -> Element {
    let source = brand::artwork_source(&item.name);
    let features: Vec<_> = item
        .features
        .iter()
        .filter(|s| !s.trim().is_empty())
        .collect();
    let extended = super::discovery::has_more_description(&item);
    let inactive = item.inactive();
    rsx! {
        document::Stylesheet { href:asset!("/assets/styling/software-catalog.css") }
        div { class:if source.is_some() {"software-product-hero"} else {"software-product-hero software-unprofiled"},
            header { class:"software-summary",
                div { class:"software-heading", "data-inactive":inactive.to_string(),
                    brand::SoftwareMark { item:item.clone(), size:80 }
                    div {
                        p { class:"software-eyebrow", "연합우주 소프트웨어" }
                        h1 { "{item.display_name}" }
                        brand::SoftwareStatus { item:item.clone() }
                    }
                }
                div { class:"software-categories",
                    for category in &item.categories {
                        Link { to:Route::Platforms { filters:super::super::catalog::CatalogQuery::default().category(category.clone()) },
                            "{categories.iter().find(|c|&c.name==category).map(super::discovery::activity).unwrap_or(category)}"
                        }
                    }
                    if let Some(family)=super::discovery::family_name(&item) {
                        span {class:"software-family","{family} 계열"}
                    }
                }
                if extended {
                    p {class:"software-lead",{super::discovery::summary(&item)}}
                    a {class:"software-description-jump",href:"#software-description-title","소개 더 읽기"}
                } else {div {class:"software-lead", RegisteredDescription {item:item.clone()}}}
                div { class:"software-primary-actions",
                    Link { class:"primary-button", to:Route::SoftwareServers { name:item.name.clone() },
                        "이 소프트웨어의 서버 찾기" ArrowRight { size:18 }
                    }
                    if let Some(url)=item.website_url.as_deref().and_then(web_link) {
                        a { class:"text-link", href:url, target:"_blank", rel:"noopener noreferrer ugc",
                            "프로젝트 웹사이트" ArrowRight { size:16 }
                        }
                    }
                }
                p { class:"software-join-note", "가입은 소프트웨어가 아닌, 이 도구로 운영하는 서버에서 해요." }
            }
            figure { class:"software-product-visual",
                if let Some(image)=brand::artwork(&item.name) {
                    a {class:"software-image-link",href:image,target:"_blank",rel:"noopener noreferrer",aria_label:format!("{} 소개 이미지 크게 보기",item.display_name),brand::SoftwareArtwork {item:item.clone()}}
                } else {brand::SoftwareArtwork { item:item.clone() }}
                if let Some(source)=source {
                    figcaption {
                        span { if item.name.eq_ignore_ascii_case("peertube") {"공식 프로젝트 일러스트"}else{"공식 소개 화면 · 서버와 앱에 따라 달라질 수 있어요."} }
                        a { href:source, target:"_blank", rel:"noopener noreferrer", "원출처 ↗" }
                    }
                }
            }
        }
        if !features.is_empty() {
            section {class:"software-use",aria_labelledby:"software-use-title",
                div {class:"software-section-heading",
                    div {h2 {id:"software-use-title","특징 한눈에"}}
                }
                div {class:"software-use-grid",
                    for (index,feature) in features.iter().take(3).enumerate() {
                        article {span {class:"software-use-number",aria_hidden:"true","0{index+1}"}
                            h3 {{super::discovery::feature_parts(feature).0}}
                            if let Some(body)=super::discovery::feature_parts(feature).1 {p {"{body}"}}
                        }
                    }
                }
            }
        }
        div { class:if extended || features.len()>3 {"software-reference-grid"}else{"software-reference-grid software-reference-compact"},
            div {class:"software-details-copy",
                if extended {
                section {class:"software-community-description",aria_labelledby:"software-description-title",
                    h2 {id:"software-description-title","소개"}
                    RegisteredDescription {item:item.clone()}
                }
                }
                if features.len()>3 {
                    section { class:"software-features", aria_labelledby:"software-features-title",
                        h2 { id:"software-features-title", "더 살펴볼 특징" }
                        ul { for feature in features.iter().skip(3) { li { "{feature}" } } }
                    }
                }
            }
            aside {class:"software-reference-facts",aria_label:"프로젝트 정보",
                h2 {"프로젝트 정보"}
                dl {
                    dt {"종류"} dd {{super::discovery::category_names(&item,&categories)}}
                    if let Some(family)=super::discovery::family_name(&item) {
                        dt {"계열"} dd {"{family}" small {"같은 계열이어도 기능과 운영 방향은 다를 수 있어요."}}
                    }
                    if let Some(tech)=item.tech_stack.as_deref().filter(|s|!s.trim().is_empty()) {
                        dt {"개발 기술"} dd {"{tech}"}
                    }
                }
                Link {class:"text-link",to:Route::SoftwareEditor{name:item.name.clone()},"정보 고치기" ArrowRight {size:15}}
            }
        }
        super::families::FamilyIndex {items:related,category:String::new(),current:Some(item.name.clone())}
        section { class:"software-context", aria_label:"서버 선택 도움말",
            div { h2 { "도구를 골랐다면, 머물 곳을 찾아요." } p { "같은 소프트웨어라도 운영자, 규칙, 모이는 사람이 달라요." } }
            Link { class:"text-link", to:Route::SoftwareServers {name:item.name.clone()}, "서버 둘러보기" ArrowRight { size:16 } }
        }
        footer { class:"software-contribute",
            div { p { "함께 채우는 소개" } span { "틀린 정보는 고치고, 변경 이력은 남겨요." } }
            div { class:"membership-actions",
                Link { class:"text-link", to:Route::SoftwareEditor { name:item.name.clone() }, "정보 수정" }
                Link { class:"text-link", to:Route::SoftwareHistory { name:item.name }, "변경 이력" }
                a { class:"text-link", href:"/about#software-marks", "이미지 출처" }
            }
        }
    }
}

#[component]
fn RegisteredDescription(item: Software) -> Element {
    rsx! {section {class:"software-description",aria_label:"등록된 소프트웨어 소개",
        if !item.description_html.trim().is_empty() {
            div {class:"rich-description",dangerous_inner_html:"{item.description_html}"}
        } else if !item.description.trim().is_empty() {p {"{item.description}"}}
        else {p {class:"software-empty","아직 소개가 등록되지 않았어요. 알고 있는 내용을 함께 채워 주세요."}}
    }}
}

#[cfg(test)]
mod tests {
    use super::*;
    use dioxus_history::{History, MemoryHistory};
    use dioxus_router::components::HistoryProvider;
    use std::rc::Rc;

    fn example() -> Software {
        Software {
            name: "independent-tool".into(),
            display_name: "새 도구".into(),
            description: "짧은 소개".into(),
            description_html: "<p>원래 소개의 본문</p>".into(),
            categories: vec![],
            features: vec!["등록된 기능".into()],
            website_url: Some("https://project.example.org/".into()),
            tech_stack: Some("Rust".into()),
            logo_available: false,
            family: None,
            development_status: None,
        }
    }

    #[derive(Clone, Debug, PartialEq, Routable)]
    enum TestRoute {
        #[route("/")]
        Fixture {},
    }

    #[component]
    fn TestApp(item: Software) -> Element {
        use_context_provider(|| Signal::new(false));
        use_context_provider(|| item);
        rsx! { HistoryProvider { history:move |_| Rc::new(MemoryHistory::with_initial_path("/")) as Rc<dyn History>,
            Router::<TestRoute> {}
        } }
    }

    #[component]
    fn Fixture() -> Element {
        let item = use_context::<Software>();
        rsx! {Overview {item,categories:vec![]}}
    }

    fn render(item: Software) -> String {
        let mut dom = VirtualDom::new_with_props(TestApp, TestAppProps { item });
        dom.rebuild_in_place();
        dioxus::ssr::render(&dom)
    }

    #[test]
    fn details_show_the_description_and_leave_clear_next_steps() {
        let html = render(example());
        assert_eq!(html.matches("<h1").count(), 1);
        assert!(html.contains("원래 소개의 본문"));
        assert!(!html.contains("<details"));
        assert!(html.contains("href=\"/software/independent-tool/servers\""));
        assert!(html.contains("href=\"https://project.example.org/\""));
        assert!(html.contains("href=\"/account/software/independent-tool/edit\""));
        assert!(html.contains("software-emblem"));
        assert!(html.contains("등록된 기능"));
        assert!(!html.contains("자동 생성한 표지"));
        assert!(!html.contains("제품 이미지 미등록"));
    }
    #[test]
    fn development_state_is_explicit_and_not_inferred_from_description() {
        let mut value = serde_json::to_value(example()).unwrap();
        value["development_status"] = "discontinued".into();
        let html = render(serde_json::from_value(value.clone()).unwrap());
        assert!(html.contains("개발 중단"));
        assert!(html.contains("data-inactive=\"true\""));
        value["development_status"] = "uncertain".into();
        let html = render(serde_json::from_value(value.clone()).unwrap());
        assert!(html.contains("유지보수 확인 필요"));
        assert!(!html.contains("data-inactive=\"true\""));
        value["development_status"] = serde_json::Value::Null;
        value["description"] = "개발 중단 프로젝트를 기반으로 한 새 프로젝트".into();
        let html = render(serde_json::from_value(value).unwrap());
        assert!(!html.contains("software-status"));
        assert!(!html.contains("data-inactive=\"true\""));
    }
    #[test]
    fn edited_known_product_and_new_fork_use_the_same_member_content() {
        let mut item = example();
        item.name = "misskey".into();
        item.description = "회원이 새로 고친 소개".into();
        item.description_html = "<p>회원이 새로 고친 소개</p>".into();
        item.features = (1..=32)
            .map(|i| format!("특징{i:02} — 회원 설명{i:02}"))
            .collect();
        let html = render(item.clone());
        assert!(html.contains("회원이 새로 고친 소개"));
        assert_eq!(html.matches("class=\"software-use-number\"").count(), 3);
        for i in 1..=32 {
            assert!(html.contains(&format!("회원 설명{i:02}")));
        }
        assert!(!html.contains("Deck 같은"));
        item.name = "new-fork".into();
        item.family = Some("misskey".into());
        item.features.clear();
        let html = render(item);
        assert!(html.contains("software-artwork-fallback"));
        assert!(html.contains("data:image/svg+xml;base64,"));
        assert!(html.contains("misskey 계열"));
        assert!(!html.contains("특징 한눈에"));
        assert!(html.contains("회원이 새로 고친 소개"));
    }
}
