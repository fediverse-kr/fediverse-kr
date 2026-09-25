//! The browser supplies neither a member ID nor a trusted verification result.
use super::{AccountState, Challenge, Member};
#[cfg(feature = "server")]
use dioxus::fullstack::HeaderMap;
use dioxus::prelude::*;
#[cfg(feature = "server")]
use server::*;

#[get("/api/member/profile-media",headers:HeaderMap)]
pub async fn profile_media() -> Result<super::ProfileMedia, ServerFnError> {
    private_response();
    let state = crate::backend::config::state()
        .await
        .map_err(|_| unavailable())?;
    state
        .db
        .own_profile_media(&required_token(state, &headers)?)
        .await
        .map_err(auth_error)
}

#[post("/api/member/profile-media/refresh", headers: HeaderMap)]
pub async fn refresh_profile_media(account_id: String) -> Result<u32, ServerFnError> {
    let state = write_state(&headers).await?;
    let session = session_details(state, &headers)
        .await?
        .ok_or_else(|| auth_error(crate::backend::auth::AuthError::Unauthenticated))?;
    crate::backend::profile_refresh::refresh(state, session, account_id_value(&account_id)?)
        .await
        .map_err(|e| {
            use crate::backend::profile_refresh::Error;
            match e {
                Error::Auth(e) => auth_error(e),
                Error::Busy => error(429, &e.to_string()),
                Error::Superseded | Error::ActorChanged => error(409, &e.to_string()),
                Error::Network => error(502, &e.to_string()),
                Error::Unavailable => error(503, &e.to_string()),
            }
        })
}

#[get("/api/member/session", headers: HeaderMap)]
pub async fn session() -> Result<AccountState, ServerFnError> {
    private_response();
    let Ok(state) = crate::backend::config::state().await else {
        return Ok(AccountState {
            member: None,
            linked_accounts: vec![],
            federation_available: false,
        });
    };
    let Some(token) = cookie(&headers, session_name(state.config.secure)) else {
        return Ok(AccountState {
            member: None,
            linked_accounts: vec![],
            federation_available: true,
        });
    };
    let member = crate::backend::auth::get_session(&state.db, &token)
        .await
        .map_err(auth_error)?;
    let linked_accounts = if let Some(member) = &member {
        crate::backend::flow::list_linked_accounts(&state.db, member.id)
            .await
            .map_err(flow_error)?
            .into_iter()
            .map(|row| super::LinkedAccount {
                id: row.id.to_string(),
                handle: row.handle,
                profile_url: row.profile_url,
                display_name: row.display_name,
                is_public: row.is_public,
            })
            .collect()
    } else {
        vec![]
    };
    Ok(AccountState {
        member: member.map(member_dto),
        linked_accounts,
        federation_available: true,
    })
}

#[post("/api/member/federated/begin", headers: HeaderMap)]
pub async fn begin_federated(handle: String) -> Result<Challenge, ServerFnError> {
    let state = write_state(&headers).await?;
    if handle.len() > 512 {
        return Err(error(400, "@이름@서버 주소를 확인해 주세요."));
    }
    let _permit = network_slot()?;
    let nonce = if let Some(nonce) = cookie(&headers, browser_name(state.config.secure)) {
        nonce
    } else {
        random_secret()?
    };
    // A fresh challenge must have its full lifetime even if this browser's
    // existing binding cookie was about to expire. Keep the binding itself.
    set_cookie(
        browser_name(state.config.secure),
        &nonce,
        3600,
        state.config.secure,
    )?;
    crate::backend::auth::consume_attempt(&state.db, "challenge_begin", nonce.as_bytes(), 12)
        .await
        .map_err(auth_error)?;
    crate::backend::auth::consume_attempt(&state.db, "challenge_begin", b"network:global", 120)
        .await
        .map_err(auth_error)?;
    let session = session_details(state, &headers).await?;
    let actor = crate::backend::identity::client_with_signer(state.signer.clone())
        .resolve_account(&handle)
        .await
        .map_err(federation_error)?;
    let issued = crate::backend::flow::begin_challenge(&state.db, &actor, &nonce, session.as_ref())
        .await
        .map_err(flow_error)?;
    Ok(Challenge {
        id: issued.id.to_string(),
        code: issued.code,
        handle: issued.handle,
        expires_at: issued.expires_at.to_rfc3339(),
    })
}

