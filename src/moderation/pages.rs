use super::Action;
use super::*;
use crate::Route;
use dioxus::prelude::*;
pub(super) fn message(e: &ServerFnError) -> String {
    match e {
        ServerFnError::ServerError { message, .. } => message.clone(),
        _ => "관리 정보를 불러오지 못했습니다. 잠시 후 다시 확인해 주세요.".into(),
    }
}
fn reason(s: &str) -> &str {
    match s {
        "spam" => "스팸",
        "abuse" => "괴롭힘·욕설",
        "offtopic" => "주제와 무관",
        _ => "기타",
    }
}
fn event_action(s: &str) -> &str {
    match s {
        "resolve" => "처리 완료",
        "dismiss" => "신고 기각",
        "reopen" => "다시 검토",
        "delete_comment" => "댓글 삭제 표시",
        "ban" => "작성자 이용 차단",
        "unban" => "작성자 차단 해제",
        _ => "관리 조치",
    }
}
fn state_label(s: &str) -> &str {
    match s {
        "visible" => "공개",
        "deleted" => "삭제 표시",
        "active" => "이용 가능",
        "banned" => "차단",
        _ => status_label(s),
    }
}
// API timestamps are UTC. A fixed rendering avoids SSR/client locale mismatch.
pub(super) fn timestamp(value: &str) -> String {
    format!("{} UTC", value.get(..16).unwrap_or(value).replace('T', " "))
}
#[component]
pub fn AdminEntry() -> Element {
    let access = use_server_future(api::access)?;
    rsx! {if let Some(Ok(true))=access(){AdminEntryCard{}}}
}
#[component]
fn AdminEntryCard() -> Element {
    rsx! {section{class:"membership-card backoffice-entry",aria_label:"백오피스",
        div{p{class:"backoffice-kicker","관리자 전용"}h2{"백오피스"}}
        p{"신고와 서버, 소프트웨어, 수집 작업을 한 작업 공간에서 확인합니다."}
        a{class:"primary-button",href:Route::Moderation{}.to_string(),"백오피스 열기"}
    }}
}
#[component]
pub fn AdminNav() -> Element {
    rsx! {nav{class:"membership-actions moderation-nav",aria_label:"관리자 메뉴",
        Link{class:"text-link",to:Route::ModerationReports{},"신고 관리"}
        Link{class:"text-link",to:Route::ModerationSites{},"서버 관리"}
        Link{class:"text-link",to:Route::ModerationCatalog{},"소프트웨어 관리"}
        Link{class:"text-link",to:Route::ModerationProfiles{},"프로필 갱신"}
        Link{class:"text-link",to:Route::ModerationWorkers{},"수집 현황"}
        Link{class:"text-link",to:Route::Account{},"내 계정"}
    }}
}
#[component]
pub fn ModerationReports() -> Element {
    let ready = use_context::<Signal<bool>>()();
    let mut status = use_signal(|| "pending".to_owned());
    let mut page = use_signal(|| 0u32);
    let mut reports = use_server_future(move || api::reports(status(), page()))?;
    rsx! {
        document::Title {"신고 — fediverse.kr"}
        super::layout::BackofficePage {
            section:super::layout::BackofficeSection::Reports,
            title:"신고".to_string(),
            description:"신고를 상태별로 찾고, 원문과 현재 내용을 비교해 필요한 조치를 남깁니다.".to_string(),
            div{class:"moderation-page",
            nav {class:"membership-actions",aria_label:"신고 상태",
                for key in ["pending","resolved","dismissed","all"] {button{r#type:"button",class:if status()==key{"primary-button"}else{"secondary-button"},disabled:!ready,aria_pressed:(status()==key).to_string(),onclick:move |_|{status.set(key.to_owned());page.set(0);},"{status_label(key)}"}}
            }
            match reports() {
                Some(Ok(data))=>rsx!{
                    if data.reports.is_empty(){p{class:"membership-notice","이 상태의 신고가 없습니다."}}
                    ul {class:"moderation-list",
                        for r in data.reports {li {key:"{r.id}",class:"membership-card",
                            div {class:"moderation-meta",span {"{status_label(&r.status)} · {reason(&r.reason)}"} time {datetime:r.created_at.clone(),"{timestamp(&r.created_at)}"}}
                            h2 {Link{to:Route::ModerationReport{id:r.id.clone()},{r.domain.as_deref().unwrap_or("연결된 서버 없음")}}}
                            p {class:"moderation-excerpt","{r.excerpt}"}
                            Link {class:"text-link",to:Route::ModerationReport{id:r.id},"신고 검토"}
                        }}
                    }
                    div {class:"membership-actions",button {class:"secondary-button",disabled:!ready||page()==0,onclick:move |_|page.set(page()-1),"이전"}span{"{page()+1}쪽"}button {class:"secondary-button",disabled:!ready||!data.has_next,onclick:move |_|page.set(page()+1),"다음"}}
                },
                Some(Err(e))=>rsx!{p{class:"membership-message membership-error",role:"alert","{message(&e)}"}Link{class:"text-link",to:Route::Login{},"로그인"}},
                None=>rsx!{p{role:"status","신고를 불러오고 있어요."}},
            }
            button {class:"secondary-button",disabled:!ready,onclick:move |_|reports.restart(),"목록 새로고침"}
            }
        }
    }
}
#[component]
pub fn ModerationReport(id: String) -> Element {
    // A different route ID remounts the editor, never reuses a former report's draft.
    rsx! {ReportLoader{key:"{id}",id}}
}
#[component]
fn ReportLoader(id: String) -> Element {
    let ready = use_context::<Signal<bool>>()();
    let mut fetched = use_server_future(move || api::report(id.clone()))?;
    rsx! {
        document::Title {"신고 검토 — fediverse.kr"}
        super::layout::BackofficePage {
            section:super::layout::BackofficeSection::Reports,
            title:"신고 검토".to_string(),
            description:"신고 당시 원문과 현재 상태를 비교한 뒤, 각 조치를 독립적으로 선택합니다.".to_string(),
            div{class:"moderation-page report-workflow",
            Link{class:"text-link backoffice-back-link",to:Route::ModerationReports{},"신고 목록"}
            match fetched(){
                Some(Ok(data))=>rsx!{ReportEditor {initial:data}},
                Some(Err(e))=>rsx!{p{class:"membership-message membership-error",role:"alert","{message(&e)}"}button{class:"secondary-button",disabled:!ready,onclick:move |_|fetched.restart(),"다시 불러오기"}Link{class:"text-link",to:Route::Account{},"내 계정에서 재인증"}},
                None=>rsx!{p{role:"status","신고를 확인하고 있어요."}},
            }
            }
        }
    }
}
#[component]
fn ReportEditor(initial: ReportDetail) -> Element {
    let ready = use_context::<Signal<bool>>()();
    let mut current = use_signal(|| initial);
    let mut note = use_signal(String::new);
    let mut selected = use_signal(|| None::<Action>);
    let mut pending = use_signal(|| false);
    let mut error = use_signal(String::new);
    let mut notice = use_signal(String::new);
    let mut generation = use_signal(|| 0u32);
    let data = current();
    let mut actions = vec![];
    if data.summary.status != "resolved" {
        actions.push(Action::Resolve)
    }
    if data.summary.status != "dismissed" {
        actions.push(Action::Dismiss)
    }
    if data.summary.status != "pending" {
        actions.push(Action::Reopen)
    }
    if let Some(c) = &data.comment {
        if !c.deleted {
            actions.push(Action::DeleteComment)
        }
        if c.author_id.is_some() && !c.author_admin {
            actions.push(if c.author_banned == Some(true) {
                Action::Unban
            } else {
                Action::Ban
            });
        }
    }
    rsx! {
        section {class:"membership-card report-decision",
            h2{"신고 판정"}
            div {class:"moderation-meta",strong{"{status_label(&data.summary.status)} · {reason(&data.summary.reason)}"}time{datetime:data.summary.created_at.clone(),"{timestamp(&data.summary.created_at)}"}}
            h3 {{data.summary.domain.as_deref().unwrap_or("연결된 서버 없음")}}
            p {class:"membership-hint","신고자: " {data.summary.reporter_name.as_deref().unwrap_or("탈퇴한 회원")}}
            p {class:"moderation-body","{data.detail}"}
            if data.detail_truncated{p{class:"membership-hint","긴 기존 신고 설명의 앞 1,000자만 표시합니다."}}
            if !data.admin_note.is_empty() {
                h3{"마지막 처리 메모"}p{class:"moderation-body","{data.admin_note}"}
                if data.note_truncated{p{class:"membership-hint","긴 기존 메모의 앞 1,000자만 표시합니다."}}
                p{class:"membership-hint",{data.resolver_name.as_deref().unwrap_or("이전 관리자")}}
            }
        }
        div {class:"membership-account-grid",
            section {class:"membership-card report-evidence",h2{"신고 당시 원문"}
                if let Some(e)=data.evidence {
                    p{class:"membership-hint","{e.author_name}"}p{class:"moderation-body",tabindex:"0","{e.body}"}
                }else{p{class:"membership-hint","별도 원문 기록이 없는 기존 신고입니다. 현재 댓글이 신고 당시와 같다고 보장할 수 없어요."}}
            }
            section {class:"membership-card report-current-content",h2{"현재 댓글"}
                if let Some(c)=data.comment {
                    p {class:"membership-hint",{c.author_name.as_deref().unwrap_or("탈퇴한 회원")}}
                    if c.deleted {p{class:"membership-notice","공개 화면에서는 삭제된 댓글입니다. 보존 원문을 표시합니다."}}
                    if c.author_banned==Some(true){p{class:"membership-notice","작성자는 fediverse.kr 이용이 차단되어 있습니다."}}
                    if c.author_admin {p{class:"membership-hint","관리자 계정 — 차단 전 관리 권한 회수 필요"}}
                    p{class:"moderation-body",tabindex:"0","{c.body}"}
                }else{p{class:"membership-hint","연결된 댓글이 없습니다."}}
            }
        }
        section {class:"membership-card report-actions",h2{"관리 조치"}
            p {class:"membership-hint","조치 사유와 이력은 관리자에게만 보입니다. 최근 15분 안에 로그인하거나 재인증해야 저장할 수 있어요."}
            div {class:"membership-actions",
                for action in actions {button {class:"secondary-button",disabled:pending()||!ready,onclick:move |_|{selected.set(Some(action));error.set(String::new());notice.set(String::new());},"{action.label()}"}}
            }
            if let Some(action)=selected(){
                form {class:"membership-form moderation-confirm",onsubmit:move |ev|{
                    ev.prevent_default();if pending()||!ready{return;}
                    if note().trim().is_empty(){error.set("조치 사유를 입력해 주세요.".into());return;}
                    pending.set(true);error.set(String::new());notice.set(String::new());
                    let data=current();let c=data.comment.as_ref();
                    let request=ActionRequest{report:data.summary.id,revision:data.revision,comment_revision:c.map(|c|c.revision.clone()),author_id:c.and_then(|c|c.author_id.clone()),author_banned:c.and_then(|c|c.author_banned),author_revision:c.and_then(|c|c.author_revision.clone()),action,note:note()};
                    spawn(async move {match api::act(request).await {
                        Ok(updated)=>{current.set(updated);selected.set(None);note.set(String::new());notice.set(format!("{}를 반영했습니다.",action.label()));generation.set(generation()+1);},
                        Err(e)=>error.set(message(&e)),
                    }pending.set(false);});
                },
                    h3{"{action.label()} 확인"}p{"{action.explanation()}"}
                    div{class:"membership-field",label{r#for:"moderation-note","조치 사유"}textarea{id:"moderation-note",maxlength:"1000",required:true,rows:4,disabled:pending()||!ready,value:note(),oninput:move |ev|note.set(ev.value())}}
                    div{class:"membership-actions",button{r#type:"submit",class:"primary-button",disabled:pending()||!ready,if pending(){"저장 중…"}else{"확인하고 적용"}}button{r#type:"button",class:"secondary-button",disabled:pending(),onclick:move |_|selected.set(None),"취소"}}
                }
            }
            if !error().is_empty(){p{class:"membership-message membership-error",role:"alert","{error}"}}
            if !notice().is_empty(){p{class:"membership-message membership-success",role:"status","{notice}"}}
            div{class:"membership-actions",
                button{class:"secondary-button",disabled:pending()||!ready,onclick:move |_|{
                    if pending()||!ready{return;}pending.set(true);let id=current().summary.id;
                    spawn(async move{match api::report(id).await{Ok(data)=>{current.set(data);selected.set(None);error.set(String::new());notice.set("최신 내용을 불러왔습니다. 사유 초안은 남겨두었습니다. 내용을 확인하고 조치를 다시 선택해 주세요.".into());generation.set(generation()+1);},Err(e)=>error.set(message(&e))}pending.set(false);});
                },"최신 내용 확인"}
                Link{class:"text-link",to:Route::Account{},"내 계정에서 재인증"}
            }
        }
        {rsx!{History{key:"{generation}",id:current().summary.id}}}
    }
}
#[component]
fn History(id: String) -> Element {
    let ready = use_context::<Signal<bool>>()();
    let mut page = use_signal(|| 0u32);
    let mut events = use_server_future(move || api::events(id.clone(), page()))?;
    rsx! {section{class:"membership-card report-history",h2{"조치 이력"}
        match events(){Some(Ok(data))=>rsx!{
            if data.events.is_empty(){p{class:"membership-hint","새 버전에서 남긴 조치가 없습니다. 기존 처리 메모는 위에 표시합니다."}}
            ol{class:"moderation-events",for e in data.events{li{key:"{e.id}",strong{"{event_action(&e.action)}"}p{class:"membership-hint",{e.actor_name.as_deref().unwrap_or("탈퇴한 관리자")} " · {timestamp(&e.created_at)}"}p{"{state_label(&e.before)} → {state_label(&e.after)}"}p{class:"moderation-body","{e.note}"}}}}
            div{class:"membership-actions",button{class:"secondary-button",disabled:!ready||page()==0,onclick:move |_|page.set(page()-1),"이전 이력"}span{"{page()+1}쪽"}button{class:"secondary-button",disabled:!ready||!data.has_next,onclick:move |_|page.set(page()+1),"다음 이력"}}
        },Some(Err(e))=>rsx!{p{class:"membership-message membership-error","{message(&e)}"}button{class:"secondary-button",disabled:!ready,onclick:move |_|events.restart(),"이력 다시 불러오기"}},None=>rsx!{p{role:"status","이력을 불러오고 있어요."}}}
    }}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn granted_admin_entry_is_one_clear_backoffice_gateway() {
        let mut dom = VirtualDom::new(|| rsx! { AdminEntryCard {} });
        dom.rebuild_in_place();
        let html = dioxus::ssr::render(&dom);

        assert!(html.contains("백오피스"), "{html}");
        assert_eq!(
            html.matches("href=\"/account/moderation\"").count(),
            1,
            "{html}"
        );
        for legacy_label in [
            "신고 관리",
            "서버 관리",
            "소프트웨어 관리",
            "프로필 갱신",
            "수집 현황",
        ] {
            assert!(!html.contains(legacy_label), "{html}");
        }
    }
}
