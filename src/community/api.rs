use super::{Access, Comment, Reason, ReplyPage, Thread, ThreadPage};
#[cfg(feature = "server")]
use crate::{
    backend::{community as service, config},
    membership::api::server::*,
};
#[cfg(feature = "server")]
use dioxus::fullstack::HeaderMap;
use dioxus::prelude::*;
#[cfg(feature = "server")]
fn failure(e: service::Error) -> ServerFnError {
    use service::Error::*;
    if let Auth(e) = e {
        return auth_error(e);
    }
    error(
        match e {
            Invalid => 400,
            Missing => 404,
            Conflict | DuplicateReport => 409,
            TooLate => 422,
            TooSoon | ReportRate => 429,
            _ => 503,
        },
        &e.to_string(),
    )
}
#[cfg(feature = "server")]
fn no_store() {
    if let Some(ctx) = dioxus::fullstack::FullstackContext::current() {
        ctx.add_response_header(
            dioxus::fullstack::http::header::CACHE_CONTROL,
            dioxus::fullstack::HeaderValue::from_static("no-store"),
        );
    }
}
#[cfg(feature = "server")]
async fn auth(
    state: &config::State,
    headers: &HeaderMap,
) -> Result<crate::backend::auth::AuthenticatedSession, ServerFnError> {
    session_details(state, headers)
        .await?
        .ok_or_else(|| auth_error(crate::backend::auth::AuthError::Unauthenticated))
}

#[get("/api/member/own-comments?page", headers:HeaderMap)]
pub async fn own_comments(page: u32) -> Result<super::OwnCommentPage, ServerFnError> {
    private_response();
    let state = config::state().await.map_err(|_| unavailable())?;
    let session = auth(state, &headers).await?;
    if page > 10_000 {
        return Err(error(400, "페이지 번호를 확인해 주세요."));
    }
    state.db.own_comments(&session, page).await.map_err(failure)
}

