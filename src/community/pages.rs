use super::{api, Access, Comment, Permission, Reason};
use crate::{membership::pages::error_message, Route};
use dioxus::prelude::*;

#[component]
pub fn Discussion(domain: String) -> Element {
    let mut page = use_signal(|| 0u32);
    let mut feed = use_server_future(use_reactive((&domain,), move |(domain,)| {
        api::threads(domain, page())
    }))?;
    let ready = use_context::<Signal<bool>>();
    let mut identity = use_signal(|| domain.clone());
    let mut retained = use_signal(|| feed().and_then(Result::ok));
    let mut generation = use_signal(|| 0u64);
    let pending = use_signal(|| false);
    use_effect(use_reactive((&domain,), move |(domain,)| {
        if *identity.peek() != domain {
            identity.set(domain);
            page.set(0);
            retained.set(None);
        }
    }));
    use_effect(move || {
        if let Some(Ok(value)) = feed() {
            retained.set(Some(value));
            let next = *generation.peek() + 1;
            generation.set(next);
        }
    });
    let access = use_resource(use_reactive((&domain,), move |(domain,)| async move {
        if !ready() {
            return Ok(Access {
                display_name: None,
                available: false,
                permissions: vec![],
            });
        }
        let ids = retained()
            .filter(|v| v.domain == domain)
            .map(|v| {
                v.threads
                    .iter()
                    .flat_map(|t| std::iter::once(&t.comment).chain(t.replies.iter()))
                    .map(|c| c.id.clone())
                    .collect::<Vec<_>>()
                    .join(",")
            })
            .unwrap_or_default();
        api::access(domain, ids).await
    }));
    let mut change_message = use_signal(String::new);
    #[cfg(target_arch = "wasm32")]
    use_future(move || async move {
        loop {
            gloo_timers::future::TimeoutFuture::new(1000).await;
            let Some(current) = retained() else {
                continue;
            };
            if current.preview {
                continue;
            }
            let visible = document::eval("return !document.hidden")
                .join::<bool>()
                .await
                .unwrap_or(false);
            if !visible {
                gloo_timers::future::TimeoutFuture::new(10_000).await;
                continue;
            }
            let active = current.domain;
            match api::wait_for_changes(active.clone()).await {
                Ok(()) => {
                    if *identity.peek() == active {
                        feed.restart();
                        change_message.set(String::new());
                    }
                }
                Err(e) => {
                    if *identity.peek() == active {
                        change_message.set(error_message(e));
                        feed.restart();
                    }
                    gloo_timers::future::TimeoutFuture::new(10_000).await;
                }
            }
        }
    });
    let data = feed().and_then(Result::ok).or_else(|| retained());
    let controls = access().and_then(Result::ok);
    rsx! {
        section{class:"community-section",aria_label:"서버 이용자 이야기",
            header{class:"community-heading",h2{"이곳을 이용해 보셨나요?"}p{"경험과 궁금한 점을 나눠 주세요."}}
            if let Some(Err(e))=feed(){p{role:"alert",class:"membership-notice",{error_message(e)}}}
            if let Some(data)=data.filter(|v|v.domain==domain) {
                if data.preview {p{class:"membership-notice","화면 검토용 예시 댓글입니다. 실제 이용 후기가 아니며 입력해도 저장되지 않아요."}}
                if !matches!(feed(),Some(Err(_))) {
                    Composer{key:"new-{domain}",domain:domain.clone(),parent:None,original:None,pending,can_save:controls.as_ref().is_some_and(|s|s.display_name.is_some())&&!data.preview,author:controls.as_ref().and_then(|s|s.display_name.clone()),on_saved:move |_|feed.restart()}
                    if !data.preview && ready() && controls.as_ref().is_some_and(|s|s.display_name.is_none()) {Link{class:"text-link",to:Route::Login{},"로그인하고 이야기 나누기"}}
                    if let Some(Err(e))=access(){p{role:"status",{error_message(e)}}}
                    if data.threads.is_empty(){p{class:"community-empty","아직 댓글이 없어요."}}
                    div{class:"community-threads",for thread in data.threads {
                        article{class:"community-thread",key:"{thread.comment.id}",
                            CommentCard{domain:domain.clone(),comment:thread.comment.clone(),preview:data.preview,permission:permission(&controls,&thread.comment.id),author:controls.as_ref().and_then(|a|a.display_name.clone()),on_changed:move |_|feed.restart()}
                            ReplyList{key:"{thread.comment.id}",domain:domain.clone(),parent:thread.comment.id.clone(),initial:thread.replies,total:thread.reply_count,preview:data.preview,access:controls.clone(),generation:generation(),on_changed:move |_|feed.restart()}
                        }
                    }}
                    nav{class:"directory-pagination",aria_label:"댓글 페이지",button{class:"secondary-button",disabled:!ready() || page()==0,onclick:move |_|page.set(page().saturating_sub(1)),"이전"}span{"{data.page+1}페이지"}button{class:"secondary-button",disabled:!ready() || !data.has_next,onclick:move |_|page.set(page()+1),"다음"}}
                    button{class:"text-link",disabled:!ready(),onclick:move |_|feed.restart(),"댓글 새로고침"}
                }
            }else{p{role:"status","댓글을 불러오는 중…"}}
            if !change_message().is_empty(){p{role:"status",class:"membership-hint","{change_message}"}}
        }
    }
}
fn permission(access: &Option<Access>, id: &str) -> Option<Permission> {
    access
        .as_ref()
        .and_then(|a| a.permissions.iter().find(|p| p.id == id).cloned())
}

