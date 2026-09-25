//! Local membership and opaque DB-backed sessions. HTTP cookies, CSRF, Origin,
//! request/IP limits and DTO serialization belong at the transport boundary.
//! No production migrations or external account claims are made in this module.

use super::db::{Database, StoreError};
use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Algorithm, Argon2, Params, Version,
};
use chrono::{DateTime, Duration, Utc};
use rand::{rngs::OsRng, RngCore};
use sha2::{Digest, Sha256};
use std::{
    fmt,
    sync::{Arc, OnceLock},
};
use tokio::sync::Semaphore;
use uuid::Uuid;

const PASSWORD_MIN_CHARS: usize = 15;
const PASSWORD_MAX_CHARS: usize = 128;
const PASSWORD_MAX_BYTES: usize = 512;
const ARGON_MEMORY_KIB: u32 = 19_456;
const ARGON_ITERATIONS: u32 = 2;
const ARGON_LANES: u32 = 1;
const SESSION_DAYS: i64 = 30;
const LOGIN_ATTEMPTS: i32 = 12;
const PASSWORD_CHANGE_ATTEMPTS: i32 = 8;

/// Deliberately safe and source-free. SQL errors and submitted secrets must not
/// be copied into responses or tracing fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthError {
    InvalidLoginId,
    InvalidDisplayName,
    InvalidPassword,
    InvalidCredentials,
    LoginIdTaken,
    CredentialAlreadySet,
    FreshAuthenticationRequired,
    FederatedAuthenticationRequired,
    LastLoginMethod,
    AccountNotFound,
    ConfirmationRequired,
    RateLimited,
    Unauthenticated,
    Unavailable,
}

impl fmt::Display for AuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidLoginId => "아이디는 영문·숫자·밑줄 3~32자로 입력해 주세요.",
            Self::InvalidDisplayName => "표시 이름은 1~64자로 입력해 주세요.",
            Self::InvalidPassword => "비밀번호는 공백을 포함해 15~128자로 입력해 주세요.",
            Self::InvalidCredentials => "아이디 또는 비밀번호를 확인해 주세요.",
            Self::LoginIdTaken => "사용할 수 없는 아이디입니다.",
            Self::CredentialAlreadySet => "이미 아이디와 비밀번호를 등록했습니다.",
            Self::FreshAuthenticationRequired => {
                "안전을 위해 다시 로그인한 뒤 15분 안에 시도해 주세요."
            }
            Self::FederatedAuthenticationRequired => {
                "이미 연결된 연합 계정으로 다시 인증한 뒤 15분 안에 시도해 주세요."
            }
            Self::LastLoginMethod => {
                "마지막 로그인 수단입니다. 다른 계정이나 ID와 암호를 먼저 등록해 주세요."
            }
            Self::AccountNotFound => "연결한 계정을 찾을 수 없습니다.",
            Self::ConfirmationRequired => "탈퇴 확인란에 ‘탈퇴’를 입력해 주세요.",
            Self::RateLimited => "시도가 많습니다. 잠시 후 다시 시도해 주세요.",
            Self::Unauthenticated => "다시 로그인해 주세요.",
            Self::Unavailable => "지금은 처리할 수 없습니다. 잠시 후 다시 시도해 주세요.",
        })
    }
}

impl std::error::Error for AuthError {}