#[post("/api/member/federated/finish", headers: HeaderMap)]
pub async fn finish_federated(challenge_id: String, code: String) -> Result<Member, ServerFnError> {
    let state = write_state(&headers).await?;
    let _permit = network_slot()?;
    let id = uuid::Uuid::parse_str(&challenge_id)
        .map_err(|_| error(400, "인증 요청을 다시 시작해 주세요."))?;
    let nonce = cookie(&headers, browser_name(state.config.secure))
        .ok_or_else(|| error(400, "인증을 시작한 브라우저에서 다시 시도해 주세요."))?;
    let session = session_details(state, &headers).await?;
    let pending = crate::backend::flow::get_pending(&state.db, id, &nonce, &code, session.as_ref())
        .await
        .map_err(flow_error)?;
    crate::backend::auth::consume_attempt(&state.db, "challenge_verify", b"network:global", 180)
        .await
        .map_err(auth_error)?;
    let proof = crate::backend::identity::client_with_signer(state.signer.clone())
        .verify_public_post(&pending.actor, &code, pending.created_at)
        .await
        .map_err(federation_error)?;
    // A revoked cookie may remain in another browser after a password change.
    // An anonymous login challenge must not inherit that stale session token.
    // Link challenges still require get_pending's exact active session binding.
    let token = session
        .as_ref()
        .and_then(|_| cookie(&headers, session_name(state.config.secure)));
    let grant = crate::backend::flow::complete_challenge(
        &state.db,
        id,
        &nonce,
        &code,
        token.as_deref(),
        &proof,
    )
    .await
    .map_err(flow_error)?;
    crate::backend::profile_refresh::after_login(state, &grant.token, proof.actor()).await;
    accept_grant(state, grant)
}

#[post("/api/member/password/login", headers: HeaderMap)]
pub async fn password_login(login_id: String, password: String) -> Result<Member, ServerFnError> {
    let state = write_state(&headers).await?;
    crate::backend::auth::consume_attempt(&state.db, "login", b"hash:global", 120)
        .await
        .map_err(auth_error)?;
    let grant = crate::backend::auth::login(&state.db, &login_id, &password)
        .await
        .map_err(auth_error)?;
    // A successful account switch must not leave this browser's old session valid.
    if let Some(old) = cookie(&headers, session_name(state.config.secure)) {
        crate::backend::auth::revoke_session(&state.db, &old)
            .await
            .map_err(auth_error)?;
    }
    accept_grant(state, grant)
}

#[post("/api/member/logout", headers: HeaderMap)]
pub async fn logout() -> Result<(), ServerFnError> {
    let state = write_state(&headers).await?;
    if let Some(token) = cookie(&headers, session_name(state.config.secure)) {
        crate::backend::auth::revoke_session(&state.db, &token)
            .await
            .map_err(auth_error)?;
    }
    set_cookie(
        session_name(state.config.secure),
        "",
        0,
        state.config.secure,
    )
}

#[post("/api/member/credentials", headers: HeaderMap)]
pub async fn set_credentials(login_id: String, password: String) -> Result<Member, ServerFnError> {
    let state = write_state(&headers).await?;
    let token = cookie(&headers, session_name(state.config.secure))
        .ok_or_else(|| auth_error(crate::backend::auth::AuthError::Unauthenticated))?;
    let grant =
        crate::backend::auth::set_local_credentials(&state.db, &token, &login_id, &password)
            .await
            .map_err(auth_error)?;
    accept_grant(state, grant)
}

#[post("/api/member/password/change", headers: HeaderMap)]
pub async fn change_password(
    current_password: String,
    new_password: String,
) -> Result<Member, ServerFnError> {
    let state = write_state(&headers).await?;
    let token = cookie(&headers, session_name(state.config.secure))
        .ok_or_else(|| auth_error(crate::backend::auth::AuthError::Unauthenticated))?;
    let grant =
        crate::backend::auth::change_password(&state.db, &token, &current_password, &new_password)
            .await
            .map_err(auth_error)?;
    accept_grant(state, grant)
}

#[post("/api/member/name", headers: HeaderMap)]
pub async fn update_name(name: String) -> Result<Member, ServerFnError> {
    let state = write_state(&headers).await?;
    let member = crate::backend::auth::update_display_name(
        &state.db,
        &required_token(state, &headers)?,
        &name,
    )
    .await
    .map_err(auth_error)?;
    Ok(member_dto(member))
}

#[post("/api/member/linked/visibility", headers: HeaderMap)]
pub async fn set_link_visibility(account_id: String, public: bool) -> Result<(), ServerFnError> {
    let state = write_state(&headers).await?;
    crate::backend::auth::set_link_visibility(
        &state.db,
        &required_token(state, &headers)?,
        account_id_value(&account_id)?,
        public,
    )
    .await
    .map_err(auth_error)
}

