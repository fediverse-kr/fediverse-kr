use super::Action;
use super::*;
use crate::{
    moderation::{
        layout::{BackofficePage, BackofficeSection},
        pages::{message, timestamp},
    },
    Route,
};
use dioxus::prelude::*;
mod logo;
#[cfg(test)]
mod render_tests;
#[cfg(test)]
mod shell_tests;
fn failure(e: ServerFnError) -> Element {
    if let ServerFnError::ServerError { code, .. } = &e {
        crate::portal::response_status(if (400..500).contains(code) {
            *code as u16
        } else {
            503
        });
    }
    rsx! {p{class:"membership-notice",role:"alert","{message(&e)}"}}
}
#[component]
fn CatalogNav(categories: bool) -> Element {
    rsx! {nav{class:"catalog-subnav",aria_label:"소프트웨어 관리 메뉴",
        Link{class:"text-link",aria_current:(!categories).then_some("page"),to:Route::ModerationCatalog{},"소프트웨어"}
        Link{class:"text-link",aria_current:categories.then_some("page"),to:Route::ModerationCategories{},"분류"}
    }}
}

#[component]
fn CatalogPage(
    title: String,
    description: String,
    #[props(default)] categories: bool,
    children: Element,
) -> Element {
    rsx! {
        document::Stylesheet{href:asset!("/assets/styling/backoffice-catalog.css")}
        BackofficePage{section:BackofficeSection::Catalog,title,description,
            div{class:"directory-workflow",CatalogNav{categories}{children}}
        }
    }
}
#[component]
pub fn ModerationCatalog() -> Element {
    let ready = use_context::<Signal<bool>>()();
    let mut query = use_signal(String::new);
    let mut draft = use_signal(String::new);
    let mut page = use_signal(|| 0u32);
    let mut locked = use_signal(|| false);
    let mut data = use_server_future(move || api::list(query(), locked(), page()))?;
    rsx! {document::Title{"소프트웨어 관리 — fediverse.kr"}
        CatalogPage{title:"소프트웨어 관리",description:"소개와 편집 잠금, 로고를 관리합니다. 분류는 별도 탭에서 정리합니다.",
            p{Link{class:"text-link",to:Route::SoftwareNew{},"소프트웨어 등록 · 공개 등록 화면으로"}}
            form{class:"membership-card membership-form",onsubmit:move|e|{e.prevent_default();query.set(draft());page.set(0);},
                div{class:"membership-field",label{r#for:"catalog-search","제품 이름·식별자 검색"}input{id:"catalog-search",value:draft,maxlength:128,disabled:!ready,oninput:move|e|draft.set(e.value())}}
                label{class:"owner-checkbox",input{r#type:"checkbox",checked:locked(),disabled:!ready,onchange:move|e|{locked.set(e.checked());page.set(0);}}"편집이 잠긴 제품만"}button{class:"primary-button",disabled:!ready,"검색"}
            }
            match data(){Some(Ok(data))=>rsx!{
                p{class:"directory-results",role:"status","이 페이지에 {data.items.len()}개" if locked(){" · 회원 편집 잠김"}else{" · 전체 잠금 상태"}if !query().is_empty(){" · 검색: {query}"}}
                if data.items.is_empty(){p{class:"membership-notice","조건에 맞는 제품이 없습니다."}}
                ul{class:"moderation-list",for item in data.items{li{key:"{item.name}",class:"membership-card",h2{Link{to:Route::ModerationSoftware{name:item.name.clone()},"{item.display_name}"}}p{"{item.name}"}p{class:"membership-hint","버전 {item.revision} · " if item.locked{"회원 편집 잠김"}else{"회원 편집 가능"}}Link{class:"text-link",to:Route::ModerationSoftwareEditor{name:item.name},"소개 수정"}}}}
                Pager{page,has_next:data.has_next}
            },Some(Err(e))=>failure(e),None=>rsx!{p{role:"status","목록을 불러오는 중…"}}}
            button{class:"secondary-button",disabled:!ready,onclick:move|_|data.restart(),"목록 새로고침"}
        }
    }
}
#[component]
pub fn ModerationSoftware(name: String) -> Element {
    rsx! {SoftwareLoader{key:"{name}",name}}
}
#[component]
fn SoftwareLoader(name: String) -> Element {
    let data = use_server_future(move || api::item(name.clone()))?;
    rsx! {document::Title{"소프트웨어 관리 — fediverse.kr"}CatalogPage{title:"제품 관리 설정",description:"현재 설정을 확인하고, 소개·로고·관리 설정을 각각 변경합니다.",
        Link{class:"text-link backoffice-back-link",to:Route::ModerationCatalog{},"소프트웨어 목록으로"}
        match data(){Some(Ok(initial))=>rsx!{Settings{initial}},Some(Err(e))=>failure(e),None=>rsx!{p{role:"status","불러오는 중…"}}}
    }}
}
#[component]
pub fn ModerationSoftwareEditor(name: String) -> Element {
    rsx! {document::Title{"소프트웨어 소개 수정 — fediverse.kr"}
        CatalogPage{title:"소프트웨어 소개 수정",description:"소개와 분류를 수정합니다. 저장한 내용과 변경 요약은 공개됩니다.",
            Link{class:"text-link backoffice-back-link",to:Route::ModerationSoftware{name:name.clone()},"관리 설정으로"}
            crate::directory::catalog_editing::pages::EditorBody{name,administrator:true}
        }
    }
}
#[component]
fn Settings(initial: Software) -> Element {
    let ready = use_context::<Signal<bool>>()();
    let mut current = use_signal(|| initial);
    let mut selected = use_signal(|| None::<Action>);
    let mut color = use_signal(String::new);
    let mut order = use_signal(String::new);
    let mut note = use_signal(String::new);
    let mut pending = use_signal(|| false);
    let mut notice = use_signal(String::new);
    let mut remote = use_signal(|| None::<Software>);
    let mut generation = use_signal(|| 0u32);
    let data = current();
    rsx! {section{class:"membership-card directory-current",p{class:"directory-kicker","현재 설정"}h2{"{data.display_name}"}p{"{data.name} · 버전 {data.revision}"}
        dl{class:"admin-site-facts",dt{"회원 편집"}dd{if data.locked{"잠김"}else{"가능"}}dt{"브랜드 색상"}dd{{data.brand_color.as_deref().unwrap_or("미설정")} if data.color_truncated{" · 일부만 표시"}}dt{"대표 제품 표식"}dd{if data.featured{"켜짐"}else{"꺼짐"}}dt{"표시 순서"}dd{"{data.display_order}"}}
        p{class:"membership-hint","표시 순서는 작은 수부터 적용됩니다. 색상·대표 표식은 기존 카탈로그 값을 관리하며, 현재 공개 화면에는 별도 강조를 적용하지 않습니다."}
        div{class:"membership-actions",Link{class:"primary-button",to:Route::ModerationSoftwareEditor{name:data.name.clone()},"소개 수정"}Link{class:"text-link",to:Route::SoftwareDetail{name:data.name.clone()},"공개 소개"}Link{class:"text-link",to:Route::SoftwareHistory{name:data.name.clone()},"공개 변경 이력"}}
    }
    section{class:"membership-card directory-actions",h2{"관리 설정 변경"}
        p{class:"membership-hint","바꿀 설정을 선택한 뒤 사유를 적고 저장합니다. 선택만으로는 변경되지 않습니다."}
        div{class:"membership-actions",for action in [Action::Locked(!data.locked),Action::BrandColor(None),Action::Featured(!data.featured),Action::DisplayOrder(data.display_order)]{
            button{class:"secondary-button",disabled:pending()||!ready,onclick:{let item=data.clone();let action=action.clone();move|_|{selected.set(Some(action.clone()));color.set(if item.color_truncated{String::new()}else{item.brand_color.clone().unwrap_or_default()});order.set(item.display_order.to_string());note.set(String::new());notice.set(String::new());remote.set(None);}},"{action.label()}"}
        }}
        if let Some(action)=selected(){form{class:"membership-form moderation-confirm",onsubmit:move|e|{
            e.prevent_default();if pending()||!ready{return;}let mut action=selected().unwrap();
            if matches!(action,Action::BrandColor(_)){action=Action::BrandColor(Some(color()));}
            if matches!(action,Action::DisplayOrder(_)){let Ok(value)=order().parse::<i32>() else{notice.set("표시 순서는 정수로 입력해 주세요.".into());return;};action=Action::DisplayOrder(value);}
            pending.set(true);let data=current();spawn(async move{let result=api::act(Request{name:data.name,revision:data.revision,action,note:note()}).await;pending.set(false);match result{Ok(saved)=>{current.set(saved);selected.set(None);note.set(String::new());remote.set(None);generation+=1;notice.set("저장했습니다.".into());},Err(e)=>notice.set(message(&e))}});
        },h3{"{action.label()}"}p{"선택한 설정만 바꿉니다. 소개·분류·로고는 유지됩니다."}
            if matches!(action,Action::Locked(_)){p{"잠금 중에는 회원의 수정·복원이 막힙니다. 관리자는 소개 수정 화면을 계속 사용할 수 있습니다."}}
            if matches!(action,Action::BrandColor(_)){div{class:"membership-field",label{r#for:"catalog-color","색상 · 비우면 해제"}input{id:"catalog-color",value:color,placeholder:"#123ABC",maxlength:7,disabled:pending(),oninput:move|e|color.set(e.value())}}}
            if matches!(action,Action::DisplayOrder(_)){div{class:"membership-field",label{r#for:"catalog-order","표시 순서"}input{id:"catalog-order",r#type:"number",step:"1",min:"-2147483648",max:"2147483647",value:order,required:true,disabled:pending(),oninput:move|e|order.set(e.value())}}}
            Reason{note,pending}div{class:"membership-actions",button{class:"primary-button",disabled:pending()||!ready||note().trim().is_empty(),"확인하고 저장"}button{r#type:"button",class:"secondary-button",disabled:pending(),onclick:move|_|selected.set(None),"취소"}}
        }}
        if !notice().is_empty(){p{role:"status",class:"membership-notice","{notice}"}}
        button{class:"secondary-button",disabled:pending()||!ready,onclick:move|_|{pending.set(true);let name=current().name;spawn(async move{match api::item(name).await{Ok(data)=>remote.set(Some(data)),Err(e)=>notice.set(message(&e))}pending.set(false);});},"최신 설정 확인"}
        if let Some(latest)=remote(){div{class:"moderation-confirm",p{"최신 버전 {latest.revision} · 편집 잠금 " if latest.locked{"켜짐"}else{"꺼짐"}}p{"색상 " {latest.brand_color.as_deref().unwrap_or("미설정")} " · 순서 {latest.display_order} · 대표 표식 " if latest.featured{"켜짐"}else{"꺼짐"}}
            button{class:"secondary-button",disabled:pending()||!ready,onclick:move|_|{current.set(latest.clone());remote.set(None);generation+=1;notice.set("최신 설정을 기준으로 다시 확인해 주세요. 작성한 사유는 유지했습니다.".into());},"최신 설정을 확인하고 계속"}
        }}
    }logo::LogoEditor{current,pending,generation}AdminHistory{name:data.name,category:false,generation:generation()}}
}
#[component]
pub fn ModerationCategories() -> Element {
    let ready = use_context::<Signal<bool>>()();
    let mut draft = use_signal(String::new);
    let mut query = use_signal(String::new);
    let mut page = use_signal(|| 0u32);
    let data = use_server_future(move || api::categories(query(), page()))?;
    rsx! {document::Title{"종류 관리 — fediverse.kr"}CatalogPage{title:"소프트웨어 종류",description:"소프트웨어를 찾는 분류입니다. 식별자는 유지하고 표시 내용을 고칩니다.",categories:true,
        p{Link{class:"secondary-button",to:Route::ModerationCategoryNew{},"종류 추가"}}
        form{class:"membership-card membership-form",onsubmit:move|e|{e.prevent_default();query.set(draft());page.set(0);},div{class:"membership-field",label{r#for:"category-search","종류 검색"}input{id:"category-search",value:draft,maxlength:128,disabled:!ready,oninput:move|e|draft.set(e.value())}}button{class:"primary-button",disabled:!ready,"검색"}}
        match data(){Some(Ok(data))=>rsx!{p{class:"directory-results",role:"status","이 페이지에 {data.items.len()}개" if !query().is_empty(){" · 검색: {query}"}}if data.items.is_empty(){p{class:"membership-notice","조건에 맞는 종류가 없습니다."}}ul{class:"moderation-list",for item in data.items{li{key:"{item.name}",class:"membership-card",h2{Link{to:Route::ModerationCategory{name:item.name.clone()},"{item.edit.emoji} {item.edit.label}"}}p{"{item.name} · 순서 {item.edit.display_order}"}}}}Pager{page,has_next:data.has_next}},Some(Err(e))=>failure(e),None=>rsx!{p{role:"status","불러오는 중…"}}}
    }}
}
#[component]
pub fn ModerationCategory(name: String) -> Element {
    rsx! {CategoryLoader{key:"{name}",name}}
}
#[component]
pub fn ModerationCategoryNew() -> Element {
    rsx! {CategoryLoader{name:String::new()}}
}
#[component]
fn CategoryLoader(name: String) -> Element {
    let data = use_server_future(move || {
        let name = name.clone();
        async move {
            if name.is_empty() {
                if crate::moderation::api::access().await? {
                    Ok(None)
                } else {
                    Err(ServerFnError::ServerError {
                        code: 403,
                        message: "관리자만 사용할 수 있어요.".into(),
                        details: None,
                    })
                }
            } else {
                api::category(name).await.map(Some)
            }
        }
    })?;
    rsx! {document::Title{"종류 편집 — fediverse.kr"}CatalogPage{title:"종류 편집",description:"분류의 이름과 표시 순서를 저장합니다. 기존 소프트웨어와의 연결은 유지됩니다.",categories:true,
        Link{class:"text-link backoffice-back-link",to:Route::ModerationCategories{},"분류 목록으로"}
        match data(){Some(Ok(initial))=>rsx!{CategoryForm{initial}},Some(Err(e))=>failure(e),None=>rsx!{p{role:"status","불러오는 중…"}}}
    }}
}
#[component]
fn CategoryForm(initial: Option<Category>) -> Element {
    let ready = use_context::<Signal<bool>>()();
    let nav = use_navigator();
    let mut current = use_signal(|| initial.clone());
    let mut name = use_signal(String::new);
    let mut edit = use_signal(|| initial.map(|c| c.edit).unwrap_or_default());
    let mut order = use_signal(|| edit().display_order.to_string());
    let mut note = use_signal(String::new);
    let mut pending = use_signal(|| false);
    let mut notice = use_signal(String::new);
    let mut remote = use_signal(|| None::<Category>);
    let mut generation = use_signal(|| 0u32);
    rsx! {form{class:"membership-card membership-form",onsubmit:move|e|{
        e.prevent_default();if !ready||pending(){return;}let Ok(order)=order().parse::<i32>()else{notice.set("표시 순서는 정수로 입력해 주세요.".into());return;};let mut value=edit();value.display_order=order;
        let data=current();let request=CategoryRequest{name:data.as_ref().map(|c|c.name.clone()).unwrap_or_else(||name()),revision:data.map(|c|c.revision),edit:value,note:note()};pending.set(true);
        let creating=request.revision.is_none();
        spawn(async move{let result=api::category_save(request).await;pending.set(false);match result{Ok(saved)=>{if creating{nav.replace(Route::ModerationCategory{name:saved.name.clone()});}edit.set(saved.edit.clone());current.set(Some(saved));note.set(String::new());remote.set(None);generation+=1;notice.set("저장했습니다.".into());},Err(e)=>notice.set(message(&e))}});
    },
        div{class:"membership-field",label{r#for:"category-name","식별자"}if let Some(data)=current(){input{id:"category-name",value:data.name,readonly:true}}else{input{id:"category-name",value:name,required:true,maxlength:64,pattern:r"[a-zA-Z0-9][a-zA-Z0-9_\-]*",disabled:pending()||!ready,oninput:move|e|name.set(e.value())}}p{class:"membership-hint","영문·숫자·하이픈·밑줄. 등록 후에는 변경하지 않습니다."}}
        div{class:"membership-field",label{r#for:"category-label","표시 이름"}input{id:"category-label",value:edit().label,required:true,maxlength:200,disabled:pending()||!ready,oninput:move|e|edit.write().label=e.value()}}
        div{class:"membership-field",label{r#for:"category-emoji","아이콘 · 문자 또는 이모지, 선택"}input{id:"category-emoji",value:edit().emoji,maxlength:32,disabled:pending()||!ready,oninput:move|e|edit.write().emoji=e.value()}}
        div{class:"membership-field",label{r#for:"category-order","표시 순서 · 작은 수부터"}input{id:"category-order",r#type:"number",step:"1",min:"-2147483648",max:"2147483647",value:order,required:true,disabled:pending()||!ready,oninput:move|e|order.set(e.value())}}
        p{class:"membership-hint","표시 내용은 즉시 공개됩니다. 기존 제품의 분류 연결은 유지합니다."}Reason{note,pending}
        button{class:"primary-button",disabled:pending()||!ready||note().trim().is_empty(),"종류 저장"}
        if !notice().is_empty(){p{class:"membership-notice",role:"status","{notice}"}}
        if current().is_some(){button{class:"secondary-button",r#type:"button",disabled:pending()||!ready,onclick:move|_|{let name=current().unwrap().name;pending.set(true);spawn(async move{match api::category(name).await{Ok(data)=>remote.set(Some(data)),Err(e)=>notice.set(message(&e))}pending.set(false);});},"최신 내용과 비교"}}
        if let Some(latest)=remote(){section{class:"moderation-confirm",h2{"최신 버전 {latest.revision}"}p{"{latest.edit.label} · {latest.edit.emoji} · 순서 {latest.edit.display_order}"}p{"입력한 값은 그대로 둡니다. 최신 내용과 비교한 뒤 저장해 주세요."}button{r#type:"button",class:"secondary-button",disabled:pending()||!ready,onclick:move|_|{current.set(Some(latest.clone()));remote.set(None);generation+=1;notice.set("최신 내용과 비교했습니다. 현재 입력으로 저장할 수 있어요.".into());},"비교했으며 현재 입력으로 계속"}}}
    }if let Some(data)=current(){AdminHistory{name:data.name,category:true,generation:generation()}}}
}
#[component]
fn Reason(mut note: Signal<String>, pending: Signal<bool>) -> Element {
    rsx! {div{class:"membership-field",label{r#for:"catalog-note","관리 사유 · 관리자에게만 공개"}textarea{id:"catalog-note",rows:3,maxlength:1000,required:true,value:note,disabled:pending(),oninput:move|e|note.set(e.value())}p{class:"membership-hint","저장에는 최근 15분 이내 인증이 필요합니다. 필요한 사유만 남겨 주세요."}}}
}
#[component]
fn Pager(mut page: Signal<u32>, has_next: bool) -> Element {
    let ready = use_context::<Signal<bool>>()();
    rsx! {nav{class:"membership-actions moderation-pagination",aria_label:"목록 페이지",button{class:"secondary-button",disabled:!ready||page()==0,onclick:move|_|page.set(page().saturating_sub(1)),"이전"}span{"{page()+1}쪽"}button{class:"secondary-button",disabled:!ready||!has_next,onclick:move|_|page+=1,"다음"}}}
}
#[component]
fn AdminHistory(name: String, category: bool, generation: u32) -> Element {
    let mut page = use_signal(|| 0u32);
    let data = use_server_future(use_reactive(
        (&name, &category, &generation),
        move |(name, category, _)| api::history(name, category, page()),
    ))?;
    use_effect(use_reactive((&generation,), move |_| page.set(0)));
    rsx! {section{class:"membership-card",h2{"비공개 관리 이력"}p{class:"membership-hint","소개 본문 수정은 공개 변경 이력에 따로 남습니다."}
        match data(){Some(Ok(data))=>rsx!{if data.items.is_empty(){p{"아직 관리 이력이 없습니다."}}ol{class:"moderation-events",for item in data.items{li{strong{"버전 {item.revision} · {action_label(&item.action)}"}p{class:"membership-hint","{timestamp(&item.created_at)}"}p{class:"moderation-excerpt","{display_value(&item.before)} → {display_value(&item.after)}"}if item.truncated{p{"긴 값은 일부만 표시합니다. 원본은 이력에 보존됩니다."}}p{class:"moderation-body","{item.note}"}}}}Pager{page,has_next:data.has_next}},Some(Err(e))=>failure(e),None=>rsx!{p{role:"status","이력을 불러오는 중…"}}}
    }}
}
fn action_label(s: &str) -> &str {
    match s {
        "locked" => "편집 잠금",
        "brand_color" => "브랜드 색상",
        "featured" => "대표 제품 표식",
        "display_order" => "표시 순서",
        "category_create" => "종류 추가",
        "category_edit" => "종류 수정",
        "logo" => "제품 로고",
        _ => "관리 설정",
    }
}
fn display_value(text: &str) -> String {
    use serde_json::Value;
    match serde_json::from_str::<Value>(text) {
        Ok(Value::Null) => "없음".into(),
        Ok(Value::Bool(value)) => if value { "켜짐" } else { "꺼짐" }.into(),
        Ok(Value::String(value)) => value,
        Ok(Value::Number(value)) => value.to_string(),
        Ok(Value::Object(value)) if value.contains_key("present") => {
            if value.get("present").and_then(Value::as_bool) != Some(true) {
                "로고 없음".into()
            } else if let (Some(mime), Some(bytes)) = (
                value.get("mime").and_then(Value::as_str),
                value.get("bytes").and_then(Value::as_i64),
            ) {
                format!("로고 있음 · {mime} · {bytes}바이트")
            } else {
                "로고 있음".into()
            }
        }
        Ok(Value::Object(value)) => format!(
            "{} {} · 순서 {}",
            value.get("emoji").and_then(Value::as_str).unwrap_or(""),
            value
                .get("label")
                .and_then(Value::as_str)
                .unwrap_or("이름 미확인"),
            value
                .get("display_order")
                .and_then(Value::as_i64)
                .map(|v| v.to_string())
                .unwrap_or_else(|| "미확인".into())
        ),
        _ => "긴 원본 값 · 요약 생략".into(),
    }
}