impl From<StoreError> for AuthError {
    fn from(_: StoreError) -> Self {
        Self::Unavailable
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthenticatedMember {
    pub id: Uuid,
    pub login_id: Option<String>,
    pub display_name: String,
    pub created_at: DateTime<Utc>,
}

/// Never serialize this type into page props. The token belongs only in the
/// transport's HttpOnly cookie. Debug intentionally excludes its value.
pub struct SessionGrant {
    pub member: AuthenticatedMember,
    pub token: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct AuthenticatedSession {
    pub id: Uuid,
    pub member: AuthenticatedMember,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub authenticated_at: DateTime<Utc>,
    pub federated_authenticated_at: Option<DateTime<Utc>>,
}

impl fmt::Debug for SessionGrant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SessionGrant")
            .field("member", &self.member)
            .field("token", &"[REDACTED]")
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

// Intentionally no Debug / Serialize: this contains a password hash.
pub(crate) struct MemberCredentials {
    pub(crate) id: Uuid,
    pub(crate) login_id: Option<String>,
    pub(crate) display_name: String,
    pub(crate) password_hash: Option<String>,
    pub(crate) is_banned: bool,
    pub(crate) created_at: DateTime<Utc>,
}

impl MemberCredentials {
    pub(crate) fn member(&self) -> AuthenticatedMember {
        AuthenticatedMember {
            id: self.id,
            login_id: self.login_id.clone(),
            display_name: self.display_name.clone(),
            created_at: self.created_at,
        }
    }
}

pub fn normalize_login_id(value: &str) -> Result<String, AuthError> {
    if !(3..=32).contains(&value.len())
        || !value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_')
    {
        return Err(AuthError::InvalidLoginId);
    }
    Ok(value.to_ascii_lowercase())
}

pub(crate) fn normalize_display_name(value: &str) -> Result<String, AuthError> {
    let value = value.trim();
    if value.is_empty()
        || value.len() > 256
        || value.chars().count() > 64
        || value.chars().any(char::is_control)
    {
        return Err(AuthError::InvalidDisplayName);
    }
    Ok(value.to_owned())
}

fn valid_password(password: &str) -> bool {
    password.len() <= PASSWORD_MAX_BYTES
        && (PASSWORD_MIN_CHARS..=PASSWORD_MAX_CHARS).contains(&password.chars().count())
}

fn argon() -> Argon2<'static> {
    Argon2::new(
        Algorithm::Argon2id,
        Version::V0x13,
        Params::new(ARGON_MEMORY_KIB, ARGON_ITERATIONS, ARGON_LANES, Some(32))
            .expect("constant Argon2 parameters are valid"),
    )
}

fn hash_sync(password: &[u8]) -> Result<String, AuthError> {
    let salt = SaltString::generate(&mut OsRng);
    argon()
        .hash_password(password, &salt)
        .map(|hash| hash.to_string())
        .map_err(|_| AuthError::Unavailable)
}

fn hash_slots() -> Arc<Semaphore> {
    static SLOTS: OnceLock<Arc<Semaphore>> = OnceLock::new();
    SLOTS.get_or_init(|| Arc::new(Semaphore::new(2))).clone()
}

async fn hash_password(password: &str) -> Result<String, AuthError> {
    if !valid_password(password) {
        return Err(AuthError::InvalidPassword);
    }
    // Fail fast instead of building an unbounded queue of password-bearing tasks.
    let permit = hash_slots()
        .try_acquire_owned()
        .map_err(|_| AuthError::Unavailable)?;
    let password = password.to_owned();
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        hash_sync(password.as_bytes())
    })
    .await
    .map_err(|_| AuthError::Unavailable)?
}

fn acceptable_hash(hash: &PasswordHash<'_>) -> bool {
    // Only hashes produced by this version are accepted. In particular, a corrupt
    // PHC string must not request unbounded memory/CPU during verification.
    hash.algorithm.as_str() == "argon2id"
        && hash.version == Some(19)
        && hash.params.get_decimal("m") == Some(ARGON_MEMORY_KIB)
        && hash.params.get_decimal("t") == Some(ARGON_ITERATIONS)
        && hash.params.get_decimal("p") == Some(ARGON_LANES)
}

async fn verify_password(password: &str, stored_hash: Option<String>) -> Result<bool, AuthError> {
    if password.len() > PASSWORD_MAX_BYTES || password.chars().count() > PASSWORD_MAX_CHARS {
        return Ok(false);
    }
    let permit = hash_slots()
        .try_acquire_owned()
        .map_err(|_| AuthError::Unavailable)?;
    let password = password.to_owned();
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        static DUMMY: OnceLock<String> = OnceLock::new();
        // Both existing and absent accounts initialize the same dummy, and absent,
        // banned and malformed rows still perform a full password verification.
        let dummy = DUMMY.get_or_init(|| {
            hash_sync(b"fedkr dummy credential, never a member")
                .expect("constant Argon2 configuration is valid")
        });
        let parsed = stored_hash
            .as_deref()
            .and_then(|value| PasswordHash::new(value).ok())
            .filter(acceptable_hash);
        let known_hash = parsed.is_some();
        let parsed =
            parsed.unwrap_or_else(|| PasswordHash::new(dummy).expect("generated dummy hash"));
        let matches = argon()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok();
        known_hash && matches
    })
    .await
    .map_err(|_| AuthError::Unavailable)
}

