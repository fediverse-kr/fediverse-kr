use super::*;
use crate::directory::catalog::{CatalogQuery, CATEGORY_PAGE_SIZE, SOFTWARE_PAGE_SIZE};

#[component]
pub fn CatalogListing(criteria: CatalogQuery) -> Element {
    let mut draft = use_signal({
        let query = criteria.query.clone();
        move || query
    });
    let result = use_server_future(use_reactive((&criteria,), |(criteria,)| {
        api::search_software(criteria.to_string())
    }))?;
    // The bounded public catalog provides examples even when they are on page two.
    // Search/pagination remain authoritative and work if this optional introduction fails.
    let overview = use_server_future(use_reactive((&criteria,), |(criteria,)| async move {
        if criteria.query.is_empty() && criteria.page == 0 && criteria.validate().is_ok() {
            api::catalog().await.map(Some)
        } else {
            Ok(None)
        }
    }))?;
    let ready = use_context::<Signal<bool>>()();
    let navigator = use_navigator();
    let for_search = criteria.clone();
    rsx! {
        document::Title { "소프트웨어 둘러보기 — fediverse.kr" }
        document::Stylesheet { href:asset!("/assets/styling/software-catalog.css") }
        main {id:"content",class:"directory-page software-catalog-page wrap",
            header {class:"page-heading",p{"소프트웨어 둘러보기"}h1{"무엇을 나누고 싶으세요?"}p{"대화부터 사진, 독서까지. 하고 싶은 일에서 시작해요."}}
            if let Err(message)=criteria.validate() {
                {crate::portal::response_status(400)}
                p {class:"data-notice",role:"alert","{message}"}
                Link {class:"text-link",to:Route::Platforms{filters:Default::default()},"전체 목록 보기"}
            } else {
                match result() {
                    Some(Ok(data))=>rsx! {
                        CatalogPreviewNote{preview:data.preview}
                        nav {class:"software-kinds",aria_label:"소프트웨어 종류",
                            Link{to:Route::Platforms{filters:criteria.category(String::new())},aria_current:if criteria.category.is_empty(){"true"}else{"false"},strong{"모두 둘러보기"}}
                            for kind in &data.kinds.categories {
                                Link{key:"{kind.name}",to:Route::Platforms{filters:criteria.category(kind.name.clone())},aria_current:if criteria.category==kind.name{"true"}else{"false"},strong{{super::discovery::activity(kind)}}}
                            }
                        }
                        if data.kinds.total>CATEGORY_PAGE_SIZE as i64 || criteria.kind_page>0 {
                            CatalogPager{criteria:criteria.clone(),kinds:true,total:data.kinds.total,has_next:data.kinds.has_next}
                        }
                        if data.kinds.categories.is_empty() && criteria.kind_page>0 {
                            p {class:"review-note","이 페이지에 표시할 종류가 없어요. " Link{class:"text-link",to:Route::Platforms{filters:CatalogQuery{kind_page:0,..criteria.clone()}},"첫 종류로"}}
                        }
                        if let Some(Ok(Some(catalog)))=overview() {
                            super::discovery::Examples {items:catalog.software.clone(),category:criteria.category.clone()}
                            super::families::FamilyIndex {items:catalog.software,category:criteria.category.clone(),current:None}
                        }
                        section {id:"software-all",class:"software-all",aria_labelledby:"software-all-title",
                        div {class:"software-directory-heading",
                            div {h2 {id:"software-all-title",if criteria.category.is_empty(){"전체 소프트웨어"}else{"이 종류의 소프트웨어"}}p{"파생 프로젝트도 빠짐없이. 이름을 알고 있다면 바로 찾아보세요."}}
                        form {class:"directory-search-form catalog-search-form",onsubmit:move|event|{
                            event.prevent_default(); if !ready{return;}
                            navigator.push(Route::Platforms{filters:CatalogQuery{query:draft().trim().into(),page:0,..for_search.clone()}});
                        },
                            label{class:"search-box",Search{size:19}input{r#type:"search",name:"q",maxlength:256,disabled:!ready,placeholder:"소프트웨어 이름 검색",aria_label:"소프트웨어 이름 검색",value:draft(),oninput:move|e|draft.set(e.value())}}
                            button{r#type:"submit",class:"secondary-button",disabled:!ready,"검색"}
                        }
                        }
                        div {class:"catalog-results-heading",
                            p {class:"directory-result-count",role:"status","소프트웨어 {number(data.total)}개"}
                            if !criteria.category.is_empty() {p{class:"catalog-kind",{data.labels.iter().find(|k|k.name==criteria.category).map(|k|k.label.clone()).unwrap_or(criteria.category.clone())}}}
                            if !criteria.query.is_empty() || !criteria.category.is_empty() {Link{class:"text-link",to:Route::Platforms{filters:Default::default()},"초기화"}}
                        }
                        div {class:"software-directory-grid",
                            for item in data.software.clone() {SoftwareCard{key:"{item.name}",item,labels:data.labels.clone()}}
                        }
                        if data.software.is_empty() {
                            div {class:"empty-state",h2{"조건에 맞는 소프트웨어가 없어요."}p{"다른 종류나 이름으로 찾아보세요."}
                                if criteria.page>0 {Link{class:"text-link",to:Route::Platforms{filters:CatalogQuery{page:0,..criteria.clone()}},"첫 페이지로"}}
                            }
                        }
                        if data.total>SOFTWARE_PAGE_SIZE as i64 || criteria.page>0 {CatalogPager{criteria:criteria.clone(),kinds:false,total:data.total,has_next:data.has_next}}
                        }
                    },
                    Some(Err(_))=>rsx!{DataError{}},
                    None=>rsx!{p{role:"status","목록을 불러오는 중…"}},
                }
            }
            section {class:"catalog-next",aria_label:"다음으로",
                div {h2{"마음에 드는 도구를 찾았다면"}p{"그 도구로 운영하는 서버를 골라 가입해요."}Link{class:"text-link",to:Route::Servers {filters:Default::default()},"가입할 서버 찾기" ArrowRight{size:16}}}
                div {h2{"함께 채우는 목록"}p{"빠진 소프트웨어를 등록하거나, 소개를 직접 고칠 수 있어요."}div {class:"catalog-contribution-links",Link{class:"text-link",to:Route::SoftwareNew{},"소프트웨어 등록"}a{class:"text-link",href:"/about#software-marks","로고 출처"}}}
            }
        }
    }
}
#[component]
fn SoftwareCard(item: Software, labels: Vec<Category>) -> Element {
    rsx! {Link{class:"software-directory-item","data-inactive":item.inactive().to_string(),to:Route::SoftwareDetail{name:item.name.clone()},
        brand::SoftwareMark{item:item.clone(),size:44}
        div {class:"software-directory-copy",
            div {class:"software-directory-name",h3{"{item.display_name}"}
                brand::SoftwareStatus{item:item.clone()}
                if let Some(family)=super::discovery::family_name(&item) {span{class:"software-family","{family} 계열"}}
            }
            span{class:"catalog-kind",{super::discovery::category_names(&item,&labels)}}
            p{{super::discovery::summary(&item)}}
        }
        ArrowRight{size:17}
    }}
}
#[component]
fn CatalogPager(criteria: CatalogQuery, kinds: bool, total: i64, has_next: bool) -> Element {
    let page = if kinds {
        criteria.kind_page
    } else {
        criteria.page
    };
    let size = if kinds {
        CATEGORY_PAGE_SIZE
    } else {
        SOFTWARE_PAGE_SIZE
    } as i64;
    let mut previous = criteria.clone();
    let mut next = criteria;
    if kinds {
        previous.kind_page = page.saturating_sub(1);
        next.kind_page = page.saturating_add(1);
    } else {
        previous.page = page.saturating_sub(1);
        next.page = page.saturating_add(1);
    }
    let pages = ((total + size - 1) / size).max(1);
    rsx! {nav{class:"directory-pagination",aria_label:if kinds{"종류 목록 페이지"}else{"소프트웨어 목록 페이지"},
        if page>0{Link{class:"secondary-button",to:Route::Platforms{filters:previous},"이전"}}else{button{class:"secondary-button",disabled:true,"이전"}}
        span{"{page+1} / {pages}페이지"}
        if has_next && page<10_000{Link{class:"secondary-button",to:Route::Platforms{filters:next},"다음"}}else{button{class:"secondary-button",disabled:true,"다음"}}
    }}
}

#[cfg(test)]
mod tests {
    use super::*;
    use dioxus_history::{History, MemoryHistory};
    use dioxus_router::components::HistoryProvider;
    use std::rc::Rc;

    #[derive(Clone, Debug, PartialEq, Routable)]
    enum TestRoute {
        #[route("/")]
        TestCards {},
    }

    #[component]
    fn TestApp() -> Element {
        use_context_provider(|| Signal::new(false));
        rsx! {HistoryProvider{history:move |_|Rc::new(MemoryHistory::with_initial_path("/")) as Rc<dyn History>,Router::<TestRoute>{}}}
    }

    #[component]
    fn TestCards() -> Element {
        let data = crate::directory::preview::catalog();
        rsx! {for item in data.software {SoftwareCard{item,labels:data.categories.clone()}}}
    }

    #[test]
    fn cards_use_generic_marks_when_runtime_media_is_absent() {
        let mut dom = VirtualDom::new(TestApp);
        dom.rebuild_in_place();
        let html = dioxus::ssr::render(&dom);
        assert_eq!(
            html.matches("class=\"software-directory-item\"").count(),
            28
        );
        assert_eq!(html.matches("software-emblem").count(), 28);
        assert_eq!(html.matches("<img").count(), 0);
        assert_eq!(html.matches("software-artwork ").count(), 0);
        for name in [
            "mastodon",
            "misskey",
            "pixelfed",
            "peertube",
            "lemmy",
            "bookwyrm",
            "cherrypick",
        ] {
            assert!(
                html.contains(&format!("href=\"/software/{name}\"")),
                "{name}"
            );
        }
        assert!(!html.contains("/api/public/software-logo/"));
    }
}
