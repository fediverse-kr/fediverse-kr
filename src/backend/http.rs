//! Small infrastructure endpoints and deliberate Phoenix URL compatibility.
mod icons;
mod media;
use dioxus::fullstack::{
    axum::{
        body::to_bytes,
        extract::Request,
        middleware::Next,
        response::{IntoResponse, Response},
    },
    http::{header, HeaderValue, Method, StatusCode},
};
use std::time::Duration;
use tokio::sync::Semaphore;

// Phoenix advertised this endpoint even though it intentionally did not retain
// or act on inbound activities. Keep its small compatibility surface without
// making an anonymous remote write path into an unbounded body sink.
const ACTOR_INBOX_BODY_LIMIT: usize = 64 * 1024;
static ACTOR_INBOX_READS: Semaphore = Semaphore::const_new(64);

fn legacy_destination(path: &str) -> Option<&'static str> {
    match path.trim_end_matches('/') {
        "/software" => Some("/platforms"),
        "/my" => Some("/account"),
        "/admin" => Some("/account/moderation"),
        "/admin/reports" => Some("/account/moderation/reports"),
        "/admin/servers" => Some("/account/moderation/sites"),
        "/auth" | "/auth/login" | "/auth/signup" | "/auth/verify" => Some("/login"),
        "/recommend" => Some("/servers"), // Recommendation algorithm is explicitly out of scope.
        _ => None,
    }
}

pub async fn guard(request: Request, next: Next) -> Response {
    if request.uri().path() == "/actor/inbox" {
        return actor_inbox_route(request).await;
    }
    if let Some(response) = media::response(
        request.method(),
        request.uri().path(),
        request.uri().query(),
        request.headers(),
    )
    .await
    {
        return response;
    }
    if let Some(response) =
        icons::response(request.method().clone(), request.uri().path().to_owned()).await
    {
        return response;
    }
    let safe_method = request.method() == Method::GET || request.method() == Method::HEAD;
    if safe_method {
        if let Some(destination) = legacy_destination(request.uri().path()) {
            // Do not forward legacy proof codes, identity handles or arbitrary return URLs.
            let mut response = StatusCode::PERMANENT_REDIRECT.into_response();
            response
                .headers_mut()
                .insert(header::LOCATION, HeaderValue::from_static(destination));
            response
                .headers_mut()
                .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
            return response;
        }
        if matches!(request.uri().path(), "/healthz" | "/readyz") {
            let ok = if request.uri().path() == "/healthz" {
                true
            } else if std::env::var_os("FEDKR_DATABASE_URL").is_none() {
                false
            } else {
                matches!(
                    tokio::time::timeout(std::time::Duration::from_secs(3), async {
                        super::config::state()
                            .await
                            .map_err(|_| ())?
                            .db
                            .ping()
                            .await
                            .map_err(|_| ())
                    })
                    .await,
                    Ok(Ok(()))
                )
            };
            let mut response = (
                if ok {
                    StatusCode::OK
                } else {
                    StatusCode::SERVICE_UNAVAILABLE
                },
                if request.method() == Method::HEAD {
                    ""
                } else if ok {
                    "ok\n"
                } else {
                    "not ready\n"
                },
            )
                .into_response();
            response
                .headers_mut()
                .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
            return response;
        }
    }
    next.run(request).await
}

fn is_json_content_type(value: Option<&HeaderValue>) -> bool {
    let Some(value) = value.and_then(|value| value.to_str().ok()) else {
        return false;
    };
    let media_type = value
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    matches!(
        media_type.as_str(),
        "application/json" | "application/ld+json" | "application/activity+json"
    ) || media_type.ends_with("+json")
}

async fn actor_inbox(request: Request) -> Response {
    actor_inbox_with_requests(request, &ACTOR_INBOX_READS).await
}

async fn actor_inbox_route(request: Request) -> Response {
    if request.method() != Method::POST {
        let mut response = inbox_response(StatusCode::METHOD_NOT_ALLOWED);
        response
            .headers_mut()
            .insert(header::ALLOW, HeaderValue::from_static("POST"));
        return response;
    }
    actor_inbox(request).await
}

