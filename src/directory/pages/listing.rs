use super::*;
use crate::directory::search::ServerQuery;

#[component]
pub fn ServerListing(criteria: ServerQuery) -> Element {
    let mut draft = use_signal({
        let initial = criteria.clone();
        move || initial
    });
    use_effect(use_reactive((&criteria,), move |(criteria,)| {
        draft.set(criteria)
    }));
    let result = use_server_future(use_reactive((&criteria,), |(criteria,)| {
        api::search_servers(criteria.to_string())
    }))?;
    let options = use_server_future(api::server_filters)?;
    let ready = use_context::<Signal<bool>>()();
    let navigator = use_navigator();
    let prev = ServerQuery {
        page: criteria.page.saturating_sub(1),
        ..criteria.clone()
    };
    let next = ServerQuery {
        page: criteria.page.saturating_add(1),
        ..criteria.clone()
    };
    let all_registration = ServerQuery {
        registration: "all".into(),
        page: 0,
        ..criteria.clone()
    };
    rsx! {
        document::Title {"서버 찾기 — fediverse.kr"}
        main {id:"content",class:"directory-page wrap",
            header {class:"page-heading",h1 {"어디에 머물고 싶으세요?"}p {"친구와 같은 곳이 아니어도 괜찮아요. 규칙과 가입 방식을 먼저 읽어보세요."}}
            if criteria.invalid {
                {crate::portal::response_status(400)}
                p {role:"alert",class:"data-notice","검색 주소의 조건이 올바르지 않아요."}
                Link {class:"secondary-button",to:Route::Servers{filters:ServerQuery::default()},"검색 초기화"}
            } else {
                form {class:"directory-filters",onsubmit:move|e|{e.prevent_default();let mut query=draft();query.page=0;navigator.push(Route::Servers{filters:query});},
                    div {class:"directory-search-row",
                        div {class:"directory-search-form",
                            label {class:"search-box",Search{size:19} input{disabled:!ready,r#type:"search",maxlength:256,aria_label:"서버 이름, 주소, 소프트웨어 검색",placeholder:"이름, 주소, 소프트웨어 검색",value:draft().query,oninput:move|e|draft.write().query=e.value()}}
                            button{r#type:"submit",class:"secondary-button",disabled:!ready,"검색"}
                        }
                        DirectoryActions {}
                    }
                    if let Some(Ok(data))=options() {
                        details {class:"directory-filter-panel",
                            summary {
                                span {class:"directory-filter-summary-copy",strong{"필터와 정렬"}span{"필요할 때만 세부 조건을 고르세요."}}
                                span {class:"directory-filter-summary-action","조건 열기"}
                            }
                            div {class:"directory-filter-panel-body",
                                div {class:"directory-filter-grid",
                                    FilterSelect{id:"directory-category",label:"종류",value:draft().category,options:data.categories.into_iter().map(|v|(v.name,v.label)).collect(),on_change:move|value:String|{let mut d=draft.write();d.category=value;d.family.clear();d.software.clear();}}
                                    FilterSelect{id:"directory-family",label:"계열",value:draft().family,options:{let mut values:Vec<_>=data.software.iter().filter(|s|draft().category.is_empty() || s.categories.contains(&draft().category)).map(|s|(s.family.clone(),s.family.clone())).collect();values.sort();values.dedup();values},on_change:move|value:String|{let mut d=draft.write();d.family=value;d.software.clear();}}
                                    FilterSelect{id:"directory-software",label:"소프트웨어",value:draft().software,options:data.software.into_iter().filter(|s|(draft().category.is_empty() || s.categories.contains(&draft().category)) && (draft().family.is_empty() || s.family==draft().family)).map(|s|(s.name,s.label)).collect(),on_change:move|value:String|draft.write().software=value}
                                    FilterSelect{id:"directory-tag",label:"태그",value:draft().tag,options:data.tags.into_iter().map(|s|(s.clone(),s)).collect(),on_change:move|value:String|draft.write().tag=value}
                                    FilterSelect{id:"directory-registration",label:"가입 방식",all:false,value:draft().registration,options:choices(&[("all","모두"),("open","가입 접수 중"),("approval","가입 승인 필요"),("invite_only","초대 필요"),("closed","가입 접수 닫힘"),("unknown","가입 방식 확인 전")]),on_change:move|value:String|draft.write().registration=value}
                                    FilterSelect{id:"directory-health",label:"응답 상태",all:false,value:draft().alive,options:choices(&[("all","모두"),("yes","최근 응답 확인"),("no","최근 응답 없음"),("unknown","아직 확인 전"),("closed","운영 종료")]),on_change:move|value:String|draft.write().alive=value}
                                    FilterSelect{id:"directory-sort",label:"정렬 기준",all:false,value:draft().sort,options:sort_options(),on_change:move|value:String|{let mut d=draft.write();d.direction=default_sort_direction(&value).into();d.sort=value;}}
                                    FilterSelect{id:"directory-direction",label:"정렬 방향",all:false,value:draft().direction,options:choices(&[("asc","오름차순"),("desc","내림차순")]),on_change:move|value:String|draft.write().direction=value}
                                    FilterSelect{id:"directory-size",label:"한 페이지에",all:false,value:draft().page_size.to_string(),options:choices(&[("24","24개"),("25","25개"),("50","50개"),("100","100개"),("200","200개")]),on_change:move|value:String|{if let Ok(size)=value.parse(){draft.write().page_size=size;}}}
                                }
                                if data.truncated {p {class:"review-note","선택 항목 일부만 표시됩니다. 주소의 직접 필터와 검색은 전체 데이터를 조회해요."}}
                                div {class:"directory-filter-actions",button{r#type:"submit",class:"primary-button",disabled:!ready,"조건 적용"}Link{class:"text-link",to:Route::Servers{filters:ServerQuery::default()},"초기화"}}
                            }
                        }
                    } else if matches!(options(),Some(Err(_))) {p{role:"alert",class:"data-notice","선택 항목을 불러오지 못했어요. 검색은 계속 사용할 수 있습니다."}}
                }
                match result() {
                    Some(Ok(data))=>rsx! {
                        PreviewNote{preview:data.listing.preview}
                        p {class:"directory-result-count",role:"status","{number(data.total)}곳"}
                        if data.without_registration>data.total {
                            p{class:"directory-filter-excluded","다른 가입 방식으로 {number(data.without_registration-data.total)}곳이 더 있어요. " Link{class:"text-link",to:Route::Servers{filters:all_registration},"가입 방식 모두 보기"}}
                        }
                        div{class:"server-grid",for site in data.listing.sites.clone(){
                            Link{class:"server-card",to:Route::ServerDetail{slug:site.domain.clone()},
                                div{class:"server-card-top",super::icon::SiteIcon{site:site.clone()}span{{site.software.as_deref().unwrap_or("소프트웨어 확인 전")}}}
                                h2{"{site.name}"}small{class:"server-domain","{site.domain}"}p{"{site.description}"}
                                if !site.guidance.tags.is_empty(){div{class:"server-card-tags",for tag in site.guidance.tags.iter().take(4){span{"#{tag}"}}}}
                                div{class:"server-card-bottom",span{"{site.status()}"}span{"{site.registration()}"}}
                                if let Some(users)=site.users {small{class:"server-card-metric","가입 계정 {number(users)}개"}}
                                if criteria.sort=="response_time" {small{class:"server-card-metric",{site.average_response_ms.map(|n|format!("7일 평균 {n} ms")).unwrap_or_else(||"평균 응답 자료 부족".into())}}}
                            }
                        }}
                        if data.listing.sites.is_empty(){div{class:"empty-state",h2{"조건에 맞는 서버가 없어요."}p{"검색어와 필터를 확인해 주세요. 확인되지 않은 정보는 임의로 채우지 않아요."}}}
                        nav{class:"directory-pagination",aria_label:"서버 목록 페이지",
                            if criteria.page>0 {Link{class:"secondary-button",to:Route::Servers{filters:prev},"이전"}}else{button{class:"secondary-button",disabled:true,"이전"}}
                            span{"{data.listing.page+1}페이지"}
                            if data.listing.has_next && criteria.page<10_000 {Link{class:"secondary-button",to:Route::Servers{filters:next},"다음"}}else{button{class:"secondary-button",disabled:true,"다음"}}
                        }
                    },
                    Some(Err(_))=>rsx!{DataError{}},None=>rsx!{p{role:"status","목록을 불러오는 중…"}},
                }
            }
            aside{class:"directory-help",strong{"서버는 소프트웨어와 운영자가 함께 만들어요."}p{"응답 확인은 가입 가능이나 운영 품질을 보증하지 않아요. 가입 전 해당 사이트의 안내를 확인하세요."}Link{class:"text-link",to:Route::Platforms { filters: Default::default() },"소프트웨어부터 둘러보기" ArrowRight{size:16}}}
        }
    }
}

#[component]
fn DirectoryActions() -> Element {
    rsx! {
        div { class: "directory-search-actions",
            Link {class:"secondary-button directory-register-link",aria_label:"목록에 없는 서버 등록",to:Route::RegisterSite{},span {class:"directory-register-full","목록에 없는 서버 등록"}span {class:"directory-register-short","서버 등록"}}
            Link {class:"secondary-button directory-manage-link",to:Route::ManagedSites{},"내 서버 정보 관리"}
        }
    }
}

fn choices(values: &[(&str, &str)]) -> Vec<(String, String)> {
    values
        .iter()
        .map(|(v, l)| (v.to_string(), l.to_string()))
        .collect()
}

fn sort_options() -> Vec<(String, String)> {
    choices(&[
        ("recommended", "추천순"),
        ("name", "이름"),
        ("domain", "주소"),
        ("users", "가입 계정 수"),
        ("recent", "응답 확인 시각"),
        ("newest", "등록 시각"),
        ("response_time", "7일 평균 응답 시간"),
    ])
}

fn default_sort_direction(sort: &str) -> &'static str {
    if ["recommended", "users", "recent", "newest"].contains(&sort) {
        "desc"
    } else {
        "asc"
    }
}
#[component]
fn FilterSelect(
    id: String,
    label: String,
    value: String,
    options: Vec<(String, String)>,
    on_change: EventHandler<String>,
    #[props(default = true)] all: bool,
) -> Element {
    let ready = use_context::<Signal<bool>>()();
    rsx! {div{class:"directory-filter",label{r#for:id.clone(),"{label}"}select{id,value:value.clone(),disabled:!ready,onchange:move|e|on_change.call(e.value()),
        if all {option{value:"",selected:value.is_empty(),"모두"}}
        if !value.is_empty() && !options.iter().any(|(key,_)|key==&value){option{value:value.clone(),selected:true,"{value}"}}
        for (key,label) in options{option{selected:key==value,value:key,"{label}"}}
    }}}
}

#[cfg(test)]
mod tests {
    use super::*;
    use dioxus_history::{History, MemoryHistory};
    use dioxus_router::components::HistoryProvider;
    use std::rc::Rc;

    #[derive(Clone, Routable, PartialEq)]
    enum DirectoryActionsTestRoute {
        #[route("/")]
        ActionsTestPage {},
    }

    #[component]
    fn ActionsTestPage() -> Element {
        rsx! { DirectoryActions {} }
    }

    #[component]
    fn DirectoryActionsTestApp() -> Element {
        rsx! {
            HistoryProvider {
                history: move |_| Rc::new(MemoryHistory::with_initial_path("/")) as Rc<dyn History>,
                Router::<DirectoryActionsTestRoute> {}
            }
        }
    }

    #[test]
    fn directory_actions_link_to_registration_and_owner_management() {
        let mut dom = VirtualDom::new(|| rsx! { DirectoryActionsTestApp {} });
        dom.rebuild_in_place();
        let html = dioxus::ssr::render(&dom);

        assert!(html.contains("href=\"/account/sites/new\""), "{html}");
        assert!(html.contains("href=\"/account/sites\""), "{html}");
        assert!(html.contains("내 서버 정보 관리"), "{html}");
    }

    #[test]
    fn recommended_sort_is_visible_and_defaults_to_descending() {
        assert_eq!(
            sort_options().first(),
            Some(&("recommended".to_string(), "추천순".to_string()))
        );
        assert_eq!(default_sort_direction("recommended"), "desc");
    }
}