#[post("/api/member/linked/remove", headers: HeaderMap)]
pub async fn unlink_account(account_id: String) -> Result<(), ServerFnError> {
    let state = write_state(&headers).await?;
    crate::backend::auth::unlink_account(
        &state.db,
        &required_token(state, &headers)?,
        account_id_value(&account_id)?,
    )
    .await
    .map_err(auth_error)?;
    clear_auth_cookies(state)
}

#[post("/api/member/withdraw", headers: HeaderMap)]
pub async fn withdraw(confirmation: String) -> Result<(), ServerFnError> {
    let state = write_state(&headers).await?;
    crate::backend::auth::withdraw(&state.db, &required_token(state, &headers)?, &confirmation)
        .await
        .map_err(auth_error)?;
    clear_auth_cookies(state)
}

#[post("/api/member/password/recover", headers: HeaderMap)]
pub async fn recover_password(new_password: String) -> Result<Member, ServerFnError> {
    let state = write_state(&headers).await?;
    let grant = crate::backend::auth::recover_password(
        &state.db,
        &required_token(state, &headers)?,
        &new_password,
    )
    .await
    .map_err(auth_error)?;
    accept_grant(state, grant)
}

/// Public key discovery for the deliberately no-op ActivityPub inbox contract.
#[get("/actor")]
pub async fn instance_actor() -> Result<serde_json::Value, ServerFnError> {
    let state = crate::backend::config::state()
        .await
        .map_err(|_| unavailable())?;
    if let Some(ctx) = dioxus::fullstack::FullstackContext::current() {
        ctx.add_response_header(
            dioxus::fullstack::http::header::CONTENT_TYPE,
            dioxus::fullstack::HeaderValue::from_static("application/activity+json"),
        );
        ctx.add_response_header(
            dioxus::fullstack::http::header::CACHE_CONTROL,
            dioxus::fullstack::HeaderValue::from_static("public, max-age=300"),
        );
    }
    let public_key = state.signer.public_key_pem().map_err(federation_error)?;
    actor_document(&state.config.origin, &public_key)
}

#[cfg(feature = "server")]
fn actor_document(origin: &str, public_key: &str) -> Result<serde_json::Value, ServerFnError> {
    // Phoenix's preferredUsername was the configured endpoint host, not a
    // hard-coded product label. `Config` already admits only canonical origins.
    let preferred_username = url::Url::parse(origin)
        .ok()
        .and_then(|origin| origin.host_str().map(str::to_owned))
        .ok_or_else(unavailable)?;
    let id = format!("{origin}/actor");
    Ok(serde_json::json!({
        "@context": ["https://www.w3.org/ns/activitystreams", "https://w3id.org/security/v1"],
        "id": id, "type": "Application", "preferredUsername": preferred_username, "name": "fediverse.kr",
        "inbox": format!("{id}/inbox"),
        "publicKey": { "id": format!("{id}#main-key"), "owner": id,
            "publicKeyPem": public_key }
    }))
}

#[cfg(all(test, feature = "server"))]
mod actor_tests {
    use super::*;

    #[test]
    fn actor_document_keeps_the_origin_key_identity_and_advertises_its_inbox() {
        let actor = actor_document("https://relay.example:8443", "public-key").unwrap();
        assert_eq!(actor["id"], "https://relay.example:8443/actor");
        assert_eq!(actor["preferredUsername"], "relay.example");
        assert_eq!(actor["inbox"], "https://relay.example:8443/actor/inbox");
        assert_eq!(
            actor["publicKey"]["id"],
            "https://relay.example:8443/actor#main-key"
        );
        assert_eq!(
            actor["publicKey"]["owner"],
            "https://relay.example:8443/actor"
        );
        assert_eq!(actor["publicKey"]["publicKeyPem"], "public-key");
    }
}

#[cfg(feature = "server")]
pub(crate) mod server {
    use super::*;
    use crate::backend::{auth, config, federation::FederationError, flow::FlowError};
    use dioxus::fullstack::{FullstackContext, HeaderValue};
    use rand::RngCore;
    use tokio::sync::{Semaphore, SemaphorePermit};

