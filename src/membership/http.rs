//! Gate untrusted bodies before Dioxus buffers/deserializes server arguments.
use super::api::server::{error, unavailable, validate_write_limit};
use dioxus::fullstack::{
    axum::{
        body::{to_bytes, Body},
        extract::Request,
        middleware::Next,
        response::{IntoResponse, Response},
    },
    http::{header, HeaderValue, Method},
};
use std::time::Duration;
use tokio::sync::Semaphore;

static REQUESTS: Semaphore = Semaphore::const_new(64);
const MAX_BODY: usize = 8192;

fn no_store(mut response: Response) -> Response {
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    response
        .headers_mut()
        .append(header::VARY, HeaderValue::from_static("Cookie"));
    response
}
async fn prepare(request: Request, origin: &str) -> Result<Request, Response> {
    let limit = match request.uri().path() {
        "/api/member/sites/edit" => 32768,
        "/api/member/moderation/software/logo" => crate::moderation::catalog::LOGO_BODY_LIMIT,
        "/api/member/software/create"
        | "/api/member/software/save"
        | "/api/member/moderation/software/save" => 65536,
        "/api/member/comments/create" | "/api/member/comments/edit" => 16384,
        _ => MAX_BODY,
    };
    validate_write_limit(request.headers(), origin, limit as u64).map_err(|e| e.into_response())?;
    let (parts, body) = request.into_parts();
    let bytes = match tokio::time::timeout(Duration::from_secs(5), to_bytes(body, limit)).await {
        Ok(Ok(bytes)) => bytes,
        Ok(Err(_)) => return Err(error(413, "요청 내용이 너무 깁니다.").into_response()),
        Err(_) => {
            return Err(
                error(408, "요청을 받는 시간이 지났습니다. 다시 시도해 주세요.").into_response(),
            )
        }
    };
    Ok(Request::from_parts(parts, Body::from(bytes)))
}

fn clear_browser_binding(mut response: Response, secure: bool) -> Response {
    if response.status().is_success() {
        response.headers_mut().append(
            header::SET_COOKIE,
            HeaderValue::from_static(if secure {
                "__Host-fedkr_browser=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0; Secure"
            } else {
                "fedkr_browser=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0"
            }),
        );
    }
    response
}

pub async fn guard(request: Request, next: Next) -> Response {
    let path = request.uri().path();
    let member_api = path.starts_with("/api/member/");
    let ends_auth = matches!(
        path,
        "/api/member/logout" | "/api/member/linked/remove" | "/api/member/withdraw"
    );
    let private =
        member_api || matches!(path, "/login" | "/account") || path.starts_with("/account/");
    if !member_api || request.method() != Method::POST {
        let response = next.run(request).await;
        return if private {
            no_store(response)
        } else {
            response
        };
    }
    let Ok(_permit) = REQUESTS.try_acquire() else {
        return no_store(
            error(429, "요청이 많습니다. 잠시 후 다시 시도해 주세요.").into_response(),
        );
    };
    let Ok(config) = crate::backend::settings::Config::from_env() else {
        return no_store(unavailable().into_response());
    };
    match prepare(request, &config.origin).await {
        Ok(request) => {
            let response = next.run(request).await;
            no_store(if ends_auth {
                clear_browser_binding(response, config.secure)
            } else {
                response
            })
        }
        Err(response) => no_store(response),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ending_auth_appends_instead_of_overwriting_session_cookie() {
        let mut response = Response::new(Body::empty());
        response.headers_mut().append(
            header::SET_COOKIE,
            HeaderValue::from_static("fedkr_session=; Max-Age=0"),
        );
        let response = clear_browser_binding(response, false);
        assert_eq!(
            response
                .headers()
                .get_all(header::SET_COOKIE)
                .iter()
                .count(),
            2
        );
        let response = clear_browser_binding(error(401, "unauthenticated").into_response(), true);
        assert!(!response.headers().contains_key(header::SET_COOKIE));
    }
    fn request(body: Vec<u8>, origin: &str) -> Request {
        Request::builder()
            .method(Method::POST)
            .uri("/api/member/logout")
            .header("origin", origin)
            .body(Body::from(body))
            .unwrap()
    }
    #[tokio::test]
    async fn actual_body_limit_does_not_depend_on_content_length() {
        assert!(prepare(
            request(vec![b'a'; MAX_BODY], "https://fediverse.kr"),
            "https://fediverse.kr"
        )
        .await
        .is_ok());
        let too_large = prepare(
            request(vec![b'a'; MAX_BODY + 1], "https://fediverse.kr"),
            "https://fediverse.kr",
        )
        .await
        .unwrap_err();
        assert_eq!(too_large.status(), 413);
    }
    #[tokio::test]
    async fn origin_is_checked_before_reading_or_deserializing_the_body() {
        let denied = prepare(
            request(vec![b'a'; MAX_BODY + 1], "https://evil.example"),
            "https://fediverse.kr",
        )
        .await
        .unwrap_err();
        assert_eq!(denied.status(), 403);
    }
    #[tokio::test]
    async fn only_named_writes_get_their_larger_body_limit() {
        for (path, limit) in [
            (
                "/api/member/moderation/software/logo",
                crate::moderation::catalog::LOGO_BODY_LIMIT,
            ),
            ("/api/member/software/create", 65536),
            ("/api/member/software/save", 65536),
            ("/api/member/software/restore", 8192),
            ("/api/member/sites/edit", 32768),
            ("/api/member/comments/create", 16384),
            ("/api/member/comments/edit", 16384),
            ("/api/member/comments/delete", 8192),
            ("/api/member/comments/report", 8192),
            ("/api/member/moderation/site/delete", 8192),
        ] {
            for (size, expected) in [(limit, 200), (limit + 1, 413)] {
                let mut req = request(vec![b'a'; size], "https://fediverse.kr");
                *req.uri_mut() = path.parse().unwrap();
                let status = match prepare(req, "https://fediverse.kr").await {
                    Ok(_) => 200,
                    Err(r) => r.status().as_u16(),
                };
                assert_eq!(status, expected, "{path}");
            }
        }
    }
}
