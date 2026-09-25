//! Owner verification is independent of member AP authentication. A member
//! proves control of DNS or a linked server operator account; no provider
//! password or client-supplied verdict is used.
pub mod api_verification;
use super::{
    auth::{self, AuthError, AuthenticatedSession},
    db::{Database, StoreError},
};
use crate::directory::management::{DnsChallenge, SiteEdit};
use rand::{rngs::OsRng, RngCore};
use sha2::{Digest, Sha256};
use std::{fmt, time::Duration};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Auth(AuthError),
    InvalidDomain,
    InvalidText,
    NotOwned,
    Conflict,
    InvalidChallenge,
    DnsUnavailable,
    DnsNotFound,
    ApiUnsupported,
    ApiNotOwner,
    ApiUnavailable,
    DnsPriority,
    RefreshTooSoon,
    Unavailable,
}
impl From<AuthError> for Error {
    fn from(e: AuthError) -> Self {
        Self::Auth(e)
    }
}
impl From<StoreError> for Error {
    fn from(_: StoreError) -> Self {
        Self::Unavailable
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Auth(e) => return e.fmt(f),
            Self::InvalidDomain => "https:// 없이 서버의 도메인만 입력해 주세요.",
            Self::InvalidText => "입력 길이와 태그 개수를 확인해 주세요.",
            Self::NotOwned => "관리할 수 있는 서버를 찾을 수 없습니다.",
            Self::Conflict => "다른 변경이 먼저 저장됐어요. 다시 불러온 뒤 수정해 주세요.",
            Self::InvalidChallenge => "인증 요청이 만료되었거나 바뀌었어요. 다시 시작해 주세요.",
            Self::DnsUnavailable => "DNS를 확인하지 못했어요. 잠시 후 다시 확인해 주세요.",
            Self::DnsNotFound => {
                "아직 일치하는 TXT 레코드가 없어요. DNS 반영 후 다시 확인해 주세요."
            }
            Self::ApiUnsupported => "이 서버는 자동 인증을 지원하지 않아요. DNS 인증을 이용해 주세요.",
            Self::ApiNotOwner => "연동된 계정과 서버의 운영자 정보가 일치하지 않아요.",
            Self::ApiUnavailable => "서버의 운영자 정보를 확인하지 못했어요. 잠시 후 다시 시도하거나 DNS 인증을 이용해 주세요.",
            Self::DnsPriority => "DNS로 인증된 관리 권한은 자동 인증으로 바꿀 수 없어요.",
            Self::Unavailable => "지금은 처리할 수 없어요. 잠시 후 다시 시도해 주세요.",
            Self::RefreshTooSoon => "수동 갱신은 서버마다 1시간에 한 번 요청할 수 있어요.",
        })
    }
}
impl std::error::Error for Error {}

pub fn domain(input: &str) -> Result<String, Error> {
    if input.len() > 253
        || input.chars().any(|c| c.is_control() || c.is_whitespace())
        || input.contains(['\\', '%'])
    {
        return Err(Error::InvalidDomain);
    }
    let domain = super::directory::canonical_domain(input).map_err(|_| Error::InvalidDomain)?;
    if format!("_fediverse-kr.{domain}").len() > 253 {
        return Err(Error::InvalidDomain);
    }
    Ok(domain)
}

pub struct ValidatedEdit {
    pub(super) edit: SiteEdit,
    pub(super) tags: Vec<String>,
}
pub fn validate_edit(mut e: SiteEdit) -> Result<ValidatedEdit, Error> {
    for (s, max) in [
        (&mut e.name, 200),
        (&mut e.description, 1000),
        (&mut e.rules, 2000),
        (&mut e.language, 32),
        (&mut e.tags, 256),
        (&mut e.owner_comment, 2000),
    ] {
        *s = s.trim().replace("\r\n", "\n");
        if s.chars().count() > max
            || s.len() > max * 4
            || s.chars().any(|c| c.is_control() && c != '\n' && c != '\t')
        {
            return Err(Error::InvalidText);
        }
    }
    let mut tags = Vec::new();
    for tag in e.tags.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        if tag.chars().count() > 32 || tag.chars().any(char::is_control) {
            return Err(Error::InvalidText);
        }
        if !tags.iter().any(|t| t == tag) {
            tags.push(tag.to_owned());
        }
    }
    if tags.len() > 12 {
        return Err(Error::InvalidText);
    }
    e.tags = tags.join(", ");
    Ok(ValidatedEdit { edit: e, tags })
}

