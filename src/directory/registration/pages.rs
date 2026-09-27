use super::{api, Preview};
use crate::{
    membership::{api as member_api, pages::error_message},
    Route,
};
use dioxus::prelude::*;

#[component]
pub fn RegisterSite() -> Element {
    rsx! {
        document::Title{"서버 등록 — fediverse.kr"}
        main{id:"content",class:"membership-page registration-page wrap",
            Link{class:"back-link",to:Route::Servers{filters:Default::default()},"서버 찾기"}
            header{class:"membership-heading",h1{"서버를 목록에 더해 주세요."}p{"직접 운영하는 곳이 아니어도 알려줄 수 있어요."}}
            section{class:"registration-guidance",aria_label:"서버 등재 안내",
                h2{"이런 커뮤니티를 등록해 주세요"}
                ul{
                    li{"한국어를 주로 사용하는 커뮤니티"}
                    li{"완전한 비공개 커뮤니티가 아닌 곳"}
                    li{"fediverse.kr에 공개되기를 원하는 경우에만 등록해 주세요."}
                }
                p{"가입 마감·승인제·초대제라는 이유만으로 비공개 커뮤니티로 보지는 않아요."}
                p{"목록 노출을 원하지 않는 경우 " Link{class:"text-link",to:Route::Contact{},"운영자에게 알려 주세요"} ". 관리자가 확인한 뒤 목록에서 내립니다."}
                h2{"자동 등록에는 NodeInfo 지원이 필요해요"}
                p{"NodeInfo는 서버의 소프트웨어와 통계 등을 알려주는 표준입니다. 서버가 이를 구현하고, fediverse.kr에서 공개된 정보를 읽을 수 있어야 자동으로 등록할 수 있어요."}
                details{class:"registration-nodeinfo",
                    summary{"운영자용 NodeInfo 기술 안내"}
                    ul{
                        li{code{"/.well-known/nodeinfo"} "에서 NodeInfo 문서 주소를 안내해야 합니다. 현재 NodeInfo 2.0과 2.1을 지원합니다."}
                        li{"연결된 JSON 문서에서 소프트웨어 정보를 확인할 수 있어야 하며, 로그인 없이 서버 응답과 NodeInfo를 읽을 수 있어야 합니다."}
                        li{"접근 차단이나 일시적인 장애가 있으면 지원하는 서버도 확인에 실패할 수 있습니다."}
                    }
                }
                p{"NodeInfo를 지원하지 않거나 직접 등록하기 어려운 경우에도 문의할 수 있어요. 등재 조건을 확인한 뒤 수동 등록 가능 여부를 검토합니다."}
                div{class:"registration-help-links",
                    a{class:"text-link",href:crate::information::REGISTRATION_REQUEST_URL,target:"_blank",rel:"noopener noreferrer","GitHub에 등록 요청 (공개 이슈)"}
                    Link{class:"text-link",to:Route::Contact{},"운영자에게 문의"}
                }
            }
            SuspenseBoundary{fallback:|_|rsx!{p{role:"status","로그인 확인 중…"}},RegistrationAccess{}}
        }
    }
}

#[component]
fn RegistrationAccess() -> Element {
    let account = use_server_future(member_api::session)?;
    rsx! {
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
                if !message().is_empty(){
                    p{role:"alert",class:"membership-message","{message}"}
                    p{class:"registration-error-help","계속 등록하기 어렵다면 " Link{class:"text-link",to:Route::Contact{},"운영자에게 문의"} "해 주세요."}
                }
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