fn new_token() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut token = String::with_capacity(64);
    for byte in bytes {
        token.push(HEX[(byte >> 4) as usize] as char);
        token.push(HEX[(byte & 15) as usize] as char);
    }
    token
}

pub(crate) fn token_hash(token: &str) -> Option<Vec<u8>> {
    if token.len() != 64
        || !token
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    {
        return None;
    }
    Some(Sha256::digest(token.as_bytes()).to_vec())
}

pub(crate) fn new_session(member: AuthenticatedMember) -> SessionGrant {
    SessionGrant {
        member,
        token: new_token(),
        expires_at: Utc::now() + Duration::days(SESSION_DAYS),
    }
}

/// Account-scoped attempts supplement HTTP/global limits. Secrets are hashed
/// before crossing the persistence boundary.
pub(crate) async fn consume_attempt(
    db: &Database,
    scope: &str,
    key: &[u8],
    limit: i32,
) -> Result<(), AuthError> {
    db.consume_attempt(scope, &Sha256::digest(key), limit).await
}

/// Optional credentials can only be attached to a fresh authenticated member.
pub async fn set_local_credentials(
    db: &Database,
    token: &str,
    login_id: &str,
    password: &str,
) -> Result<SessionGrant, AuthError> {
    let session = get_session_details(db, token)
        .await?
        .ok_or(AuthError::Unauthenticated)?;
    if Utc::now() - session.authenticated_at >= Duration::minutes(15) {
        return Err(AuthError::FreshAuthenticationRequired);
    }
    if session.member.login_id.is_some() {
        return Err(AuthError::CredentialAlreadySet);
    }
    let login = normalize_login_id(login_id)?;
    let hash = hash_password(password).await?;
    db.set_credentials(&session, &login, &hash).await
}

pub async fn login(
    db: &Database,
    login_id: &str,
    password: &str,
) -> Result<SessionGrant, AuthError> {
    let normalized = normalize_login_id(login_id).ok();
    consume_attempt(
        db,
        "login",
        normalized
            .as_deref()
            .unwrap_or("!invalid-login-id")
            .as_bytes(),
        LOGIN_ATTEMPTS,
    )
    .await?;
    let row = match normalized {
        Some(login) => db.credentials_by_login(&login).await?,
        None => None,
    };
    let valid =
        verify_password(password, row.as_ref().and_then(|r| r.password_hash.clone())).await?;
    let row = row
        .filter(|r| valid && !r.is_banned)
        .ok_or(AuthError::InvalidCredentials)?;
    // Hashing is outside the DB transaction. The repository rechecks the exact
    // credential snapshot under the member lock before issuing a session.
    db.login_session(&row).await
}

pub async fn get_session(
    db: &Database,
    token: &str,
) -> Result<Option<AuthenticatedMember>, AuthError> {
    Ok(get_session_details(db, token).await?.map(|s| s.member))
}
pub async fn get_session_details(
    db: &Database,
    token: &str,
) -> Result<Option<AuthenticatedSession>, AuthError> {
    let Some(hash) = token_hash(token) else {
        return Ok(None);
    };
    Ok(db.session(&hash).await?)
}
pub async fn revoke_session(db: &Database, token: &str) -> Result<(), AuthError> {
    if let Some(hash) = token_hash(token) {
        db.revoke(&hash).await?;
    }
    Ok(())
}
pub async fn change_password(
    db: &Database,
    token: &str,
    current_password: &str,
    new_password: &str,
) -> Result<SessionGrant, AuthError> {
    let hash = token_hash(token).ok_or(AuthError::Unauthenticated)?;
    let row = db
        .credentials_by_session(&hash)
        .await?
        .ok_or(AuthError::Unauthenticated)?;
    consume_attempt(
        db,
        "password_change",
        row.id.as_bytes(),
        PASSWORD_CHANGE_ATTEMPTS,
    )
    .await?;
    if !verify_password(current_password, row.password_hash.clone()).await? {
        return Err(AuthError::InvalidCredentials);
    }
    let next = hash_password(new_password).await?;
    db.change_credentials(&row, &hash, &next).await
}
pub async fn cleanup_expired(db: &Database) -> Result<u64, AuthError> {
    Ok(db.cleanup_members().await?)
}