pub trait TxtResolver {
    async fn txt(&self, fqdn: &str) -> Result<Vec<Vec<Vec<u8>>>, Error>;
}
pub struct SystemDns;
impl TxtResolver for SystemDns {
    async fn txt(&self, fqdn: &str) -> Result<Vec<Vec<Vec<u8>>>, Error> {
        use hickory_resolver::{proto::rr::RData, TokioResolver};
        let mut builder = TokioResolver::builder_tokio().map_err(|_| Error::DnsUnavailable)?;
        builder.options_mut().timeout = Duration::from_secs(3);
        builder.options_mut().attempts = 1;
        // A fresh local cache per explicit verification. Recursive cache TTLs
        // still apply; no custom public DNS service or invented DNS wire parser.
        let resolver = builder.build().map_err(|_| Error::DnsUnavailable)?;
        let lookup = tokio::time::timeout(Duration::from_secs(5), resolver.txt_lookup(fqdn))
            .await
            .map_err(|_| Error::DnsUnavailable)?
            .map_err(|_| Error::DnsUnavailable)?;
        let mut result = Vec::new();
        for record in lookup.answers() {
            if let RData::TXT(txt) = &record.data {
                if result.len() >= 64 || txt.txt_data.iter().map(|s| s.len()).sum::<usize>() > 1024
                {
                    return Err(Error::DnsUnavailable);
                }
                result.push(txt.txt_data.iter().map(|s| s.to_vec()).collect());
            }
        }
        Ok(result)
    }
}

// Only the verifier can construct a successful proof, never the HTTP caller.
pub struct VerifiedDns {
    id: Uuid,
    domain: String,
    hash: Vec<u8>,
}
impl VerifiedDns {
    pub(crate) fn id(&self) -> Uuid {
        self.id
    }
    pub(crate) fn domain(&self) -> &str {
        &self.domain
    }
    pub(crate) fn hash(&self) -> &[u8] {
        &self.hash
    }
}
pub async fn begin(
    db: &Database,
    session: &AuthenticatedSession,
    input: &str,
) -> Result<DnsChallenge, Error> {
    let domain = domain(input)?;
    auth::consume_attempt(
        db,
        "challenge_begin",
        format!("site:{}", session.member.id).as_bytes(),
        12,
    )
    .await?;
    let mut bytes = [0u8; 32];
    OsRng
        .try_fill_bytes(&mut bytes)
        .map_err(|_| Error::Unavailable)?;
    let code: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    let id = Uuid::new_v4();
    db.begin_owner_challenge(session, id, &domain, &Sha256::digest(code.as_bytes()))
        .await?;
    Ok(DnsChallenge {
        id: id.to_string(),
        record: format!("_fediverse-kr.{domain}"),
        domain,
        value: format!("fk-verify={code}"),
    })
}
pub async fn finish(
    db: &Database,
    session: &AuthenticatedSession,
    id: Uuid,
    value: &str,
    resolver: &impl TxtResolver,
) -> Result<String, Error> {
    let code = value
        .strip_prefix("fk-verify=")
        .ok_or(Error::InvalidChallenge)?;
    if code.len() != 64
        || !code
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(Error::InvalidChallenge);
    }
    auth::consume_attempt(
        db,
        "challenge_verify",
        format!("site:{}", session.member.id).as_bytes(),
        24,
    )
    .await?;
    auth::consume_attempt(db, "challenge_verify", b"site:dns:global", 180).await?;
    let hash = Sha256::digest(code.as_bytes()).to_vec();
    let domain = db.pending_owner_challenge(session, id, &hash).await?;
    let records = resolver.txt(&format!("_fediverse-kr.{domain}.")).await?;
    if !records.iter().any(|parts| {
        parts.iter().map(Vec::len).sum::<usize>() == value.len()
            && parts.concat() == value.as_bytes()
    }) {
        return Err(Error::DnsNotFound);
    }
    let proof = VerifiedDns {
        id,
        domain: domain.clone(),
        hash,
    };
    db.claim_site_dns(session, &proof).await?;
    Ok(domain)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn owner_inputs_are_bounded_plain_text_and_domains_only() {
        for s in [
            "localhost",
            "127.0.0.1",
            "https://example.org",
            "x.example/path",
            "x.example\\bad",
            "x.example%00",
            " x.example",
        ] {
            assert!(domain(s).is_err(), "{s}");
        }
        assert_eq!(domain("EXAMPLE.ORG").unwrap(), "example.org");
        let mut e = SiteEdit::default();
        e.owner_comment = "가".repeat(2001);
        assert!(validate_edit(e.clone()).is_err());
        e.owner_comment = " <script>alert(1)</script> ".into();
        e.tags = "사진, 개발, 사진".into();
        let v = validate_edit(e).unwrap();
        assert_eq!(v.tags, vec!["사진", "개발"]);
        assert!(v.edit.owner_comment.starts_with("<script>")); // RSX escapes, never raw HTML
    }
}
