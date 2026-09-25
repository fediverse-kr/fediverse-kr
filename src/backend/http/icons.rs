use super::*;
const PREFIX: &str = "/api/public/server-icon/";
pub(super) async fn response(method: Method, path: String) -> Option<Response> {
    let domain = path.strip_prefix(PREFIX)?;
    let mut response = if method != Method::GET && method != Method::HEAD {
        let mut r = StatusCode::METHOD_NOT_ALLOWED.into_response();
        r.headers_mut()
            .insert(header::ALLOW, HeaderValue::from_static("GET, HEAD"));
        r
    } else if super::super::directory::canonical_domain(domain).as_deref() != Ok(domain) {
        StatusCode::BAD_REQUEST.into_response()
    } else if std::env::var_os("FEDKR_DATABASE_URL").is_none() {
        StatusCode::NOT_FOUND.into_response()
    } else {
        match super::super::config::state().await {
            Ok(state) => match state.db.public_site_icon(domain).await {
                Ok(Some(icon)) => {
                    let length = icon.bytes.len();
                    let mut r = if method == Method::HEAD {
                        StatusCode::OK.into_response()
                    } else {
                        (StatusCode::OK, icon.bytes).into_response()
                    };
                    r.headers_mut()
                        .insert(header::CONTENT_TYPE, HeaderValue::from_static(icon.mime));
                    if let Ok(v) = HeaderValue::from_str(&length.to_string()) {
                        r.headers_mut().insert(header::CONTENT_LENGTH, v);
                    }
                    r
                }
                Ok(None) => super::media::render(
                    &method,
                    crate::backend::media::read(
                        &state.db,
                        state.media.as_ref(),
                        &crate::backend::media::Target::Site(domain.into()),
                        None,
                    )
                    .await,
                ),
                Err(_) => StatusCode::SERVICE_UNAVAILABLE.into_response(),
            },
            Err(_) => StatusCode::SERVICE_UNAVAILABLE.into_response(),
        }
    };
    // Do not retain a readable cache after hiding or redirect to remote storage.
    let headers = response.headers_mut();
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static("default-src 'none'; sandbox"),
    );
    headers.insert(
        "cross-origin-resource-policy",
        HeaderValue::from_static("same-origin"),
    );
    super::media::secure(&mut response, false);
    Some(response)
}