pub async fn update_display_name(
    db: &Database,
    token: &str,
    name: &str,
) -> Result<AuthenticatedMember, AuthError> {
    let name = normalize_display_name(name)?;
    let session = get_session_details(db, token)
        .await?
        .ok_or(AuthError::Unauthenticated)?;
    db.update_member_name(&session, &name).await
}

pub async fn set_link_visibility(
    db: &Database,
    token: &str,
    account: Uuid,
    public: bool,
) -> Result<(), AuthError> {
    let session = get_session_details(db, token)
        .await?
        .ok_or(AuthError::Unauthenticated)?;
    db.set_link_visibility(&session, account, public).await
}

/// Logs out every browser, including the caller. A remaining login method must
/// be used next; rotating this session must not extend its authentication age.
pub async fn unlink_account(db: &Database, token: &str, account: Uuid) -> Result<(), AuthError> {
    let session = get_session_details(db, token)
        .await?
        .ok_or(AuthError::Unauthenticated)?;
    db.unlink_account(&session, account).await
}

pub async fn withdraw(db: &Database, token: &str, confirmation: &str) -> Result<(), AuthError> {
    if confirmation != "탈퇴" {
        return Err(AuthError::ConfirmationRequired);
    }
    let session = get_session_details(db, token)
        .await?
        .ok_or(AuthError::Unauthenticated)?;
    db.withdraw_member(&session).await
}

