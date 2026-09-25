use super::{api, Preview};
use crate::{
    membership::{api as member_api, pages::error_message},
    Route,
};
use dioxus::prelude::*;

#[component]
pub fn RegisterSite() -> Element {
    let account = use_server_future(member_api::session)?;
    rsx! {
        document::Title{"서버 등록 — fediverse.kr"}
        main{id:"content",class:"membership-page wrap",
            Link{class:"back-link",to:Route::Servers{filters:Default::default()},"서버 찾기"}
            header{class:"membership-heading",h1{"서버를 목록에 더해 주세요."}p{"직접 운영하는 곳이 아니어도 알려줄 수 있어요."}}
            match account() {
                Some(Ok(state)) if state.member.is_some()=>rsx!{RegistrationForm{}},
                Some(Ok(state))=>rsx!{section{class:"membership-card",
                    h2{"로그인 후 등록할 수 있어요."}
                    Link{class:"primary-button",to:Route::Login{},"로그인"}
                    if !state.federation_available {p{class:"membership-hint","지금은 DB가 연결되지 않은 검토용 화면입니다."}}
                }},
                Some(Err(e))=>rsx!{p{role:"alert",{error_message(e)}}},
                None=>rsx!{p{role:"status","로그인 확인 중…"}},
            }
        }
    }
}
#[component]
fn RegistrationForm() -> Element {
    let mut allowed = use_server_future(api::eligibility)?;
    let mut domain = use_signal(String::new);
    let mut preview = use_signal(|| None::<Preview>);
    let mut pending = use_signal(|| false);
    let mut message = use_signal(String::new);
    let mut registered = use_signal(|| None::<String>);
    let ready = use_context::<Signal<bool>>()();
    let can_submit = ready && matches!(allowed(), Some(Ok(())));
    rsx! {
        match allowed() {
            Some(Err(e))=>rsx!{section{class:"membership-card",p{role:"alert",{error_message(e)}}
                Link{class:"secondary-button",to:Route::Account{},"연동 계정 확인"}
                p{class:"membership-hint","연동 계정의 생성일을 해당 서버에서 다시 확인합니다."}
                button{class:"secondary-button",disabled:!ready || pending(),onclick:move |_|{
                    if !ready || pending(){return;}
                    pending.set(true);message.set(String::new());
                    spawn(async move {
                        if let Err(e)=api::recheck_eligibility().await {message.set(error_message(e));}
                        allowed.restart();pending.set(false);
                    });
                },if pending(){"계정 생성일 확인 중…"}else{"다시 확인"}}
            }},
            None=>rsx!{p{role:"status","등록 권한 확인 중…"}},
            _=>rsx!{},
        }
        if let Some(saved)=registered() {
            section{class:"membership-card",h2{"목록에 등록했어요."}p{role:"status","{saved}"}
                p{class:"membership-hint","서버 정보는 주기적으로 갱신됩니다. 운영자 권한은 별도로 인증해요."}
                div{class:"membership-actions",
                    Link{class:"primary-button",to:Route::ServerDetail{slug:saved},"서버 정보 보기"}
                    Link{class:"secondary-button",to:Route::ManagedSites{},"운영자 인증"}
                }
            }
        } else {
            form{class:"membership-card membership-form",onsubmit:move|e|{
                e.prevent_default();if pending() || !can_submit{return;}
                pending.set(true);message.set(String::new());preview.set(None);
                let input=domain();spawn(async move{match api::preview(input).await {Ok(data)=>preview.set(Some(data)),Err(e)=>message.set(error_message(e))}pending.set(false);});
            },
                div{class:"membership-field",label{r#for:"registration-domain","서버 주소"}
                    input{id:"registration-domain",value:domain,placeholder:"example.org",required:true,maxlength:1024,disabled:pending() || !can_submit,oninput:move|e|{domain.set(e.value());preview.set(None);message.set(String::new());}}
                    p{class:"membership-hint","글이나 프로필 주소가 아닌 사이트 주소를 입력해 주세요."}
                }
                button{r#type:"submit",class:"secondary-button",disabled:pending() || !can_submit,if pending(){"서버 확인 중…"}else{"서버 정보 확인"}}
                if !message().is_empty(){p{role:"alert",class:"membership-message","{message}"}}
            }
            if let Some(data)=preview() {
                section{class:"membership-card registration-preview",aria_label:"확인한 서버 정보",
                    h2{{data.name.clone().unwrap_or_else(||data.domain.clone())}}
                    p{"{data.domain}"}
                    if let Some(description)=data.description {p{class:"registration-description","{description}"}}
                    dl{class:"registration-facts",
                        dt{"소프트웨어"}dd{"{data.software}"}
                        dt{"가입 계정"}dd{if let Some(users)=data.users{"{users}"}else{"확인할 수 없음"}}
                        dt{"가입 접수"}dd{match data.registration_open {Some(true)=>"열림",Some(false)=>"닫힘",None=>"확인할 수 없음"}}
                    }
                    p{class:"membership-hint","등록하면 목록에 공개됩니다. 확인을 위해 정보를 한 번 더 가져옵니다. 등록한 사람이 운영자가 되는 것은 아니에요."}
                    button{class:"primary-button",disabled:pending() || !can_submit,onclick:move |_|{
                        if pending() || !can_submit{return;}pending.set(true);message.set(String::new());let confirmed=data.domain.clone();
                        spawn(async move {match api::create(confirmed).await {Ok(saved)=>registered.set(Some(saved)),Err(e)=>message.set(error_message(e))}pending.set(false);});
                    },if pending(){"다시 확인하고 등록 중…"}else{"이 서버 등록"}}
                }
            }
        }
    }
}