fn inbox_response(status: StatusCode) -> Response {
    let mut response = status.into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

async fn actor_inbox_with_requests(request: Request, requests: &Semaphore) -> Response {
    // Reject before even inspecting a potentially slow or over-sized body.
    // This limit is independent of member-write CSRF protection because this
    // compatibility endpoint is anonymous and deliberately no-op.
    let Ok(_permit) = requests.try_acquire() else {
        return inbox_response(StatusCode::TOO_MANY_REQUESTS);
    };
    if !is_json_content_type(request.headers().get(header::CONTENT_TYPE)) {
        return inbox_response(StatusCode::UNSUPPORTED_MEDIA_TYPE);
    }
    let (_, body) = request.into_parts();
    match tokio::time::timeout(
        Duration::from_secs(5),
        to_bytes(body, ACTOR_INBOX_BODY_LIMIT),
    )
    .await
    {
        // Deliberately opaque: the Phoenix endpoint returned 202 without
        // parsing, retaining, logging, or delivering inbound activities.
        Ok(Ok(_)) => inbox_response(StatusCode::ACCEPTED),
        Ok(Err(_)) => inbox_response(StatusCode::PAYLOAD_TOO_LARGE),
        Err(_) => inbox_response(StatusCode::REQUEST_TIMEOUT),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dioxus::fullstack::axum::body::Body;

    fn inbox_request(content_type: &str, body: Vec<u8>) -> Request {
        Request::builder()
            .method(Method::POST)
            .uri("/actor/inbox")
            .header(header::CONTENT_TYPE, content_type)
            .body(Body::from(body))
            .unwrap()
    }
    #[test]
    fn compatibility_is_explicit_and_does_not_redirect_api_or_member_writes() {
        assert_eq!(
            legacy_destination("/admin/reports"),
            Some("/account/moderation/reports")
        );
        assert_eq!(
            legacy_destination("/admin/servers"),
            Some("/account/moderation/sites")
        );
        assert_eq!(legacy_destination("/software/"), Some("/platforms"));
        assert_eq!(legacy_destination("/recommend"), Some("/servers"));
        assert_eq!(legacy_destination("/my"), Some("/account"));
        assert_eq!(legacy_destination("/software/mastodon"), None);
        assert_eq!(legacy_destination("/auth/logout"), None);
        assert_eq!(legacy_destination("/actor"), None);
    }

    #[tokio::test]
    async fn actor_inbox_accepts_json_and_activity_json_without_browser_credentials() {
        for (content_type, body) in [
            ("application/json", br#"{"type":"Follow"}"#.to_vec()),
            (
                "application/activity+json",
                br#"{"@context":"https://www.w3.org/ns/activitystreams","type":"Create"}"#.to_vec(),
            ),
            (
                "application/ld+json; profile=\"https://www.w3.org/ns/activitystreams\"",
                br#"[{"type":"Undo"}]"#.to_vec(),
            ),
        ] {
            assert_eq!(
                actor_inbox(inbox_request(content_type, body))
                    .await
                    .status(),
                StatusCode::ACCEPTED,
                "{content_type}"
            );
        }
    }

    #[tokio::test]
    async fn actor_inbox_returns_an_empty_no_store_response_without_cookies() {
        let response = actor_inbox(inbox_request(
            "application/activity+json",
            br#"{"type":"Follow"}"#.to_vec(),
        ))
        .await;
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        assert_eq!(
            response.headers().get(header::CACHE_CONTROL).unwrap(),
            "no-store"
        );
        assert!(!response.headers().contains_key(header::SET_COOKIE));
        assert!(to_bytes(response.into_body(), 1).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn actor_inbox_exact_path_rejects_non_post_without_ssr_fallback() {
        let request = Request::builder()
            .method(Method::GET)
            .uri("/actor/inbox")
            .body(Body::empty())
            .unwrap();
        let response = actor_inbox_route(request).await;
        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(response.headers().get(header::ALLOW).unwrap(), "POST");
        assert_eq!(
            response.headers().get(header::CACHE_CONTROL).unwrap(),
            "no-store"
        );
        assert!(to_bytes(response.into_body(), 1).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn actor_inbox_rejects_non_json_and_actual_oversize_bodies() {
        assert_eq!(
            actor_inbox(inbox_request("text/plain", b"ignored".to_vec()))
                .await
                .status(),
            StatusCode::UNSUPPORTED_MEDIA_TYPE
        );
        assert_eq!(
            actor_inbox(inbox_request(
                "application/activity+json",
                vec![b'x'; ACTOR_INBOX_BODY_LIMIT + 1],
            ))
            .await
            .status(),
            StatusCode::PAYLOAD_TOO_LARGE
        );
    }

    #[tokio::test]
    async fn actor_inbox_rejects_at_capacity_without_reading_a_body() {
        let requests = Semaphore::new(1);
        let held = requests.try_acquire().unwrap();
        let response = actor_inbox_with_requests(
            inbox_request(
                "application/activity+json",
                vec![b'x'; ACTOR_INBOX_BODY_LIMIT + 1],
            ),
            &requests,
        )
        .await;
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(
            response.headers().get(header::CACHE_CONTROL).unwrap(),
            "no-store"
        );
        drop(held);
    }
}
