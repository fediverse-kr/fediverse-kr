use super::{api, DnsChallenge, OwnedSite, SiteEdit};
use crate::{
    membership::{api as member_api, pages::error_message},
    Route,
};
use dioxus::prelude::*;
#[path = "introduction.rs"]
mod introduction;

#[component]
pub fn ManagedSites() -> Element {
    let account = use_server_future(member_api::session)?;
    rsx! {
        document::Title { "내 서버 관리 — fediverse.kr" }
        main {id:"content",class:"membership-page wrap",
            Link {class:"back-link",to:Route::Account{},"내 계정"}
            introduction::ManagementIntroduction {}
            p{class:"membership-hint","아직 목록에 없다면 " Link{to:Route::RegisterSite{},"서버 등록"} "부터 할 수 있어요."}
            match account() {
                Some(Ok(state)) if state.member.is_some()=>rsx!{OwnerPanel{accounts:state.linked_accounts}},
                Some(Ok(state))=>rsx!{section {class:"membership-card",h2 {"먼저 로그인해 주세요."} p {class:"membership-hint","서버 인증은 로그인한 계정에 연결됩니다."} Link {class:"primary-button",to:Route::Login{},"로그인"} if !state.federation_available {p {class:"membership-hint","현재는 DB가 연결되지 않은 검토용 화면입니다."}}}},
                Some(Err(e))=>rsx!{p {role:"alert",{error_message(e)}}},
                None=>rsx!{p {role:"status","로그인 확인 중…"}},
            }
        }
    }
}
#[component]
fn OwnerPanel(accounts: Vec<crate::membership::LinkedAccount>) -> Element {
    let mut page = use_signal(|| 0u32);
    let mut owned = use_server_future(move || api::owned(page()))?;
    let ready = use_context::<Signal<bool>>()();
    rsx! {
        div {class:"owner-sections",
            section {class:"membership-card",h2 {"내가 관리하는 서버"}
                match owned() {
                    Some(Ok(data))=>rsx!{
                        if data.sites.is_empty() {p {class:"membership-hint","이 페이지에 관리하는 서버가 없어요. 아래에서 운영자 인증을 시작할 수 있습니다."}}
                        for site in data.sites {OwnerEditor {key:"{site.domain}-{site.revision}",site,on_saved:move |_|owned.restart()}}
                        nav {class:"directory-pagination",aria_label:"내 서버 목록 페이지",
                            button {class:"secondary-button",disabled:!ready || page()==0,onclick:move |_|page.set(page().saturating_sub(1)),"이전"}
                            span {"{data.page+1}페이지"}
                            button {class:"secondary-button",disabled:!ready || !data.has_next,onclick:move |_|page.set(page()+1),"다음"}
                        }
                    },
                    Some(Err(e))=>rsx!{p {role:"alert",{error_message(e)}} button {class:"secondary-button",disabled:!ready,onclick:move |_|owned.restart(),"다시 불러오기"}},
                    None=>rsx!{p {role:"status","불러오는 중…"}},
                }
            }
            section {class:"membership-card",h2 {"연동 계정으로 운영자 인증"} ApiForm {accounts,on_verified:move |_|{page.set(0);owned.restart();}}}
            section {class:"membership-card",h2 {"DNS로 운영자 인증"} DnsForm {on_verified:move |_|{page.set(0);owned.restart();}}}
        }
    }
}
#[component]
fn ApiForm(
    accounts: Vec<crate::membership::LinkedAccount>,
    on_verified: EventHandler<()>,
) -> Element {
    let initial = accounts.first().map(|a| a.id.clone()).unwrap_or_default();
    let mut selected = use_signal(move || initial);
    let mut domain = use_signal(String::new);
    let mut pending = use_signal(|| false);
    let mut message = use_signal(String::new);
    let ready = use_context::<Signal<bool>>()();
    rsx! {
        p {class:"membership-hint","마스토돈 계열은 서버 연락 계정, 미스키 계열은 관리자 계정인지 확인해요. 이미 연동한 계정의 서버만 인증할 수 있습니다."}
        if accounts.is_empty() {
            p {class:"membership-hint","먼저 운영자 계정을 내 계정에 연동해 주세요."}
            Link {class:"secondary-button",to:Route::Account{},"계정 연동하기"}
        } else {
            form {class:"membership-form",onsubmit:move|e|{
                e.prevent_default();if pending() || !ready{return;} pending.set(true);message.set(String::new());
                spawn(async move {match api::verify_operator(selected(),domain()).await {Ok(domain)=>{message.set(format!("{domain} 운영자 인증이 완료됐어요."));on_verified.call(());},Err(e)=>message.set(error_message(e))}pending.set(false);});
            },
                div {class:"membership-field",label {r#for:"owner-account","연동한 운영자 계정"}
                    select {id:"owner-account",value:selected,disabled:pending() || !ready,onchange:move|e|selected.set(e.value()),
                        for account in accounts {option {value:account.id,"{account.handle}"}}
                    }
                }
                div {class:"membership-field",label {r#for:"owner-api-domain","운영하는 서버 도메인"}
                    input {id:"owner-api-domain",required:true,maxlength:253,placeholder:"social.example.org",value:domain,disabled:pending() || !ready,oninput:move|e|domain.set(e.value())}
                }
                button {class:"primary-button",r#type:"submit",disabled:pending() || !ready,if pending(){"운영자 확인 중…"}else{"운영자 확인"}}
            }
        }
        if !message().is_empty(){p {class:"membership-notice",role:"status","{message}"}}
    }
}
#[component]
fn DnsForm(on_verified: EventHandler<()>) -> Element {
    let mut domain = use_signal(String::new);
    let mut pending = use_signal(|| false);
    let mut challenge = use_signal(|| None::<DnsChallenge>);
    let mut message = use_signal(String::new);
    let ready = use_context::<Signal<bool>>()();
    rsx! {
        p {class:"membership-hint","등록된 서버의 DNS를 수정할 수 있다면 직접 인증할 수 있어요. 기존 운영자 인증이 있어도 DNS 제어권이 우선합니다."}
        if let Some(c)=challenge() {
            div {class:"membership-form",
                p {"아래 TXT 레코드를 추가하고 확인해 주세요."}
                label {r#for:"owner-dns-name","레코드 이름"}
                input {id:"owner-dns-name",readonly:true,value:c.record}
                label {r#for:"owner-dns-value","TXT 값"}
                textarea {id:"owner-dns-value",readonly:true,rows:3,value:c.value}
                p {class:"membership-hint","15분 동안 유효합니다. DNS 반영 시간이 길어지면 다시 로그인하고 새 인증값을 받아 주세요. 인증 후에는 이 레코드를 지워도 됩니다."}
                div {class:"membership-actions",
                    button {class:"primary-button",disabled:!ready || pending(),onclick:move |_|{
                        if pending(){return;} let Some(c)=challenge() else{return;}; pending.set(true);message.set(String::new());
                        spawn(async move{match api::finish_dns(c.id,c.value).await {Ok(domain)=>{challenge.set(None);message.set(format!("{domain} 운영자 인증이 완료됐어요."));on_verified.call(());},Err(e)=>message.set(error_message(e))}pending.set(false);});
                    },if pending(){"DNS 확인 중…"}else{"설정했어요 · 확인"}}
                    button {class:"secondary-button",disabled:pending(),onclick:move |_|{challenge.set(None);message.set(String::new());},"다시 시작"}
                }
            }
        } else {
            form {class:"membership-form",onsubmit:move|e|{e.prevent_default();if pending() || !ready{return;}pending.set(true);message.set(String::new());spawn(async move{match api::begin_dns(domain()).await{Ok(c)=>challenge.set(Some(c)),Err(e)=>message.set(error_message(e))}pending.set(false);});},
                div {class:"membership-field",label {r#for:"owner-domain","서버 도메인"} input {id:"owner-domain",required:true,maxlength:253,placeholder:"social.example.org",value:domain,disabled:!ready || pending(),oninput:move|e|domain.set(e.value())}}
                button {class:"primary-button",r#type:"submit",disabled:!ready || pending(),"DNS 인증 시작"}
            }
        }
        if !message().is_empty() {p {class:"membership-notice",role:"status","{message}"}}
    }
}

fn boolean_value(value: Option<bool>) -> &'static str {
    match value {
        Some(true) => "yes",
        Some(false) => "no",
        None => "unknown",
    }
}
fn parse_boolean(value: &str) -> Option<bool> {
    match value {
        "yes" => Some(true),
        "no" => Some(false),
        _ => None,
    }
}

#[component]
fn OwnerEditor(site: OwnedSite, on_saved: EventHandler<()>) -> Element {
    let initial = site.edit.clone();
    let mut edit = use_signal(move || initial);
    let mut pending = use_signal(|| false);
    let mut message = use_signal(String::new);
    let mut confirmation = use_signal(String::new);
    let ready = use_context::<Signal<bool>>()();
    let domain = site.domain.clone();
    let save_domain = domain.clone();
    let resign_domain = domain.clone();
    rsx! {
        details {class:"owner-editor",
            summary {strong {"{domain}"} span {if site.force_hidden {"관리자에 의해 숨겨짐"} else if site.edit.hidden {"목록에서 숨김"} else {"공개 목록에 표시"}}}
            RefreshPanel {domain:domain.clone(),status:site.refresh.clone()}
            form {class:"membership-form",onsubmit:move|e|{
                e.prevent_default();if pending() || !ready{return;}pending.set(true);message.set(String::new());let domain=save_domain.clone();
                spawn(async move{match api::edit(domain,site.revision,edit()).await{Ok(())=>on_saved.call(()),Err(e)=>message.set(error_message(e))}pending.set(false);});
            },
                p {class:"membership-hint","저장하면 관리자 승인 없이 반영됩니다. 변경 이력은 보관됩니다. 빈 이름·소개는 수집된 값으로 표시해요."}
                for (field,label,max,rows) in [("name","서버 이름",200,1),("description","소개",1000,3),("rules","규칙",2000,5),("language","주로 쓰는 언어",32,1),("tags","태그 · 쉼표로 구분, 최대 12개",256,1),("owner_comment","운영자 한마디",2000,4)] {
                    div {class:"membership-field",
                        label {r#for:"owner-{domain}-{field}","{label}"}
                        textarea {id:"owner-{domain}-{field}",rows:rows,maxlength:max,disabled:pending() || !ready,value:field_value(&edit(),field),oninput:move|e|set_field(&mut edit.write(),field,e.value())}
                    }
                }
                div {class:"membership-field",label {r#for:"invite-{domain}","초대가 필요한가요?"} select {id:"invite-{domain}",disabled:pending() || !ready,value:boolean_value(edit().invite_only),onchange:move|e|edit.write().invite_only=parse_boolean(&e.value()),option {value:"unknown","확인 전"} option {value:"yes","초대 필요"} option {value:"no","초대 불필요"}}}
                div {class:"membership-field",label {r#for:"approval-{domain}","가입 승인이 필요한가요?"} select {id:"approval-{domain}",disabled:pending() || !ready,value:boolean_value(edit().approval_required),onchange:move|e|edit.write().approval_required=parse_boolean(&e.value()),option {value:"unknown","확인 전"} option {value:"yes","운영자 승인 필요"} option {value:"no","별도 승인 불필요"}}}
                label {class:"owner-checkbox",input {r#type:"checkbox",checked:edit().hidden,disabled:pending() || !ready,onchange:move|e|edit.write().hidden=e.checked()}"공개 목록에서 숨기기"}
                p {class:"membership-hint","숨기면 목록·서버 상세·통계에서 빠집니다. 서버 자체나 수집 작업을 중단하는 기능은 아니에요."}
                if site.force_hidden {p {class:"membership-notice","관리자가 숨긴 상태는 이 설정으로 해제되지 않습니다."}}
                if site.closed {p {class:"membership-notice","운영 종료로 기록된 서버입니다. 이 화면에서 운영 상태를 바꾸지 않습니다."}}
                if !message().is_empty(){p {role:"status",class:"membership-notice","{message}"}}
                div {class:"membership-actions",button {r#type:"submit",class:"primary-button",disabled:pending() || !ready,if pending(){"처리 중…"}else{"변경 저장"}} button {r#type:"button",class:"secondary-button",disabled:pending() || !ready,onclick:move |_|on_saved.call(()),"다시 불러오기"}}
            }
            details {class:"membership-disclosure owner-resign",summary {"운영자 관리 권한 내려놓기"}p {class:"membership-hint","서버와 안내 내용은 남습니다. 권한을 다시 얻으려면 운영자 인증이 필요해요."}
                label {r#for:"resign-{domain}","확인을 위해 {domain} 입력"}input {id:"resign-{domain}",value:confirmation,disabled:pending() || !ready,oninput:move|e|confirmation.set(e.value())}
                button {class:"secondary-button",disabled:pending() || !ready || confirmation()!=domain,onclick:move |_|{if pending(){return;}pending.set(true);let domain=resign_domain.clone();spawn(async move{match api::resign(domain,site.revision,confirmation()).await{Ok(())=>on_saved.call(()),Err(e)=>message.set(error_message(e))}pending.set(false);});},"관리 권한 내려놓기"}
            }
        }
    }
}
#[component]
fn RefreshPanel(domain: String, status: Option<super::RefreshStatus>) -> Element {
    let mut state = use_signal(move || status);
    let mut pending = use_signal(|| false);
    let mut message = use_signal(String::new);
    let ready = use_context::<Signal<bool>>()();
    let reload_domain = domain.clone();
    rsx! {
        section {class:"owner-refresh",aria_label:"수집 정보 갱신",
            h3 {"수집 정보"}
            p {class:"membership-hint","회원 수·소프트웨어·아이콘을 다시 확인해요. 수동 요청은 1시간에 한 번 가능합니다."}
            if let Some(current)=state() {p {role:"status",class:"membership-notice",{current.state.label()}}}
            if !message().is_empty(){p {role:"status",class:"membership-notice","{message}"}}
            div {class:"membership-actions",
                button {class:"secondary-button",disabled:!ready || pending(),onclick:move |_|{
                    if pending(){return;}pending.set(true);message.set(String::new());let domain=domain.clone();
                    spawn(async move {match api::refresh(domain.clone()).await {Ok(())=>{
                        // The acknowledgement means queued, never completed.
                        state.set(Some(super::RefreshStatus{state:super::RefreshState::Pending,requested_at:String::new()}));
                    },Err(e)=>message.set(error_message(e))}pending.set(false);});
                },"정보 갱신 요청"}
                button {class:"secondary-button",disabled:!ready || pending(),onclick:move |_|{
                    if pending(){return;}pending.set(true);message.set(String::new());let domain=reload_domain.clone();
                    spawn(async move {match api::refresh_status(domain).await{Ok(status)=>state.set(status),Err(e)=>message.set(error_message(e))}pending.set(false);});
                },"진행 상태 확인"}
            }
        }
    }
}
fn field_value(e: &SiteEdit, field: &str) -> String {
    match field {
        "name" => &e.name,
        "description" => &e.description,
        "rules" => &e.rules,
        "language" => &e.language,
        "tags" => &e.tags,
        _ => &e.owner_comment,
    }
    .clone()
}
fn set_field(e: &mut SiteEdit, field: &str, value: String) {
    *match field {
        "name" => &mut e.name,
        "description" => &mut e.description,
        "rules" => &mut e.rules,
        "language" => &mut e.language,
        "tags" => &mut e.tags,
        _ => &mut e.owner_comment,
    } = value;
}
