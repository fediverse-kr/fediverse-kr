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
    let ready = use_context::<Signal<bool>>()();
    let navigator = use_navigator();
    let for_search = criteria.clone();
    rsx! {
        document::Title { "소프트웨어 둘러보기 — fediverse.kr" }
        main {id:"content",class:"directory-page wrap",
            header {class:"page-heading",p{"소프트웨어 둘러보기"}h1{"무엇을 나누고 싶으세요?"}p{"같은 연합우주, 서로 다른 쓰임새. 종류부터 골라보세요."}}
            if let Err(message)=criteria.validate() {
                {crate::portal::response_status(400)}
                p {class:"data-notice",role:"alert","{message}"}
                Link {class:"text-link",to:Route::Platforms{filters:Default::default()},"전체 목록 보기"}
            } else {
                match result() {
                    Some(Ok(data))=>rsx! {
                        PreviewNote{preview:data.preview}
                        nav {class:"kind-grid",aria_label:"소프트웨어 종류",
                            Link{to:Route::Platforms{filters:criteria.category(String::new())},aria_current:if criteria.category.is_empty(){"true"}else{"false"},span{"✳"}strong{"모든 종류"}}
                            for kind in &data.kinds.categories {
                                Link{key:"{kind.name}",to:Route::Platforms{filters:criteria.category(kind.name.clone())},aria_current:if criteria.category==kind.name{"true"}else{"false"},span{"{kind.emoji}"}strong{"{kind.label}"}}
                            }
                        }
                        if data.kinds.total>CATEGORY_PAGE_SIZE as i64 || criteria.kind_page>0 {
                            CatalogPager{criteria:criteria.clone(),kinds:true,total:data.kinds.total,has_next:data.kinds.has_next}
                        }
                        if data.kinds.categories.is_empty() && criteria.kind_page>0 {
                            p {class:"review-note","이 페이지에 표시할 종류가 없어요. " Link{class:"text-link",to:Route::Platforms{filters:CatalogQuery{kind_page:0,..criteria.clone()}},"첫 종류로"}}
                        }
                        form {class:"directory-search-form catalog-search-form",onsubmit:move|event|{
                            event.prevent_default(); if !ready{return;}
                            navigator.push(Route::Platforms{filters:CatalogQuery{query:draft().trim().into(),page:0,..for_search.clone()}});
                        },
                            label{class:"search-box",Search{size:19}input{r#type:"search",name:"q",maxlength:256,disabled:!ready,placeholder:"소프트웨어 이름 검색",aria_label:"소프트웨어 이름 검색",value:draft(),oninput:move|e|draft.set(e.value())}}
                            button{r#type:"submit",class:"secondary-button",disabled:!ready,"검색"}
                        }
                        div {class:"catalog-results-heading",
                            p {class:"directory-result-count",role:"status","소프트웨어 {number(data.total)}개"}
                            if !criteria.category.is_empty() {p{class:"catalog-kind",{data.labels.iter().find(|k|k.name==criteria.category).map(|k|k.label.clone()).unwrap_or(criteria.category.clone())}}}
                            if !criteria.query.is_empty() || !criteria.category.is_empty() {Link{class:"text-link",to:Route::Platforms{filters:Default::default()},"초기화"}}
                        }
                        div {class:"software-grid",
                            for item in data.software.clone() {SoftwareCard{key:"{item.name}",item,labels:data.labels.clone()}}
                        }
                        if data.software.is_empty() {
                            div {class:"empty-state",h2{"조건에 맞는 소프트웨어가 없어요."}p{"다른 종류나 이름으로 찾아보세요."}
                                if criteria.page>0 {Link{class:"text-link",to:Route::Platforms{filters:CatalogQuery{page:0,..criteria.clone()}},"첫 페이지로"}}
                            }
                        }
                        if data.total>SOFTWARE_PAGE_SIZE as i64 || criteria.page>0 {CatalogPager{criteria:criteria.clone(),kinds:false,total:data.total,has_next:data.has_next}}
                    },
                    Some(Err(_))=>rsx!{DataError{}},
                    None=>rsx!{p{role:"status","목록을 불러오는 중…"}},
                }
            }
            aside {class:"directory-help",strong{"소프트웨어와 서버는 달라요."}p{"소프트웨어는 공간을 만드는 도구, 서버는 누군가 그 도구로 운영하는 실제 공간이에요."}Link{class:"text-link",to:Route::Start{},"화면으로 이해하기" ArrowRight{size:16}}}
            aside {class:"directory-help",strong{"빠진 도구가 있나요?"}p{"회원이 직접 소개를 등록하고, 틀린 정보를 고칠 수 있어요. 변경 이력은 함께 남습니다."}Link{class:"text-link",to:Route::SoftwareNew{},"소프트웨어 등록"}}
        }
    }
}
#[component]
fn SoftwareCard(item: Software, labels: Vec<Category>) -> Element {
    rsx! {Link{class:"software-card",to:Route::SoftwareDetail{name:item.name.clone()},
        span{class:"catalog-kind",{item.categories.iter().map(|c|labels.iter().find(|k|&k.name==c).map(|k|k.label.clone()).unwrap_or(c.clone())).collect::<Vec<_>>().join(" · ")}}
        h2{class:"software-title",if item.logo_available{crate::media_ui::ImageMark{source:crate::media_ui::software_source(&item.name),fallback:item.display_name.chars().next().unwrap_or('·').to_string(),class:"software-mark"}}span{"{item.display_name}"}}
        p{"{item.description}"}span{class:"text-link","살펴보기" ArrowRight{size:16}}
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
