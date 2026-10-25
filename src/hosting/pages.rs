use super::{api, HostingEdit as HostingFields, HostingService, OperatorRegion};
use crate::{
    membership::{api as member_api, pages::error_message},
    Route,
};
use dioxus::prelude::*;

fn unknown(value: &str) -> &str {
    if value.is_empty() {
        "미확인"
    } else {
        value
    }
}
fn support_label(value: Option<bool>) -> &'static str {
    match value {
        Some(true) => "한국어 지원",
        Some(false) => "한국어 미지원",
        None => "미확인",
    }
}
#[component]
fn HostingTags(region: OperatorRegion, korean_support: Option<bool>) -> Element {
    rsx! {
        div { class: "hosting-tags",
            span { class: "hosting-tag", "data-region": region.as_str(), "{region.label()}" }
            if korean_support.is_some() {
                span { class: "hosting-tag hosting-language", "{support_label(korean_support)}" }
            }
        }
    }
}
#[component]
pub fn HostingList() -> Element {
    let mut cursor = use_signal(|| (0u32, None::<u32>));
    let result = use_server_future(move || api::list(cursor().0, cursor().1))?;
    let ready = use_context::<Signal<bool>>()();
    rsx! {
        document::Title { "관리형 서비스 — fediverse.kr" }
        document::Stylesheet { href: asset!("/assets/hosting.css") }
        main { id: "content", class: "hosting-page hosting-directory wrap",
            header { class: "page-heading hosting-heading",
                div {
                    p { class: "hosting-eyebrow", "내 서버 시작하기" }
                    h1 { "관리형 서비스" }
                    p { "설치와 유지보수는 맡기고, 커뮤니티 운영에 집중하세요." }
                }
                Link { class: "secondary-button", to: Route::HostingNew {}, "서비스 등록" }
            }
            aside { class: "hosting-order", aria_label: "목록 표시 기준",
                p { strong { "한국 서비스 우선" } span { " · 같은 그룹은 매일 순환" } }
                details {
                    summary { "표시 기준 알아보기" }
                    p { "한국에 기반한 서비스, 해외 서비스, 운영 지역 미확인 순으로 표시해요. 데이터센터 위치가 아닌 운영 주체 기준이며, 한국어 지원 여부는 별도로 표시합니다." }
                    p { "같은 그룹 안에서는 한국 시간으로 매일 순번을 한 칸씩 바꿔요. 이름이나 등록 순서에 따른 추천 순위가 아닙니다. 페이지를 넘기는 동안에는 같은 순서를 유지해요." }
                }
            }
            match result() {
                Some(Ok(data)) => rsx! {
                    if data.items.is_empty() {
                        p { class: "hosting-empty",
                            if data.page == 0 { "아직 등록된 서비스가 없어요. 공식 안내를 확인한 회원이 첫 정보를 등록할 수 있어요." }
                            else { "이 페이지에는 서비스가 없어요. 이전 페이지로 돌아가 주세요." }
                        }
                    }
                    ul { class: "hosting-list",
                        for item in data.items {
                            li { key: "{item.slug}",
                                Link { class: "hosting-card", to: Route::HostingDetail { slug: item.slug.clone() },
                                    HostingTags { region: item.edit.operator_region, korean_support: item.edit.korean_support }
                                    h2 { "{item.edit.name}" }
                                    p { class: "hosting-scope", "{unknown(&item.edit.scope)}" }
                                    div { class: "hosting-software",
                                        span { "지원 소프트웨어" }
                                        p { "{unknown(&item.edit.software)}" }
                                    }
                                    span { class: "hosting-card-action", "범위와 근거 보기" span { aria_hidden: "true", " →" } }
                                }
                            }
                        }
                    }
                    if data.page > 0 || data.has_next {
                        nav { class: "hosting-actions", aria_label: "목록 페이지",
                            button { class: "secondary-button", disabled: !ready || data.page == 0,
                                onclick: move |_| cursor.set((data.page.saturating_sub(1), Some(data.rotation_day))), "이전" }
                            span { "{data.page + 1}페이지" }
                            button { class: "secondary-button", disabled: !ready || !data.has_next,
                                onclick: move |_| cursor.set((data.page + 1, Some(data.rotation_day))), "다음" }
                        }
                    }
                },
                Some(Err(e)) => rsx! { p { role: "alert", "{error_message(e.clone())}" } },
                _ => rsx! { p { role: "status", "불러오는 중…" } },
            }
            footer { class: "hosting-guide",
                p { "관리형 서비스도 가입 정책과 이용자 대응은 직접 맡아야 할 수 있어요. 계약 전 제공 범위와 최신 요금을 확인하세요." }
                Link { class: "text-link", to: "/operate", "서버 운영 방식 알아보기 →" }
            }
        }
    }
}
#[component]
pub fn HostingDetail(slug: String) -> Element {
    let data = use_server_future(use_reactive((&slug,), |(slug,)| api::detail(slug)))?;
    rsx! {
        document::Title { "관리형 서비스 — fediverse.kr" }
        document::Stylesheet { href: asset!("/assets/hosting.css") }
        main { id: "content", class: "hosting-page hosting-detail wrap",
            Link { class: "back-link", to: Route::HostingList {}, "← 관리형 서비스 목록" }
            match data() {
                Some(Ok(item)) if item.slug == slug => rsx! {
                    header { class: "page-heading",
                        HostingTags { region: item.edit.operator_region, korean_support: item.edit.korean_support }
                        h1 { "{item.edit.name}" }
                        p { "{unknown(&item.edit.scope)}" }
                    }
                    a { class: "primary-button", href: item.edit.website_url.clone(), target: "_blank", rel: "noopener noreferrer", "서비스 웹사이트에서 문의하기 ↗" }
                    dl { class: "hosting-facts",
                        div { dt { "지원 소프트웨어" } dd { "{unknown(&item.edit.software)}" } }
                        div { dt { "운영 주체의 지역" } dd { "{item.edit.operator_region.label()}" } }
                        div { dt { "한국어 지원" } dd { "{support_label(item.edit.korean_support)}" } }
                    }
                    h2 { class: "hosting-section-title", "어디까지 맡길 수 있나요?" }
                    dl { class: "hosting-responsibilities",
                        div { dt { "제공자가 맡는 일" } dd { "{unknown(&item.edit.provider_responsibilities)}" } }
                        div { dt { "고객이 맡는 일" } dd { "{unknown(&item.edit.customer_responsibilities)}" } }
                    }
                    section { class: "hosting-provenance", aria_label: "정보의 근거",
                        dl { class: "hosting-facts",
                            div { dt { "공식 안내·출처" } dd {
                                if item.edit.source_url.is_empty() { "미확인" }
                                else { a { href: item.edit.source_url.clone(), target: "_blank", rel: "noopener noreferrer", "서비스 안내 원문 ↗" } }
                            } }
                            div { dt { "분류 근거" } dd {
                                if item.edit.classification_source_url.is_empty() { "미확인" }
                                else { a { href: item.edit.classification_source_url.clone(), target: "_blank", rel: "noopener noreferrer", "운영 지역·언어 안내 원문 ↗" } }
                            } }
                            div { dt { "확인 날짜" } dd { "{unknown(&item.edit.checked_on)}" } }
                        }
                        p { "확인하지 않은 항목은 미확인으로 남깁니다. 태그는 서비스의 품질이나 계약 가능 여부를 보증하지 않아요." }
                        div { class: "hosting-actions",
                            Link { class: "secondary-button", to: Route::HostingEdit { slug: slug.clone() }, "정보 수정" }
                            Link { class: "secondary-button", to: Route::HostingHistory { slug: slug.clone() }, "변경 이력" }
                        }
                    }
                },
                Some(Err(e)) => rsx! { p { role: "alert", "{error_message(e.clone())}" } },
                _ => rsx! { p { role: "status", "불러오는 중…" } },
            }
        }
    }
}
#[component]
pub fn HostingNew() -> Element {
    rsx! {HostingEditor{slug:String::new()}}
}
#[component]
pub fn HostingEdit(slug: String) -> Element {
    rsx! {HostingEditor{slug}}
}
#[component]
fn HostingEditor(slug: String) -> Element {
    let account = use_server_future(member_api::session)?;
    let current = use_server_future(use_reactive((&slug,), |(slug,)| async move {
        if slug.is_empty() {
            Ok(None)
        } else {
            api::detail(slug).await.map(Some)
        }
    }))?;
    rsx! {
        document::Title{"관리형 서비스 등록·수정 — fediverse.kr"}
        document::Stylesheet{href:asset!("/assets/hosting.css")}
        main{id:"content",class:"hosting-page wrap",
            Link{class:"back-link",to:Route::HostingList{},"관리형 서비스 목록"}
            header{class:"page-heading",h1{if slug.is_empty(){"서비스 등록"}else{"서비스 정보 수정"}}p{"확인한 내용만 적고, 모르는 내용은 비워 두세요."}}
            match (account(),current()) {
                (Some(Ok(account)),Some(Ok(item))) if item.as_ref().is_none_or(|v|v.slug==slug)=>rsx!{EditorForm{key:"{slug}",slug,item,available:account.federation_available,member:account.member.is_some()}},
                (Some(Err(e)),_)|(_,Some(Err(e)))=>rsx!{p{role:"alert","{error_message(e.clone())}"}},
                _=>rsx!{p{role:"status","불러오는 중…"}},
            }
        }
    }
}
fn field(edit: &HostingFields, name: &str) -> String {
    match name {
        "name" => &edit.name,
        "website_url" => &edit.website_url,
        "scope" => &edit.scope,
        "software" => &edit.software,
        "provider_responsibilities" => &edit.provider_responsibilities,
        "customer_responsibilities" => &edit.customer_responsibilities,
        "source_url" => &edit.source_url,
        "classification_source_url" => &edit.classification_source_url,
        _ => &edit.checked_on,
    }
    .clone()
}
fn set(edit: &mut HostingFields, name: &str, value: String) {
    *match name {
        "name" => &mut edit.name,
        "website_url" => &mut edit.website_url,
        "scope" => &mut edit.scope,
        "software" => &mut edit.software,
        "provider_responsibilities" => &mut edit.provider_responsibilities,
        "customer_responsibilities" => &mut edit.customer_responsibilities,
        "source_url" => &mut edit.source_url,
        "classification_source_url" => &mut edit.classification_source_url,
        _ => &mut edit.checked_on,
    } = value;
}
#[component]
fn HostingSnapshot(edit: HostingFields) -> Element {
    rsx! { dl { class:"hosting-facts",
        for (label,value) in [("이름",edit.name),("웹사이트",edit.website_url),("소개",edit.scope),("소프트웨어",edit.software),("제공자가 맡는 일",edit.provider_responsibilities),("고객이 맡는 일",edit.customer_responsibilities),("출처",edit.source_url),("확인 날짜",edit.checked_on),("운영 주체의 지역",edit.operator_region.label().into()),("한국어 지원",support_label(edit.korean_support).into()),("분류 근거",edit.classification_source_url)] {
            div { dt { "{label}" } dd { "{unknown(&value)}" } }
        }
    } }
}
#[component]
fn EditorForm(
    slug: String,
    item: Option<HostingService>,
    available: bool,
    member: bool,
) -> Element {
    let mut draft = use_signal(|| item.as_ref().map(|i| i.edit.clone()).unwrap_or_default());
    let mut identifier = use_signal(String::new);
    let mut summary = use_signal(String::new);
    let mut pending = use_signal(|| false);
    let mut message = use_signal(String::new);
    let ready = use_context::<Signal<bool>>()();
    let nav = use_navigator();
    let can_save = ready && available && member;
    rsx! {
        if !available {p{class:"membership-notice","미리보기에서는 저장할 수 없어요."}}
        else if !member {p{class:"membership-notice","회원이면 누구나 바로 수정할 수 있어요."}Link{class:"secondary-button",to:Route::Login{},"로그인"}}
        form{class:"membership-card membership-form",onsubmit:move|e|{
            e.prevent_default();if !can_save || pending(){return;}pending.set(true);message.set(String::new());
            let edit=draft();let note=summary();let old=item.clone();let slug=slug.clone();let new_slug=identifier();
            spawn(async move{
                let result=if let Some(old)=old {api::save(slug,old.revision,edit,note).await}else{api::create(new_slug,edit,note).await};
                pending.set(false);
                match result {Ok(saved)=>{nav.push(Route::HostingDetail{slug:saved.slug});},Err(e)=>message.set(error_message(e.clone()))}
            });
        },
            p{class:"membership-hint","저장 즉시 공개되고 이력에 남습니다. 가격이나 보장 범위를 추측해 적지 마세요."}
            if item.is_none(){div{class:"membership-field",label{r#for:"hosting-slug","주소 식별자"}input{id:"hosting-slug",value:identifier,required:true,maxlength:64,pattern:r"[a-zA-Z0-9][a-zA-Z0-9\-]*",oninput:move|e|identifier.set(e.value())}}}
            for (name,label,max,rows) in [("name","서비스 이름",160,1),("website_url","서비스 웹사이트",2048,1),("scope","한 줄 소개 · 선택",240,1),("software","지원 소프트웨어 · 선택",240,1),("provider_responsibilities","제공자가 맡는 일 · 선택",1000,3),("customer_responsibilities","고객이 맡는 일 · 선택",1000,3),("source_url","공식 안내·출처 · 선택",2048,1),("checked_on","확인 날짜 · 출처가 있을 때만",10,1)] {
                div{class:"membership-field",label{r#for:"hosting-{name}","{label}"}
                    if rows==1 {input{id:"hosting-{name}",r#type:if name=="website_url" || name=="source_url"{"url"}else if name=="checked_on"{"date"}else{"text"},value:field(&draft(),name),maxlength:max,required:name=="name" || name=="website_url",disabled:pending(),oninput:move|e|set(&mut draft.write(),name,e.value())}}
                    else {textarea{id:"hosting-{name}",value:field(&draft(),name),rows,maxlength:max,disabled:pending(),oninput:move|e|set(&mut draft.write(),name,e.value())}}
                }
            }
            fieldset { class: "hosting-classification",
                legend { "목록에 표시할 태그" }
                p { class: "membership-hint", "공식 안내에서 확인한 내용만 선택해 주세요. 운영 지역은 데이터센터가 아닌 운영 주체 기준입니다." }
                div { class: "membership-field",
                    label { r#for: "hosting-region", "운영 주체의 지역" }
                    select { id: "hosting-region", value: draft().operator_region.as_str(), disabled: pending(),
                        onchange: move |e| draft.write().operator_region = match e.value().as_str() { "korea" => OperatorRegion::Korea, "overseas" => OperatorRegion::Overseas, _ => OperatorRegion::Unknown },
                        for region in [OperatorRegion::Unknown, OperatorRegion::Korea, OperatorRegion::Overseas] {
                            option { value: region.as_str(), selected: draft().operator_region == region, "{region.label()}" }
                        }
                    }
                }
                div { class: "membership-field",
                    label { r#for: "hosting-korean-support", "한국어 지원 여부" }
                    select { id: "hosting-korean-support", value: match draft().korean_support { Some(true) => "yes", Some(false) => "no", None => "unknown" }, disabled: pending(),
                        onchange: move |e| draft.write().korean_support = match e.value().as_str() { "yes" => Some(true), "no" => Some(false), _ => None },
                        option { value: "unknown", selected: draft().korean_support.is_none(), "미확인" }
                        option { value: "yes", selected: draft().korean_support == Some(true), "한국어 지원" }
                        option { value: "no", selected: draft().korean_support == Some(false), "한국어 미지원" }
                    }
                }
                div { class: "membership-field",
                    label { r#for: "hosting-classification_source_url", "분류 근거 URL · 태그를 지정할 때 필수" }
                    input { id: "hosting-classification_source_url", r#type: "url", value: draft().classification_source_url, maxlength: 2048, disabled: pending(),
                        required: draft().operator_region != OperatorRegion::Unknown || draft().korean_support.is_some(),
                        oninput: move |e| draft.write().classification_source_url = e.value() }
                }
            }
            div{class:"membership-field",label{r#for:"hosting-summary","변경 요약"}input{id:"hosting-summary",value:summary,required:true,maxlength:200,oninput:move|e|summary.set(e.value())}}
            if !message().is_empty(){p{role:"alert","{message}"}if item.is_some(){Link{to:Route::HostingDetail{slug:slug.clone()},"최신 공개 내용 보기"}}}
            button{class:"primary-button",r#type:"submit",disabled:!can_save || pending(),if pending(){"처리 중…"}else{"저장하기"}}
        }
    }
}
#[component]
pub fn HostingHistory(slug: String) -> Element {
    let mut page = use_signal(|| 0u32);
    let mut selected = use_signal(|| None::<(i64, HostingFields)>);
    let mut message = use_signal(String::new);
    let mut pending = use_signal(|| false);
    let mut summary = use_signal(String::new);
    let mut confirmed = use_signal(|| false);
    let history = use_server_future(use_reactive((&slug,), move |(slug,)| {
        api::history(slug, page())
    }))?;
    let mut current = use_server_future(use_reactive((&slug,), |(slug,)| api::detail(slug)))?;
    let account = use_server_future(member_api::session)?;
    let ready = use_context::<Signal<bool>>()();
    let nav = use_navigator();
    let can_restore = ready
        && account().is_some_and(|a| a.is_ok_and(|a| a.federation_available && a.member.is_some()));
    let mut identity = use_signal(|| slug.clone());
    use_effect(use_reactive((&slug,), move |(next,)| {
        if *identity.peek() != next {
            identity.set(next);
            page.set(0);
            selected.set(None);
            confirmed.set(false);
            summary.set(String::new());
            message.set(String::new());
        }
    }));
    rsx! {
        document::Title{"서비스 변경 이력 — fediverse.kr"}
        document::Stylesheet{href:asset!("/assets/hosting.css")}
        main{id:"content",class:"hosting-page wrap",
            Link{class:"back-link",to:Route::HostingDetail{slug:slug.clone()},"서비스 소개"}
            header{class:"page-heading",h1{"변경 이력"}p{"과거 내용을 확인하고 되돌릴 수 있어요."}}
            match (history(),current(),account()) {
                (Some(Ok(records)),Some(Ok(live)),Some(Ok(_account))) if live.slug==slug=>rsx!{
                    ol{class:"hosting-history",
                        for entry in records.entries {
                            li{
                                strong{"버전 {entry.revision} · {entry.action}"}
                                p{"{entry.summary} · {entry.created_at}"}
                                button{class:"secondary-button",disabled:!ready || pending(),onclick:{
                                    let slug=slug.clone();
                                    move|_|{
                                        let slug=slug.clone();
                                        pending.set(true);
                                        spawn(async move{
                                            let result=api::read_revision(slug,entry.revision).await;
                                            pending.set(false);
                                            match result{Ok(edit)=>{selected.set(Some((entry.revision,edit)));confirmed.set(false);},Err(e)=>message.set(error_message(e.clone()))}
                                        });
                                    }
                                },"내용 보기"}
                            }
                        }
                    }
                    nav{class:"hosting-actions",button{class:"secondary-button",disabled:page()==0,onclick:move|_|page.set(page().saturating_sub(1)),"이전"}button{class:"secondary-button",disabled:!records.has_next,onclick:move|_|page.set(page()+1),"다음"}}
                    if identity()==slug {if let Some((revision,edit))=selected(){section{class:"membership-card",h2{"선택한 버전 {revision}"}HostingSnapshot{edit}
                        h2{"현재 버전 {live.revision}"}HostingSnapshot{edit:live.edit.clone()}
                        if revision!=live.revision {form{onsubmit:{let slug=slug.clone();move|e|{e.prevent_default();if !confirmed() || !can_restore || pending(){return;}pending.set(true);let slug=slug.clone();spawn(async move{let result=api::restore(slug,live.revision,revision,summary()).await;pending.set(false);match result{Ok(saved)=>{nav.push(Route::HostingDetail{slug:saved.slug});},Err(e)=>{message.set(error_message(e.clone()));confirmed.set(false);current.restart();}}});}},
                            label{r#for:"hosting-restore-summary","복원 이유"}input{id:"hosting-restore-summary",value:summary,required:true,maxlength:200,oninput:move|e|summary.set(e.value())}
                            label{input{r#type:"checkbox",checked:confirmed(),onchange:move|e|confirmed.set(e.checked())}"현재 버전과 비교했습니다."}
                            button{class:"primary-button",disabled:!confirmed() || pending() || !can_restore,"이 버전으로 복원"}
                        }}
                    }}}
                    },
                    (Some(Err(e)),_,_)|(_,Some(Err(e)),_)|(_,_,Some(Err(e)))=>rsx!{p{role:"alert","{error_message(e.clone())}"}},
                _=>rsx!{p{role:"status","불러오는 중…"}},
            }
            if !message().is_empty(){p{role:"alert","{message}"}}
        }
    }
}
