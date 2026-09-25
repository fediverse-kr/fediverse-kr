use super::{api, merge, EditableSoftware, Kinds, SoftwareEdit};
use crate::{
    membership::{api as member_api, pages::error_message, AccountState},
    Route,
};
use dioxus::prelude::*;

#[component]
pub fn SoftwareNew() -> Element {
    rsx! {EditorPage{name:String::new()}}
}
#[component]
pub fn SoftwareEditor(name: String) -> Element {
    rsx! {EditorPage{name}}
}
#[component]
pub(crate) fn EditorPage(name: String) -> Element {
    rsx! {
        document::Title{"소프트웨어 등록·수정 — fediverse.kr"}
        main{id:"content",class:"membership-page wrap catalog-editor",
            Link{class:"back-link",to:Route::Platforms { filters: Default::default() },"소프트웨어 목록"}
            header{class:"membership-heading",h1{if name.is_empty(){"소프트웨어 등록"}else{"소프트웨어 정보 수정"}}p{"틀린 내용은 고치고, 새로운 도구는 함께 소개해요."}}
            EditorBody{name,administrator:false}
        }
    }
}

// Each page owns its landmarks; both reuse the loader and keyed mutation form.
#[component]
pub(crate) fn EditorBody(name: String, administrator: bool) -> Element {
    let account = use_server_future(member_api::session)?;
    let kinds = use_server_future(api::kinds)?;
    let current = use_server_future(use_reactive(
        (&name, &administrator),
        |(name, administrator)| async move {
            if administrator {
                crate::moderation::catalog::api::current(name).await
            } else if name.is_empty() {
                Ok(EditableSoftware {
                    name,
                    revision: 0,
                    locked: false,
                    edit: SoftwareEdit::default(),
                })
            } else {
                api::current(name).await
            }
        },
    ))?;
    rsx! {
        match (account(),kinds(),current()) {
            (Some(Ok(account)),Some(Ok(kinds)),Some(Ok(current))) if current.name==name=>rsx!{EditForm{key:"{name}-{administrator}",account,kinds,current,administrator}},
            (Some(Err(e)),_,_)|(_,Some(Err(e)),_)|(_,_,Some(Err(e)))=>load_error(e),
            _=>rsx!{p{role:"status","편집할 정보를 불러오는 중…"}},
        }
    }
}
#[component]
fn AccessNote(account: AccountState) -> Element {
    rsx! {
        if !account.federation_available {p{class:"membership-notice","검토용 화면입니다. 입력은 시험할 수 있지만 저장되지 않습니다."}}
        else if account.member.is_none(){p{class:"membership-notice","누구나 읽고, 로그인한 회원은 직접 등록·수정할 수 있어요."}Link{class:"secondary-button",to:Route::Login{},"로그인"}}
    }
}
#[component]
fn EditForm(
    account: AccountState,
    kinds: Kinds,
    current: EditableSoftware,
    administrator: bool,
) -> Element {
    let initial = current.clone();
    let mut base = use_signal(move || initial);
    let mut draft = use_signal(|| current.edit.clone());
    let mut slug = use_signal(String::new);
    let mut features = use_signal(|| current.edit.features.join("\n"));
    let mut summary = use_signal(String::new);
    let mut message = use_signal(String::new);
    let mut pending = use_signal(|| false);
    let mut latest = use_signal(|| None::<EditableSoftware>);
    let mut conflicts = use_signal(Vec::<String>::new);
    let mut confirmed = use_signal(|| false);
    // Router reuse must not carry one product's draft into another. Changes to
    // the same record do not silently overwrite an unsaved draft.
    use_effect(use_reactive((&current,), move |(current,)| {
        if base.peek().name != current.name {
            draft.set(current.edit.clone());
            features.set(current.edit.features.join("\n"));
            base.set(current);
            slug.set(String::new());
            summary.set(String::new());
            message.set(String::new());
            latest.set(None);
            conflicts.set(vec![]);
            confirmed.set(false);
            pending.set(false);
        }
    }));
    let ready = use_context::<Signal<bool>>()();
    let can_save = ready
        && account.member.is_some()
        && account.federation_available
        && (!base().locked || administrator);
    let locked = base().locked && !administrator;
    let creating = base().name.is_empty();
    let nav = use_navigator();
    let mut options = kinds.items;
    for value in &draft().categories {
        if !options.iter().any(|o| &o.name == value) {
            options.push(crate::directory::Category {
                name: value.clone(),
                label: format!("{value} · 기존 분류"),
                emoji: String::new(),
            });
        }
    }
    rsx! {
        AccessNote{account}
        if administrator {p{class:"membership-notice","관리자 편집입니다. 잠금은 유지되며, 본문과 변경 요약은 공개됩니다. 저장에는 최근 15분 이내 인증이 필요해요."}}
        if base().locked {p{class:"membership-notice",role:"status","이 항목은 현재 편집이 잠겨 있어요."}}
        form{class:"membership-card membership-form",onsubmit:move|e|{
            e.prevent_default();if pending() || !can_save || (!conflicts().is_empty() && !confirmed()){return;}
            pending.set(true);message.set(String::new());
            let mut edit=draft();edit.features=feature_lines(&features());let record=base();
            spawn(async move{
                let result=if administrator{crate::moderation::catalog::api::save(record.name.clone(),record.revision,edit,summary()).await}else if record.name.is_empty(){api::create(slug(),edit,summary()).await}else{api::save(record.name.clone(),record.revision,edit,summary()).await};
                if base.peek().name!=record.name {return;}
                pending.set(false);
                match result {Ok(saved)=>{nav.push(Route::SoftwareDetail{name:saved.name});},Err(e)=>message.set(error_message(e))}
            });
        },
            p{class:"membership-hint","저장하면 바로 공개되고 변경 이력에 남아요. 개인정보는 넣지 마세요."}
            div{class:"membership-field",label{r#for:"software-slug","식별자"}
                if creating {input{id:"software-slug",value:slug,required:true,maxlength:64,pattern:r"[a-zA-Z0-9][a-zA-Z0-9_\-]*",placeholder:"example-social",disabled:pending() || !ready,oninput:move|e|slug.set(e.value())}p{class:"membership-hint","주소에 쓰는 영문 소문자·숫자·하이픈·밑줄입니다. 등록 후에는 바꾸지 않아요."}}
                else {input{id:"software-slug",value:base().name,readonly:true}}
            }
            for (field,label,max,rows) in [("display_name","이름",200,1),("description","소개",4000,5),("family","계열 · 선택",128,1),("website_url","프로젝트 웹사이트 · 선택",2048,1),("tech_stack","사용 기술 · 선택",1000,3)] {
                div{class:"membership-field",label{r#for:"software-{field}","{label}"}
                    if rows==1 {input{id:"software-{field}",r#type:if field=="website_url" {"url"}else{"text"},value:field_value(&draft(),field),maxlength:max,required:field=="display_name",disabled:pending() || !ready || locked,oninput:move|e|set_field(&mut draft.write(),field,e.value())}}
                    else {textarea{id:"software-{field}",value:field_value(&draft(),field),rows,maxlength:max,disabled:pending() || !ready || locked,oninput:move|e|set_field(&mut draft.write(),field,e.value())}}
                }
            }
            fieldset{class:"catalog-kinds",disabled:pending() || !ready || locked,legend{"종류 · 하나 이상"}
                for kind in options {label{class:"owner-checkbox",input{r#type:"checkbox",checked:draft().categories.contains(&kind.name),onchange:move|e|{let mut edit=draft.write();if e.checked(){if !edit.categories.contains(&kind.name){edit.categories.push(kind.name.clone());}}else{edit.categories.retain(|v|v!=&kind.name);}}}"{kind.emoji} {kind.label}"}}
                if kinds.truncated {p{class:"membership-hint","분류가 많아 앞의 200개만 표시합니다. 기존 분류는 유지할 수 있어요."}}
            }
            div{class:"membership-field",label{r#for:"software-features","주요 기능 · 한 줄에 하나, 최대 32개"}textarea{id:"software-features",value:features,rows:5,maxlength:9632,disabled:pending() || !ready || locked,oninput:move|e|features.set(e.value())}}
            div{class:"membership-field",label{r#for:"software-summary","변경 요약"}input{id:"software-summary",value:summary,required:true,maxlength:200,placeholder:"어떤 내용을 추가하거나 바로잡았나요?",disabled:pending() || !ready || locked,oninput:move|e|summary.set(e.value())}}
            if !message().is_empty(){p{class:"membership-notice",role:"status","{message}"}}
            if !creating {
                button{r#type:"button",class:"secondary-button",disabled:pending() || !ready,onclick:move|_|{
                    if pending(){return;}pending.set(true);let name=base().name;
                    spawn(async move{let result=api::current(name.clone()).await;if base.peek().name!=name{return;}match result{Ok(value)=>latest.set(Some(value)),Err(e)=>message.set(error_message(e))}pending.set(false);});
                },"최신 내용과 비교"}
                if let Some(remote)=latest(){
                    section{class:"catalog-comparison",aria_label:"최신 내용 비교",
                        h2{"서버의 최신 내용 · 버전 {remote.revision}"}ReadOnlyEdit{edit:remote.edit.clone()}
                        button{r#type:"button",class:"secondary-button",disabled:pending() || !ready || remote.revision==base().revision,onclick:move|_|{
                            let mut mine=draft();mine.features=feature_lines(&features());
                            let (merged,overlap)=merge(&base().edit,&mine,&remote.edit);
                            features.set(merged.features.join("\n"));draft.set(merged);base.set(remote.clone());conflicts.set(overlap);confirmed.set(false);
                            message.set("겹치지 않은 수정은 합쳤어요. 입력란과 아래 최신 내용을 확인해 주세요.".into());
                        },"최신 변경을 합쳐서 계속 편집"}
                    }
                }
            }
            if !conflicts().is_empty(){p{class:"membership-notice",{format!("서로 다르게 수정한 항목: {}. 입력란에는 내가 쓴 내용을 남겼어요.",conflicts().join(", "))}}label{class:"owner-checkbox",input{r#type:"checkbox",checked:confirmed(),disabled:pending(),onchange:move|e|confirmed.set(e.checked())}"겹친 항목을 비교했으며 현재 입력으로 저장합니다."}}
            div{class:"membership-actions",button{class:"primary-button",r#type:"submit",disabled:pending() || !can_save || (!conflicts().is_empty() && !confirmed()),if pending(){"처리 중…"}else if creating{"등록하기"}else{"변경 저장"}}
                if !creating {Link{class:"secondary-button",to:Route::SoftwareHistory{name:base().name},"변경 이력"}}
            }
        }
    }
}
#[component]
pub fn SoftwareHistory(name: String) -> Element {
    let mut page = use_signal(|| 0u32);
    let mut selected = use_signal(|| None::<(i64, SoftwareEdit)>);
    let mut message = use_signal(String::new);
    let mut pending = use_signal(|| false);
    let mut identity = use_signal(|| name.clone());
    let result = use_server_future(use_reactive((&name,), move |(name,)| {
        api::history(name, page())
    }))?;
    use_effect(use_reactive((&name,), move |(name,)| {
        identity.set(name);
        pending.set(false);
        page.set(0);
        selected.set(None);
        message.set(String::new());
    }));
    let ready = use_context::<Signal<bool>>()();
    rsx! {
        document::Title{"소프트웨어 변경 이력 — fediverse.kr"}
        main{id:"content",class:"membership-page wrap catalog-editor",
            Link{class:"back-link",to:Route::SoftwareDetail{name:name.clone()},"소프트웨어 소개"}
            header{class:"membership-heading",h1{"변경 이력"}p{"이전 내용을 읽거나, 확인한 버전으로 되돌릴 수 있어요."}}
            match result(){
                Some(Ok(data)) if data.name==name=>rsx!{
                    if data.entries.is_empty(){p{class:"membership-notice","아직 변경 이력이 없어요."}}
                    ol{class:"catalog-history",for entry in data.entries{
                        li{div{strong{"버전 {entry.revision} · {action_label(&entry.action)}"}time{datetime:entry.created_at.clone(),"{entry.created_at}"}p{"{entry.summary}"}}
                            button{class:"secondary-button",disabled:pending() || !ready,onclick:{let name=name.clone();move|_|{if pending(){return;}pending.set(true);let name=name.clone();spawn(async move{let result=api::read_revision(name.clone(),entry.revision).await;if *identity.peek()!=name{return;}match result{Ok(edit)=>selected.set(Some((entry.revision,edit))),Err(e)=>message.set(error_message(e))}pending.set(false);});}},"버전 {entry.revision} 보기"}
                        }
                    }}
                    nav{class:"directory-pagination",aria_label:"변경 이력 페이지",button{class:"secondary-button",disabled:!ready || page()==0,onclick:move|_|page.set(page().saturating_sub(1)),"이전"}span{"{data.page+1}페이지"}button{class:"secondary-button",disabled:!ready || !data.has_next,onclick:move|_|page.set(page()+1),"다음"}}
                    if let Some((revision,edit))=selected(){section{class:"membership-card",h2{"버전 {revision}"}ReadOnlyEdit{edit}if !data.locked && revision!=data.current_revision {Link{class:"secondary-button",to:Route::SoftwareRestore{name:name.clone(),revision},"이 버전으로 복원 검토"}}}}
                },
                Some(Err(e))=>load_error(e),
                _=>rsx!{p{role:"status","변경 이력을 불러오는 중…"}},
            }
            if !message().is_empty(){p{role:"status","{message}"}}
        }
    }
}
#[component]
pub fn SoftwareRestore(name: String, revision: i64) -> Element {
    let account = use_server_future(member_api::session)?;
    let mut current = use_server_future(use_reactive((&name,), |(name,)| api::current(name)))?;
    let source = use_server_future(use_reactive(
        (&name, &revision),
        |(name, revision)| async move {
            api::read_revision(name.clone(), revision)
                .await
                .map(|edit| (name, revision, edit))
        },
    ))?;
    let mut summary = use_signal(String::new);
    let mut confirmed = use_signal(|| false);
    let mut pending = use_signal(|| false);
    let mut message = use_signal(String::new);
    let ready = use_context::<Signal<bool>>()();
    let nav = use_navigator();
    use_effect(use_reactive((&name, &revision), move |_| {
        confirmed.set(false);
        summary.set(String::new());
        message.set(String::new());
    }));
    rsx! {
        document::Title{"소프트웨어 버전 복원 — fediverse.kr"}
        main{id:"content",class:"membership-page wrap catalog-editor",
            Link{class:"back-link",to:Route::SoftwareHistory{name:name.clone()},"변경 이력"}
            header{class:"membership-heading",h1{"버전 {revision} 복원"}p{"소개 내용만 되돌립니다. 이전 변경 이력은 지우지 않아요."}}
            match (account(),current(),source()) {
                (Some(Ok(account)),Some(Ok(current_data)),Some(Ok((source_name,source_revision,source)))) if source_name==name && source_revision==revision && current_data.name==name=>{
                    let can_save=ready && account.member.is_some() && account.federation_available && !current_data.locked && current_data.revision!=revision;
                    rsx!{
                        AccessNote{account}
                        if current_data.locked {p{class:"membership-notice","이 항목은 현재 편집이 잠겨 있어요."}}
                        div{class:"catalog-restore-grid",section{class:"membership-card",h2{"현재 · 버전 {current_data.revision}"}ReadOnlyEdit{edit:current_data.edit}}section{class:"membership-card",h2{"복원할 버전 {revision}"}ReadOnlyEdit{edit:source}}}
                        form{class:"membership-card membership-form",onsubmit:{let name=name.clone();move|e|{
                            e.prevent_default();if pending() || !can_save || !confirmed(){return;}pending.set(true);message.set(String::new());let name=name.clone();
                            spawn(async move{let result=api::restore(name,current_data.revision,revision,summary()).await;pending.set(false);match result{Ok(saved)=>{nav.push(Route::SoftwareDetail{name:saved.name});},Err(e)=>message.set(error_message(e))}});
                        }},
                            label{r#for:"restore-summary","복원 이유"}input{id:"restore-summary",required:true,maxlength:200,value:summary,disabled:!ready || pending(),oninput:move|e|summary.set(e.value())}
                            label{class:"owner-checkbox",input{r#type:"checkbox",checked:confirmed(),disabled:!can_save || pending(),onchange:move|e|confirmed.set(e.checked())}"두 내용을 비교했고 이 버전으로 복원합니다."}
                            if !message().is_empty(){p{class:"membership-notice",role:"status","{message}"}}
                            div{class:"membership-actions",button{class:"primary-button",r#type:"submit",disabled:!can_save || !confirmed() || pending(),"확인한 버전으로 복원"}button{class:"secondary-button",r#type:"button",disabled:!ready || pending(),onclick:move|_|{confirmed.set(false);current.restart();},"최신 내용 다시 비교"}}
                        }
                    }
                },
                (Some(Err(e)),_,_)|(_,Some(Err(e)),_)|(_,_,Some(Err(e)))=>load_error(e),
                _=>rsx!{p{role:"status","비교할 내용을 불러오는 중…"}},
            }
        }
    }
}
#[component]
fn ReadOnlyEdit(edit: SoftwareEdit) -> Element {
    rsx! {dl{class:"catalog-snapshot",
        for (label,value) in [("이름",edit.display_name),("소개",edit.description),("계열",edit.family),("종류",edit.categories.join(" · ")),("기능",edit.features.join("\n")),("웹사이트",edit.website_url),("기술",edit.tech_stack)]{
            dt{"{label}"}dd{if value.is_empty(){"—"}else{"{value}"}}
        }
    }}
}
fn action_label(value: &str) -> &str {
    match value {
        "baseline" => "기존 내용",
        "create" => "등록",
        "restore" => "복원",
        "admin_edit" => "관리자 수정",
        "admin_settings" => "관리 설정",
        _ => "수정",
    }
}
fn load_error(error: ServerFnError) -> Element {
    let status = match &error {
        ServerFnError::ServerError { code, .. } if (400..500).contains(code) => *code as u16,
        _ => 503,
    };
    crate::portal::response_status(status);
    rsx! {p{role:"alert",{error_message(error)}}}
}
fn feature_lines(value: &str) -> Vec<String> {
    value
        .lines()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_owned)
        .collect()
}
fn field_value(edit: &SoftwareEdit, field: &str) -> String {
    match field {
        "display_name" => &edit.display_name,
        "family" => &edit.family,
        "description" => &edit.description,
        "website_url" => &edit.website_url,
        _ => &edit.tech_stack,
    }
    .clone()
}
fn set_field(edit: &mut SoftwareEdit, field: &str, value: String) {
    *match field {
        "display_name" => &mut edit.display_name,
        "family" => &mut edit.family,
        "description" => &mut edit.description,
        "website_url" => &mut edit.website_url,
        _ => &mut edit.tech_stack,
    } = value;
}

#[cfg(test)]
mod editor_shell_tests {
    use super::*;
    use dioxus_history::{History, MemoryHistory};
    use dioxus_router::components::HistoryProvider;
    use std::rc::Rc;

    #[derive(Clone, PartialEq)]
    struct Case {
        creating: bool,
        locked: bool,
        administrator: bool,
    }

    #[derive(Clone, Debug, PartialEq, Routable)]
    enum TestRoute {
        #[route("/")]
        FormPage {},
    }

    #[component]
    fn TestApp(case: Case) -> Element {
        use_context_provider(|| case);
        use_context_provider(|| Signal::new(true));
        rsx! { HistoryProvider {
            history: move |_| Rc::new(MemoryHistory::with_initial_path("/")) as Rc<dyn History>,
            Router::<TestRoute> {}
        } }
    }

    #[component]
    fn FormPage() -> Element {
        let case = use_context::<Case>();
        rsx! { EditForm {
            account: AccountState {
                member: Some(crate::membership::Member {
                    id: "test-member".into(), login_id: None, display_name: "테스트 회원".into(),
                }),
                linked_accounts: vec![], federation_available: true,
            },
            kinds: Kinds { items: vec![], truncated: false },
            current: EditableSoftware {
                name: if case.creating { String::new() } else { "example-tool".into() },
                revision: 7, locked: case.locked,
                edit: SoftwareEdit { display_name: "예제 도구".into(), description: "보존할 소개".into(), ..Default::default() },
            },
            administrator: case.administrator,
        } }
    }

    fn render(case: Case) -> String {
        let mut dom = VirtualDom::new_with_props(TestApp, TestAppProps { case });
        dom.rebuild_in_place();
        dioxus::ssr::render(&dom)
    }

    fn field<'a>(html: &'a str, id: &str) -> &'a str {
        let start = html.find(&format!("id=\"{id}\"")).unwrap();
        html[start..].split('>').next().unwrap()
    }

    #[test]
    fn ordinary_create_and_edit_keep_form_contract_after_body_extraction() {
        let create = render(Case {
            creating: true,
            locked: false,
            administrator: false,
        });
        assert!(create.contains("등록하기"));
        assert!(!field(&create, "software-slug").contains("readonly"));
        assert!(!field(&create, "software-description").contains("disabled"));
        let edit = render(Case {
            creating: false,
            locked: false,
            administrator: false,
        });
        assert!(field(&edit, "software-slug").contains("readonly"));
        assert!(edit.contains("보존할 소개"));
        assert!(edit.contains("최신 내용과 비교"));
        assert!(edit.contains("변경 저장"));
        assert!(edit.contains("/software/example-tool/history"));
    }

    #[test]
    fn extracted_admin_form_has_no_landmarks_and_does_not_unlock_public_editor() {
        let member = render(Case {
            creating: false,
            locked: true,
            administrator: false,
        });
        assert!(field(&member, "software-description").contains("disabled"));
        let admin = render(Case {
            creating: false,
            locked: true,
            administrator: true,
        });
        assert!(!field(&admin, "software-description").contains("disabled"));
        assert!(admin.contains("관리자 편집입니다"));
        assert!(admin.contains("이 항목은 현재 편집이 잠겨 있어요"));
        assert!(!admin.contains("<main"));
        assert!(!admin.contains("<h1"));
        assert!(!admin.contains("<nav"));
    }
}