    pub fn error(code: u16, message: &str) -> ServerFnError {
        ServerFnError::ServerError {
            message: message.into(),
            code,
            details: None,
        }
    }
    pub fn unavailable() -> ServerFnError {
        error(
            503,
            "회원 기능이 아직 준비되지 않았습니다. 잠시 후 다시 시도해 주세요.",
        )
    }
    pub fn auth_error(e: auth::AuthError) -> ServerFnError {
        let code = match e {
            auth::AuthError::Unauthenticated | auth::AuthError::InvalidCredentials => 401,
            auth::AuthError::RateLimited => 429,
            auth::AuthError::Unavailable => 503,
            _ => 400,
        };
        error(code, &e.to_string())
    }
    pub fn flow_error(e: FlowError) -> ServerFnError {
        let code = match e {
            FlowError::Unauthenticated => 401,
            FlowError::AlreadyLinked | FlowError::LegacyIdentity => 409,
            FlowError::RateLimited => 429,
            FlowError::Unavailable => 503,
            _ => 400,
        };
        error(code, &e.to_string())
    }
    pub fn federation_error(e: FederationError) -> ServerFnError {
        error(400, &e.to_string())
    }
    pub fn member_dto(m: auth::AuthenticatedMember) -> Member {
        Member {
            id: m.id.to_string(),
            login_id: m.login_id,
            display_name: m.display_name,
        }
    }
    pub fn required_token(
        state: &config::State,
        headers: &HeaderMap,
    ) -> Result<String, ServerFnError> {
        cookie(headers, session_name(state.config.secure))
            .ok_or_else(|| auth_error(auth::AuthError::Unauthenticated))
    }
    pub fn account_id_value(value: &str) -> Result<uuid::Uuid, ServerFnError> {
        uuid::Uuid::parse_str(value).map_err(|_| auth_error(auth::AuthError::AccountNotFound))
    }
    pub fn clear_auth_cookies(state: &config::State) -> Result<(), ServerFnError> {
        // Dioxus 0.7 inserts rather than appends response headers. The outer
        // HTTP guard appends the independent browser-binding cookie.
        set_cookie(
            session_name(state.config.secure),
            "",
            0,
            state.config.secure,
        )
    }
    pub fn private_response() {
        if let Some(ctx) = FullstackContext::current() {
            ctx.add_response_header(
                dioxus::fullstack::http::header::CACHE_CONTROL,
                HeaderValue::from_static("private, no-store"),
            );
            ctx.add_response_header(
                dioxus::fullstack::http::header::VARY,
                HeaderValue::from_static("Cookie"),
            );
            ctx.add_response_header(
                dioxus::fullstack::http::header::REFERRER_POLICY,
                HeaderValue::from_static("same-origin"),
            );
        }
    }
    pub fn validate_write(headers: &HeaderMap, origin: &str) -> Result<(), ServerFnError> {
        validate_write_limit(headers, origin, 8192)
    }
    pub fn validate_write_limit(
        headers: &HeaderMap,
        origin: &str,
        limit: u64,
    ) -> Result<(), ServerFnError> {
        if headers.get_all("origin").iter().count() != 1
            || headers.get("origin").and_then(|h| h.to_str().ok()) != Some(origin)
            || headers
                .get("sec-fetch-site")
                .is_some_and(|h| h != "same-origin" && h != "none")
        {
            return Err(error(
                403,
                "이 사이트에서 시작한 요청만 처리할 수 있습니다.",
            ));
        }
        if headers.get("content-length").is_some_and(|h| {
            h.to_str()
                .ok()
                .and_then(|s| s.parse::<u64>().ok())
                .is_none_or(|n| n > limit)
        }) {
            return Err(error(413, "요청 내용이 너무 깁니다."));
        }
        Ok(())
    }
    pub async fn write_state(headers: &HeaderMap) -> Result<&'static config::State, ServerFnError> {
        write_state_limit(headers, 8192).await
    }
    pub async fn write_state_limit(
        headers: &HeaderMap,
        limit: u64,
    ) -> Result<&'static config::State, ServerFnError> {
        private_response();
        // Validate origin even when the DB is unavailable. No Host/proxy fallback.
        let configured = config::Config::from_env().map_err(|_| unavailable())?;
        validate_write_limit(headers, &configured.origin, limit)?;
        config::state().await.map_err(|_| unavailable())
    }
    pub fn session_name(secure: bool) -> &'static str {
        if secure {
            "__Host-fedkr_session"
        } else {
            "fedkr_session"
        }
    }
    pub fn browser_name(secure: bool) -> &'static str {
        if secure {
            "__Host-fedkr_browser"
        } else {
            "fedkr_browser"
        }
    }
    pub fn cookie(headers: &HeaderMap, name: &str) -> Option<String> {
        let mut found = None;
        for header in headers.get_all("cookie") {
            for parsed in ::cookie::Cookie::split_parse(header.to_str().ok()?) {
                let Ok(parsed) = parsed else { continue };
                if parsed.name() != name {
                    continue;
                }
                let value = parsed.value();
                // The parser handles HTTP whitespace. Credential values still
                // require exactly 64 lowercase hex digits; duplicates fail closed.
                if found.is_some()
                    || value.len() != 64
                    || !value
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                {
                    return None;
                }
                found = Some(value.to_owned());
            }
        }
        found
    }
    pub fn random_secret() -> Result<String, ServerFnError> {
        let mut bytes = [0u8; 32];
        rand::rngs::OsRng
            .try_fill_bytes(&mut bytes)
            .map_err(|_| unavailable())?;
        Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
    }
    pub fn set_cookie(
        name: &str,
        value: &str,
        max_age: i64,
        secure: bool,
    ) -> Result<(), ServerFnError> {
        let header = ::cookie::Cookie::build((name, value))
            .path("/")
            .http_only(true)
            .same_site(::cookie::SameSite::Lax)
            .max_age(::cookie::time::Duration::seconds(max_age))
            .secure(secure)
            .build()
            .to_string();
        let header = HeaderValue::from_str(&header).map_err(|_| unavailable())?;
        FullstackContext::current()
            .ok_or_else(unavailable)?
            .add_response_header(dioxus::fullstack::http::header::SET_COOKIE, header);
        Ok(())
    }
    pub fn accept_grant(
        state: &config::State,
        grant: auth::SessionGrant,
    ) -> Result<Member, ServerFnError> {
        set_cookie(
            session_name(state.config.secure),
            &grant.token,
            (grant.expires_at - chrono::Utc::now()).num_seconds().max(0),
            state.config.secure,
        )?;
        Ok(member_dto(grant.member))
    }
    pub async fn session_details(
        state: &config::State,
        headers: &HeaderMap,
    ) -> Result<Option<auth::AuthenticatedSession>, ServerFnError> {
        match cookie(headers, session_name(state.config.secure)) {
            Some(token) => auth::get_session_details(&state.db, &token)
                .await
                .map_err(auth_error),
            None => Ok(None),
        }
    }
    static NETWORK: Semaphore = Semaphore::const_new(4);
    pub fn network_slot() -> Result<SemaphorePermit<'static>, ServerFnError> {
        NETWORK
            .try_acquire()
            .map_err(|_| error(429, "인증 요청이 많습니다. 잠시 후 다시 시도해 주세요."))
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn writes_require_exact_origin_and_reject_cross_site_or_large_payloads() {
            let mut headers = HeaderMap::new();
            assert!(validate_write(&headers, "https://fediverse.kr").is_err());
            for origin in [
                "null",
                "https://evil.example",
                "https://fediverse.kr.evil.example",
            ] {
                headers.insert("origin", origin.parse().unwrap());
                assert!(validate_write(&headers, "https://fediverse.kr").is_err());
            }
            headers.insert("origin", "https://fediverse.kr".parse().unwrap());
            assert!(validate_write(&headers, "https://fediverse.kr").is_ok());
            headers.insert("sec-fetch-site", "same-site".parse().unwrap());
            assert!(validate_write(&headers, "https://fediverse.kr").is_err());
            headers.remove("sec-fetch-site");
            headers.insert("content-length", "8193".parse().unwrap());
            assert!(validate_write(&headers, "https://fediverse.kr").is_err());
        }
        #[test]
        fn cookie_names_are_exact_and_duplicates_are_not_credentials() {
            let secret = "a".repeat(64);
            let mut headers = HeaderMap::new();
            headers.insert(
                "cookie",
                format!("other={secret}; fedkr_session={secret}")
                    .parse()
                    .unwrap(),
            );
            assert_eq!(cookie(&headers, "fedkr_session"), Some(secret.clone()));
            headers.append("cookie", format!("fedkr_session={secret}").parse().unwrap());
            assert!(cookie(&headers, "fedkr_session").is_none());
            headers.clear();
            headers.insert("cookie", "fedkr_session=short".parse().unwrap());
            assert!(cookie(&headers, "fedkr_session").is_none());
            assert_eq!(session_name(true), "__Host-fedkr_session");
            headers.clear();
            headers.insert(
                "cookie",
                format!("ignored=%3D; fedkr_session={secret}; empty=")
                    .parse()
                    .unwrap(),
            );
            assert_eq!(cookie(&headers, "fedkr_session"), Some(secret.clone()));
            headers.insert(
                "cookie",
                format!("fedkr_session={secret}; fedkr_session={secret}")
                    .parse()
                    .unwrap(),
            );
            assert!(cookie(&headers, "fedkr_session").is_none());
            headers.insert(
                "cookie",
                format!("fedkr_session=%61{}", &secret[1..])
                    .parse()
                    .unwrap(),
            );
            assert!(cookie(&headers, "fedkr_session").is_none());
        }
    }
}
