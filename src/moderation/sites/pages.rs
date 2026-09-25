use super::{api, Action, DeleteRequest, Request, Site};
use crate::{
    moderation::{
        layout::{BackofficePage, BackofficeSection},
        pages::{message, timestamp},
    },
    Route,
};
use dioxus::prelude::*;
fn flag(on: bool) -> &'static str {
    if on {
        "켜짐"
    } else {
        "꺼짐"
    }
}
fn invite(value: Option<bool>) -> &'static str {
    match value {
        Some(true) => "초대제",
        Some(false) => "초대제 아님",
        None => "미확인",
    }
}
fn filter_name(value: &str) -> &str {
    match value {
        "all" => "전체",
        "visible" => "목록에 공개",
        "hidden" => "일반 숨김",
        "force_hidden" => "강제 숨김",
        "closed" => "운영 종료",
        _ => value,
    }
}

fn count_label(value: i64) -> String {
    format!("{value}개")
}

#[component]
pub fn ModerationSites() -> Element {
    let ready = use_context::<Signal<bool>>()();
    let mut draft = use_signal(String::new);
    let mut query = use_signal(String::new);
    let mut status = use_signal(|| "all".to_owned());
    let mut sort = use_signal(|| "domain".to_owned());
    let mut page = use_signal(|| 0u32);
    let mut fetched = use_server_future(move || api::sites(query(), status(), sort(), page()))?;
    rsx! {
        document::Title{"서버 관리 — fediverse.kr"}
        document::Stylesheet{href:asset!("/assets/styling/backoffice-catalog.css")}
        BackofficePage{section:BackofficeSection::Sites,title:"서버 관리",description:"서버를 찾아 현재 상태를 확인한 뒤 필요한 항목만 변경합니다.",
          div{class:"directory-workflow",
            form{class:"membership-card membership-form",onsubmit:move|e|{e.prevent_default();if ready{query.set(draft());page.set(0);}},
                div{class:"membership-field",label{r#for:"admin-site-search","도메인·이름 검색"}input{id:"admin-site-search",maxlength:128,value:draft(),disabled:!ready,oninput:move|e|draft.set(e.value())}}
                div{class:"membership-actions",
                    div{class:"membership-field",label{r#for:"admin-site-filter","표시 상태"}select{id:"admin-site-filter",value:status(),disabled:!ready,onchange:move|e|{status.set(e.value());page.set(0);},for key in ["all","visible","hidden","force_hidden","closed"]{option{value:key,selected:status()==key,"{filter_name(key)}"}}}}
                    div{class:"membership-field",label{r#for:"admin-site-sort","정렬"}select{id:"admin-site-sort",value:sort(),disabled:!ready,onchange:move|e|{sort.set(e.value());page.set(0);},option{value:"domain",selected:sort()=="domain","도메인순"}option{value:"recent",selected:sort()=="recent","최근 등록순"}option{value:"users",selected:sort()=="users","회원 수 많은 순"}}}
                    button{class:"primary-button",r#type:"submit",disabled:!ready,"검색"}
                }
            }
            match fetched(){
                Some(Ok(data))=>rsx!{
                    p{class:"directory-results",role:"status","이 페이지에 {data.sites.len()}개 · {filter_name(&status())}" if !query().is_empty(){" · 검색: {query}"}}
                    if data.sites.is_empty(){p{class:"membership-notice","조건에 맞는 서버가 없습니다."}}
                    ul{class:"moderation-list",for site in data.sites{li{key:"{site.id}",class:"membership-card",
                        h2{Link{to:Route::ModerationSite{id:site.id.clone()},"{site.domain}"}}
                        p{"{site.name}"}
                        p{class:"directory-status",if site.closed{"운영 종료 · "}if site.force_hidden{"강제 숨김"}else if site.hidden{"일반 숨김"}else{"목록에 공개"}}
                        p{class:"membership-hint",{site.software.as_deref().unwrap_or("소프트웨어 미확인")} " · " {site.users.map(|n|format!("{n}명")).unwrap_or_else(||"회원 수 미확인".into())}}
                        Link{class:"text-link",to:Route::ModerationSite{id:site.id},"상태와 조치 보기"}
                    }}}
                    div{class:"membership-actions",button{class:"secondary-button",disabled:!ready||page()==0,onclick:move|_|page.set(page()-1),"이전"}span{"{page()+1}쪽"}button{class:"secondary-button",disabled:!ready||!data.has_next,onclick:move|_|page.set(page()+1),"다음"}}
                },
                Some(Err(e))=>rsx!{p{class:"membership-message membership-error",role:"alert","{message(&e)}"}},
                None=>rsx!{p{role:"status","서버를 불러오고 있어요."}},
            }
            button{class:"secondary-button",disabled:!ready,onclick:move|_|fetched.restart(),"목록 새로고침"}
          }
        }
    }
}
#[component]
pub fn ModerationSite(id: String) -> Element {
    rsx! {Loader{key:"{id}",id}}
}
#[component]
fn Loader(id: String) -> Element {
    let ready = use_context::<Signal<bool>>()();
    let mut fetched = use_server_future(move || api::site(id.clone()))?;
    rsx! {document::Title{"서버 관리 — fediverse.kr"}
        document::Stylesheet{href:asset!("/assets/styling/backoffice-catalog.css")}
        BackofficePage{section:BackofficeSection::Sites,title:"서버 상태와 조치",description:"현재 상태, 변경 작업, 이력과 삭제를 구분해 확인합니다.",
          div{class:"directory-workflow",
            Link{class:"backoffice-back-link text-link",to:Route::ModerationSites{},"서버 목록으로"}
            match fetched(){
                Some(Ok(data))=>rsx!{Editor{initial:data}},
                Some(Err(e))=>rsx!{p{class:"membership-message membership-error",role:"alert","{message(&e)}"}button{class:"secondary-button",disabled:!ready,onclick:move|_|fetched.restart(),"다시 불러오기"}},
                None=>rsx!{p{role:"status","서버 정보를 확인하고 있어요."}},
            }
          }
        }
    }
}
#[component]
fn Editor(initial: Site) -> Element {
    let ready = use_context::<Signal<bool>>()();
    let mut current = use_signal(|| initial);
    let mut selected = use_signal(|| None::<Action>);
    let mut tags = use_signal(String::new);
    let mut note = use_signal(String::new);
    let mut pending = use_signal(|| false);
    let mut error = use_signal(String::new);
    let mut notice = use_signal(String::new);
    let mut generation = use_signal(|| 0u32);
    let mut deleted = use_signal(|| false);
    let data = current();
    let mut actions = vec![
        Action::Hidden(!data.hidden),
        Action::ForceHidden(!data.force_hidden),
        Action::Closed(!data.closed),
        Action::Tags(String::new()),
        Action::Refresh,
    ];
    for v in [Some(true), Some(false), None] {
        if v != data.invite_only {
            actions.push(Action::InviteOnly(v));
        }
    }
    rsx! {
        if deleted() {
            section{class:"membership-card membership-danger",role:"status",
                h2{"서버가 삭제되었습니다"}
                p{"fediverse.kr의 등록 정보와 연결된 댓글·답글·수집 상태를 삭제했습니다. 외부 서버 자체는 변경되지 않습니다."}
                Link{class:"text-link",to:Route::ModerationSites{},"서버 관리 목록으로 돌아가기"}
            }
        } else {
        section{class:"membership-card directory-current",p{class:"directory-kicker","현재 상태"}h2{"{data.domain}"}p{"{data.name}"}
            dl{class:"admin-site-facts",
                dt{"일반 숨김"}dd{"{flag(data.hidden)}"}
                dt{"강제 숨김"}dd{"{flag(data.force_hidden)}"}
                dt{"운영 종료"}dd{"{flag(data.closed)}"}
                dt{"초대제"}dd{"{invite(data.invite_only)}"}
                dt{"운영자"}dd{{data.owner_name.as_deref().unwrap_or("미지정")}}
                dt{"태그"}dd{{if data.tags.is_empty(){"없음"}else{&data.tags}}}
            }
            if data.tags_truncated{p{class:"membership-hint","긴 기존 태그의 앞 600자만 표시합니다. 원본은 보존되어 있습니다."}}
            if let Some(refresh)=data.refresh{p{class:"membership-notice","최근 수동 수집: {refresh.state.label()} · {timestamp(&refresh.requested_at)}"}}
            if !data.hidden&&!data.force_hidden{Link{class:"text-link",to:Route::ServerDetail{slug:data.domain.clone()},"공개 서버 안내 보기"}}
        }
        section{class:"membership-card directory-actions",h2{"관리 조치"}
            p{class:"membership-hint","한 번에 한 항목만 변경합니다. 사유는 관리자에게만 보이며, 저장하려면 최근 15분 안에 인증해야 합니다."}
            div{class:"membership-actions",for action in actions{
                button{class:"secondary-button",disabled:!ready||pending(),onclick:{let action=action.clone();move|_|{if matches!(action,Action::Tags(_)){tags.set(current().tags);}selected.set(Some(action.clone()));error.set(String::new());notice.set(String::new());}},"{action.label()}"}
            }}
            if let Some(action)=selected(){
                form{class:"membership-form moderation-confirm",onsubmit:move|e|{
                    e.prevent_default();if !ready||pending(){return;}
                    let Some(mut action)=selected()else{return;};
                    if note().trim().is_empty(){error.set("조치 사유를 입력해 주세요.".into());return;}
                    if matches!(action,Action::Tags(_)){action=Action::Tags(tags());}
                    let data=current();let request=Request{id:data.id,revision:data.revision,action,note:note()};
                    pending.set(true);error.set(String::new());notice.set(String::new());
                    spawn(async move{match api::act(request).await{Ok(data)=>{current.set(data);selected.set(None);note.set(String::new());notice.set("요청을 반영했습니다.".into());generation.set(generation()+1);},Err(e)=>error.set(message(&e))}pending.set(false);});
                },
                    h3{"{action.label()} 확인"}p{"{action.explanation()}"}
                    if matches!(action,Action::Tags(_)){
                        div{class:"membership-field",label{r#for:"admin-site-tags","교체할 태그"}textarea{id:"admin-site-tags",maxlength:256,rows:3,disabled:pending(),value:tags(),oninput:move|e|tags.set(e.value())}}
                        if current().tags_truncated{p{class:"membership-message membership-error","현재 원문은 표시보다 깁니다. 저장하면 태그 전체를 입력한 내용으로 교체합니다."}}
                    }
                    div{class:"membership-field",label{r#for:"admin-site-note","조치 사유"}textarea{id:"admin-site-note",required:true,maxlength:1000,rows:4,disabled:pending(),value:note(),oninput:move|e|note.set(e.value())}}
                    div{class:"membership-actions",button{class:"primary-button",r#type:"submit",disabled:!ready||pending(),if pending(){"저장 중…"}else{"확인하고 적용"}}button{class:"secondary-button",r#type:"button",disabled:pending(),onclick:move|_|selected.set(None),"취소"}}
                }
            }
            if !error().is_empty(){p{class:"membership-message membership-error",role:"alert","{error}"}}
            if !notice().is_empty(){p{class:"membership-message membership-success",role:"status","{notice}"}}
            div{class:"membership-actions",button{class:"secondary-button",disabled:!ready||pending(),onclick:move|_|{
                if !ready||pending(){return;}pending.set(true);let id=current().id;
                spawn(async move{match api::site(id).await{Ok(data)=>{current.set(data);selected.set(None);error.set(String::new());notice.set("최신 내용을 불러왔습니다. 사유 초안은 유지합니다. 조치를 다시 선택해 주세요.".into());generation.set(generation()+1);},Err(e)=>error.set(message(&e))}pending.set(false);});
            },"최신 상태 확인"}Link{class:"text-link",to:Route::Account{},"내 계정에서 재인증"}}
        }
        {rsx!{History{key:"{generation}",id:current().id}}}
        DeletePanel{site:current,deleted,pending}
        }
    }
}

#[component]
fn DeletePanel(
    site: Signal<Site>,
    mut deleted: Signal<bool>,
    mut pending: Signal<bool>,
) -> Element {
    let ready = use_context::<Signal<bool>>()();
    let mut open = use_signal(|| false);
    let mut domain = use_signal(String::new);
    let mut note = use_signal(String::new);
    let mut confirmed = use_signal(|| false);
    let mut error = use_signal(String::new);
    let mut confirmed_revision = use_signal(|| site().revision);
    let mut impact = use_server_future(move || {
        let is_open = open();
        let current = site();
        async move {
            if !is_open {
                Ok(None)
            } else {
                api::delete_impact(current.id).await.map(Some)
            }
        }
    })?;
    use_effect(move || {
        let revision = site().revision;
        if confirmed_revision() != revision {
            confirmed_revision.set(revision);
            confirmed.set(false);
            if open() {
                impact.restart();
            }
        }
    });
    rsx! {section{class:"membership-card membership-danger directory-danger",p{class:"directory-kicker","위험 구역 · 복구 불가"}h2{"서버 삭제"}
        p{class:"membership-hint","fediverse.kr의 등록 항목만 삭제합니다. 외부 서버 자체는 변경하지 않습니다. 등록 정보·댓글·답글·수집 상태는 삭제되며, 신고와 증거 자료는 보존됩니다. 이 화면에서 복구할 수 없으므로 숨김 또는 운영 종료가 필요하면 위의 기존 조치를 사용하세요."}
        if !open(){button{class:"secondary-button",disabled:!ready||pending(),onclick:move|_|{if ready&&!pending(){open.set(true);confirmed.set(false);error.set(String::new());}},"서버 삭제"}}
        else {match impact(){
            Some(Ok(Some(value)))=>rsx!{
                p{class:"membership-notice","삭제 대상: {value.site.domain}"}
                dl{class:"admin-site-facts",dt{"서버 등록 정보"}dd{"1개"}dt{"댓글·답글"}dd{"{count_label(value.comments)}"}dt{"신고"}dd{"{count_label(value.reports)} · 보존"}dt{"수집 상태"}dd{"{count_label(value.health_checks)}"}}
                p {
                    "계속하려면 도메인 "
                    strong { {value.site.domain.clone()} }
                    "을(를) 입력하세요."
                }
                form{class:"membership-form moderation-confirm",onsubmit:move|e|{e.prevent_default();if !ready||pending(){return;}let value=value.clone();let typed=domain().trim().to_owned();let reason=note().trim().to_owned();if typed!=value.site.domain{error.set("도메인을 정확히 입력해 주세요.".into());return;}if reason.is_empty(){error.set("삭제 사유를 입력해 주세요.".into());return;}if reason.chars().count()>1000{error.set("삭제 사유는 1000자 이내로 입력해 주세요.".into());return;}if !confirmed(){error.set("삭제 내용을 확인했다는 체크가 필요합니다.".into());return;}let request=DeleteRequest{id:value.site.id,revision:value.site.revision,domain:typed,note:reason,confirmed:true};pending.set(true);error.set(String::new());spawn(async move{match api::delete_site(request).await{Ok(())=>{deleted.set(true);open.set(false);confirmed.set(false);},Err(e)=>error.set(message(&e))}pending.set(false);});},
                    div{class:"membership-field",label{r#for:"admin-site-delete-domain","도메인 확인"}input{id:"admin-site-delete-domain",maxlength:256,required:true,disabled:pending(),value:domain(),oninput:move|e|domain.set(e.value())}}
                    div{class:"membership-field",label{r#for:"admin-site-delete-note","삭제 사유"}textarea{id:"admin-site-delete-note",maxlength:1000,required:true,rows:4,disabled:pending(),value:note(),oninput:move|e|note.set(e.value())}}
                    label{class:"owner-checkbox",
                        input{r#type:"checkbox",disabled:pending(),checked:confirmed(),onchange:move|e|confirmed.set(e.checked())}
                        "삭제 대상과 복구할 수 없음을 확인했습니다."
                    }
                    div{class:"membership-actions",button{class:"primary-button",r#type:"submit",disabled:!ready||pending(),if pending(){"삭제 중…"}else{"확인하고 서버 삭제"}}button{class:"secondary-button",r#type:"button",disabled:pending(),onclick:move|_|{open.set(false);confirmed.set(false);error.set(String::new());},"취소"}}
                }
                button{class:"secondary-button",disabled:!ready||pending(),onclick:move|_|{confirmed.set(false);impact.restart();},"영향도 다시 확인"}
            },
            Some(Ok(None))=>rsx!{p{role:"status","삭제 영향을 계산하고 있어요."}},
                    Some(Err(e))=>rsx!{p{class:"membership-message membership-error",role:"alert",{message(&e)}}button{class:"secondary-button",disabled:!ready||pending(),onclick:move|_|{confirmed.set(false);impact.restart();},"영향도 다시 불러오기"}},
            None=>rsx!{p{role:"status","삭제 영향을 확인하고 있어요."}},
        }}
        if !error().is_empty(){p{class:"membership-message membership-error",role:"alert",{error()}}}
    }}
}

#[component]
fn History(id: String) -> Element {
    let ready = use_context::<Signal<bool>>()();
    let mut page = use_signal(|| 0u32);
    let mut fetched = use_server_future(move || api::history(id.clone(), page()))?;
    rsx! {section{class:"membership-card",h2{"관리자 변경 이력"}p{class:"membership-hint","새 버전의 관리자 조치만 표시합니다. 운영자의 편집과 구버전 변경 전체를 보여주는 이력은 아닙니다."}
        match fetched(){
            Some(Ok(data))=>rsx!{
                if data.events.is_empty(){p{class:"membership-notice","관리자 조치 이력이 없습니다."}}
                ol{class:"moderation-events",for e in data.events{li{key:"{e.id}",strong{"{e.change.field}"}p{class:"membership-hint",{e.actor_name.as_deref().unwrap_or("탈퇴한 관리자")} " · {timestamp(&e.created_at)} · 버전 {e.revision}"}p{class:"moderation-body","{e.change.before} → {e.change.after}"}if e.change.truncated{p{class:"membership-hint","긴 값은 앞 600자만 표시합니다. 원본 기록은 보존됩니다."}}p{class:"moderation-body","{e.note}"}}}}
                div{class:"membership-actions",button{class:"secondary-button",disabled:!ready||page()==0,onclick:move|_|page.set(page()-1),"이전 이력"}span{"{page()+1}쪽"}button{class:"secondary-button",disabled:!ready||!data.has_next,onclick:move|_|page.set(page()+1),"다음 이력"}}
            },
            Some(Err(e))=>rsx!{p{class:"membership-message membership-error","{message(&e)}"}button{class:"secondary-button",disabled:!ready,onclick:move|_|fetched.restart(),"이력 다시 불러오기"}},
            None=>rsx!{p{role:"status","이력을 불러오고 있어요."}},
        }
    }}
}