/// An old password is not required only after a fresh proof for an identity
/// already attached to this membership. Linking an unrelated account is not
/// reauthentication. Persistence rechecks this under the member/session locks.
pub async fn recover_password(
    db: &Database,
    token: &str,
    password: &str,
) -> Result<SessionGrant, AuthError> {
    let session = get_session_details(db, token)
        .await?
        .ok_or(AuthError::Unauthenticated)?;
    if session
        .federated_authenticated_at
        .is_none_or(|at| Utc::now() - at >= Duration::minutes(15))
    {
        return Err(AuthError::FederatedAuthenticationRequired);
    }
    if session.member.login_id.is_none() {
        return Err(AuthError::InvalidCredentials);
    }
    consume_attempt(
        db,
        "password_change",
        session.member.id.as_bytes(),
        PASSWORD_CHANGE_ATTEMPTS,
    )
    .await?;
    let hash = hash_password(password).await?;
    db.recover_credentials(&session, &hash).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::db::fixtures;

    #[test]
    fn login_id_is_canonical_ascii_not_an_external_identity() {
        assert_eq!(normalize_login_id("Shuna_01").unwrap(), "shuna_01");
        for invalid in [
            "ab",
            "슈나",
            " shuna",
            "shuna ",
            "shuna@dog.example",
            "a-b",
            "a.b",
            "аbc",
        ] {
            assert_eq!(normalize_login_id(invalid), Err(AuthError::InvalidLoginId));
        }
        assert!(normalize_login_id(&"a".repeat(32)).is_ok());
        assert!(normalize_login_id(&"a".repeat(33)).is_err());
    }

    #[test]
    fn password_limits_count_unicode_and_never_trim() {
        assert!(!valid_password(&"가".repeat(14)));
        assert!(valid_password(&"가".repeat(15)));
        assert!(valid_password(&"🦊".repeat(128)));
        assert!(!valid_password(&"🦊".repeat(129)));
        assert!(valid_password(" 1234567890123 "));
        assert!(!valid_password("1234567890123"));
        assert!(valid_password(&"a".repeat(128)));
        assert!(!valid_password(&"a".repeat(129)));
    }

    #[test]
    fn password_policy_is_argon2id_only_not_bcrypt_or_other_phc_algorithms() {
        assert!(PasswordHash::new("$2b$12$not-a-supported-password-hash").is_err());
        let hash = hash_sync(b"a password for algorithm policy").unwrap();
        assert!(hash.starts_with("$argon2id$v=19$"));
        assert!(!acceptable_hash(
            &PasswordHash::new(&hash.replace("argon2id", "argon2i")).unwrap()
        ));
        assert!(!acceptable_hash(
            &PasswordHash::new(&hash.replace("argon2id", "argon2d")).unwrap()
        ));
    }

    #[test]
    fn display_names_are_bounded_and_no_control_characters() {
        assert_eq!(normalize_display_name(" 슈나 ").unwrap(), "슈나");
        for invalid in ["", "   ", "슈\n나", "슈\0나"] {
            assert!(normalize_display_name(invalid).is_err());
        }
        assert!(normalize_display_name(&"a".repeat(65)).is_err());
    }

    #[test]
    fn session_tokens_have_256_bits_and_only_hashes_are_persistable() {
        let a = new_token();
        let b = new_token();
        assert_eq!(a.len(), 64);
        assert_ne!(a, b);
        assert_eq!(token_hash(&a).unwrap().len(), 32);
        assert_ne!(token_hash(&a), token_hash(&b));
        assert!(token_hash("").is_none());
        assert!(token_hash(&"z".repeat(64)).is_none());
        assert!(token_hash(&a.to_uppercase()).is_none());
    }

    #[tokio::test]
    async fn password_hashing_is_salted_bounded_and_preserves_exact_secret() {
        let password = " 가을밤 산책에 나가고 싶어요 ";
        let a = hash_password(password).await.unwrap();
        let b = hash_password(password).await.unwrap();
        assert_ne!(a, b);
        let parsed = PasswordHash::new(&a).unwrap();
        assert!(acceptable_hash(&parsed));
        assert!(verify_password(password, Some(a.clone())).await.unwrap());
        assert!(!verify_password(password.trim(), Some(a)).await.unwrap());
        assert!(!verify_password(password, None).await.unwrap());
        assert!(!verify_password(password, Some("invalid hash".to_owned()))
            .await
            .unwrap());
        let dangerous = b.replace("m=19456", "m=4294967295");
        assert!(!verify_password(password, Some(dangerous)).await.unwrap());
        assert_eq!(
            hash_password("too short").await,
            Err(AuthError::InvalidPassword)
        );
    }

    #[test]
    fn session_debug_is_redacted() {
        let grant = SessionGrant {
            member: AuthenticatedMember {
                id: Uuid::nil(),
                login_id: Some("test".into()),
                display_name: "테스트".into(),
                created_at: Utc::now(),
            },
            token: "sensitive-session-value".into(),
            expires_at: Utc::now(),
        };
        let debug = format!("{grant:?}");
        assert!(debug.contains("[REDACTED]"));
        assert!(!debug.contains("sensitive-session-value"));
    }

    /// Explicitly opt in with a disposable database. This ignored test applies
    /// the fixture migrations but never truncates or deletes existing users.
    #[tokio::test]
    #[ignore = "requires migrated disposable FEDKR_TEST_DATABASE_URL"]
    async fn postgres_member_session_lifecycle() {
        let pool = fixtures::database().await;
        let login_id = format!("test_{}", &Uuid::new_v4().simple().to_string()[..22]);
        let old = " a sufficiently long first password ";
        let new = " a sufficiently long second password ";
        // Fixture stands in for the separately tested verified-AP transaction.
        // No public backend function creates a member without federation proof.
        let proof_session = fixtures::member(&pool).await;
        assert!(proof_session.member.login_id.is_none());
        assert_eq!(
            set_local_credentials(&pool, "invalid token", &login_id, old)
                .await
                .unwrap_err(),
            AuthError::Unauthenticated
        );
        let registered =
            set_local_credentials(&pool, &proof_session.token, &login_id.to_uppercase(), old)
                .await
                .unwrap();
        let member_id = registered.member.id;
        assert_eq!(registered.member.login_id, Some(login_id.clone()));
        assert!(get_session(&pool, &proof_session.token)
            .await
            .unwrap()
            .is_none());
        assert_eq!(
            get_session(&pool, &registered.token)
                .await
                .unwrap()
                .unwrap()
                .id,
            member_id
        );
        let stored = fixtures::session_hash(&pool, member_id).await;
        assert_eq!(stored, token_hash(&registered.token).unwrap());
        assert_ne!(stored, registered.token.as_bytes());
        let another = fixtures::member(&pool).await;
        assert_eq!(
            set_local_credentials(&pool, &another.token, &login_id, old)
                .await
                .unwrap_err(),
            AuthError::LoginIdTaken
        );
        assert_eq!(
            set_local_credentials(&pool, &registered.token, "other_id", old)
                .await
                .unwrap_err(),
            AuthError::CredentialAlreadySet
        );
        let another_session = get_session_details(&pool, &another.token)
            .await
            .unwrap()
            .unwrap();
        fixtures::age_session(&pool, another_session.id).await;
        assert_eq!(
            set_local_credentials(&pool, &another.token, "unused_login", old)
                .await
                .unwrap_err(),
            AuthError::FreshAuthenticationRequired
        );
        assert_eq!(
            login(&pool, &login_id, "wrong password value")
                .await
                .unwrap_err(),
            AuthError::InvalidCredentials
        );
        assert_eq!(
            login(&pool, &format!("{login_id}_no"), old)
                .await
                .unwrap_err(),
            AuthError::InvalidCredentials
        );
        let second = login(&pool, &login_id, old).await.unwrap();
        assert_ne!(registered.token, second.token);
        assert_eq!(
            change_password(&pool, &second.token, "wrong password value", new)
                .await
                .unwrap_err(),
            AuthError::InvalidCredentials
        );
        let changed = change_password(&pool, &second.token, old, new)
            .await
            .unwrap();
        assert!(get_session(&pool, &registered.token)
            .await
            .unwrap()
            .is_none());
        assert!(get_session(&pool, &second.token).await.unwrap().is_none());
        assert!(get_session(&pool, &changed.token).await.unwrap().is_some());
        assert_eq!(
            login(&pool, &login_id, old).await.unwrap_err(),
            AuthError::InvalidCredentials
        );
        let valid = login(&pool, &login_id, new).await.unwrap();
        fixtures::ban(&pool, member_id, true).await;
        assert!(get_session(&pool, &valid.token).await.unwrap().is_none());
        assert_eq!(
            login(&pool, &login_id, new).await.unwrap_err(),
            AuthError::InvalidCredentials
        );
        fixtures::ban(&pool, member_id, false).await;
        revoke_session(&pool, &valid.token).await.unwrap();
        assert!(get_session(&pool, &valid.token).await.unwrap().is_none());
        // The rate-limit SQL remains atomic under concurrent attempts.
        let rate_key = format!("test-rate-{}", Uuid::new_v4());
        let mut jobs = Vec::new();
        for _ in 0..20 {
            let pool = pool.clone();
            let key = rate_key.clone();
            jobs.push(tokio::spawn(async move {
                consume_attempt(&pool, "login", key.as_bytes(), 5).await
            }));
        }
        let mut admitted = 0;
        for job in jobs {
            match job.await.unwrap() {
                Ok(()) => admitted += 1,
                Err(AuthError::RateLimited) => {}
                Err(error) => panic!("unexpected safe auth error: {error}"),
            }
        }
        assert_eq!(admitted, 5);
        // Cleanup exact UUID-owned fixture rows only, including their sessions.
        fixtures::delete_members(&pool, &[member_id, another.member.id]).await;
        let unknown_login = format!("{login_id}_no");
        for (scope, key) in [
            ("login", login_id.as_bytes()),
            ("login", unknown_login.as_bytes()),
            ("login", rate_key.as_bytes()),
            ("password_change", member_id.as_bytes().as_slice()),
        ] {
            fixtures::delete_rate(&pool, scope, &Sha256::digest(key)).await;
        }
    }
}