#[component]
fn ReplyList(
    domain: String,
    parent: String,
    initial: Vec<Comment>,
    total: i64,
    preview: bool,
    access: Option<Access>,
    generation: u64,
    on_changed: EventHandler<()>,
) -> Element {
    let mut expanded = use_signal(|| false);
    let ready = use_context::<Signal<bool>>()();
    rsx! {if total>0 {div{class:"community-replies",
        if expanded(){ReplyPages{domain,parent,preview,generation,on_changed}}
        else {for comment in initial {CommentCard{key:"{comment.id}",domain:domain.clone(),permission:permission(&access,&comment.id),author:access.as_ref().and_then(|a|a.display_name.clone()),comment,preview,on_changed}}}
        if total>3 {button{class:"text-link",disabled:!ready,onclick:move |_|expanded.set(!expanded()),if expanded(){"답글 접기"}else{"답글 {total}개 모두 보기"}}}
    }}}
}
#[component]
fn ReplyPages(
    domain: String,
    parent: String,
    preview: bool,
    generation: u64,
    on_changed: EventHandler<()>,
) -> Element {
    let mut page = use_signal(|| 0u32);
    let mut result = use_server_future(use_reactive(
        (&domain, &parent, &generation),
        move |(domain, parent, _)| api::replies(domain, parent, page()),
    ))?;
    let ready = use_context::<Signal<bool>>();
    let access = use_resource(use_reactive((&domain,), move |(domain,)| async move {
        let ids = result()
            .and_then(Result::ok)
            .map(|p| {
                p.replies
                    .iter()
                    .map(|c| c.id.clone())
                    .collect::<Vec<_>>()
                    .join(",")
            })
            .unwrap_or_default();
        if !ready() {
            return Ok(Access {
                display_name: None,
                available: false,
                permissions: vec![],
            });
        }
        api::access(domain, ids).await
    }));
    let controls = access().and_then(Result::ok);
    rsx! {match result(){
        Some(Ok(data)) if data.domain==domain && data.parent==parent=>rsx!{
            for comment in data.replies {CommentCard{key:"{comment.id}",domain:domain.clone(),permission:permission(&controls,&comment.id),author:controls.as_ref().and_then(|a|a.display_name.clone()),comment,preview,on_changed:move |_|{result.restart();on_changed.call(());}}}
            nav{class:"directory-pagination",aria_label:"답글 페이지",button{class:"secondary-button",disabled:!ready()||page()==0,onclick:move |_|page.set(page().saturating_sub(1)),"이전 답글"}span{"{data.page+1}페이지"}button{class:"secondary-button",disabled:!ready()||!data.has_next,onclick:move |_|page.set(page()+1),"다음 답글"}}
        },
        Some(Err(e))=>rsx!{p{role:"alert",{error_message(e)}}},
        _=>rsx!{p{role:"status","답글을 불러오는 중…"}},
    }}
}

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Read,
    Reply,
    Edit,
    Delete,
    Report,
}
#[component]
fn CommentCard(
    domain: String,
    comment: Comment,
    preview: bool,
    permission: Option<Permission>,
    author: Option<String>,
    on_changed: EventHandler<()>,
) -> Element {
    let mut mode = use_signal(|| Mode::Read);
    let mut message = use_signal(String::new);
    let mut pending = use_signal(|| false);
    let ready = use_context::<Signal<bool>>()();
    let logged_in = author.is_some() && !preview;
    let own = permission.as_ref().is_some_and(|p| p.own);
    let can_edit = permission.as_ref().is_some_and(|p| p.can_edit);
    let can_report = permission.as_ref().is_some_and(|p| p.can_report);
    let mut selected = use_signal(|| comment.clone());
    rsx! {
        div{class:"community-comment",id:"comment-{comment.id}",
            if comment.deleted {p{class:"community-tombstone","삭제된 댓글입니다."}}
            else {
                header{strong{"{comment.author_name}"}time{datetime:comment.created_at.clone(),"{date_label(&comment.created_at)}"}if comment.created_at!=comment.updated_at{span{class:"membership-hint","수정됨"}}}
                p{class:"community-body","{comment.body}"}
                if comment.truncated {p{class:"membership-hint","이전 자료의 긴 댓글은 앞의 2,000자만 표시합니다."}}
                div{class:"community-actions",
                    if comment.parent_id.is_none() && (logged_in||preview){button{disabled:!ready||pending(),onclick:move |_|mode.set(Mode::Reply),"답글"}}
                    if can_edit{button{disabled:!ready||pending(),onclick:move |_|mode.set(Mode::Edit),"수정"}}
                    if own{button{disabled:!ready||pending(),onclick:{let current=comment.clone();move |_|{selected.set(current.clone());mode.set(Mode::Delete);}},"삭제"}}
                    if can_report||preview{button{disabled:!ready||pending(),onclick:move |_|mode.set(Mode::Report),"신고"}}
                }
                match mode(){
                    Mode::Reply=>rsx!{Composer{domain:domain.clone(),parent:Some(comment.id.clone()),original:None,pending,can_save:logged_in,author:author.clone(),on_saved:move |_|{mode.set(Mode::Read);on_changed.call(());}}},
                    Mode::Edit=>rsx!{Composer{domain:domain.clone(),parent:None,original:Some(comment.clone()),pending,can_save:logged_in,author:author.clone(),on_saved:move |_|{mode.set(Mode::Read);on_changed.call(());}}},
                    Mode::Delete=>rsx!{div{class:"membership-notice",p{"댓글을 삭제할까요? 답글은 남고, 본문은 더 이상 공개되지 않습니다. 신고 처리를 위한 원문은 보관됩니다."}button{class:"secondary-button",disabled:pending(),onclick:{let domain=domain.clone();move |_|{
                        if pending(){return;}pending.set(true);let domain=domain.clone();let comment=selected();
                        spawn(async move{match api::delete(domain,comment.id,comment.revision).await{Ok(_)=>{mode.set(Mode::Read);on_changed.call(());},Err(e)=>message.set(error_message(e))}pending.set(false);});
                    }},"확인 · 댓글 삭제"}}},
                    Mode::Report=>rsx!{ReportForm{domain:domain.clone(),comment:comment.id.clone(),pending,can_save:logged_in,on_saved:move |_|{mode.set(Mode::Read);message.set("신고가 접수되었습니다. 바로 삭제되는 것은 아니에요.".into());on_changed.call(());}}},
                    Mode::Read=>rsx!{},
                }
                if mode()!=Mode::Read{button{class:"text-link",disabled:pending(),onclick:move |_|mode.set(Mode::Read),"닫기"}}
            }
            if !message().is_empty(){p{role:"status",class:"membership-notice","{message}"}}
        }
    }
}
#[component]
fn Composer(
    domain: String,
    parent: Option<String>,
    original: Option<Comment>,
    mut pending: Signal<bool>,
    can_save: bool,
    author: Option<String>,
    on_saved: EventHandler<()>,
) -> Element {
    // Pin text and revision together. Incoming live updates must not silently
    // rebase an unsaved edit onto the newer revision.
    let mut base = use_signal(|| original.clone());
    let mut body = use_signal(|| {
        original
            .as_ref()
            .map(|c| c.body.clone())
            .unwrap_or_default()
    });
    let mut message = use_signal(String::new);
    let ready = use_context::<Signal<bool>>()();
    let name = base()
        .as_ref()
        .map(|c| format!("edit-{}", c.id))
        .unwrap_or_else(|| parent.clone().unwrap_or_else(|| "root".into()));
    rsx! {form{class:"membership-card membership-form community-form",onsubmit:move|e|{
        e.prevent_default();if !can_save||pending()||!ready{return;}pending.set(true);message.set(String::new());let domain=domain.clone();let parent=parent.clone();
        spawn(async move{let result=if let Some(old)=base(){api::edit(domain,old.id,old.revision,body()).await}else{api::create(domain,parent,body()).await};
            match result{Ok(_)=>{body.set(String::new());on_saved.call(());},Err(e)=>message.set(error_message(e))}pending.set(false);
        });
    },
        if let Some(author)=author {p{class:"membership-hint","작성자 이름은 ‘{author}’로 공개됩니다. 연동한 SNS 계정 주소는 표시하지 않아요."}}
        label{r#for:"comment-body-{name}",if base().is_some(){"댓글 수정"}else if name=="root"{"이 서버에 대한 경험이나 질문"}else{"답글 내용"}}
        textarea{id:"comment-body-{name}",rows:4,maxlength:2000,required:true,value:body,disabled:!ready||pending(),oninput:move|e|body.set(e.value())}
        p{class:"membership-hint","최대 2,000자 · 같은 서버에 1분 간격 · 작성 후 30분 동안 수정 가능"}
        if let Some(latest)=original {if base().as_ref().is_some_and(|old|old.revision!=latest.revision){section{class:"membership-notice",h3{"먼저 저장된 내용"}p{class:"community-body","{latest.body}"}p{"아래 버튼은 작성 중인 내용을 최신 내용으로 바꿉니다."}button{r#type:"button",class:"secondary-button",disabled:pending(),onclick:move |_|{body.set(latest.body.clone());base.set(Some(latest.clone()));message.set(String::new());},"입력을 버리고 최신 내용 불러오기"}}}}
        if !message().is_empty(){p{role:"status",class:"membership-notice","{message}"}}
        button{r#type:"submit",class:"primary-button",disabled:!ready||pending()||!can_save,if pending(){"저장 중…"}else if base().is_some(){"수정 저장"}else{"댓글 작성"}}
    }}
}
#[component]
fn ReportForm(
    domain: String,
    comment: String,
    mut pending: Signal<bool>,
    can_save: bool,
    on_saved: EventHandler<()>,
) -> Element {
    let mut reason = use_signal(|| Reason::Spam);
    let mut detail = use_signal(String::new);
    let mut message = use_signal(String::new);
    let target = comment.clone();
    rsx! {form{class:"membership-card membership-form community-form",onsubmit:move|e|{
        e.prevent_default();if !can_save||pending(){return;}pending.set(true);let domain=domain.clone();let comment=target.clone();
        spawn(async move{match api::report(domain,comment,reason(),detail()).await{Ok(())=>on_saved.call(()),Err(e)=>message.set(error_message(e))}pending.set(false);});
    },h3{"댓글 신고"}p{class:"membership-hint","신고 사유와 설명은 공개되지 않습니다. 이 사이트의 댓글을 신고하는 기능이에요."}
        label{r#for:"reason-{comment}","신고 사유"}select{id:"reason-{comment}",value:reason().key(),disabled:pending(),onchange:move|e|{if let Some(value)=Reason::parse(&e.value()){reason.set(value);}},for item in [Reason::Spam,Reason::Abuse,Reason::Offtopic,Reason::Other]{option{value:item.key(),selected:item==reason(),"{item.label()}"}}}
        label{r#for:"report-detail-{comment}","신고 설명 · 선택"}textarea{id:"report-detail-{comment}",rows:3,maxlength:500,value:detail,disabled:pending(),oninput:move|e|detail.set(e.value())}
        if !message().is_empty(){p{role:"status",class:"membership-notice","{message}"}}
        button{r#type:"submit",class:"primary-button",disabled:pending()||!can_save,"신고 접수"}
    }}
}
fn date_label(value: &str) -> String {
    value.get(..16).unwrap_or(value).replace('T', " ") + " UTC"
}