#[get("/api/public/comments?domain&page")]
pub async fn threads(domain: String, page: u32) -> Result<ThreadPage, ServerFnError> {
    no_store();
    service::page(page).map_err(failure)?;
    if std::env::var_os("FEDKR_DATABASE_URL").is_none() {
        if crate::directory::api::server(domain.clone())
            .await?
            .site
            .is_none()
        {
            return Err(failure(service::Error::Missing));
        }
        return Ok(preview(domain, page));
    }
    let domain = service::domain(&domain).map_err(failure)?;
    config::state()
        .await
        .map_err(|_| unavailable())?
        .db
        .comment_threads(&domain, page)
        .await
        .map_err(failure)
}
#[get("/api/public/comment-replies?domain&parent&page")]
pub async fn replies(
    domain: String,
    parent: String,
    page: u32,
) -> Result<ReplyPage, ServerFnError> {
    no_store();
    let parent_id = service::id(&parent).map_err(failure)?;
    service::page(page).map_err(failure)?;
    if std::env::var_os("FEDKR_DATABASE_URL").is_none() {
        let current = threads(domain.clone(), 0).await?;
        let row = current
            .threads
            .into_iter()
            .find(|t| t.comment.id == parent)
            .ok_or_else(|| failure(service::Error::Missing))?;
        return Ok(ReplyPage {
            domain,
            parent,
            page,
            has_next: false,
            replies: if page == 0 { row.replies } else { vec![] },
        });
    }
    let domain = service::domain(&domain).map_err(failure)?;
    config::state()
        .await
        .map_err(|_| unavailable())?
        .db
        .comment_replies(&domain, parent_id, page)
        .await
        .map_err(failure)
}
#[get("/api/member/comment-access?domain&ids",headers:HeaderMap)]
pub async fn access(domain: String, ids: String) -> Result<Access, ServerFnError> {
    private_response();
    if ids.len() > 3699 {
        return Err(failure(service::Error::Invalid));
    }
    let ids = if ids.is_empty() {
        vec![]
    } else {
        ids.split(',')
            .map(service::id)
            .collect::<Result<Vec<_>, _>>()
            .map_err(failure)?
    };
    if ids.len() > 100 {
        return Err(failure(service::Error::Invalid));
    }
    if std::env::var_os("FEDKR_DATABASE_URL").is_none() {
        return Ok(Access {
            display_name: None,
            available: false,
            permissions: vec![],
        });
    }
    let domain = service::domain(&domain).map_err(failure)?;
    let state = config::state().await.map_err(|_| unavailable())?;
    let Some(session) = session_details(state, &headers).await? else {
        return Ok(Access {
            display_name: None,
            available: true,
            permissions: vec![],
        });
    };
    state
        .db
        .comment_access(&session, &domain, &ids)
        .await
        .map_err(failure)
}
#[post("/api/member/comments/create",headers:HeaderMap)]
pub async fn create(
    domain: String,
    parent: Option<String>,
    body: String,
) -> Result<Comment, ServerFnError> {
    let state = write_state_limit(&headers, 16384).await?;
    let session = auth(state, &headers).await?;
    let parent = parent
        .as_deref()
        .map(service::id)
        .transpose()
        .map_err(failure)?;
    state
        .db
        .create_comment(&session, &domain, parent, body)
        .await
        .map_err(failure)
}
#[post("/api/member/comments/edit",headers:HeaderMap)]
pub async fn edit(
    domain: String,
    id: String,
    revision: String,
    body: String,
) -> Result<Comment, ServerFnError> {
    let state = write_state_limit(&headers, 16384).await?;
    let session = auth(state, &headers).await?;
    state
        .db
        .change_comment(
            &session,
            &domain,
            service::id(&id).map_err(failure)?,
            service::revision(&revision).map_err(failure)?,
            Some(body),
        )
        .await
        .map_err(failure)
}
#[post("/api/member/comments/delete",headers:HeaderMap)]
pub async fn delete(
    domain: String,
    id: String,
    revision: String,
) -> Result<Comment, ServerFnError> {
    let state = write_state(&headers).await?;
    let session = auth(state, &headers).await?;
    state
        .db
        .change_comment(
            &session,
            &domain,
            service::id(&id).map_err(failure)?,
            service::revision(&revision).map_err(failure)?,
            None,
        )
        .await
        .map_err(failure)
}
#[post("/api/member/comments/report",headers:HeaderMap)]
pub async fn report(
    domain: String,
    id: String,
    reason: Reason,
    detail: String,
) -> Result<(), ServerFnError> {
    let state = write_state(&headers).await?;
    let session = auth(state, &headers).await?;
    state
        .db
        .report_comment(
            &session,
            &domain,
            service::id(&id).map_err(failure)?,
            reason,
            detail,
        )
        .await
        .map_err(failure)
}
#[get("/api/public/comment-changes?domain")]
pub async fn wait_for_changes(domain: String) -> Result<(), ServerFnError> {
    no_store();
    let domain = service::domain(&domain).map_err(failure)?;
    static READERS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(128);
    let _permit = READERS
        .try_acquire()
        .map_err(|_| error(429, "댓글 연결이 많아요. 잠시 후 다시 확인합니다."))?;
    if std::env::var_os("FEDKR_DATABASE_URL").is_none() {
        return Err(failure(service::Error::Unavailable));
    }
    // Subscribe before the visibility check. No connection/transaction is held
    // while waiting. Timeout also catches another replica or external changes.
    let mut receiver = service::changes::subscribe();
    let state = config::state().await.map_err(|_| unavailable())?;
    let site = state.db.comment_site(&domain).await.map_err(failure)?;
    let _ = tokio::time::timeout(std::time::Duration::from_secs(20), async {
        loop {
            match receiver.recv().await {
                Ok(id) if id == site => break,
                Ok(_) => {}
                Err(_) => break,
            }
        }
    })
    .await;
    state.db.comment_site(&domain).await.map_err(failure)?;
    Ok(())
}
#[cfg(feature = "server")]
fn preview(domain: String, page: u32) -> ThreadPage {
    let first = Comment {
        id: "eeeeeeee-0000-4000-8000-000000000001".into(),
        parent_id: None,
        author_name: "초롱".into(),
        body: "가입하기 전에 규칙을 읽어봤어요. 처음 온 사람도 질문할 수 있나요?".into(),
        deleted: false,
        truncated: false,
        created_at: "2026-09-13T10:00:00Z".into(),
        updated_at: "2026-09-13T10:00:00Z".into(),
        revision: "2026-09-13T10:00:00.000000".into(),
    };
    let reply = Comment {
        id: "eeeeeeee-0000-4000-8000-000000000002".into(),
        parent_id: Some(first.id.clone()),
        author_name: "구름".into(),
        body: "저도 처음엔 궁금한 게 많았어요. 서버 안내에 적힌 연락처로 물어봤어요.".into(),
        created_at: "2026-09-13T10:01:00Z".into(),
        updated_at: "2026-09-13T10:01:00Z".into(),
        revision: "2026-09-13T10:01:00.000000".into(),
        ..first.clone()
    };
    ThreadPage {
        domain,
        preview: true,
        page,
        has_next: false,
        threads: if page == 0 {
            vec![Thread {
                comment: first,
                replies: vec![reply],
                reply_count: 1,
            }]
        } else {
            vec![]
        },
    }
}
