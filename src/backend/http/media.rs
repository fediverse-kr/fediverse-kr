use super::*;
use crate::backend::media::{self, Error, Image, Target};
use crate::membership::api::server::{cookie, session_name};
use dioxus::fullstack::HeaderMap;

pub(super) async fn response(
    method: &Method,
    path: &str,
    query: Option<&str>,
    headers: &HeaderMap,
) -> Option<Response> {
    let target = if let Some(name) = path.strip_prefix("/api/public/software-logo/") {
        media::path_component(name).map(Target::Software)
    } else if path == "/api/member/avatar" {
        Some(Target::OwnAvatar)
    } else if path == "/api/member/emoji" {
        query.and_then(media::emoji_query).map(Target::OwnEmoji)
    } else if let Some(name) = path.strip_prefix("/api/member/emoji/") {
        media::emoji_component(name).map(Target::OwnEmoji)
    } else {
        return None;
    };
    let private = path.starts_with("/api/member/");
    let mut response = if method != Method::GET && method != Method::HEAD {
        let mut r = StatusCode::METHOD_NOT_ALLOWED.into_response();
        r.headers_mut()
            .insert(header::ALLOW, HeaderValue::from_static("GET, HEAD"));
        r
    } else if target.is_none() {
        StatusCode::BAD_REQUEST.into_response()
    } else if private
        && headers
            .get("sec-fetch-site")
            .is_some_and(|v| v != "same-origin" && v != "none")
    {
        StatusCode::FORBIDDEN.into_response()
    } else if std::env::var_os("FEDKR_DATABASE_URL").is_none() {
        StatusCode::NOT_FOUND.into_response()
    } else {
        match crate::backend::config::state().await {
            Ok(state) => {
                let token = cookie(headers, session_name(state.config.secure));
                render(
                    method,
                    media::read(
                        &state.db,
                        state.media.as_ref(),
                        &target.unwrap(),
                        token.as_deref(),
                    )
                    .await,
                )
            }
            Err(_) => StatusCode::SERVICE_UNAVAILABLE.into_response(),
        }
    };
    secure(&mut response, private);
    Some(response)
}

pub(super) fn render(method: &Method, result: Result<Option<Image>, Error>) -> Response {
    match result {
        Ok(Some(image)) => {
            let len = image.bytes.len();
            let mut response = if method == Method::HEAD {
                StatusCode::OK.into_response()
            } else {
                (StatusCode::OK, image.bytes).into_response()
            };
            response
                .headers_mut()
                .insert(header::CONTENT_TYPE, HeaderValue::from_static(image.mime));
            response.headers_mut().insert(
                header::CONTENT_LENGTH,
                HeaderValue::from_str(&len.to_string()).expect("bounded length"),
            );
            response
        }
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(Error::Unauthorized) => StatusCode::UNAUTHORIZED.into_response(),
        Err(Error::Unsupported) => StatusCode::UNSUPPORTED_MEDIA_TYPE.into_response(),
        Err(Error::Unavailable | Error::Busy) => StatusCode::SERVICE_UNAVAILABLE.into_response(),
    }
}
pub(super) fn secure(response: &mut Response, private: bool) {
    let headers = response.headers_mut();
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static(if private {
            "private, no-store"
        } else {
            "no-store"
        }),
    );
    if private {
        headers.insert(header::VARY, HeaderValue::from_static("Cookie"));
    }
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(header::CONTENT_SECURITY_POLICY,HeaderValue::from_static("default-src 'none'; style-src 'unsafe-inline'; img-src data:; sandbox; frame-ancestors 'none'; base-uri 'none'; form-action 'none'"));
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    headers.insert(
        "cross-origin-resource-policy",
        HeaderValue::from_static("same-origin"),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn image_responses_are_bounded_sandboxed_and_uncacheable() {
        for method in [Method::GET, Method::HEAD] {
            let mut r = render(
                &method,
                Ok(Some(Image {
                    mime: "image/svg+xml",
                    bytes: b"<svg></svg>".to_vec(),
                })),
            );
            secure(&mut r, true);
            assert_eq!(r.status(), StatusCode::OK);
            assert_eq!(r.headers()[header::CONTENT_LENGTH], "11");
            assert_eq!(r.headers()[header::VARY], "Cookie");
            assert_eq!(r.headers()[header::CACHE_CONTROL], "private, no-store");
            let csp = r.headers()[header::CONTENT_SECURITY_POLICY]
                .to_str()
                .unwrap();
            assert!(csp.contains("sandbox") && csp.contains("default-src 'none'"));
            let body = dioxus::fullstack::axum::body::to_bytes(r.into_body(), 100)
                .await
                .unwrap();
            assert_eq!(body.len(), if method == Method::HEAD { 0 } else { 11 });
        }
        let r = response(&Method::POST, "/api/member/avatar", None, &HeaderMap::new())
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(
            response(
                &Method::GET,
                "/api/public/software-logo/a%2fb",
                None,
                &HeaderMap::new()
            )
            .await
            .unwrap()
            .status(),
            StatusCode::BAD_REQUEST
        );
        let mut cross = HeaderMap::new();
        cross.insert("sec-fetch-site", HeaderValue::from_static("same-site"));
        assert_eq!(
            response(&Method::GET, "/api/member/avatar", None, &cross)
                .await
                .unwrap()
                .status(),
            StatusCode::FORBIDDEN
        );
        let encoded = response(
            &Method::GET,
            "/api/member/emoji",
            Some("name=blob-cat.%40%2F%ED%95%9C%EA%B5%AD%EC%96%B4&v=7"),
            &cross,
        )
        .await
        .unwrap();
        assert_eq!(encoded.status(), StatusCode::FORBIDDEN);
        assert_eq!(
            encoded.headers()[header::CACHE_CONTROL],
            "private, no-store"
        );
        assert_eq!(
            encoded.headers()["cross-origin-resource-policy"],
            "same-origin"
        );
        assert_eq!(
            response(
                &Method::GET,
                "/api/member/emoji",
                Some("name=..&v=0"),
                &cross,
            )
            .await
            .unwrap()
            .status(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            response(&Method::GET, "/api/member/emoji/wave", None, &cross,)
                .await
                .unwrap()
                .status(),
            StatusCode::FORBIDDEN
        );
    }
}
